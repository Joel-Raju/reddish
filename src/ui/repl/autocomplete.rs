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

        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
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

        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        scored.into_iter().take(limit).map(|(s, _)| s).collect()
    }

    /// Subcommands of `command`, if it has a documented set.
    ///
    /// Used to suggest the second token: typing `CONFIG` and pressing Tab
    /// should offer `GET`/`SET`/`REWRITE` rather than key names, since the
    /// second argument of these commands is a subcommand and nothing else.
    pub fn subcommands_for(command: &str) -> &'static [&'static str] {
        let upper = command.to_uppercase();
        SUBCOMMANDS
            .iter()
            .find(|(name, _)| *name == upper)
            .map(|(_, subs)| *subs)
            .unwrap_or(&[])
    }

    /// Whether `command` has subcommands worth suggesting.
    pub fn has_subcommands(command: &str) -> bool {
        !Self::subcommands_for(command).is_empty()
    }

    /// Fuzzy matches among `command`'s subcommands.
    ///
    /// An empty `input` offers the first `limit` subcommands in their
    /// documented order, which is what Tab on a bare `CONFIG ` should show.
    pub fn suggest_subcommands(&mut self, command: &str, input: &str, limit: usize) -> Vec<String> {
        let subs = Self::subcommands_for(command);
        if subs.is_empty() {
            return Vec::new();
        }

        // A trailing space means the user has finished the subcommand token and
        // wants to see the options rather than filter them.
        if input.is_empty() {
            return subs.iter().take(limit).map(|s| s.to_string()).collect();
        }

        let needle = Utf32String::from(input);
        let mut scored: Vec<(String, u16)> = subs
            .iter()
            .filter_map(|sub| {
                let haystack = Utf32String::from(*sub);
                self.matcher
                    .fuzzy_match_greedy(haystack.slice(..), needle.slice(..))
                    .map(|score| (sub.to_string(), score))
            })
            .collect();

        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        scored.into_iter().take(limit).map(|(s, _)| s).collect()
    }
}

/// Commands whose second argument is a subcommand rather than a key.
///
/// Only commands with a small, stable subcommand set are listed; suggesting
/// nothing is better than suggesting an incomplete list. Order is the order a
/// reader would expect to scan them in.
const SUBCOMMANDS: &[(&str, &[&str])] = &[
    (
        "ACL",
        &[
            "CAT", "DELUSER", "GETUSER", "LIST", "LOAD", "LOG", "SAVE", "SETUSER", "USERS",
            "WHOAMI",
        ],
    ),
    (
        "CLIENT",
        &[
            "GETNAME", "ID", "INFO", "KILL", "LIST", "NO-EVICT", "PAUSE", "REPLY", "SETNAME",
            "UNPAUSE",
        ],
    ),
    (
        "CLUSTER",
        &[
            "COUNTKEYSINSLOT",
            "FAILOVER",
            "FORGET",
            "GETKEYSINSLOT",
            "INFO",
            "KEYSLOT",
            "MEET",
            "MYID",
            "NODES",
            "REPLICATE",
            "RESET",
            "SAVECONFIG",
            "SETSLOT",
            "SHARDS",
            "SLAVES",
            "SLOTS",
        ],
    ),
    ("CONFIG", &["GET", "RESETSTAT", "REWRITE", "SET"]),
    (
        "FUNCTION",
        &[
            "DELETE", "DUMP", "FLUSH", "KILL", "LIST", "LOAD", "RESTORE", "STATS",
        ],
    ),
    (
        "LATENCY",
        &["DOCTOR", "GRAPH", "HISTORY", "HISTOGRAM", "LATEST", "RESET"],
    ),
    (
        "MEMORY",
        &["DOCTOR", "MALLOC-STATS", "PURGE", "STATS", "USAGE"],
    ),
    (
        "OBJECT",
        &["ENCODING", "FREQ", "IDLETIME", "REFCOUNT", "SLOWLOG"],
    ),
    ("SCRIPT", &["DEBUG", "EXISTS", "FLUSH", "KILL", "LOAD"]),
    ("SLOWLOG", &["GET", "LEN", "RESET"]),
    ("XINFO", &["CONSUMERS", "GROUPS", "STREAM"]),
];

const COMMON_REDIS_COMMANDS: &[&str] = &[
    "ACL",
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
    "FUNCTION",
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
    "LATENCY",
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
    "MEMORY",
    "MGET",
    "MONITOR",
    "MOVE",
    "MSET",
    "MSETNX",
    "MULTI",
    "OBJECT",
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

#[cfg(test)]
mod subcommand_tests {
    use super::*;

    fn ac() -> ReplAutocomplete {
        ReplAutocomplete::new()
    }

    // ---- lookup ----

    #[test]
    fn commands_with_subcommands_are_recognised() {
        for cmd in ["CONFIG", "config", "CoNfIg", "CLIENT", "XINFO"] {
            assert!(
                ReplAutocomplete::has_subcommands(cmd),
                "{cmd} should have subcommands"
            );
        }
    }

    #[test]
    fn commands_without_subcommands_are_not_claimed_to_have_them() {
        // GET/SET take keys, so suggesting subcommands here would be wrong.
        for cmd in ["GET", "SET", "MGET", "DEL", "LPUSH", "HGETALL", ""] {
            assert!(
                !ReplAutocomplete::has_subcommands(cmd),
                "{cmd} has no subcommand set"
            );
        }
    }

    #[test]
    fn unknown_command_returns_an_empty_set() {
        assert!(ReplAutocomplete::subcommands_for("NOTACOMMAND").is_empty());
    }

    // ---- suggestions ----

    #[test]
    fn empty_input_lists_the_options_in_documented_order() {
        let mut a = ac();
        let subs = a.suggest_subcommands("CONFIG", "", 10);
        assert_eq!(subs, vec!["GET", "RESETSTAT", "REWRITE", "SET"]);
    }

    #[test]
    fn a_trailing_space_still_lists_the_options() {
        // Tab on `CONFIG ` should show the choices, not an empty list.
        let mut a = ac();
        assert!(!a.suggest_subcommands("CONFIG", "", 8).is_empty());
    }

    #[test]
    fn prefix_filters_the_subcommands() {
        let mut a = ac();
        let subs = a.suggest_subcommands("CLIENT", "l", 10);
        assert!(subs.contains(&"LIST".to_string()), "got {subs:?}");
        assert!(
            !subs.contains(&"GETNAME".to_string()),
            "LIST prefix should not match GETNAME: {subs:?}"
        );
    }

    #[test]
    fn fuzzy_matching_finds_a_mid_word_subcommand() {
        let mut a = ac();
        let subs = a.suggest_subcommands("MEMORY", "usag", 5);
        assert_eq!(subs, vec!["USAGE"]);
    }

    #[test]
    fn subcommand_limit_is_respected() {
        let mut a = ac();
        assert_eq!(a.suggest_subcommands("CLUSTER", "", 3).len(), 3);
    }

    #[test]
    fn command_without_subcommands_yields_nothing() {
        let mut a = ac();
        assert!(a.suggest_subcommands("GET", "", 8).is_empty());
    }

    #[test]
    fn every_listed_command_is_also_in_the_command_list() {
        // A subcommand set for a command the completer does not know about
        // would be unreachable.
        let a = ac();
        for (name, _) in SUBCOMMANDS {
            assert!(
                a.commands.iter().any(|c| c == name),
                "{name} has subcommands but is not in COMMON_REDIS_COMMANDS"
            );
        }
    }
}
