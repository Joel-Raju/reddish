use ratatui::backend::TestBackend;
use reddish_tui::redis::client::{RedisClientHandle, RedisType};
use reddish_tui::ui::value_viewer::formatters::{detect_format, FormatHint, preview, stringify};
use reddish_tui::ui::value_viewer::ValueViewer;
use reddish_tui::ui::widgets::text_area_editor::TextAreaEditor;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use reddish_tui::events::Event;

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
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    redis::cmd("SET").arg("m2_key").arg("hello").query_async::<()>(&mut c).await.unwrap();

    let val = client.get("m2_key").await.unwrap();
    assert_eq!(val, b"hello");

    client.set("m2_key2", b"world").await.unwrap();
    let val2: Vec<u8> = redis::cmd("GET").arg("m2_key2").query_async(&mut c).await.unwrap();
    assert_eq!(val2, b"world");

    redis::cmd("DEL").arg("m2_key").arg("m2_key2").query_async::<()>(&mut c).await.unwrap();
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
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    redis::cmd("HSET").arg("m2_hash").arg("a").arg("1").arg("b").arg("2").query_async::<()>(&mut c).await.unwrap();

    let vals = client.hgetall("m2_hash").await.unwrap();
    let mut map = std::collections::HashMap::new();
    for (k, v) in vals {
        map.insert(k, v);
    }
    assert_eq!(map.get("a").unwrap(), b"1");
    assert_eq!(map.get("b").unwrap(), b"2");

    redis::cmd("DEL").arg("m2_hash").query_async::<()>(&mut c).await.unwrap();
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
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    redis::cmd("LPUSH").arg("m2_list").arg("c").arg("b").arg("a").query_async::<()>(&mut c).await.unwrap();

    let vals = client.lrange("m2_list", 0, -1).await.unwrap();
    let strings: Vec<String> = vals.into_iter().map(|v| String::from_utf8(v).unwrap()).collect();
    assert_eq!(strings, vec!["a", "b", "c"]);

    redis::cmd("DEL").arg("m2_list").query_async::<()>(&mut c).await.unwrap();
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
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    redis::cmd("SADD").arg("m2_set").arg("a").arg("b").arg("c").query_async::<()>(&mut c).await.unwrap();

    let vals = client.smembers("m2_set").await.unwrap();
    let mut strings: Vec<String> = vals.into_iter().map(|v| String::from_utf8(v).unwrap()).collect();
    strings.sort();
    assert_eq!(strings, vec!["a", "b", "c"]);

    redis::cmd("DEL").arg("m2_set").query_async::<()>(&mut c).await.unwrap();
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
    };
    let client = RedisClientHandle::connect(&profile).await.unwrap();
    let mut c = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };
    redis::cmd("ZADD").arg("m2_zset").arg(1.0).arg("a").arg(2.0).arg("b").query_async::<()>(&mut c).await.unwrap();

    let vals = client.zrange_withscores("m2_zset", 0, -1).await.unwrap();
    let strings: Vec<(String, f64)> = vals.into_iter().map(|(v, s)| (String::from_utf8(v).unwrap(), s)).collect();
    assert_eq!(strings, vec![("a".to_string(), 1.0), ("b".to_string(), 2.0)]);

    redis::cmd("DEL").arg("m2_zset").query_async::<()>(&mut c).await.unwrap();
}
