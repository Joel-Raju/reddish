pub mod connections;
pub mod keybindings;

use color_eyre::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub namespace_separator: Option<String>,
    pub scan_count: Option<u32>,
    pub refresh_interval_ms: Option<u64>,
    pub log_level: Option<String>,
    pub color_scheme: Option<String>,
    pub mouse_enabled: Option<bool>,
    pub confirm_deletes: Option<bool>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)
            .map_err(|e| color_eyre::eyre::eyre!("Invalid TOML in config file {:?}: {}", path, e))?;
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
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| color_eyre::eyre::eyre!("Could not determine config directory"))?;
    Ok(base.join("redis-tui").join("config.toml"))
}
