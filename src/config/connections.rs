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

/// Build a connection profile from a `redis://` / `rediss://` URL.
///
/// Previously this lived in `main.rs` (the binary crate) with no test
/// coverage at all — the README's headline usage (`--url`) was tested
/// nowhere. Pure, so it lives here where tests can reach it.
pub fn profile_from_url(url: &str) -> color_eyre::Result<ConnectionProfile> {
    use redis::IntoConnectionInfo;
    let info = url.into_connection_info()?;

    let (host, port, tls_enabled) = match info.addr {
        redis::ConnectionAddr::Tcp(host, port) => (host, port, false),
        redis::ConnectionAddr::TcpTls { host, port, .. } => (host, port, true),
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "unsupported connection address in URL"
            ));
        }
    };

    let db = u8::try_from(info.redis.db)
        .map_err(|_| color_eyre::eyre::eyre!("URL db index out of range for u8"))?;

    Ok(ConnectionProfile {
        name: "cli-url".to_string(),
        host,
        port,
        db,
        username: info.redis.username,
        password: info.redis.password.map(PasswordRef::Plaintext),
        last_connected: None,
        mode: ConnectionMode::Standalone,
        tls: if tls_enabled {
            Some(TlsConfig {
                enabled: true,
                verify_certs: true,
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
        } else {
            None
        },
        ssh_tunnel: None,
    })
}

#[cfg(test)]
mod url_tests {
    use super::profile_from_url;

    #[test]
    fn plain_redis_url_parses_every_field() {
        let p = profile_from_url("redis://user:secret@db.example:6380/3").unwrap();
        assert_eq!(p.host, "db.example");
        assert_eq!(p.port, 6380);
        assert_eq!(p.db, 3);
        assert_eq!(p.username.as_deref(), Some("user"));
        assert!(
            matches!(p.password, Some(crate::config::connections::PasswordRef::Plaintext(ref s)) if s == "secret"),
            "the URL password must arrive as a plaintext ref"
        );
        assert!(p.tls.is_none(), "redis:// must not enable TLS");
    }

    #[test]
    fn defaults_apply_when_url_has_no_extras() {
        let p = profile_from_url("redis://127.0.0.1").unwrap();
        assert_eq!(p.host, "127.0.0.1");
        assert_eq!(p.port, 6379, "the redis crate applies the default port");
        assert_eq!(p.db, 0);
    }

    #[test]
    fn rediss_enables_tls_with_verification() {
        let p = profile_from_url("rediss://secure.example:6380/0").unwrap();
        let tls = p.tls.as_ref().expect("rediss:// must enable TLS");
        assert!(tls.enabled);
        assert!(tls.verify_certs, "TLS verification must default to on");
    }

    #[test]
    fn a_db_index_that_cannot_fit_u8_is_rejected() {
        // db=16 is *valid* here: how many databases a server has is
        // server-side state (databases=32 servers exist), so the client
        // accepts it and lets SELECT fail if the server disagrees. Only
        // values that cannot fit the u8 field are rejected up front.
        profile_from_url("redis://127.0.0.1:6379/16").unwrap();
        let err = profile_from_url("redis://127.0.0.1:6379/300").unwrap_err();
        assert!(
            err.to_string().contains("out of range"),
            "db 300 must be rejected, got: {err}"
        );
    }

    #[test]
    fn garbage_is_rejected_not_panicked() {
        assert!(profile_from_url("not a url at all").is_err());
        assert!(profile_from_url("http://wrong-scheme").is_err());
    }
}
