pub mod connections;
pub mod keybindings;

use color_eyre::Result;
use serde::Deserialize;
use std::path::PathBuf;

/// Event-loop tick interval when the config does not say.
const DEFAULT_REFRESH_INTERVAL_MS: u64 = 1000;
/// Floor for the tick interval. See [`Config::refresh_interval_ms`].
const MIN_REFRESH_INTERVAL_MS: u64 = 50;
/// Ceiling for the tick interval. See [`Config::refresh_interval_ms`].
const MAX_REFRESH_INTERVAL_MS: u64 = 60_000;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    pub namespace_separator: Option<String>,
    pub scan_count: Option<u32>,
    pub refresh_interval_ms: Option<u64>,
    pub log_level: Option<String>,
    pub color_scheme: Option<String>,
    pub mouse_enabled: Option<bool>,
    pub confirm_deletes: Option<bool>,
    pub max_keys_in_memory: Option<usize>,
    /// Editor used for string editing. When absent, `$EDITOR` is consulted,
    /// and failing that the built-in editor is used.
    pub editor: Option<String>,
    /// Upper bound on bytes rendered for a single value. Larger values are
    /// truncated with a visible marker rather than drawn in full.
    pub max_value_display_bytes: Option<usize>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents).map_err(|e| {
            color_eyre::eyre::eyre!("Invalid TOML in config file {:?}: {}", path, e)
        })?;
        Ok(config)
    }

    pub fn namespace_separator(&self) -> &str {
        self.namespace_separator.as_deref().unwrap_or(":")
    }

    pub fn scan_count(&self) -> u32 {
        self.scan_count.unwrap_or(200)
    }

    pub fn refresh_interval_ms(&self) -> u64 {
        // Clamped rather than passed through. This drives the event loop's tick
        // rate, and the periodic tasks (latency probe, info refresh, stream
        // follow) run every few ticks, so a value of 1 would mean a PING every
        // few milliseconds against the server. A very large value would make
        // the UI feel wedged, so that is clamped too.
        let requested = self
            .refresh_interval_ms
            .unwrap_or(DEFAULT_REFRESH_INTERVAL_MS);
        requested.clamp(MIN_REFRESH_INTERVAL_MS, MAX_REFRESH_INTERVAL_MS)
    }

    pub fn log_level(&self) -> &str {
        self.log_level.as_deref().unwrap_or("info")
    }

    pub fn color_scheme(&self) -> &str {
        self.color_scheme.as_deref().unwrap_or("default")
    }

    pub fn mouse_enabled(&self) -> bool {
        self.mouse_enabled.unwrap_or(true)
    }

    pub fn confirm_deletes(&self) -> bool {
        self.confirm_deletes.unwrap_or(true)
    }

    pub fn max_keys_in_memory(&self) -> usize {
        self.max_keys_in_memory.unwrap_or(500000)
    }

    /// Configured editor, if it names one.
    ///
    /// A blank value is treated as unset so that clearing the key in the config
    /// file falls back to `$EDITOR` rather than trying to run an empty command.
    pub fn editor(&self) -> Option<&str> {
        self.editor
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
    }

    /// Byte budget for rendering a single value.
    ///
    /// Defaults to 1 MiB. Values are fetched with `GET` in full regardless of
    /// this setting, so it bounds rendering work, not memory use; a value
    /// larger than this is truncated with a marker saying so.
    ///
    /// An explicit `0` means no limit. A zero budget would otherwise suppress
    /// every value, which is never what a config author means.
    pub fn max_value_display_bytes(&self) -> usize {
        match self.max_value_display_bytes {
            Some(0) => usize::MAX,
            Some(n) => n,
            None => 1024 * 1024,
        }
    }
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| color_eyre::eyre::eyre!("Could not determine config directory"))?;
    Ok(base.join("redis-tui").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn parse(toml: &str) -> Config {
        toml::from_str(toml).expect("valid TOML")
    }

    // ---- editor (4.9) ----

    #[test]
    fn editor_defaults_to_unset() {
        assert_eq!(Config::default().editor(), None);
    }

    #[test]
    fn editor_is_read_from_config() {
        let c = parse(r#"editor = "hx""#);
        assert_eq!(c.editor(), Some("hx"));
    }

    #[test]
    fn editor_may_carry_arguments() {
        let c = parse(r#"editor = "code --wait""#);
        assert_eq!(c.editor(), Some("code --wait"));
    }

    /// Clearing the key should fall back to `$EDITOR`, not try to run "".
    #[test]
    fn blank_editor_is_treated_as_unset() {
        for toml in [r#"editor = """#, r#"editor = "   ""#, r#"editor = "\t""#] {
            assert_eq!(parse(toml).editor(), None, "{toml}");
        }
    }

    #[test]
    fn editor_is_trimmed() {
        let c = parse(r#"editor = "  nvim  ""#);
        assert_eq!(c.editor(), Some("nvim"));
    }

    // ---- max_value_display_bytes (4.9) ----

    #[test]
    fn max_value_display_bytes_defaults_to_one_mib() {
        assert_eq!(Config::default().max_value_display_bytes(), 1024 * 1024);
    }

    #[test]
    fn max_value_display_bytes_is_read_from_config() {
        let c = parse("max_value_display_bytes = 4096");
        assert_eq!(c.max_value_display_bytes(), 4096);
    }

    /// Zero would suppress every value, which is never the intent. Treating it
    /// as "no limit" is the least surprising reading.
    #[test]
    fn zero_max_value_display_bytes_means_unlimited() {
        let c = parse("max_value_display_bytes = 0");
        assert_eq!(c.max_value_display_bytes(), usize::MAX);
    }

    #[test]
    fn existing_config_keys_still_parse() {
        let c = parse(
            r#"
            namespace_separator = "::"
            scan_count = 500
            refresh_interval_ms = 250
            log_level = "debug"
            color_scheme = "nord"
            mouse_enabled = false
            confirm_deletes = false
            max_keys_in_memory = 1000
            "#,
        );
        assert_eq!(c.namespace_separator(), "::");
        assert_eq!(c.scan_count(), 500);
        assert_eq!(c.refresh_interval_ms(), 250);
        assert_eq!(c.log_level(), "debug");
        assert_eq!(c.color_scheme(), "nord");
        assert!(!c.mouse_enabled());
        assert!(!c.confirm_deletes());
        assert_eq!(c.max_keys_in_memory(), 1000);
    }

    /// Adding keys must not break a file written for an older version.
    #[test]
    fn partial_config_uses_defaults_for_absent_keys() {
        let c = parse("scan_count = 10");
        assert_eq!(c.scan_count(), 10);
        assert_eq!(c.editor(), None);
        assert_eq!(c.max_value_display_bytes(), 1024 * 1024);
    }
}

#[cfg(test)]
mod reachability_tests {
    use super::tests::parse;
    use super::*;

    /// A config key that nothing reads is a lie in the documentation: the
    /// value parses, appears in the example file, and does nothing. Five such
    /// keys turned up during this work (editor, max_value_display_bytes,
    /// color_scheme, confirm_deletes, refresh_interval_ms), so the accessor
    /// list is pinned here.
    ///
    /// This test cannot see whether a reader exists, only that each key has an
    /// accessor with the right name — which is the part that silently drifts
    /// when a key is added.
    #[test]
    fn every_config_key_has_an_accessor() {
        let c = Config::default();
        // Compile-time proof that each accessor exists and returns its type.
        let _: &str = c.namespace_separator();
        let _: u32 = c.scan_count();
        let _: u64 = c.refresh_interval_ms();
        let _: &str = c.log_level();
        let _: &str = c.color_scheme();
        let _: bool = c.mouse_enabled();
        let _: bool = c.confirm_deletes();
        let _: usize = c.max_keys_in_memory();
        let _: Option<&str> = c.editor();
        let _: usize = c.max_value_display_bytes();
    }

    /// The refresh interval is the tick rate, so a zero or absurd value would
    /// either busy-loop or effectively disable the UI.
    #[test]
    fn refresh_interval_has_a_usable_default() {
        let default = Config::default().refresh_interval_ms();
        assert!(
            (MIN_REFRESH_INTERVAL_MS..=MAX_REFRESH_INTERVAL_MS).contains(&default),
            "default refresh interval of {default}ms is outside a usable range"
        );
    }

    /// Too fast would spin the event loop and hammer the server with the
    /// periodic tasks; too slow would make the UI feel wedged.
    #[test]
    fn refresh_interval_is_clamped_to_a_usable_range() {
        for too_fast in ["0", "1", "10"] {
            assert_eq!(
                parse(&format!("refresh_interval_ms = {too_fast}")).refresh_interval_ms(),
                MIN_REFRESH_INTERVAL_MS,
                "{too_fast}ms was not clamped up"
            );
        }
        assert_eq!(
            parse("refresh_interval_ms = 99999999").refresh_interval_ms(),
            MAX_REFRESH_INTERVAL_MS
        );
        // A value inside the range is passed through untouched.
        assert_eq!(
            parse("refresh_interval_ms = 500").refresh_interval_ms(),
            500
        );
        assert_eq!(parse("refresh_interval_ms = 50").refresh_interval_ms(), 50);
    }

    #[test]
    fn refresh_interval_is_read_from_config() {
        let c: Config = toml::from_str("refresh_interval_ms = 500").expect("valid TOML");
        assert_eq!(c.refresh_interval_ms(), 500);
    }
}
