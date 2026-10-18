pub mod connections;
pub mod keybindings;

use color_eyre::Result;
use serde::Deserialize;
use std::path::PathBuf;

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
        self.refresh_interval_ms.unwrap_or(1000)
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

    fn parse(toml: &str) -> Config {
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
