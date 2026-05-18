use std::net::TcpStream;
use std::time::Duration;

use color_eyre::Result;
use tokio::process::{Child, Command};

use crate::config::connections::SshTunnelConfig;

/// A running SSH local-forward tunnel.
/// The child process is killed when this struct is dropped.
pub struct SshTunnel {
    _child: Child,
    pub local_port: u16,
}

impl SshTunnel {
    /// Spawn `ssh -N -L local_port:redis_host:redis_port jump_host` and wait
    /// until the local port is accepting connections (up to `timeout`).
    pub async fn open(
        cfg: &SshTunnelConfig,
        redis_host: &str,
        redis_port: u16,
    ) -> Result<Self> {
        let forward = format!("{}:{}:{}", cfg.local_port, redis_host, redis_port);
        let destination = format!("{}@{}", cfg.user, cfg.host);

        let child = Command::new("ssh")
            .args([
                "-N",
                "-L",
                &forward,
                "-p",
                &cfg.port.to_string(),
                "-i",
                cfg.key_path.to_str().unwrap_or_default(),
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "BatchMode=yes",
                "-o",
                "ExitOnForwardFailure=yes",
                &destination,
            ])
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to spawn ssh: {e}"))?;

        wait_for_port(cfg.local_port, Duration::from_secs(10)).await?;

        Ok(Self {
            _child: child,
            local_port: cfg.local_port,
        })
    }
}

/// Poll `localhost:port` until it accepts a TCP connection or we time out.
async fn wait_for_port(port: u16, timeout: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(color_eyre::eyre::eyre!(
                "SSH tunnel: port {} not ready within {}s",
                port,
                timeout.as_secs()
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
