use color_eyre::Result;

use crate::redis::client::RedisClientHandle;

pub struct ServerInfo(pub String);

impl ServerInfo {
    pub fn section(&self, name: &str) -> Option<&str> {
        let pattern = format!("# {}", name);
        self.0.split("\r\n\r\n").find_map(|chunk| {
            if chunk.starts_with(&pattern) {
                Some(chunk.strip_prefix(&pattern).unwrap_or(chunk))
            } else {
                None
            }
        })
    }

    pub fn key_value(&self, section_name: &str, key: &str) -> Option<String> {
        self.section(section_name).and_then(|s| {
            s.lines().find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if k == key { Some(v.to_string()) } else { None }
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlowLogEntry {
    pub id: u64,
    pub timestamp: u64,
    pub duration_us: u64,
    pub command: Vec<String>,
    pub client: Option<String>,
    pub name: Option<String>,
}

pub async fn slowlog_get(client: &RedisClientHandle, count: usize) -> Result<Vec<SlowLogEntry>> {
    let mut conn = match &client.client {
        crate::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    let raw: Vec<Vec<redis::Value>> = redis::cmd("SLOWLOG")
        .arg("GET")
        .arg(count)
        .query_async(&mut conn)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("SLOWLOG failed: {}", e))?;

    let mut entries = Vec::new();
    for item in raw {
        if let Ok(entry) = parse_slowlog_entry(item) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn parse_slowlog_entry(raw: Vec<redis::Value>) -> color_eyre::Result<SlowLogEntry> {
    let mut iter = raw.into_iter();
    let id = next_u64(&mut iter)?;
    let timestamp = next_u64(&mut iter)?;
    let duration_us = next_u64(&mut iter)?;
    let command = match iter.next() {
        Some(redis::Value::Array(cmds)) => cmds
            .into_iter()
            .filter_map(value_to_string)
            .collect(),
        _ => Vec::new(),
    };

    let mut client = None;
    let mut name = None;

    // Optional client name fields (Redis 4.0+)
    for val in iter {
        if let Some(s) = value_to_string(val) {
            if client.is_none() {
                client = Some(s.clone());
            } else {
                name = Some(s);
            }
        }
    }

    Ok(SlowLogEntry {
        id,
        timestamp,
        duration_us,
        command,
        client,
        name,
    })
}

fn value_to_string(v: redis::Value) -> Option<String> {
    match v {
        redis::Value::BulkString(s) => String::from_utf8(s).ok(),
        redis::Value::SimpleString(s) => Some(s),
        redis::Value::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

fn next_u64(iter: &mut std::vec::IntoIter<redis::Value>) -> color_eyre::Result<u64> {
    match iter.next() {
        Some(redis::Value::Int(v)) => Ok(v as u64),
        Some(redis::Value::BulkString(bytes)) => String::from_utf8(bytes)?
            .parse::<u64>()
            .map_err(|e| color_eyre::eyre::eyre!("Invalid u64: {}", e)),
        Some(redis::Value::SimpleString(s)) => s
            .parse::<u64>()
            .map_err(|e| color_eyre::eyre::eyre!("Invalid u64: {}", e)),
        _ => Err(color_eyre::eyre::eyre!("Missing u64 field")),
    }
}
