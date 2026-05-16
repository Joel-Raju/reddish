use std::sync::atomic::AtomicU64;
use std::time::Duration;

use color_eyre::Result;
use redis::aio::MultiplexedConnection;
use thiserror::Error;

use crate::config::connections::{ConnectionMode, ConnectionProfile, SentinelNode};
use crate::redis::types::{
    RedisValue, StreamEntry, StreamGroup, ZSetEntry, bytes_to_string_lossy, map_pairs_to_index_map,
    stream_fields_from_map,
};

pub type RedisResult<T> = std::result::Result<T, RedisError>;

#[derive(Debug, Error)]
pub enum RedisError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("operation timed out: {0}")]
    Timeout(&'static str),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("command failed: {0}")]
    CommandFailed(String),
}

pub enum RedisClient {
    Standalone(MultiplexedConnection),
}

fn resolve_password(profile: &ConnectionProfile) -> RedisResult<Option<String>> {
    match &profile.password {
        Some(password_ref) => password_ref
            .resolve()
            .map(Some)
            .map_err(|e| RedisError::InvalidConfig(format!("password resolution failed: {e}"))),
        None => Ok(None),
    }
}

fn build_redis_url(profile: &ConnectionProfile, host: &str, port: u16) -> RedisResult<String> {
    let password = resolve_password(profile)?;
    let scheme = if profile.tls.as_ref().is_some_and(|tls| tls.enabled) {
        "rediss"
    } else {
        "redis"
    };

    let auth = match (profile.username.as_deref(), password.as_deref()) {
        (Some(user), Some(pass)) => format!("{user}:{pass}@"),
        (Some(user), None) => format!("{user}@"),
        (None, Some(pass)) => format!(":{pass}@"),
        (None, None) => String::new(),
    };

    Ok(format!("{scheme}://{auth}{host}:{port}/{}", profile.db))
}

async fn connect_with_timeout(url: &str) -> RedisResult<MultiplexedConnection> {
    let client = redis::Client::open(url)
        .map_err(|e| RedisError::ConnectionFailed(format!("invalid redis URL: {e}")))?;

    tokio::time::timeout(
        Duration::from_secs(5),
        client.get_multiplexed_async_connection(),
    )
    .await
    .map_err(|_| RedisError::Timeout("connect"))
    .and_then(|res| {
        res.map_err(|e| RedisError::ConnectionFailed(format!("unable to connect: {e}")))
    })
}

async fn resolve_sentinel_master(
    profile: &ConnectionProfile,
    master_name: &str,
    sentinels: &[SentinelNode],
) -> RedisResult<(String, u16)> {
    if sentinels.is_empty() {
        return Err(RedisError::InvalidConfig(
            "sentinel mode requires at least one sentinel node".to_string(),
        ));
    }

    let mut last_err: Option<RedisError> = None;
    for sentinel in sentinels {
        let url = build_redis_url(profile, &sentinel.host, sentinel.port)?;
        match connect_with_timeout(&url).await {
            Ok(mut conn) => {
                let response: RedisResult<Vec<String>> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SENTINEL")
                        .arg("get-master-addr-by-name")
                        .arg(master_name)
                        .query_async(&mut conn),
                )
                .await
                .map_err(|_| RedisError::Timeout("sentinel get-master-addr-by-name"))
                .and_then(|r| {
                    r.map_err(|e| {
                        RedisError::CommandFailed(format!(
                            "sentinel lookup failed on {}:{}: {e}",
                            sentinel.host, sentinel.port
                        ))
                    })
                });

                match response {
                    Ok(values) if values.len() >= 2 => {
                        let port = values[1].parse::<u16>().map_err(|e| {
                            RedisError::CommandFailed(format!(
                                "invalid sentinel master port '{}': {e}",
                                values[1]
                            ))
                        })?;
                        return Ok((values[0].clone(), port));
                    }
                    Ok(values) => {
                        last_err = Some(RedisError::CommandFailed(format!(
                            "sentinel returned invalid master tuple: {values:?}"
                        )));
                    }
                    Err(err) => {
                        last_err = Some(err);
                    }
                }
            }
            Err(err) => {
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        RedisError::ConnectionFailed("unable to resolve sentinel master".to_string())
    }))
}

impl Clone for RedisClientHandle {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            profile: self.profile.clone(),
            req_id: AtomicU64::new(self.req_id.load(std::sync::atomic::Ordering::Relaxed)),
        }
    }
}

impl Clone for RedisClient {
    fn clone(&self) -> Self {
        match self {
            RedisClient::Standalone(conn) => RedisClient::Standalone(conn.clone()),
        }
    }
}

pub struct RedisClientHandle {
    pub client: RedisClient,
    pub profile: ConnectionProfile,
    pub req_id: AtomicU64,
}

impl RedisClientHandle {
    pub fn connection_url(profile: &ConnectionProfile) -> RedisResult<String> {
        build_redis_url(profile, &profile.host, profile.port)
    }

    pub async fn connect(profile: &ConnectionProfile) -> RedisResult<Self> {
        let conn = match &profile.mode {
            ConnectionMode::Standalone | ConnectionMode::Cluster => {
                let url = build_redis_url(profile, &profile.host, profile.port)?;
                connect_with_timeout(&url).await?
            }
            ConnectionMode::Sentinel {
                master_name,
                sentinels,
            } => {
                let (master_host, master_port) =
                    resolve_sentinel_master(profile, master_name, sentinels).await?;
                let url = build_redis_url(profile, &master_host, master_port)?;
                connect_with_timeout(&url).await?
            }
        };

        Ok(Self {
            client: RedisClient::Standalone(conn),
            profile: profile.clone(),
            req_id: AtomicU64::new(0),
        })
    }

    pub async fn ping(&self) -> Result<Duration> {
        let start = std::time::Instant::now();
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("PING").query_async::<String>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("Ping timeout"))??;
            }
        }
        Ok(start.elapsed())
    }

    pub async fn dbsize(&self) -> Result<u64> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let size: u64 = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("DBSIZE").query_async::<u64>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("DBSIZE timeout"))??;
                Ok(size)
            }
        }
    }

    pub async fn key_type(&self, key: &str) -> Result<RedisType> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let t: String = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("TYPE").arg(key).query_async::<String>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("TYPE timeout"))??;
                Ok(RedisType::from(t.as_str()))
            }
        }
    }

    pub async fn ttl(&self, key: &str) -> Result<Ttl> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let ttl_val: i64 = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("TTL").arg(key).query_async::<i64>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("TTL timeout"))??;
                if ttl_val < 0 {
                    if ttl_val == -2 {
                        Ok(Ttl::KeyNotFound)
                    } else {
                        Ok(Ttl::NoExpiry)
                    }
                } else {
                    Ok(Ttl::Expires(Duration::from_secs(ttl_val as u64)))
                }
            }
        }
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("DEL").arg(key).query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("DEL timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("RENAME")
                        .arg(from)
                        .arg(to)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("RENAME timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn set_ttl(&self, key: &str, seconds: i64) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                if seconds < 0 {
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        redis::cmd("PERSIST").arg(key).query_async::<()>(&mut c),
                    )
                    .await
                    .map_err(|_| color_eyre::eyre::eyre!("PERSIST timeout"))??;
                } else {
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        redis::cmd("EXPIRE")
                            .arg(key)
                            .arg(seconds)
                            .query_async::<()>(&mut c),
                    )
                    .await
                    .map_err(|_| color_eyre::eyre::eyre!("EXPIRE timeout"))??;
                }
                Ok(())
            }
        }
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: Vec<u8> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("GET").arg(key).query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("GET timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SET timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn hgetall(&self, key: &str) -> Result<Vec<(String, Vec<u8>)>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: Vec<(String, Vec<u8>)> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("HGETALL").arg(key).query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("HGETALL timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn lrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<Vec<u8>>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: Vec<Vec<u8>> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("LRANGE")
                        .arg(key)
                        .arg(start)
                        .arg(stop)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("LRANGE timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn smembers(&self, key: &str) -> Result<Vec<Vec<u8>>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: Vec<Vec<u8>> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SMEMBERS").arg(key).query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SMEMBERS timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn zrange_withscores(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<(Vec<u8>, f64)>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: Vec<(Vec<u8>, f64)> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("ZRANGE")
                        .arg(key)
                        .arg(start)
                        .arg(stop)
                        .arg("WITHSCORES")
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("ZRANGE timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn info(&self, section: &str) -> Result<String> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: String = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("INFO").arg(section).query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("INFO timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn publish(&self, channel: &str, message: &str) -> Result<u64> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let val: u64 = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("PUBLISH")
                        .arg(channel)
                        .arg(message)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("PUBLISH timeout"))??;
                Ok(val)
            }
        }
    }

    pub async fn get_value(&self, key: &str, r_type: RedisType) -> Result<RedisValue> {
        match r_type {
            RedisType::String => {
                let raw = self.get(key).await?;
                Ok(RedisValue::String(bytes_to_string_lossy(raw)))
            }
            RedisType::List => {
                let vals = self.lrange(key, 0, -1).await?;
                Ok(RedisValue::List(
                    vals.into_iter().map(bytes_to_string_lossy).collect(),
                ))
            }
            RedisType::Hash => {
                let vals = self.hgetall(key).await?;
                Ok(RedisValue::Hash(map_pairs_to_index_map(vals)))
            }
            RedisType::Set => {
                let vals = self.smembers(key).await?;
                Ok(RedisValue::Set(
                    vals.into_iter().map(bytes_to_string_lossy).collect(),
                ))
            }
            RedisType::ZSet => {
                let vals = self.zrange_withscores(key, 0, -1).await?;
                Ok(RedisValue::ZSet(
                    vals.into_iter()
                        .map(|(member, score)| ZSetEntry {
                            score,
                            member: bytes_to_string_lossy(member),
                        })
                        .collect(),
                ))
            }
            RedisType::Stream => {
                let vals = self.xrange(key).await?;
                Ok(RedisValue::Stream(vals))
            }
            RedisType::Unknown => Err(color_eyre::eyre::eyre!(
                "Cannot load value for unknown redis type"
            )),
        }
    }

    pub async fn set_string(&self, key: &str, value: &str) -> Result<()> {
        self.set(key, value.as_bytes()).await
    }

    pub async fn list_push(&self, key: &str, value: &str, head: bool) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let cmd = if head { "LPUSH" } else { "RPUSH" };
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd(cmd)
                        .arg(key)
                        .arg(value)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("{cmd} timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn list_set(&self, key: &str, index: i64, value: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("LSET")
                        .arg(key)
                        .arg(index)
                        .arg(value)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("LSET timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn list_remove(&self, key: &str, value: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("LREM")
                        .arg(key)
                        .arg(0)
                        .arg(value)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("LREM timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn hash_set(&self, key: &str, field: &str, value: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("HSET")
                        .arg(key)
                        .arg(field)
                        .arg(value)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("HSET timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn hash_del(&self, key: &str, fields: &[&str]) -> Result<()> {
        if fields.is_empty() {
            return Ok(());
        }
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let mut cmd = redis::cmd("HDEL");
                cmd.arg(key);
                for field in fields {
                    cmd.arg(field);
                }
                tokio::time::timeout(Duration::from_secs(5), cmd.query_async::<()>(&mut c))
                    .await
                    .map_err(|_| color_eyre::eyre::eyre!("HDEL timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn set_add(&self, key: &str, member: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SADD")
                        .arg(key)
                        .arg(member)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SADD timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn set_rem(&self, key: &str, member: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SREM")
                        .arg(key)
                        .arg(member)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SREM timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn zadd(&self, key: &str, score: f64, member: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("ZADD")
                        .arg(key)
                        .arg(score)
                        .arg(member)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("ZADD timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn zrem(&self, key: &str, member: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("ZREM")
                        .arg(key)
                        .arg(member)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("ZREM timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn zscore_update(&self, key: &str, member: &str, score: f64) -> Result<()> {
        self.zadd(key, score, member).await
    }

    pub async fn xadd(&self, key: &str, entry_id: &str, fields: &[(&str, &str)]) -> Result<String> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let mut cmd = redis::cmd("XADD");
                cmd.arg(key).arg(entry_id);
                for (field, value) in fields {
                    cmd.arg(field).arg(value);
                }
                let id: String =
                    tokio::time::timeout(Duration::from_secs(5), cmd.query_async(&mut c))
                        .await
                        .map_err(|_| color_eyre::eyre::eyre!("XADD timeout"))??;
                Ok(id)
            }
        }
    }

    pub async fn xdel(&self, key: &str, id: &str) -> Result<()> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("XDEL")
                        .arg(key)
                        .arg(id)
                        .query_async::<()>(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("XDEL timeout"))??;
                Ok(())
            }
        }
    }

    pub async fn xrange(&self, key: &str) -> Result<Vec<StreamEntry>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let raw: Vec<(String, std::collections::HashMap<String, redis::Value>)> =
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        redis::cmd("XRANGE")
                            .arg(key)
                            .arg("-")
                            .arg("+")
                            .query_async(&mut c),
                    )
                    .await
                    .map_err(|_| color_eyre::eyre::eyre!("XRANGE timeout"))??;

                Ok(raw
                    .into_iter()
                    .map(|(id, fields)| StreamEntry {
                        id,
                        fields: stream_fields_from_map(fields),
                    })
                    .collect())
            }
        }
    }

    pub async fn xgroups(&self, key: &str) -> Result<Vec<StreamGroup>> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let rows: Vec<Vec<String>> = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("XINFO")
                        .arg("GROUPS")
                        .arg(key)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("XINFO GROUPS timeout"))??;

                let mut out = Vec::new();
                for row in rows {
                    if row.len() >= 8 {
                        out.push(StreamGroup {
                            name: row.get(1).cloned().unwrap_or_default(),
                            consumers: row.get(3).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
                            pending: row.get(5).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
                            last_delivered_id: row.get(7).cloned().unwrap_or_default(),
                        });
                    }
                }
                Ok(out)
            }
        }
    }

    pub async fn memory_usage(&self, key: &str) -> Result<u64> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let bytes: u64 = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("MEMORY")
                        .arg("USAGE")
                        .arg(key)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("MEMORY USAGE timeout"))??;
                Ok(bytes)
            }
        }
    }

    pub async fn object_encoding(&self, key: &str) -> Result<String> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let encoding: String = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("OBJECT")
                        .arg("ENCODING")
                        .arg(key)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("OBJECT ENCODING timeout"))??;
                Ok(encoding)
            }
        }
    }

    pub async fn scan_keys(&self, cursor: u64, pattern: &str, count: u32) -> Result<(u64, Vec<String>)> {
        match &self.client {
            RedisClient::Standalone(conn) => {
                let mut c = conn.clone();
                let (new_cursor, keys): (u64, Vec<String>) = tokio::time::timeout(
                    Duration::from_secs(5),
                    redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(pattern)
                        .arg("COUNT")
                        .arg(count)
                        .query_async(&mut c),
                )
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SCAN timeout"))??;
                Ok((new_cursor, keys))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedisType {
    String,
    List,
    Hash,
    Set,
    ZSet,
    Stream,
    Unknown,
}

impl From<&str> for RedisType {
    fn from(s: &str) -> Self {
        match s {
            "string" => RedisType::String,
            "list" => RedisType::List,
            "hash" => RedisType::Hash,
            "set" => RedisType::Set,
            "zset" => RedisType::ZSet,
            "stream" => RedisType::Stream,
            _ => RedisType::Unknown,
        }
    }
}

impl RedisType {
    pub fn badge_char(&self) -> &'static str {
        match self {
            RedisType::String => "S",
            RedisType::List => "L",
            RedisType::Hash => "H",
            RedisType::Set => "St",
            RedisType::ZSet => "Z",
            RedisType::Stream => "X",
            RedisType::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ttl {
    NoExpiry,
    Expires(Duration),
    KeyNotFound,
}

impl Ttl {
    pub fn display(&self) -> String {
        match self {
            Ttl::NoExpiry => "∞".to_string(),
            Ttl::Expires(d) => {
                let secs = d.as_secs();
                if secs == 0 {
                    "exp".to_string()
                } else if secs >= 86400 {
                    format!("{}d{}h", secs / 86400, (secs % 86400) / 3600)
                } else if secs >= 3600 {
                    format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
                } else if secs >= 60 {
                    format!("{}m{}s", secs / 60, secs % 60)
                } else {
                    format!("{}s", secs)
                }
            }
            Ttl::KeyNotFound => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterNode {
    pub id: String,
    pub addr: String,
    pub flags: Vec<String>,
    pub slots: Vec<(u16, u16)>,
    pub ping_sent: u64,
    pub pong_recv: u64,
    pub link_state: String,
}

pub fn parse_cluster_nodes(raw: &str) -> color_eyre::Result<Vec<ClusterNode>> {
    let mut nodes = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            continue;
        }
        let id = parts[0].to_string();
        let addr = parts[1].to_string();
        let flags = parts[2].split(',').map(String::from).collect();
        let ping_sent = parts[4].parse().unwrap_or(0);
        let pong_recv = parts[5].parse().unwrap_or(0);
        let link_state = parts[7].to_string();

        let mut slots = Vec::new();
        for part in &parts[8..] {
            if let Some((start, end)) = part.split_once('-') {
                if let (Ok(s), Ok(e)) = (start.parse::<u16>(), end.parse::<u16>()) {
                    slots.push((s, e));
                }
            } else if let Ok(single) = part.parse::<u16>() {
                slots.push((single, single));
            }
        }

        nodes.push(ClusterNode {
            id,
            addr,
            flags,
            slots,
            ping_sent,
            pong_recv,
            link_state,
        });
    }
    Ok(nodes)
}
