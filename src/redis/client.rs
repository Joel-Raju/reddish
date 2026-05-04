use std::sync::atomic::AtomicU64;
use std::time::Duration;

use color_eyre::Result;
use redis::aio::MultiplexedConnection;

use crate::config::connections::ConnectionProfile;

pub enum RedisClient {
    Standalone(MultiplexedConnection),
}

pub struct RedisClientHandle {
    pub client: RedisClient,
    pub profile: ConnectionProfile,
    pub req_id: AtomicU64,
}

impl RedisClientHandle {
    pub async fn connect(profile: &ConnectionProfile) -> Result<Self> {
        let url = if let Some(ref user) = profile.username {
            format!(
                "redis://{}@{}:{}/{}",
                user, profile.host, profile.port, profile.db
            )
        } else {
            format!(
                "redis://{}:{}/{}",
                profile.host, profile.port, profile.db
            )
        };
        let client = redis::Client::open(url)?;
        let conn = tokio::time::timeout(Duration::from_secs(5), client.get_multiplexed_async_connection())
            .await
            .map_err(|_| color_eyre::eyre::eyre!("Connection timeout"))??;
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
                tokio::time::timeout(Duration::from_secs(5), redis::cmd("PING").query_async::<String>(&mut c))
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
                let size: u64 = tokio::time::timeout(Duration::from_secs(5), redis::cmd("DBSIZE").query_async::<u64>(&mut c))
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
                let t: String = tokio::time::timeout(Duration::from_secs(5), redis::cmd("TYPE").arg(key).query_async::<String>(&mut c))
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
                let ttl_val: i64 = tokio::time::timeout(Duration::from_secs(5), redis::cmd("TTL").arg(key).query_async::<i64>(&mut c))
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
                tokio::time::timeout(Duration::from_secs(5), redis::cmd("DEL").arg(key).query_async::<()>(&mut c))
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
                tokio::time::timeout(Duration::from_secs(5), redis::cmd("RENAME").arg(from).arg(to).query_async::<()>(&mut c))
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
                    tokio::time::timeout(Duration::from_secs(5), redis::cmd("PERSIST").arg(key).query_async::<()>(&mut c))
                        .await
                        .map_err(|_| color_eyre::eyre::eyre!("PERSIST timeout"))??;
                } else {
                    tokio::time::timeout(Duration::from_secs(5), redis::cmd("EXPIRE").arg(key).arg(seconds).query_async::<()>(&mut c))
                        .await
                        .map_err(|_| color_eyre::eyre::eyre!("EXPIRE timeout"))??;
                }
                Ok(())
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

#[derive(Debug, Clone, PartialEq)]
pub enum Ttl {
    NoExpiry,
    Expires(Duration),
    KeyNotFound,
}
