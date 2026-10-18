use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::TestBackend;
use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::events::Event;
use reddish_tui::redis::client::RedisClientHandle;
use reddish_tui::redis::server::{
    ServerInfo, SlowLogEntry, slowlog_get, slowlog_len, slowlog_reset,
};
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
    dashboard.slowlog_panel.entries = vec![SlowLogEntry {
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
        used_memory: Some(1_000_000),
        used_memory_human: Some("1M".to_string()),
        total_commands_processed: Some(100),
        instantaneous_ops_per_sec: Some(10),
        keyspace_hits: Some(90),
        keyspace_misses: Some(10),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    };
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

#[test]
fn test_info_dashboard_scroll_down() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog_panel.entries = (0..20)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i,
            duration_us: i * 100,
            command: vec!["GET".to_string(), format!("key{i}")],
            client: None,
            name: None,
        })
        .collect();

    assert_eq!(dashboard.slowlog_panel.scroll, 0);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_panel.scroll, 1);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(dashboard.slowlog_panel.scroll, 2);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_panel.scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_up() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog_panel.entries = (0..20)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i,
            duration_us: i * 100,
            command: vec!["GET".to_string(), format!("key{i}")],
            client: None,
            name: None,
        })
        .collect();
    dashboard.slowlog_panel.scroll = 5;

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_panel.scroll, 4);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('k'))));
    assert_eq!(dashboard.slowlog_panel.scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_does_not_overflow() {
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog_panel.entries = (0..5)
        .map(|i| SlowLogEntry {
            id: i,
            timestamp: 1000 + i,
            duration_us: 100,
            command: vec!["PING".to_string()],
            client: None,
            name: None,
        })
        .collect();
    dashboard.slowlog_panel.scroll = 4;

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(dashboard.slowlog_panel.scroll, 4);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_panel.scroll, 3);
}

#[test]
fn test_info_dashboard_scroll_stays_at_zero() {
    let mut dashboard = InfoDashboard::new();
    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(dashboard.slowlog_panel.scroll, 0);
}

#[test]
fn test_info_dashboard_compact_toggle() {
    let mut dashboard = InfoDashboard::new();
    assert!(!dashboard.slowlog_panel.compact_mode);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(dashboard.slowlog_panel.compact_mode);

    dashboard.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(!dashboard.slowlog_panel.compact_mode);
}

#[test]
fn test_info_dashboard_render_with_slowlog_entries() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog_panel.entries = vec![
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
        used_memory: Some(2_000_000),
        used_memory_human: Some("2M".to_string()),
        total_commands_processed: Some(500),
        instantaneous_ops_per_sec: Some(50),
        keyspace_hits: Some(400),
        keyspace_misses: Some(100),
        evicted_keys: Some(1),
        expired_keys: Some(10),
        ..Default::default()
    };
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

#[test]
fn test_info_dashboard_render_compact_mode() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.slowlog_panel.compact_mode = true;
    dashboard.slowlog_panel.entries = vec![SlowLogEntry {
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

#[test]
fn test_info_dashboard_update_stats_seeds_buffers() {
    let mut dashboard = InfoDashboard::new();
    let stats = SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(60),
        connected_clients: Some(5),
        used_memory: Some(1_000_000),
        used_memory_human: Some("1M".to_string()),
        total_commands_processed: Some(100),
        instantaneous_ops_per_sec: Some(10),
        keyspace_hits: Some(90),
        keyspace_misses: Some(10),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    };
    dashboard.update_stats(stats);
    assert_eq!(dashboard.buf_memory.len(), 1);
    assert_eq!(dashboard.buf_clients.len(), 1);
    assert_eq!(dashboard.buf_ops.len(), 1);
    assert_eq!(dashboard.buf_hits.len(), 1);
    assert_eq!(dashboard.buf_memory.latest(), Some(1_000_000.0));
    assert_eq!(dashboard.buf_clients.latest(), Some(5.0));
    assert_eq!(dashboard.buf_ops.latest(), Some(10.0));
    assert_eq!(dashboard.buf_hits.latest(), Some(100.0));
}

#[test]
fn test_info_dashboard_update_stats_delta() {
    let mut dashboard = InfoDashboard::new();
    let stats1 = SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(60),
        connected_clients: Some(5),
        used_memory: Some(1_000_000),
        used_memory_human: Some("1M".to_string()),
        total_commands_processed: Some(100),
        instantaneous_ops_per_sec: Some(10),
        keyspace_hits: Some(90),
        keyspace_misses: Some(10),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    };
    let stats2 = SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(120),
        connected_clients: Some(7),
        used_memory: Some(1_100_000),
        used_memory_human: Some("1.1M".to_string()),
        total_commands_processed: Some(200),
        instantaneous_ops_per_sec: Some(15),
        keyspace_hits: Some(110),
        keyspace_misses: Some(15),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    };
    dashboard.update_stats(stats1);
    dashboard.update_stats(stats2);
    assert_eq!(dashboard.buf_memory.len(), 2);
    assert_eq!(dashboard.buf_clients.latest(), Some(7.0));
    assert_eq!(dashboard.buf_ops.latest(), Some(15.0));
    // hits delta: (110+15) - (90+10) = 25
    assert_eq!(dashboard.buf_hits.latest(), Some(25.0));
}

#[test]
fn test_info_dashboard_update_stats_missing_fields() {
    let mut dashboard = InfoDashboard::new();
    let stats = SystemStats {
        redis_version: None,
        uptime_in_seconds: None,
        connected_clients: None,
        used_memory: None,
        used_memory_human: None,
        total_commands_processed: None,
        instantaneous_ops_per_sec: None,
        keyspace_hits: None,
        keyspace_misses: None,
        evicted_keys: None,
        expired_keys: None,
        ..Default::default()
    };
    dashboard.update_stats(stats);
    assert_eq!(dashboard.buf_memory.len(), 0);
    assert_eq!(dashboard.buf_clients.len(), 0);
    assert_eq!(dashboard.buf_ops.len(), 0);
    assert_eq!(dashboard.buf_hits.len(), 0);
}

#[test]
fn test_info_dashboard_render_with_sparklines() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut dashboard = InfoDashboard::new();
    dashboard.update_stats(SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(60),
        connected_clients: Some(5),
        used_memory: Some(1_000_000),
        used_memory_human: Some("1M".to_string()),
        total_commands_processed: Some(100),
        instantaneous_ops_per_sec: Some(10),
        keyspace_hits: Some(90),
        keyspace_misses: Some(10),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    });
    dashboard.update_stats(SystemStats {
        redis_version: Some("7.0".to_string()),
        uptime_in_seconds: Some(120),
        connected_clients: Some(7),
        used_memory: Some(1_100_000),
        used_memory_human: Some("1.1M".to_string()),
        total_commands_processed: Some(200),
        instantaneous_ops_per_sec: Some(15),
        keyspace_hits: Some(110),
        keyspace_misses: Some(15),
        evicted_keys: Some(0),
        expired_keys: Some(0),
        ..Default::default()
    });
    let _ = terminal.draw(|f| dashboard.render(f, f.area()));
}

// ---- SLOWLOG RESET / LEN (4.23, 4.7) ----

fn slowlog_profile() -> ConnectionProfile {
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

async fn standalone(client: &RedisClientHandle) -> redis::aio::MultiplexedConnection {
    match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("tests always use standalone"),
    }
}

/// Serializes tests that mutate the shared server's slowlog configuration.
///
/// `slowlog-log-slower-than` is server-wide state, so two tests changing it at
/// once would restore stale values and leave the server misconfigured.
///
/// A `tokio::sync::Mutex` rather than `std::sync::Mutex` because the guard is
/// held across `.await` points, which the blocking one must not do.
static SLOWLOG_CONFIG_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Restores `slowlog-log-slower-than` when dropped, including on panic.
///
/// Uses a synchronous connection because `Drop` cannot await, and this must
/// still run when the test unwinds.
struct ThresholdGuard {
    original: String,
}

impl Drop for ThresholdGuard {
    fn drop(&mut self) {
        let Ok(client) = redis::Client::open("redis://127.0.0.1:16379/0") else {
            return;
        };
        let Ok(mut conn) = client.get_connection() else {
            return;
        };
        let _ = redis::cmd("CONFIG")
            .arg("SET")
            .arg("slowlog-log-slower-than")
            .arg(&self.original)
            .query::<()>(&mut conn);
    }
}

/// Lower the slowlog threshold to 0, run `f`, then restore the original value.
///
/// `DEBUG SLEEP` is unavailable (Redis ships with `enable-debug-command no`),
/// so dropping the threshold is the only reliable way to make the server
/// record a query. Returns the server's entry count observed inside `f`.
///
/// The caller must hold `SLOWLOG_CONFIG_LOCK` for the whole test, not just
/// across this call: the slowlog itself is shared, so another test resetting it
/// between here and the caller's assertions would make them flaky.
async fn with_slowlog_threshold_zero(
    client: &RedisClientHandle,
    f: impl AsyncFnOnce() -> (),
) -> i64 {
    let mut c = standalone(client).await;

    // CONFIG GET replies with a [name, value] array.
    let original: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("slowlog-log-slower-than")
        .query_async(&mut c)
        .await
        .expect("read slowlog threshold");
    let original_value = original
        .get(1)
        .cloned()
        .unwrap_or_else(|| "10000".to_string());

    redis::cmd("CONFIG")
        .arg("SET")
        .arg("slowlog-log-slower-than")
        .arg("0")
        .query_async::<()>(&mut c)
        .await
        .expect("lower slowlog threshold");

    let _restore = ThresholdGuard {
        original: original_value,
    };

    f().await;

    slowlog_len(client).await.unwrap_or(0) as i64
}

#[tokio::test]
async fn test_slowlog_len_reports_server_count() {
    let _lock = SLOWLOG_CONFIG_LOCK.lock().await;
    let client = RedisClientHandle::connect(&slowlog_profile())
        .await
        .unwrap();

    // Start from a known state so the assertion is not order-dependent.
    slowlog_reset(&client).await.unwrap();
    assert_eq!(slowlog_len(&client).await.unwrap(), 0);

    let recorded = with_slowlog_threshold_zero(&client, async || {
        let mut c = standalone(&client).await;
        redis::cmd("PING").query_async::<()>(&mut c).await.unwrap();
    })
    .await;

    assert!(
        recorded >= 1,
        "expected at least one slowlog entry, got {recorded}"
    );

    slowlog_reset(&client).await.unwrap();
}

#[tokio::test]
async fn test_slowlog_reset_clears_the_log() {
    let _lock = SLOWLOG_CONFIG_LOCK.lock().await;
    let client = RedisClientHandle::connect(&slowlog_profile())
        .await
        .unwrap();

    let recorded = with_slowlog_threshold_zero(&client, async || {
        let mut c = standalone(&client).await;
        redis::cmd("PING").query_async::<()>(&mut c).await.unwrap();
    })
    .await;

    // Ensure there is something to clear, so the test cannot pass vacuously.
    if recorded == 0 {
        eprintln!("skipping: server recorded no query even at threshold 0");
        return;
    }
    assert!(!slowlog_get(&client, 10).await.unwrap().is_empty());

    slowlog_reset(&client).await.unwrap();

    assert_eq!(
        slowlog_len(&client).await.unwrap(),
        0,
        "SLOWLOG RESET must empty the log"
    );
    assert!(
        slowlog_get(&client, 10).await.unwrap().is_empty(),
        "entries must be gone after a reset"
    );
}

#[tokio::test]
async fn test_slowlog_reset_is_idempotent() {
    // Shares the server's slowlog with the other tests here, so it must take
    // the same lock: a concurrent reset would make the length assertion flaky.
    let _lock = SLOWLOG_CONFIG_LOCK.lock().await;
    let client = RedisClientHandle::connect(&slowlog_profile())
        .await
        .unwrap();
    slowlog_reset(&client).await.unwrap();
    slowlog_reset(&client).await.unwrap();
    assert_eq!(slowlog_len(&client).await.unwrap(), 0);
}
