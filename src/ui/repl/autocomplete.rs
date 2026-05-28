use nucleo::{Config, Matcher, Utf32String};

pub struct ReplAutocomplete {
    matcher: Matcher,
    commands: Vec<String>,
    key_cache: Vec<String>,
}

impl Default for ReplAutocomplete {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplAutocomplete {
    pub fn new() -> Self {
        let commands: Vec<String> = COMMON_REDIS_COMMANDS
            .iter()
            .map(|&s| s.to_string())
            .collect();
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            commands,
            key_cache: Vec::new(),
        }
    }

    pub fn update_key_cache(&mut self, keys: Vec<String>) {
        self.key_cache = keys;
    }

    /// Returns up to `limit` fuzzy matches sorted by score (highest first).
    /// Only matches the first token (command name). If input contains a space,
    /// no command suggestions are returned.
    pub fn suggest_commands(&mut self, input: &str, limit: usize) -> Vec<String> {
        let trimmed = input.trim_start();
        if trimmed.is_empty() || trimmed.contains(' ') {
            return Vec::new();
        }
        let upper = trimmed.to_uppercase();

        // Exact or prefix matches rank highest
        let mut scored: Vec<(String, u16)> = self
            .commands
            .iter()
            .filter_map(|cmd| {
                if cmd == &upper {
                    Some((cmd.clone(), u16::MAX))
                } else {
                    let haystack = Utf32String::from(cmd.as_str());
                    let needle = Utf32String::from(upper.as_str());
                    self.matcher
                        .fuzzy_match_greedy(haystack.slice(..), needle.slice(..))
                        .map(|score| (cmd.clone(), score))
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().take(limit).map(|(s, _)| s).collect()
    }

    /// Returns up to `limit` fuzzy matches from the key cache.
    pub fn suggest_keys(&mut self, input: &str, limit: usize) -> Vec<String> {
        if input.len() < 2 || self.key_cache.is_empty() {
            return Vec::new();
        }
        let needle = Utf32String::from(input);
        let mut scored: Vec<(String, u16)> = self
            .key_cache
            .iter()
            .filter_map(|key| {
                let haystack = Utf32String::from(key.as_str());
                self.matcher
                    .fuzzy_match_greedy(haystack.slice(..), needle.slice(..))
                    .map(|score| (key.clone(), score))
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().take(limit).map(|(s, _)| s).collect()
    }
}

const COMMON_REDIS_COMMANDS: &[&str] = &[
    "APPEND",
    "BITCOUNT",
    "CLIENT",
    "CLUSTER",
    "CONFIG",
    "DBSIZE",
    "DECR",
    "DECRBY",
    "DEL",
    "DISCARD",
    "DUMP",
    "EVAL",
    "EXEC",
    "EXISTS",
    "EXPIRE",
    "EXPIREAT",
    "FLUSHALL",
    "FLUSHDB",
    "GET",
    "GETBIT",
    "GETRANGE",
    "GETSET",
    "HDEL",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HINCRBY",
    "HKEYS",
    "HLEN",
    "HMGET",
    "HMSET",
    "HSET",
    "HSETNX",
    "HSTRLEN",
    "HVALS",
    "INCR",
    "INCRBY",
    "INFO",
    "KEYS",
    "LINDEX",
    "LINSERT",
    "LLEN",
    "LPOP",
    "LPUSH",
    "LPUSHX",
    "LRANGE",
    "LREM",
    "LSET",
    "LTRIM",
    "MGET",
    "MONITOR",
    "MOVE",
    "MSET",
    "MSETNX",
    "MULTI",
    "PERSIST",
    "PEXPIRE",
    "PING",
    "PSUBSCRIBE",
    "PTTL",
    "PUBLISH",
    "PUNSUBSCRIBE",
    "RENAME",
    "RENAMENX",
    "RESTORE",
    "RPOP",
    "RPOPLPUSH",
    "RPUSH",
    "RPUSHX",
    "SADD",
    "SCAN",
    "SCARD",
    "SCRIPT",
    "SDIFF",
    "SDIFFSTORE",
    "SELECT",
    "SET",
    "SETBIT",
    "SETEX",
    "SETNX",
    "SETRANGE",
    "SINTER",
    "SINTERSTORE",
    "SISMEMBER",
    "SLOWLOG",
    "SMEMBERS",
    "SMOVE",
    "SORT",
    "SPOP",
    "SRANDMEMBER",
    "SREM",
    "STRLEN",
    "SUBSCRIBE",
    "SUNION",
    "SUNIONSTORE",
    "SUNSUBSCRIBE",
    "SWAPDB",
    "SYNC",
    "TIME",
    "TTL",
    "TYPE",
    "UNSUBSCRIBE",
    "UNWATCH",
    "WATCH",
    "XADD",
    "XDEL",
    "XGROUP",
    "XINFO",
    "XRANGE",
    "XREAD",
    "XREADGROUP",
    "XREVRANGE",
    "XTRIM",
    "ZADD",
    "ZCARD",
    "ZCOUNT",
    "ZDIFF",
    "ZDIFFSTORE",
    "ZINCRBY",
    "ZINTER",
    "ZINTERSTORE",
    "ZLEXCOUNT",
    "ZPOPMAX",
    "ZPOPMIN",
    "ZRANGE",
    "ZRANGEBYLEX",
    "ZRANGEBYSCORE",
    "ZRANK",
    "ZREM",
    "ZREMRANGEBYLEX",
    "ZREMRANGEBYRANK",
    "ZREMRANGEBYSCORE",
    "ZREVRANGE",
    "ZREVRANGEBYSCORE",
    "ZREVRANK",
    "ZSCAN",
    "ZSCORE",
    "ZUNION",
    "ZUNIONSTORE",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autocomplete_exact_match() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("GET", 5);
        assert_eq!(results.first(), Some(&"GET".to_string()));
    }

    #[test]
    fn test_autocomplete_fuzzy_prefix() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("ge", 5);
        assert!(results.contains(&"GET".to_string()));
    }

    #[test]
    fn test_autocomplete_fuzzy_gap() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("hga", 5);
        assert!(results.contains(&"HGETALL".to_string()));
    }

    #[test]
    fn test_autocomplete_no_match_empty() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("xyz", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_autocomplete_no_match_after_space() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("GET foo", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_autocomplete_ranks_better_matches_higher() {
        let mut ac = ReplAutocomplete::new();
        let results = ac.suggest_commands("set", 5);
        // "SET" should be first (exact match or very close)
        assert_eq!(results.first(), Some(&"SET".to_string()));
    }

    #[test]
    fn test_key_cache_suggestions() {
        let mut ac = ReplAutocomplete::new();
        ac.update_key_cache(vec![
            "user:123".to_string(),
            "user:456".to_string(),
            "session:abc".to_string(),
        ]);
        let results = ac.suggest_keys("usr", 5);
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.starts_with("user:")));
    }

    #[test]
    fn test_key_cache_empty_without_input() {
        let mut ac = ReplAutocomplete::new();
        ac.update_key_cache(vec!["foo".to_string()]);
        let results = ac.suggest_keys("", 5);
        assert!(results.is_empty());
    }
}
