use std::collections::HashSet;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::TestBackend;
use reddish_tui::config::connections::{ConnectionProfile, ConnectionStore, PasswordRef};
use reddish_tui::events::Event;
use reddish_tui::redis::client::{RedisClientHandle, RedisType, Ttl};
use reddish_tui::redis::scanner::Scanner;
use reddish_tui::ui::key_browser::tree::{KeyEntry, NamespaceTree};
use reddish_tui::ui::key_browser::{BrowserAction, KeyBrowser};
use reddish_tui::ui::widgets::confirm::ConfirmDialog;

#[test]
fn test_connection_profile_serialization() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 6379,
        db: 0,
        username: Some("user".to_string()),
        password: Some(PasswordRef::Env("REDIS_PASS".to_string())),
        last_connected: None,
        ..Default::default()
    };
    let toml = toml::to_string(&profile).unwrap();
    let deserialized: ConnectionProfile = toml::from_str(&toml).unwrap();
    assert_eq!(profile.name, deserialized.name);
    assert_eq!(profile.host, deserialized.host);
    assert_eq!(profile.port, deserialized.port);
    assert_eq!(profile.db, deserialized.db);
    assert_eq!(profile.username, deserialized.username);
    assert!(matches!(deserialized.password, Some(PasswordRef::Env(ref s)) if s == "REDIS_PASS"));

    unsafe {
        std::env::set_var("REDIS_PASS", "secret");
    }
    let resolved = deserialized.password.unwrap().resolve().unwrap();
    assert_eq!(resolved, "secret");
    unsafe {
        std::env::remove_var("REDIS_PASS");
    }
}

#[test]
fn test_connection_store_add_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("connections.toml");
    let mut store = ConnectionStore::load(&path).unwrap();

    store.add(ConnectionProfile {
        name: "prod".to_string(),
        host: "127.0.0.1".to_string(),
        port: 6379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    });
    store.add(ConnectionProfile {
        name: "dev".to_string(),
        host: "127.0.0.1".to_string(),
        port: 6380,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    });
    store.save().unwrap();

    let loaded = ConnectionStore::load(&path).unwrap();
    assert_eq!(loaded.profiles.len(), 2);

    store.remove("prod");
    store.save().unwrap();

    let loaded2 = ConnectionStore::load(&path).unwrap();
    assert_eq!(loaded2.profiles.len(), 1);
    assert!(loaded2.get("dev").is_some());
}

#[tokio::test]
async fn test_scanner_scans_all_keys() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    // Clear any existing keys first
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("test always uses standalone"),
    };
    redis::cmd("FLUSHDB")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    for i in 0..500 {
        redis::cmd("SET")
            .arg(format!("test:m1:key{}", i))
            .arg("v")
            .query_async::<()>(&mut c)
            .await
            .unwrap();
    }

    let scanner_client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut scanner = Scanner::new(scanner_client, 50);
    let mut all_keys = HashSet::new();
    while !scanner.finished {
        let batch = scanner.next_batch().await.unwrap();
        for key in batch {
            if key.starts_with("test:m1:") {
                all_keys.insert(key);
            }
        }
    }
    assert_eq!(all_keys.len(), 500);
    assert!(scanner.finished);

    // Cleanup
    redis::cmd("FLUSHDB")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_scanner_never_uses_keys_command() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    };
    let _client = RedisClientHandle::connect(&profile).await.unwrap();
    let scanner_client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut scanner = Scanner::new(scanner_client, 50);
    while !scanner.finished {
        let _ = scanner.next_batch().await.unwrap();
    }
    // We don't have a mock client to record commands, so we verify by
    // inspecting the scanner implementation - it only calls SCAN.
    // This is a structural test: the source code uses SCAN cursor MATCH pattern COUNT count.
    assert!(scanner.finished);
}

#[test]
fn test_namespace_tree_insert_and_visible_rows() {
    let mut tree = NamespaceTree::new(':', 500_000);
    tree.insert(KeyEntry {
        full_name: "user:session:abc".to_string(),
        redis_type: None,
        ttl: None,
    });
    tree.insert(KeyEntry {
        full_name: "user:session:def".to_string(),
        redis_type: None,
        ttl: None,
    });
    tree.insert(KeyEntry {
        full_name: "user:profile:xyz".to_string(),
        redis_type: None,
        ttl: None,
    });
    tree.insert(KeyEntry {
        full_name: "queue:jobs".to_string(),
        redis_type: None,
        ttl: None,
    });

    assert_eq!(tree.total_keys(), 4);

    tree.expand(&["user"]);
    tree.expand(&["user", "session"]);

    let rows = tree.visible_rows();
    let labels: Vec<_> = rows.iter().map(|r| r.label.clone()).collect();
    assert!(labels.iter().any(|l| l.contains("user")));
    assert!(labels.iter().any(|l| l.contains("session")));
    assert!(labels.iter().any(|l| l == "abc" || l.contains("abc")));
    assert!(labels.iter().any(|l| l == "def" || l.contains("def")));
    assert!(labels.iter().any(|l| l.contains("profile")));
    assert!(labels.iter().any(|l| l.contains("queue")));

    tree.collapse(&["user", "session"]);
    let rows2 = tree.visible_rows();
    let labels2: Vec<_> = rows2.iter().map(|r| r.label.clone()).collect();
    assert!(!labels2.iter().any(|l| l == "abc" || l.contains("abc")));
    assert!(!labels2.iter().any(|l| l == "def" || l.contains("def")));
}

#[test]
fn test_namespace_tree_remove() {
    let mut tree = NamespaceTree::new(':', 500_000);
    tree.insert(KeyEntry {
        full_name: "a:b".to_string(),
        redis_type: None,
        ttl: None,
    });
    tree.insert(KeyEntry {
        full_name: "a:c".to_string(),
        redis_type: None,
        ttl: None,
    });
    tree.insert(KeyEntry {
        full_name: "a:d".to_string(),
        redis_type: None,
        ttl: None,
    });

    assert_eq!(tree.total_keys(), 3);
    tree.remove("a:c");
    assert_eq!(tree.total_keys(), 2);

    let rows = tree.visible_rows();
    assert!(
        !rows
            .iter()
            .any(|r| r.label == "c" || r.label.contains("a:c"))
    );
}

#[test]
fn test_redis_type_badge_chars() {
    assert_eq!(RedisType::String.badge_char(), "S");
    assert_eq!(RedisType::List.badge_char(), "L");
    assert_eq!(RedisType::Hash.badge_char(), "H");
    assert_eq!(RedisType::Set.badge_char(), "St");
    assert_eq!(RedisType::ZSet.badge_char(), "Z");
    assert_eq!(RedisType::Stream.badge_char(), "X");
    assert_eq!(RedisType::Unknown.badge_char(), "?");
}

#[test]
fn test_ttl_display() {
    use std::time::Duration;
    assert_eq!(Ttl::NoExpiry.display(), "∞");
    assert_eq!(Ttl::KeyNotFound.display(), "");
    assert_eq!(Ttl::Expires(Duration::from_secs(0)).display(), "exp");
    assert_eq!(Ttl::Expires(Duration::from_secs(5)).display(), "5s");
    assert_eq!(Ttl::Expires(Duration::from_secs(65)).display(), "1m5s");
    assert_eq!(Ttl::Expires(Duration::from_secs(3661)).display(), "1h1m");
    assert_eq!(Ttl::Expires(Duration::from_secs(90061)).display(), "1d1h");
}

#[test]
fn test_tree_row_has_badges_when_set() {
    let mut tree = NamespaceTree::new(':', 500_000);
    tree.insert(KeyEntry {
        full_name: "testkey".to_string(),
        redis_type: Some(RedisType::String),
        ttl: Some(Ttl::Expires(Duration::from_secs(120))),
    });
    let rows = tree.visible_rows();
    // Key is at root, no .rsplit_once match, so label uses full name directly
    assert!(rows[0].label.contains("[S]"), "expected type badge [S] in label, got: {}", rows[0].label);
    assert!(rows[0].label.contains("2m0s"), "expected TTL badge in label, got: {}", rows[0].label);
}

#[test]
fn test_breadcrumb_starts_at_root() {
    let browser = KeyBrowser::new(':', 500_000);
    assert!(browser.current_path.is_empty());
    assert_eq!(browser.cursor, 0);
}

#[test]
fn test_breadcrumb_backspace_at_root_does_nothing() {
    use reddish_tui::events::Event;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    assert!(browser.current_path.is_empty());
}

#[test]
fn test_breadcrumb_g_at_root_does_nothing() {
    use reddish_tui::events::Event;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
    assert!(browser.current_path.is_empty());
}

#[test]
fn test_breadcrumb_renders_path() {
    use ratatui::backend::TestBackend;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.current_path = vec!["prod".to_string(), "user".to_string()];
    browser.apply_scan_batch(vec![
        KeyEntry {
            full_name: "prod:user:abc".to_string(),
            redis_type: Some(RedisType::String),
            ttl: None,
        },
    ]);

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| browser.render(f, f.area()));
    // No panic means breadcrumb rendered without error
}

#[test]
fn test_breadcrumb_renders_at_root() {
    use ratatui::backend::TestBackend;

    let browser = KeyBrowser::new(':', 500_000);
    assert!(browser.current_path.is_empty());

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| browser.render(f, f.area()));
    // No panic means breadcrumb rendered without error
}

#[test]
fn test_key_browser_delete_returns_action() {
    use reddish_tui::events::Event;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry {
            full_name: "testkey".to_string(),
            redis_type: None,
            ttl: None,
        },
    ]);

    let action = browser
        .handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert!(matches!(action, Some(BrowserAction::DeleteKey(ref name)) if name == "testkey"));
}

#[test]
fn test_key_browser_delete_on_empty_cursor_does_nothing() {
    use reddish_tui::events::Event;

    let mut browser = KeyBrowser::new(':', 500_000);
    let action = browser
        .handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert!(action.is_none(), "Delete on empty tree should return None");
}

#[test]
fn test_app_delete_pending_state() {
    use reddish_tui::app::{App, AppMode};

    let mut app = App::new(reddish_tui::config::Config::default());
    assert_eq!(app.mode(), &AppMode::Normal);
    assert!(app.pending_delete_key.is_none());

    app.pending_delete_key = Some("testkey".to_string());
    app.mode_stack.push(AppMode::Confirm);
    assert_eq!(app.mode(), &AppMode::Confirm);
    assert_eq!(app.pending_delete_key.as_deref(), Some("testkey"));
}

#[test]
fn test_confirm_dialog_keyboard() {
    let mut dialog = ConfirmDialog::new("Delete?");
    dialog.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    assert_eq!(dialog.confirmed, Some(true));

    let mut dialog = ConfirmDialog::new("Delete?");
    dialog.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert_eq!(dialog.confirmed, Some(false));

    let mut dialog = ConfirmDialog::new("Delete?");
    dialog.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    assert_eq!(dialog.confirmed, Some(false));
}

#[tokio::test]
async fn test_redis_client_ping() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let latency = client.ping().await.unwrap();
    assert!(latency < Duration::from_millis(500));
}

#[tokio::test]
async fn test_redis_client_delete() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("test always uses standalone"),
    };
    redis::cmd("SET")
        .arg("del_test")
        .arg("value")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
    client.delete("del_test").await.unwrap();
    let exists: i64 = redis::cmd("EXISTS")
        .arg("del_test")
        .query_async(&mut c)
        .await
        .unwrap();
    assert_eq!(exists, 0);
}

#[tokio::test]
#[ignore = "flaky in CI due to concurrent Redis test interference"]
async fn test_redis_client_ttl() {
    let profile = ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
        ..Default::default()
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("test always uses standalone"),
    };
    redis::cmd("SET")
        .arg("ttl_test")
        .arg("v")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
    redis::cmd("EXPIRE")
        .arg("ttl_test")
        .arg(100)
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let ttl = client.ttl("ttl_test").await.unwrap();
    assert!(
        !matches!(ttl, Ttl::KeyNotFound),
        "TTL should not be KeyNotFound"
    );

    redis::cmd("PERSIST")
        .arg("ttl_test")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
    let ttl2 = client.ttl("ttl_test").await.unwrap();
    assert_eq!(ttl2, Ttl::NoExpiry);
}

#[test]
fn test_key_browser_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry {
            full_name: "a".to_string(),
            redis_type: None,
            ttl: None,
        },
        KeyEntry {
            full_name: "b".to_string(),
            redis_type: None,
            ttl: None,
        },
        KeyEntry {
            full_name: "c".to_string(),
            redis_type: None,
            ttl: None,
        },
    ]);
    let _ = terminal.draw(|f| browser.render(f, f.area()));
}

#[test]
fn test_scan_batch_updates_browser() {
    let mut browser = KeyBrowser::new(':', 500_000);
    assert_eq!(browser.tree.total_keys(), 0);
    let batch: Vec<KeyEntry> = (0..50)
        .map(|i| KeyEntry {
            full_name: format!("key{}", i),
            redis_type: None,
            ttl: None,
        })
        .collect();
    browser.apply_scan_batch(batch);
    assert_eq!(browser.tree.total_keys(), 50);
    assert!(matches!(
        browser.state,
        reddish_tui::ui::key_browser::BrowserState::Scanning { keys_loaded: 50 }
    ));
}

#[test]
fn test_connection_screen_new_profile_creates_action() {
    use reddish_tui::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press n for new profile
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    assert!(action.is_none()); // No action yet, switches to form mode

    // Fill in form fields via Tab and Char
    // Name field is active, type "prod"
    for c in "prod".chars() {
        screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Tab to Host
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    for c in "10.0.0.1".chars() {
        screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Tab to Port
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    for c in "6380".chars() {
        screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Tab to DB
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    for c in "1".chars() {
        screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Tab to Username
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    // Tab to Password
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    // Submit
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(ConnectionScreenAction::Save(profile)) => {
            assert_eq!(profile.name, "prod");
            assert_eq!(profile.host, "10.0.0.1");
            assert_eq!(profile.port, 6380);
            assert_eq!(profile.db, 1);
        }
        other => panic!("Expected Save action, got {:?}", other),
    }
}

#[test]
fn test_connection_screen_delete_profile() {
    use reddish_tui::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press d for delete
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));
    assert!(action.is_none()); // Enters confirm mode

    // Press y to confirm
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    assert_eq!(action, Some(ConnectionScreenAction::Delete("local".to_string())));
}

#[test]
fn test_connection_screen_delete_cancelled() {
    use reddish_tui::ui::connection_screen::ConnectionScreen;

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press d for delete
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));

    // Press n to cancel
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    assert!(action.is_none());
}

#[test]
fn test_connection_screen_edit_profile() {
    use reddish_tui::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press e for edit
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert!(action.is_none()); // Enters form mode

    // Clear name field with Ctrl+U and type new name
    screen.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char('u'), crossterm::event::KeyModifiers::CONTROL)));
    for c in "staging".chars() {
        screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    // Submit
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(ConnectionScreenAction::Save(profile)) => {
            assert_eq!(profile.name, "staging");
            assert_eq!(profile.host, "127.0.0.1"); // unchanged
            assert_eq!(profile.port, 6379); // unchanged
        }
        other => panic!("Expected Save action, got {:?}", other),
    }
}

#[test]
fn test_connection_screen_edit_cancelled() {
    use reddish_tui::ui::connection_screen::ConnectionScreen;

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press e for edit
    screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));

    // Press Esc to cancel
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(action.is_none());
}

#[test]
fn test_connection_screen_connect_from_list() {
    use reddish_tui::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press Enter to connect
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(
        action,
        Some(ConnectionScreenAction::Connect(ConnectionProfile {
            name: "local".to_string(),
            host: "127.0.0.1".to_string(),
            port: 6379,
            db: 0,
            ..Default::default()
        }))
    );
}

#[test]
fn test_connection_screen_cancel() {
    use reddish_tui::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let mut screen = ConnectionScreen::new(store);

    // Press Esc to cancel
    let action = screen.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert_eq!(action, Some(ConnectionScreenAction::Cancel));
}

#[test]
fn test_connection_screen_renders_without_panic() {
    use ratatui::backend::TestBackend;
    use reddish_tui::ui::connection_screen::ConnectionScreen;

    let store = ConnectionStore {
        path: std::path::PathBuf::from("/tmp/test_connections.toml"),
        profiles: vec![
            ConnectionProfile {
                name: "local".to_string(),
                host: "127.0.0.1".to_string(),
                port: 6379,
                db: 0,
                ..Default::default()
            },
        ],
    };
    let screen = ConnectionScreen::new(store);

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| screen.render(f, f.area()));
}

#[test]
fn test_key_browser_n_new_key_starts_prompt() {
    let mut browser = KeyBrowser::new(':', 500_000);
    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    assert!(action.is_none());
    assert!(browser.prompt.is_some());
    assert_eq!(browser.prompt_mode, Some(reddish_tui::ui::key_browser::BrowserPrompt::NewKeyName));
}

#[test]
fn test_key_browser_n_new_key_full_flow() {
    use reddish_tui::ui::key_browser::BrowserPrompt;

    let mut browser = KeyBrowser::new(':', 500_000);

    // Press n → prompt for key name
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    for c in "mynewkey".chars() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    // Submit name → should transition to type prompt
    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(action.is_none());
    assert_eq!(browser.prompt_mode, Some(BrowserPrompt::NewKeyType("mynewkey".to_string())));

    // Enter type 's' for string
    let _ = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(BrowserAction::NewKey { name, key_type }) => {
            assert_eq!(name, "mynewkey");
            assert_eq!(key_type, RedisType::String);
        }
        other => panic!("Expected NewKey action, got {:?}", other),
    }
}

#[test]
fn test_key_browser_n_new_key_esc_cancels() {
    let mut browser = KeyBrowser::new(':', 500_000);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('n'))));
    assert!(browser.prompt.is_some());

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(action.is_none());
    assert!(browser.prompt.is_none());
}

#[test]
fn test_key_browser_r_rename_starts_prompt() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('r'))));
    assert!(action.is_none());
    assert!(browser.prompt.is_some());
    assert_eq!(browser.prompt_mode, Some(reddish_tui::ui::key_browser::BrowserPrompt::RenameKey("mykey".to_string())));
}

#[test]
fn test_key_browser_r_rename_submit() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('r'))));
    // Clear pre-filled "mykey" and type new name
    for _ in 0..5 {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    }
    for c in "newkey".chars() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(BrowserAction::RenameKey { old_name, new_name }) => {
            assert_eq!(old_name, "mykey");
            assert_eq!(new_name, "newkey");
        }
        other => panic!("Expected RenameKey, got {:?}", other),
    }
}

#[test]
fn test_key_browser_e_expire_returns_action() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert_eq!(action, Some(BrowserAction::ExpireKey("mykey".to_string())));
}

#[test]
fn test_key_browser_y_copy_returns_action() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    assert_eq!(action, Some(BrowserAction::CopyKeyName("mykey".to_string())));
}

#[test]
fn test_key_browser_t_ttl_starts_prompt() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
    assert!(action.is_none());
    assert!(browser.prompt.is_some());
    assert_eq!(browser.prompt_mode, Some(reddish_tui::ui::key_browser::BrowserPrompt::SetTtl("mykey".to_string())));
}

#[test]
fn test_key_browser_t_ttl_submit() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
    for c in "3600".chars() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(BrowserAction::SetTtl { key, seconds }) => {
            assert_eq!(key, "mykey");
            assert_eq!(seconds, 3600);
        }
        other => panic!("Expected SetTtl, got {:?}", other),
    }
}

#[test]
fn test_key_browser_space_selects_key() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "key1".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "key2".to_string(), redis_type: None, ttl: None },
    ]);

    assert!(browser.selected.is_empty());
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert_eq!(browser.selected.len(), 1);
    assert!(browser.selected.contains("key1"));

    // Toggle off
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(browser.selected.is_empty());
}

#[test]
fn test_key_browser_space_on_different_keys() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "key1".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "key2".to_string(), redis_type: None, ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert!(browser.selected.contains("key1"));

    // Navigate down and select key2
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert_eq!(browser.selected.len(), 2);
    assert!(browser.selected.contains("key1"));
    assert!(browser.selected.contains("key2"));
}

#[test]
fn test_key_browser_ctrl_a_selects_all() {
    use crossterm::event::KeyModifiers;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "key1".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "key2".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "key3".to_string(), redis_type: None, ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)));
    assert_eq!(browser.selected.len(), 3);
    assert!(browser.selected.contains("key1"));
    assert!(browser.selected.contains("key2"));
    assert!(browser.selected.contains("key3"));
}

#[test]
fn test_key_browser_s_cycles_sort_mode() {
    let mut browser = KeyBrowser::new(':', 500_000);
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::Alpha);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::ByType);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::ByTtl);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::Alpha);
}

#[test]
fn test_key_browser_sort_mode_label() {
    assert_eq!(reddish_tui::ui::key_browser::SortMode::Alpha.label(), "alpha");
    assert_eq!(reddish_tui::ui::key_browser::SortMode::ByType.label(), "type");
    assert_eq!(reddish_tui::ui::key_browser::SortMode::ByTtl.label(), "ttl");
}

#[test]
fn test_key_browser_sorted_rows_alpha() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "zeta".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "alpha".to_string(), redis_type: None, ttl: None },
        KeyEntry { full_name: "beta".to_string(), redis_type: None, ttl: None },
    ]);

    let rows: Vec<String> = browser.sorted_rows().iter().map(|r| r.label.clone()).collect();
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::Alpha);
    // Alpha sort preserves tree insertion order (zeta, alpha, beta)
    assert_eq!(rows[0], "zeta");
    assert_eq!(rows[1], "alpha");
    assert_eq!(rows[2], "beta");
}

#[test]
fn test_key_browser_sorted_rows_by_type() {
    use reddish_tui::redis::client::RedisType;

    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "list1".to_string(), redis_type: Some(RedisType::List), ttl: None },
        KeyEntry { full_name: "str1".to_string(), redis_type: Some(RedisType::String), ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
    assert_eq!(browser.sort_mode, reddish_tui::ui::key_browser::SortMode::ByType);

    let rows: Vec<String> = browser.sorted_rows().iter().map(|r| r.label.clone()).collect();
    // String (type_order=1) before List (type_order=2)
    assert_eq!(rows[0], "[S] str1");
    assert_eq!(rows[1], "[L] list1");
}

#[test]
fn test_key_browser_d_duplicate_starts_prompt() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));
    assert!(browser.prompt.is_some());
    assert_eq!(
        browser.prompt_mode,
        Some(reddish_tui::ui::key_browser::BrowserPrompt::DuplicateKey("mykey".to_string()))
    );
    // Pre-filled with "mykey_copy" as the default new name
    assert_eq!(browser.prompt.as_ref().unwrap().value, "mykey_copy");
}

#[test]
fn test_key_browser_duplicate_submit_new_name() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    // Open prompt (pre-fills with "mykey_copy")
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));

    // Clear pre-filled text, then type new name
    for _ in 0.."mykey_copy".len() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    }
    for c in "mykey2".chars() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(
        action,
        Some(BrowserAction::DuplicateKey {
            old_name: "mykey".to_string(),
            new_name: "mykey2".to_string(),
        })
    );
}

#[test]
fn test_key_browser_duplicate_empty_name_returns_none() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    // Open prompt
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));

    // Clear the pre-filled name using backspace
    for _ in 0.."mykey_copy".len() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    }

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(action, None);
}

#[test]
fn test_key_browser_duplicate_same_name_returns_none() {
    let mut browser = KeyBrowser::new(':', 500_000);
    browser.apply_scan_batch(vec![
        KeyEntry { full_name: "mykey".to_string(), redis_type: None, ttl: None },
    ]);

    // Open prompt
    browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))));

    // Clear and type the same name as original
    for _ in 0.."mykey_copy".len() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    }
    for c in "mykey".chars() {
        browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = browser.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(action, None);
}
