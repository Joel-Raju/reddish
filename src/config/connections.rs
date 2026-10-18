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

/// How a connection's password is obtained.
///
/// `Debug` is implemented by hand rather than derived: `Plaintext` holds the
/// secret itself, and the derived output would print it into any log line,
/// panic message, or test failure that formats a profile. The derived
/// `Serialize` is intentional and separate — it is what writes the value to
/// `connections.toml`.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum PasswordRef {
    Plaintext(String),
    Env(String),
    Keychain { service: String, account: String },
}

/// Placeholder substituted for a secret in `Debug` output.
const REDACTED: &str = "<redacted>";

impl std::fmt::Debug for PasswordRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Never print the secret itself.
            PasswordRef::Plaintext(_) => write!(f, "Plaintext({REDACTED})"),
            // The variable name is not a secret and is useful when diagnosing
            // a misconfigured profile.
            PasswordRef::Env(var) => write!(f, "Env({var:?})"),
            PasswordRef::Keychain { service, account } => write!(
                f,
                "Keychain {{ service: {service:?}, account: {account:?} }}"
            ),
        }
    }
}

impl PasswordRef {
    pub fn resolve(&self) -> Result<String> {
        match self {
            PasswordRef::Plaintext(s) => Ok(s.clone()),
            PasswordRef::Env(var) => std::env::var(var)
                .map_err(|_| color_eyre::eyre::eyre!("Environment variable {} not set", var)),
            PasswordRef::Keychain { service, account } => Err(color_eyre::eyre::eyre!(
                "Keychain password resolution requires store initialization. Run: `keyring set --service={} --user={} <password>`",
                service,
                account
            )),
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

#[cfg(test)]
mod secret_tests {
    use super::*;

    const SECRET: &str = "hunter2-super-secret";

    /// Spec 8.10: a plaintext password must never appear in `Debug` output,
    /// because `Debug` is what lands in log lines and panic messages.
    #[test]
    fn plaintext_password_is_redacted_in_debug() {
        let p = PasswordRef::Plaintext(SECRET.to_string());
        let rendered = format!("{p:?}");
        assert!(
            !rendered.contains(SECRET),
            "secret leaked via Debug: {rendered}"
        );
        assert!(rendered.contains("<redacted>"), "got: {rendered}");
    }

    #[test]
    fn profile_debug_does_not_leak_a_plaintext_password() {
        let profile = ConnectionProfile {
            name: "prod".to_string(),
            password: Some(PasswordRef::Plaintext(SECRET.to_string())),
            ..Default::default()
        };
        let rendered = format!("{profile:?}");
        assert!(
            !rendered.contains(SECRET),
            "secret leaked via profile Debug: {rendered}"
        );
    }

    #[test]
    fn connection_store_debug_does_not_leak() {
        let store = ConnectionStore {
            path: std::path::PathBuf::from("/tmp/connections.toml"),
            profiles: vec![ConnectionProfile {
                name: "prod".to_string(),
                password: Some(PasswordRef::Plaintext(SECRET.to_string())),
                ..Default::default()
            }],
        };
        let rendered = format!("{store:?}");
        assert!(
            !rendered.contains(SECRET),
            "secret leaked via store Debug: {rendered}"
        );
    }

    /// Losing the env var name would make a misconfigured profile hard to
    /// diagnose, and it is not a secret.
    #[test]
    fn env_password_reference_keeps_the_variable_name() {
        let p = PasswordRef::Env("REDIS_PASSWORD".to_string());
        assert_eq!(format!("{p:?}"), "Env(\"REDIS_PASSWORD\")");
    }

    #[test]
    fn keychain_reference_keeps_service_and_account() {
        let p = PasswordRef::Keychain {
            service: "redis-tui".to_string(),
            account: "prod".to_string(),
        };
        let rendered = format!("{p:?}");
        assert!(rendered.contains("redis-tui"));
        assert!(rendered.contains("prod"));
    }

    /// Redaction must not break equality or serde round-tripping.
    #[test]
    fn redaction_does_not_change_semantics() {
        let a = PasswordRef::Plaintext("a".to_string());
        let b = PasswordRef::Plaintext("a".to_string());
        let c = PasswordRef::Plaintext("b".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);

        let toml = PasswordRef::Plaintext("x".to_string());
        let encoded = toml::to_string(&toml).unwrap();
        assert!(encoded.contains("x"), "Serialize must still emit the value");
        let decoded: PasswordRef = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, toml);
    }

    #[test]
    fn resolve_still_returns_the_plaintext_value() {
        let p = PasswordRef::Plaintext(SECRET.to_string());
        assert_eq!(p.resolve().unwrap(), SECRET);
    }
}
