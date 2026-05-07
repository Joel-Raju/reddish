use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq)]
pub enum RedisValue {
    String(String),
    List(Vec<String>),
    Hash(IndexMap<String, String>),
    Set(BTreeSet<String>),
    ZSet(Vec<ZSetEntry>),
    Stream(Vec<StreamEntry>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZSetEntry {
    pub score: f64,
    pub member: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamEntry {
    pub id: String,
    pub fields: IndexMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamGroup {
    pub name: String,
    pub consumers: u64,
    pub pending: u64,
    pub last_delivered_id: String,
}

pub fn bytes_to_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).to_string())
}

pub fn map_pairs_to_index_map(pairs: Vec<(String, Vec<u8>)>) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for (k, v) in pairs {
        out.insert(k, bytes_to_string_lossy(v));
    }
    out
}

pub fn stream_fields_from_map(raw: HashMap<String, redis::Value>) -> IndexMap<String, String> {
    let mut fields = IndexMap::new();
    for (k, v) in raw {
        let val = match v {
            redis::Value::BulkString(b) => bytes_to_string_lossy(b),
            redis::Value::SimpleString(s) => s,
            redis::Value::Int(i) => i.to_string(),
            _ => String::new(),
        };
        fields.insert(k, val);
    }
    fields
}
