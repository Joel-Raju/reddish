use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use color_eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db: u8,
    pub username: Option<String>,
    pub password: Option<PasswordRef>,
    pub last_connected: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mode: ConnectionMode,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ConnectionMode {
    #[default]
    Standalone,
    Cluster,
    Sentinel {
        master_name: String,
        sentinels: Vec<SentinelNode>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentinelNode {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TlsConfig {
    pub enabled: bool,
    pub verify_certs: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshTunnelConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PasswordRef {
    Plaintext(String),
    Env(String),
    Keychain { service: String, account: String },
}

impl PasswordRef {
    pub fn resolve(&self) -> Result<String> {
        match self {
            PasswordRef::Plaintext(s) => Ok(s.clone()),
            PasswordRef::Env(var) => std::env::var(var)
                .map_err(|_| color_eyre::eyre::eyre!("Environment variable {} not set", var)),
            PasswordRef::Keychain { .. } => {
                // keyring crate integration deferred; for M1 we return Err
                Err(color_eyre::eyre::eyre!("Keychain resolution not yet implemented"))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ConnectionStore {
    #[serde(skip)]
    pub path: PathBuf,
    pub profiles: Vec<ConnectionProfile>,
}

impl ConnectionStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path: path.to_path_buf(),
                profiles: Vec::new(),
            });
        }
        let contents = std::fs::read_to_string(path)?;
        let mut store: ConnectionStore = toml::from_str(&contents)
            .map_err(|e| color_eyre::eyre::eyre!("Invalid connections TOML: {}", e))?;
        store.path = path.to_path_buf();
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&self.path, contents)?;
        Ok(())
    }

    pub fn add(&mut self, profile: ConnectionProfile) {
        self.profiles.push(profile);
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < before
    }

    pub fn get(&self, name: &str) -> Option<&ConnectionProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }
}
