use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::TestBackend;
use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::events::Event;
use reddish_tui::redis::client::RedisClientHandle;
use reddish_tui::redis::server::{ServerInfo, SlowLogEntry, slowlog_get};
use reddish_tui::ui::info_dashboard::{InfoDashboard, SystemStats};

fn test_profile() -> ConnectionProfile {
    ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    }
}

#[tokio::test]
async fn test_info_sections_present() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let info: String = client.info("default").await.unwrap();

    assert!(info.contains("# Server"));
    assert!(info.contains("redis_version:"));
    assert!(info.contains("# Stats"));

    let parsed = ServerInfo(info);
    let version = parsed.section("Server");
    assert!(version.is_some());
    assert!(version.unwrap().contains("redis_version"));

    let version_val = parsed.key_value("Server", "redis_version");
    assert!(version_val.is_some());
}

#[tokio::test]
async fn test_slowlog_parses_without_panic() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let entries = slowlog_get(&client, 10).await.unwrap();
    // Should not panic even if empty
    let _ = entries.len();
}

#[test]
fn test_system_stats_from_info() {
    let info = concat!(
        "# Server\r\n",
        "redis_version:7.0.0\r\n",
        "uptime_in_seconds:1234\r\n",
        "\r\n",
        "# Clients\r\n",
        "connected_clients:42\r\n",
        "\r\n",
        "# Memory\r\n",
        "used_memory_human:1.23M\r\n",
        "\r\n",
        "# Stats\r\n",
        "total_commands_processed:999\r\n",
        "instantaneous_ops_per_sec:10\r\n",
        "keyspace_hits:100\r\n",
        "keyspace_misses:5\r\n",
        "evicted_keys:0\r\n",
        "expired_keys:2\r\n",
    );
    let stats = SystemStats::from_info_sections(info);
    assert_eq!(stats.redis_version, Some("7.0.0".to_string()));
    assert_eq!(stats.uptime_in_seconds, Some(1234));
    assert_eq!(stats.connected_clients, Some(42));
    assert_eq!(stats.used_memory_human, Some("1.23M".to_string()));
    assert_eq!(stats.total_commands_processed, Some(999));
    assert_eq!(stats.instantaneous_ops_per_sec, Some(10));
    assert_eq!(stats.keyspace_hits, Some(100));
    assert_eq!(stats.keyspace_misses, Some(5));
    assert_eq!(stats.evicted_keys, Some(0));
    assert_eq!(stats.expired_keys, Some(2));
}

#[test]
fn test_info_dashboard_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog = vec![SlowLogEntry {
        id: 1,
        timestamp: 1234567890,
        duration_us: 150,
        command: vec!["GET".to_string(), "foo".to_string()],
        client: None,
        name: None,
    }];
    dashboard.stats = SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(60),
        connected_clients: Some(1),
        used_memory_human: Some("1M".to_string()),
        total_commands_processed: Some(100),
        instantaneous_ops_per_sec: Some(10),
        keyspace_hits: Some(90),
        keyspace_misses: Some(10),
        evicted_keys: Some(0),
        expired_keys: Some(0),
    };
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

#[test]
fn test_info_dashboard_scroll_down() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog = (0..20)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i as u64,
            duration_us: i as u64 * 100,
            command: vec!["GET".to_string(), format!("key{i}")],
            client: None,
            name: None,
        })
        .collect();

    assert_eq!(dashboard.slowlog_scroll, 0);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_scroll, 1);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(dashboard.slowlog_scroll, 2);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_up() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog = (0..20)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i as u64,
            duration_us: i as u64 * 100,
            command: vec!["GET".to_string(), format!("key{i}")],
            client: None,
            name: None,
        })
        .collect();
    dashboard.slowlog_scroll = 5;

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_scroll, 4);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('k'))));
    assert_eq!(dashboard.slowlog_scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_does_not_overflow() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog = (0..5)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i as u64,
            duration_us: 100,
            command: vec!["PING".to_string()],
            client: None,
            name: None,
        })
        .collect();
    dashboard.slowlog_scroll = 4;

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_scroll, 4);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_stays_at_zero() {
    let mut dashboard = InfoDashboard::new();
    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_scroll, 0);
}

#[test]
fn test_info_dashboard_compact_toggle() {
    let mut dashboard = InfoDashboard::new();
    assert!(!dashboard.compact_mode);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(dashboard.compact_mode);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(!dashboard.compact_mode);
}

#[test]
fn test_info_dashboard_render_with_slowlog_entries() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog = vec![
        SlowLogEntry {
            id: 1,
            timestamp: 1000,
            duration_us: 200,
            command: vec!["SET".to_string(), "a".to_string(), "b".to_string()],
            client: None,
            name: None,
        },
        SlowLogEntry {
            id: 2,
            timestamp: 2000,
            duration_us: 300,
            command: vec!["GET".to_string(), "x".to_string()],
            client: None,
            name: None,
        },
    ];
    dashboard.stats = SystemStats {
        redis_version: Some("6.0".to_string()),
        uptime_in_seconds: Some(3600),
        connected_clients: Some(5),
        used_memory_human: Some("2M".to_string()),
        total_commands_processed: Some(500),
        instantaneous_ops_per_sec: Some(50),
        keyspace_hits: Some(400),
        keyspace_misses: Some(100),
        evicted_keys: Some(1),
        expired_keys: Some(10),
    };
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

#[test]
fn test_info_dashboard_render_compact_mode() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.compact_mode = true;
    dashboard.slowlog = vec![SlowLogEntry {
        id: 42,
        timestamp: 3000,
        duration_us: 500,
        command: vec!["FLUSHALL".to_string()],
        client: None,
        name: None,
    }];
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

#[test]
fn test_info_dashboard_render_empty_slowlog() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let dashboard = InfoDashboard::new();
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}
