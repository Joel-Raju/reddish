use reddish_tui::app::App;
use reddish_tui::backoff::backoff_sequence;
use reddish_tui::config::connections::{
    ConnectionMode, ConnectionProfile, SentinelNode, SshTunnelConfig, TlsConfig,
};
use reddish_tui::redis::client::parse_cluster_nodes;
use reddish_tui::ui::connection_screen::ConnectionScreen;
use reddish_tui::ui::repl::parse_pipeline;

#[test]
fn test_parse_cluster_nodes() {
    let raw = "07c37dfeb235213a872192d90877d0cd55XXXX 10.0.0.1:7000@17000 myself,master - 0 1 0 connected 0-5460\n\
                 67ed2db8d677a963000000000000000000000 10.0.0.2:7000@17000 master - 0 1 0 connected 5461-10922\n\
                 292f8b365bb7edb5e0caf67a305441XXXXXXXXXXXXXXXX 10.0.0.3:7000@17000 master - 0 1 0 connected 10923-16383";

    let nodes = parse_cluster_nodes(raw).unwrap();
    assert_eq!(nodes.len(), 3);

    let myself = nodes
        .iter()
        .find(|n| n.flags.contains(&"myself".to_string()))
        .unwrap();
    assert!(myself.flags.contains(&"master".to_string()));
    assert_eq!(myself.slots.len(), 1);
    assert_eq!(myself.slots[0], (0, 5460));

    // Check all slots cover 0..=16383
    let mut all_slots: Vec<(u16, u16)> = nodes.iter().flat_map(|n| n.slots.clone()).collect();
    all_slots.sort_by_key(|s| s.0);
    assert_eq!(all_slots[0], (0, 5460));
    assert_eq!(all_slots[1], (5461, 10922));
    assert_eq!(all_slots[2], (10923, 16383));
}

#[test]
fn test_reconnect_backoff_sequence() {
    let seq = backoff_sequence();
    assert_eq!(seq.len(), 7);

    let expected_ms = [100, 200, 400, 800, 1600, 5000, 30000];
    for (i, dur) in seq.iter().enumerate() {
        let ms = dur.as_millis() as f64;
        let expected = expected_ms[i] as f64;
        let tolerance = expected * 0.15;
        assert!(
            (ms - expected).abs() <= tolerance,
            "backoff[{}] = {}ms, expected ~{}ms",
            i,
            ms,
            expected
        );
    }
}

#[test]
fn test_tls_config_roundtrip() {
    let config = TlsConfig {
        enabled: true,
        verify_certs: false,
        ca_cert_path: Some(std::path::PathBuf::from("/tmp/ca.crt")),
        client_cert_path: Some(std::path::PathBuf::from("/tmp/client.crt")),
        client_key_path: Some(std::path::PathBuf::from("/tmp/client.key")),
    };
    let toml = toml::to_string(&config).unwrap();
    let back: TlsConfig = toml::from_str(&toml).unwrap();
    assert_eq!(config.enabled, back.enabled);
    assert_eq!(config.verify_certs, back.verify_certs);
    assert_eq!(config.ca_cert_path, back.ca_cert_path);
    assert_eq!(config.client_cert_path, back.client_cert_path);
    assert_eq!(config.client_key_path, back.client_key_path);
}

#[test]
fn test_ssh_tunnel_config_roundtrip() {
    use std::path::PathBuf;
    let cfg = SshTunnelConfig {
        host: "bastion.example.com".to_string(),
        port: 22,
        user: "deploy".to_string(),
        key_path: PathBuf::from("/home/deploy/.ssh/id_ed25519"),
        local_port: 16379,
    };
    let toml = toml::to_string(&cfg).unwrap();
    let back: SshTunnelConfig = toml::from_str(&toml).unwrap();
    assert_eq!(cfg.host, back.host);
    assert_eq!(cfg.port, back.port);
    assert_eq!(cfg.user, back.user);
    assert_eq!(cfg.key_path, back.key_path);
    assert_eq!(cfg.local_port, back.local_port);
}

#[test]
fn test_ssh_tunnel_in_connection_profile_roundtrip() {
    use std::path::PathBuf;
    let profile = ConnectionProfile {
        name: "tunneled".to_string(),
        host: "private-redis.internal".to_string(),
        port: 6379,
        db: 1,
        username: None,
        password: None,
        last_connected: None,
        mode: ConnectionMode::Standalone,
        tls: None,
        ssh_tunnel: Some(SshTunnelConfig {
            host: "bastion.example.com".to_string(),
            port: 22,
            user: "ops".to_string(),
            key_path: PathBuf::from("/home/ops/.ssh/id_rsa"),
            local_port: 26379,
        }),
    };
    let toml = toml::to_string(&profile).unwrap();
    let back: ConnectionProfile = toml::from_str(&toml).unwrap();
    let tunnel = back.ssh_tunnel.expect("ssh_tunnel should round-trip");
    assert_eq!(tunnel.host, "bastion.example.com");
    assert_eq!(tunnel.local_port, 26379);
    assert_eq!(back.host, "private-redis.internal");
}

#[test]
fn test_connection_screen_renders_without_panic() {
    use ratatui::backend::TestBackend;
    use reddish_tui::config::connections::ConnectionStore;
    use std::path::PathBuf;

    let store = ConnectionStore {
        path: PathBuf::new(),
        profiles: vec![
            ConnectionProfile {
                name: "prod".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                username: None,
                password: None,
                last_connected: None,
                mode: ConnectionMode::Standalone,
                tls: None,
                ssh_tunnel: None,
            },
            ConnectionProfile {
                name: "dev".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6380,
                db: 0,
                username: None,
                password: None,
                last_connected: None,
                mode: ConnectionMode::Standalone,
                tls: None,
                ssh_tunnel: None,
            },
            ConnectionProfile {
                name: "cluster".to_string(),
                host: "127.0.0.1".to_string(),
                port: 7000,
                db: 0,
                username: None,
                password: None,
                last_connected: None,
                mode: ConnectionMode::Cluster,
                tls: None,
                ssh_tunnel: None,
            },
        ],
    };
    let screen = ConnectionScreen::new(store);
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| screen.render(f, f.area()));
}

#[tokio::test]
async fn test_readonly_repl_blocks_set() {
    let mut app = App::new(reddish_tui::config::Config::default());
    app.readonly = true;

    // Drive execute_repl_command directly; no Redis connection needed because
    // the policy check fires before any client call.
    reddish_tui::app::execute_repl_command_for_test(&mut app, "SET foo bar".to_string()).await;

    assert!(
        app.error_message
            .as_deref()
            .is_some_and(|m| m.contains("Read-only mode")),
        "expected Read-only error, got: {:?}",
        app.error_message,
    );
    assert!(
        !app.status_bar.error_notifications.is_empty(),
        "error should also appear in status bar toast"
    );
}

#[tokio::test]
async fn test_readonly_repl_allows_get() {
    let mut app = App::new(reddish_tui::config::Config::default());
    app.readonly = true;

    // GET is a read command — the policy should not block it.
    // Without a client the stage will fail with "Not connected", NOT with a policy error.
    reddish_tui::app::execute_repl_command_for_test(&mut app, "GET foo".to_string()).await;

    assert!(
        app.error_message
            .as_deref()
            .map(|m| !m.contains("Read-only mode"))
            .unwrap_or(true),
        "GET should not be blocked by read-only policy"
    );
}

#[test]
fn test_danger_policy_blocks_flushdb() {
    let stages = parse_pipeline("FLUSHDB");
    let err = reddish_tui::app::repl_policy_error_for_test(&stages, false);
    assert!(
        err.as_deref()
            .is_some_and(|m| m.contains("blocked by safety policy")),
        "FLUSHDB should be blocked unconditionally, got: {err:?}"
    );
}

#[test]
fn test_danger_policy_blocks_keys_star() {
    let stages = parse_pipeline("KEYS *");
    let err = reddish_tui::app::repl_policy_error_for_test(&stages, false);
    assert!(
        err.as_deref()
            .is_some_and(|m| m.contains("blocked by safety policy")),
        "KEYS * should be blocked, got: {err:?}"
    );
}

#[test]
fn test_danger_policy_allows_keys_prefix() {
    // KEYS with a non-wildcard pattern is not blocked by danger policy
    let stages = parse_pipeline("KEYS user:*");
    let err = reddish_tui::app::repl_policy_error_for_test(&stages, false);
    assert!(err.is_none(), "KEYS user:* should not be blocked, got: {err:?}");
}

#[test]
fn test_readonly_policy_blocks_del() {
    let stages = parse_pipeline("DEL mykey");
    let err = reddish_tui::app::repl_policy_error_for_test(&stages, true);
    assert!(
        err.as_deref().is_some_and(|m| m.contains("Read-only mode")),
        "DEL should be blocked in readonly, got: {err:?}"
    );
}

#[test]
fn test_readonly_policy_allows_hgetall() {
    let stages = parse_pipeline("HGETALL myhash");
    let err = reddish_tui::app::repl_policy_error_for_test(&stages, true);
    assert!(err.is_none(), "HGETALL should not be blocked in readonly");
}

#[test]
fn test_connection_profile_with_mode_roundtrip() {
    let profile = ConnectionProfile {
        name: "sentinel".to_string(),
        host: "127.0.0.1".to_string(),
        port: 26379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        mode: ConnectionMode::Sentinel {
            master_name: "mymaster".to_string(),
            sentinels: vec![SentinelNode {
                host: "127.0.0.1".to_string(),
                port: 26379,
            }],
        },
        tls: Some(TlsConfig {
            enabled: true,
            verify_certs: true,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }),
        ssh_tunnel: Some(SshTunnelConfig {
            host: "bastion.example.com".to_string(),
            port: 22,
            user: "admin".to_string(),
            key_path: std::path::PathBuf::from("/tmp/key"),
            local_port: 0,
        }),
    };
    let toml = toml::to_string(&profile).unwrap();
    let back: ConnectionProfile = toml::from_str(&toml).unwrap();
    assert_eq!(profile.name, back.name);
    assert_eq!(profile.mode, back.mode);
    assert_eq!(profile.tls, back.tls);
    assert_eq!(profile.ssh_tunnel, back.ssh_tunnel);
}
