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

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
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

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    for c in " world".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

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

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
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
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.list_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(inspector.list_cursor, 2);
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
    inspector.set_value("mylist".to_string(), RedisValue::List(vec!["a".to_string()]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    for c in "new_item".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::ListPush { key, value, head }) => {
            assert_eq!(key, "mylist");
            assert_eq!(value, "new_item");
            assert!(!head);
        }
        other => panic!("Expected ListPush, got {:?}", other),
    }
}

#[test]
fn test_list_editor_p_lpush() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mylist".to_string(), RedisValue::List(vec!["a".to_string()]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('p'))));
    for c in "first".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::ListPush { value, head, .. }) => {
            assert_eq!(value, "first");
            assert!(head);
        }
        other => panic!("Expected ListPush, got {:?}", other),
    }
}

#[test]
fn test_list_editor_d_remove() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mylist".to_string(), RedisValue::List(vec!["a".to_string(), "b".to_string()]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
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
    inspector.set_value("mylist".to_string(), RedisValue::List(vec!["hello".to_string()]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
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

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
}

#[test]
fn test_set_editor_navigation() {
    use std::collections::BTreeSet;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut members = BTreeSet::new();
    members.insert("a".to_string());
    members.insert("b".to_string());
    members.insert("c".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myset".to_string(), RedisValue::Set(members));

    assert_eq!(inspector.set_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.set_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(inspector.set_cursor, 0);
}

#[test]
fn test_set_editor_a_add() {
    use std::collections::BTreeSet;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut members = BTreeSet::new();
    members.insert("a".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myset".to_string(), RedisValue::Set(members));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    for c in "new_member".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::SetAdd { key, member }) => {
            assert_eq!(key, "myset");
            assert_eq!(member, "new_member");
        }
        other => panic!("Expected SetAdd, got {:?}", other),
    }
}

#[test]
fn test_set_editor_d_remove() {
    use std::collections::BTreeSet;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut members = BTreeSet::new();
    members.insert("a".to_string());
    members.insert("b".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myset".to_string(), RedisValue::Set(members));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));

    match action {
        Some(InspectorAction::SetRem { key, member }) => {
            assert_eq!(key, "myset");
            assert_eq!(member, "b");
        }
        other => panic!("Expected SetRem, got {:?}", other),
    }
}

#[test]
fn test_set_editor_renders_without_panic() {
    use std::collections::BTreeSet;
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut members = BTreeSet::new();
    members.insert("a".to_string());
    members.insert("b".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myset".to_string(), RedisValue::Set(members));

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
}

#[test]
fn test_zset_editor_navigation() {
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::ValueInspector;

    let entries = vec![
        ZSetEntry { member: "a".into(), score: 1.0 },
        ZSetEntry { member: "b".into(), score: 2.0 },
        ZSetEntry { member: "c".into(), score: 3.0 },
    ];

    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(entries));

    assert_eq!(inspector.zset_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.zset_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(inspector.zset_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(inspector.zset_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('k'))));
    assert_eq!(inspector.zset_cursor, 0);
}

#[test]
fn test_zset_editor_a_add() {
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(vec![
        ZSetEntry { member: "a".into(), score: 1.0 },
    ]));

    // First step: enter member name
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    for c in "new_member".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Submit member → score prompt
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(action.is_none()); // should transition to score prompt

    // Second step: enter score
    for c in "2.5".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::ZAdd { key, score, member }) => {
            assert_eq!(key, "myzset");
            assert_eq!(member, "new_member");
            assert!((score - 2.5).abs() < 1e-9);
        }
        other => panic!("Expected ZAdd, got {:?}", other),
    }
}

#[test]
fn test_zset_editor_e_edit_score() {
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(vec![
        ZSetEntry { member: "a".into(), score: 1.0 },
    ]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    for c in "3".chars() {
        // Clear existing via Ctrl+U then type new
        inspector.handle_event(&Event::Key(KeyEvent::new(
            KeyCode::Char('u'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::ZAdd { key, score, member }) => {
            assert_eq!(key, "myzset");
            assert_eq!(member, "a");
            assert!((score - 3.0).abs() < 1e-9);
        }
        other => panic!("Expected ZAdd, got {:?}", other),
    }
}

#[test]
fn test_zset_editor_d_remove() {
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(vec![
        ZSetEntry { member: "a".into(), score: 1.0 },
        ZSetEntry { member: "b".into(), score: 2.0 },
    ]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));

    match action {
        Some(InspectorAction::ZRem { key, member }) => {
            assert_eq!(key, "myzset");
            assert_eq!(member, "b");
        }
        other => panic!("Expected ZRem, got {:?}", other),
    }
}

#[test]
fn test_zset_editor_renders_without_panic() {
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(vec![
        ZSetEntry { member: "a".into(), score: 1.0 },
        ZSetEntry { member: "b".into(), score: 2.0 },
    ]));

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));

    // Render with prompt open
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
}

#[test]
fn test_stream_editor_navigation() {
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::ValueInspector;
    use indexmap::IndexMap;

    let entries = vec![
        StreamEntry { id: "1-0".into(), fields: IndexMap::new() },
        StreamEntry { id: "2-0".into(), fields: IndexMap::new() },
        StreamEntry { id: "3-0".into(), fields: IndexMap::new() },
    ];

    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(entries));

    assert_eq!(inspector.stream_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.stream_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(inspector.stream_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(inspector.stream_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('k'))));
    assert_eq!(inspector.stream_cursor, 0);
}

#[test]
fn test_stream_editor_a_add() {
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};
    use indexmap::IndexMap;

    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(vec![
        StreamEntry { id: "1-0".into(), fields: IndexMap::new() },
    ]));

    // First step: Entry ID (empty = auto *)
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(action.is_none()); // transitions to fields prompt

    // Second step: fields
    for c in "temp=25".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::StreamAdd { key, entry_id, fields }) => {
            assert_eq!(key, "mystream");
            assert_eq!(entry_id, "*");
            assert_eq!(fields, vec![("temp".to_string(), "25".to_string())]);
        }
        other => panic!("Expected StreamAdd, got {:?}", other),
    }
}

#[test]
fn test_stream_editor_d_remove() {
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};
    use indexmap::IndexMap;

    let mut fields = IndexMap::new();
    fields.insert("temp".to_string(), "25".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(vec![
        StreamEntry { id: "1-0".into(), fields: fields.clone() },
        StreamEntry { id: "2-0".into(), fields },
    ]));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));

    match action {
        Some(InspectorAction::StreamRem { key, entry_id }) => {
            assert_eq!(key, "mystream");
            assert_eq!(entry_id, "2-0");
        }
        other => panic!("Expected StreamRem, got {:?}", other),
    }
}

#[test]
fn test_stream_editor_g_jumps_to_end() {
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::ValueInspector;
    use indexmap::IndexMap;

    let entries = vec![
        StreamEntry { id: "1-0".into(), fields: IndexMap::new() },
        StreamEntry { id: "2-0".into(), fields: IndexMap::new() },
        StreamEntry { id: "3-0".into(), fields: IndexMap::new() },
    ];

    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(entries));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
    assert_eq!(inspector.stream_cursor, 2);
}

#[test]
fn test_stream_editor_renders_without_panic() {
    use indexmap::IndexMap;
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut fields = IndexMap::new();
    fields.insert("temp".to_string(), "25".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(vec![
        StreamEntry { id: "1-0".into(), fields: fields.clone() },
        StreamEntry { id: "2-0".into(), fields },
    ]));

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));

    // Toggle full view
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('f'))));
    let _ = terminal.draw(|f| inspector.render(f, f.area()));

    // Render with prompt open
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
}

#[test]
fn test_hash_editor_navigation() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());
    entries.insert("age".to_string(), "30".to_string());
    entries.insert("city".to_string(), "NYC".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    assert_eq!(inspector.hash_cursor, 0);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));
    assert_eq!(inspector.hash_cursor, 1);
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(inspector.hash_cursor, 0);
}

#[test]
fn test_hash_editor_a_add() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    // a → prompt for field name
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    assert!(inspector.list_prompt.is_some());

    // Type field name
    for c in "score".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Enter → now prompts for value
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(action.is_none()); // value prompt shown, not yet submitted
    assert!(inspector.list_prompt.is_some());

    // Type value
    for c in "100".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    // Enter → submit
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::HashSet { key, field, value }) => {
            assert_eq!(key, "myhash");
            assert_eq!(field, "score");
            assert_eq!(value, "100");
        }
        other => panic!("Expected HashSet, got {:?}", other),
    }
}

#[test]
fn test_hash_editor_d_delete() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());
    entries.insert("age".to_string(), "30".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    // Move to index 1 (age)
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('j'))));

    // D to delete
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));

    match action {
        Some(InspectorAction::HashDel { key, field }) => {
            assert_eq!(key, "myhash");
            assert_eq!(field, "age");
        }
        other => panic!("Expected HashDel, got {:?}", other),
    }
}

#[test]
fn test_hash_editor_e_edit() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    // e to edit value of current field
    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
    assert!(inspector.list_prompt.is_some());

    // Clear and type new value
    inspector.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        crossterm::event::KeyModifiers::CONTROL,
    )));
    for c in "bob".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));

    match action {
        Some(InspectorAction::HashSet { key, field, value }) => {
            assert_eq!(key, "myhash");
            assert_eq!(field, "name");
            assert_eq!(value, "bob");
        }
        other => panic!("Expected HashSet, got {:?}", other),
    }
}

#[test]
fn test_hash_editor_renders_without_panic() {
    use indexmap::IndexMap;
    use ratatui::backend::TestBackend;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());
    entries.insert("age".to_string(), "30".to_string());

    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| inspector.render(f, f.area()));
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

#[test]
fn test_inspector_t_ttl_opens_prompt() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::ValueInspector;

    let mut inspector = ValueInspector::new();
    inspector.set_value("mykey".to_string(), RedisValue::String("hello".to_string()));

    assert!(inspector.list_prompt.is_none());
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
    assert!(action.is_none());
    assert!(inspector.list_prompt.is_some());
}

#[test]
fn test_inspector_t_ttl_submit() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mykey".to_string(), RedisValue::String("hello".to_string()));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
    for c in "3600".chars() {
        inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(c))));
    }
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::SetTtl { key, seconds }) => {
            assert_eq!(key, "mykey");
            assert_eq!(seconds, 3600);
        }
        other => panic!("Expected SetTtl, got {:?}", other),
    }
}

#[test]
fn test_inspector_t_ttl_blank_persists() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mykey".to_string(), RedisValue::String("hello".to_string()));

    inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    match action {
        Some(InspectorAction::SetTtl { key, seconds }) => {
            assert_eq!(key, "mykey");
            assert_eq!(seconds, -1);
        }
        other => panic!("Expected SetTtl(-1), got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_string() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mykey".to_string(), RedisValue::String("hello".to_string()));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "hello");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_list() {
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut inspector = ValueInspector::new();
    inspector.set_value("mylist".to_string(), RedisValue::List(vec!["a".to_string(), "b".to_string()]));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "a");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_hash() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut entries = IndexMap::new();
    entries.insert("name".to_string(), "alice".to_string());
    let mut inspector = ValueInspector::new();
    inspector.set_value("myhash".to_string(), RedisValue::Hash(entries));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "alice");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_set() {
    use std::collections::BTreeSet;
    use reddish_tui::redis::types::RedisValue;
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let mut members = BTreeSet::new();
    members.insert("member1".to_string());
    let mut inspector = ValueInspector::new();
    inspector.set_value("myset".to_string(), RedisValue::Set(members));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "member1");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_zset() {
    use reddish_tui::redis::types::{RedisValue, ZSetEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let entries = vec![ZSetEntry { member: "zmember".to_string(), score: 1.0 }];
    let mut inspector = ValueInspector::new();
    inspector.set_value("myzset".to_string(), RedisValue::ZSet(entries));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "zmember");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}

#[test]
fn test_inspector_y_copy_stream() {
    use indexmap::IndexMap;
    use reddish_tui::redis::types::{RedisValue, StreamEntry};
    use reddish_tui::ui::value_inspector::{InspectorAction, ValueInspector};

    let entries = vec![StreamEntry { id: "123-0".to_string(), fields: IndexMap::new() }];
    let mut inspector = ValueInspector::new();
    inspector.set_value("mystream".to_string(), RedisValue::Stream(entries));

    let action = inspector.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('y'))));
    match action {
        Some(InspectorAction::CopyValue(val)) => {
            assert_eq!(val, "123-0");
        }
        other => panic!("Expected CopyValue, got {:?}", other),
    }
}
