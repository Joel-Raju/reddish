use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use reddish_tui::events::Event;
use reddish_tui::redis::client::{RedisClientHandle, RedisType};
use reddish_tui::ui::value_viewer::ValueViewer;
use reddish_tui::ui::value_viewer::formatters::{FormatHint, detect_format, preview, stringify};
use reddish_tui::ui::widgets::text_area_editor::TextAreaEditor;

#[test]
fn test_format_json_detects_json() {
    let raw = br#"{"hello": "world"}"#;
    assert_eq!(detect_format(raw), FormatHint::Json);
}

#[test]
fn test_format_base64_detects_binary() {
    let raw: Vec<u8> = (0..80).collect();
    assert_eq!(detect_format(&raw), FormatHint::Blob);
}

#[test]
fn test_stringify_and_preview() {
    let raw = br#"{"hello": "world"}"#;
    let hint = detect_format(raw);
    let s = stringify(raw, hint.clone());
    assert!(s.contains("\"hello\""));
    let p = preview(raw, 10, hint);
    assert!(p.ends_with("..."));
    assert_eq!(p.chars().count(), 10);
}

#[test]
fn test_text_area_editor_typing_and_return() {
    let mut editor = TextAreaEditor::new("hello");
    assert_eq!(editor.cursor_line(), 0);
    assert_eq!(editor.cursor_col(), 5);

    editor.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
    assert_eq!(editor.text, "hello ");

    editor.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(editor.text, "hello \n");
    assert_eq!(editor.cursor_line(), 1);

    editor.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));
}

#[test]
fn test_value_viewer_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let viewer = ValueViewer::new("mykey", RedisType::String, br#"{"a":1}"#.to_vec());
    let _ = terminal.draw(|f| viewer.render(f, f.area()));
}

#[test]
fn test_value_inspector_metadata_starts_empty() {
    let inspector = reddish_tui::ui::value_inspector::ValueInspector::new();
    assert!(inspector.encoding.is_none());
    assert!(inspector.memory_bytes.is_none());
    assert!(inspector.ttl.is_none());
}

#[test]
fn test_value_inspector_set_metadata() {
    use reddish_tui::redis::client::Ttl;
    use std::time::Duration;

    let mut inspector = reddish_tui::ui::value_inspector::ValueInspector::new();
    inspector.set_metadata(
        Some("embstr".to_string()),
        Some(1024),
        Some(Ttl::Expires(Duration::from_secs(300))),
    );
    assert_eq!(inspector.encoding.as_deref(), Some("embstr"));
    assert_eq!(inspector.memory_bytes, Some(1024));
    assert_eq!(inspector.ttl, Some(Ttl::Expires(Duration::from_secs(300))));
}

#[test]
fn test_value_inspector_metadata_cleared_on_set_loading() {
    use reddish_tui::redis::client::Ttl;

    let mut inspector = reddish_tui::ui::value_inspector::ValueInspector::new();
    inspector.set_metadata(Some("embstr".to_string()), Some(1024), Some(Ttl::NoExpiry));
    inspector.set_loading("test_key".to_string());
    assert!(inspector.encoding.is_none());
    assert!(inspector.memory_bytes.is_none());
    assert!(inspector.ttl.is_none());
}

#[test]
fn test_value_inspector_renders_with_metadata() {
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::client::Ttl;
    use reddish_tui::redis::types::RedisValue;
    use std::time::Duration;

    let mut inspector = reddish_tui::ui::value_inspector::ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::String("hello".to_string()));
    inspector.set_metadata(
        Some("embstr".to_string()),
        Some(512),
        Some(Ttl::Expires(Duration::from_secs(60))),
    );

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
}

#[test]
fn test_value_inspector_enter_edit_mode_on_e() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::String("hello".to_string()));

    // Press e to enter edit mode
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert!(action.is_none());
    assert!(inspector.edit_mode);
    assert!(inspector.text_editor.is_some());
    assert_eq!(inspector.text_editor.as_ref().unwrap().text, "hello");
}

#[test]
fn test_value_inspector_edit_mode_ignores_non_string() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::List(vec!["a".to_string()]));

    // Press e — should NOT enter edit mode for non-string
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert!(action.is_none());
    assert!(!inspector.edit_mode);
}

#[test]
fn test_value_inspector_edit_mode_esc_cancels() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::String("hello".to_string()));

    // Enter edit mode
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));

    // Press Esc to cancel
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(action.is_none());
    assert!(!inspector.edit_mode);
    assert!(inspector.text_editor.is_none());
}

#[test]
fn test_value_inspector_edit_mode_ctrl_s_saves() {
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};
    use reddish_tui::redis::types::RedisValue;

    let mut inspector = ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::String("hello".to_string()));

    // Enter edit mode
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));

    // Type " world"
    for c in " world".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    // Ctrl+S to save
    let action = inspector.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        crossterm::event::KeyModifiers::CONTROL,
    )));

    match action {
        Some(InspectorAction::WriteString { key, value }) => {
            assert_eq!(key, "testkey");
            assert_eq!(value, "hello world");
        }
        other => panic!("Expected WriteString action, got {:?}", other),
    }
    assert!(!inspector.edit_mode);
}

#[test]
fn test_value_inspector_edit_mode_ctrl_x_cancels() {
    use reddish_tui::ui::value_inspector::ValueInspector;
    use reddish_tui::redis::types::RedisValue;

    let mut inspector = ValueInspector::new();
    inspector.set_value("testkey".to_string(), RedisValue::String("hello".to_string()));

    // Enter edit mode
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));

    // Ctrl+X to cancel (no changes typed)
    let action = inspector.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        crossterm::event::KeyModifiers::CONTROL,
    )));

    assert!(action.is_none());
    assert!(!inspector.edit_mode);
}

#[test]
fn test_text_area_editor_cancelled_flag() {
    use reddish_tui::ui::widgets::text_area_editor::TextAreaEditor;

    let mut editor = TextAreaEditor::new("hello");
    assert!(!editor.cancelled);

    // Ctrl+X sets cancelled
    editor.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        crossterm::event::KeyModifiers::CONTROL,
    )));
    assert!(editor.cancelled);
}

#[test]
fn test_text_area_editor_ctrl_s_does_not_set_cancelled() {
    use reddish_tui::ui::widgets::text_area_editor::TextAreaEditor;

    let mut editor = TextAreaEditor::new("hello");

    // Ctrl+S does not set cancelled
    editor.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        crossterm::event::KeyModifiers::CONTROL,
    )));
    assert!(!editor.cancelled);
}

#[test]
fn test_list_editor_navigation() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
    );

    assert_eq!(inspector.list_cursor, 0);

    // Down / j
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.list_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(inspector.list_cursor, 2);

    // Up / k
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('k'))));
    assert_eq!(inspector.list_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(inspector.list_cursor, 0);
}

#[test]
fn test_list_editor_a_rpush() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["a".to_string()]),
    );

    // Press a to trigger RPUSH prompt
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    assert!(inspector.list_prompt.is_some());

    // Type value
    for c in "new_item".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    // Enter to submit
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::ListPush { key, value, head }) => {
            assert_eq!(key, "mylist");
            assert_eq!(value, "new_item");
            assert!(!head); // RPUSH
        }
        other => panic!("Expected ListPush, got {:?}", other),
    }
}

#[test]
fn test_list_editor_p_lpush() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["a".to_string()]),
    );

    // Press p to trigger LPUSH prompt
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('p'))));
    assert!(inspector.list_prompt.is_some());

    for c in "first".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::ListPush { value, head, .. }) => {
            assert_eq!(value, "first");
            assert!(head); // LPUSH
        }
        other => panic!("Expected ListPush, got {:?}", other),
    }
}

#[test]
fn test_list_editor_d_remove() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["a".to_string(), "b".to_string()]),
    );

    // Move cursor to index 1
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.list_cursor, 1);

    // D to delete
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));

    match action {
        Some(InspectorAction::ListRemove { key, value }) => {
            assert_eq!(key, "mylist");
            assert_eq!(value, "b");
        }
        other => panic!("Expected ListRemove, got {:?}", other),
    }
}

#[test]
fn test_list_editor_e_edit() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["hello".to_string()]),
    );

    // Press e to edit first item
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert!(inspector.list_prompt.is_some());

    // Clear with Ctrl+U and type new value
    inspector.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        crossterm::event::KeyModifiers::CONTROL,
    )));
    for c in "world".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::ListSet { key, index, value }) => {
            assert_eq!(key, "mylist");
            assert_eq!(index, 0);
            assert_eq!(value, "world");
        }
        other => panic!("Expected ListSet, got {:?}", other),
    }
}

#[test]
fn test_list_editor_renders_without_panic() {
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value(
        "mylist".to_string(),
        RedisValue::List(vec!["a".to_string(), "b".to_string()]),
    );

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));

    // Also test that list prompt renders
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
}

#[tokio::test]
async fn test_redis_get_set_string() {
    let profile = reddish_tui::config::connections::ConnectionProfile {
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
    };
    redis::cmd("SET")
        .arg("m2_key")
        .arg("hello")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let val = client.get("m2_key").await.unwrap();
    assert_eq!(val, b"hello");

    client.set("m2_key2", b"world").await.unwrap();
    let val2: Vec<u8> = redis::cmd("GET")
        .arg("m2_key2")
        .query_async(&mut c)
        .await
        .unwrap();
    assert_eq!(val2, b"world");

    redis::cmd("DEL")
        .arg("m2_key")
        .arg("m2_key2")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_hgetall() {
    let profile = reddish_tui::config::connections::ConnectionProfile {
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
    };
    redis::cmd("HSET")
        .arg("m2_hash")
        .arg("a")
        .arg("1")
        .arg("b")
        .arg("2")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let vals = client.hgetall("m2_hash").await.unwrap();
    let mut map = std::collections::HashMap::new();
    for (k, v) in vals {
        map.insert(k, v);
    }
    assert_eq!(map.get("a").unwrap(), b"1");
    assert_eq!(map.get("b").unwrap(), b"2");

    redis::cmd("DEL")
        .arg("m2_hash")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_lrange() {
    let profile = reddish_tui::config::connections::ConnectionProfile {
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
    };
    redis::cmd("LPUSH")
        .arg("m2_list")
        .arg("c")
        .arg("b")
        .arg("a")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let vals = client.lrange("m2_list", 0, -1).await.unwrap();
    let strings: Vec<String> = vals
        .into_iter()
        .map(|v| String::from_utf8(v).unwrap())
        .collect();
    assert_eq!(strings, vec!["a", "b", "c"]);

    redis::cmd("DEL")
        .arg("m2_list")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_smembers() {
    let profile = reddish_tui::config::connections::ConnectionProfile {
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
    };
    redis::cmd("SADD")
        .arg("m2_set")
        .arg("a")
        .arg("b")
        .arg("c")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let vals = client.smembers("m2_set").await.unwrap();
    let mut strings: Vec<String> = vals
        .into_iter()
        .map(|v| String::from_utf8(v).unwrap())
        .collect();
    strings.sort();
    assert_eq!(strings, vec!["a", "b", "c"]);

    redis::cmd("DEL")
        .arg("m2_set")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_zrange_withscores() {
    let profile = reddish_tui::config::connections::ConnectionProfile {
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
    };
    redis::cmd("ZADD")
        .arg("m2_zset")
        .arg(1.0)
        .arg("a")
        .arg(2.0)
        .arg("b")
        .query_async::<()>(&mut c)
        .await
        .unwrap();

    let vals = client.zrange_withscores("m2_zset", 0, -1).await.unwrap();
    let strings: Vec<(String, f64)> = vals
        .into_iter()
        .map(|(v, s)| (String::from_utf8(v).unwrap(), s))
        .collect();
    assert_eq!(
        strings,
        vec![("a".to_string(), 1.0), ("b".to_string(), 2.0)]
    );

    redis::cmd("DEL")
        .arg("m2_zset")
        .query_async::<()>(&mut c)
        .await
        .unwrap();
}
