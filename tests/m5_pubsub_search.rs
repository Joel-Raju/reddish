use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::backend::TestBackend;
use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::events::Event;
use reddish_tui::redis::client::RedisClientHandle;
use reddish_tui::ui::pubsub::{PubSubAction, PubSubMessage, PubSubWidget, PubSubInputMode};
use reddish_tui::ui::search::{GlobalSearch, SearchAction};

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
async fn test_publish_and_subscribe() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();

    let _conn = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("test always uses standalone"),
    };

    // Use a PubSub connection via redis crate
    let client2 = redis::Client::open("redis://127.0.0.1:16379/0").unwrap();
    let mut pubsub = client2.get_async_pubsub().await.unwrap();
    pubsub.subscribe("test_channel").await.unwrap();

    // Publish via the regular client
    let receivers = client.publish("test_channel", "hello").await.unwrap();
    assert_eq!(receivers, 1);

    let msg = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        pubsub.on_message().next(),
    )
    .await;
    assert!(msg.is_ok());
    let payload: String = msg.unwrap().unwrap().get_payload().unwrap();
    assert_eq!(payload, "hello");
}

#[test]
fn test_pubsub_widget_stores_messages() {
    let mut widget = PubSubWidget::new();
    assert!(widget.messages.is_empty());

    widget.push_message(PubSubMessage {
        channel: "ch1".to_string(),
        pattern: None,
        payload: "hello".to_string(),
        timestamp: std::time::Instant::now(),
    });
    assert_eq!(widget.messages.len(), 1);

    widget.push_message(PubSubMessage {
        channel: "ch1".to_string(),
        pattern: None,
        payload: "world".to_string(),
        timestamp: std::time::Instant::now(),
    });
    assert_eq!(widget.messages.len(), 2);
}

#[test]
fn test_pubsub_widget_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let widget = PubSubWidget::new();
    let _ = terminal.draw(|f| widget.render(f, f.area()));
}

#[test]
fn test_global_search_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let search = GlobalSearch::new();
    let _ = terminal.draw(|f| search.render(f, f.area()));
}

#[test]
fn test_global_search_typing_and_close() {
    let mut search = GlobalSearch::new();
    let a = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    assert_eq!(a, Some(SearchAction::QueryChanged));
    let b = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('b'))));
    assert_eq!(b, Some(SearchAction::QueryChanged));
    assert_eq!(search.query, "ab");

    let action = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(action, Some(SearchAction::Execute(ref s)) if s == "ab"));

    let action2 = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(matches!(action2, Some(SearchAction::Close)));
}

#[test]
fn test_global_search_char_triggers_query_changed() {
    let mut search = GlobalSearch::new();
    let action = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('x'))));
    assert_eq!(search.query, "x");
    assert_eq!(action, Some(SearchAction::QueryChanged));
}

#[test]
fn test_global_search_backspace_triggers_query_changed() {
    let mut search = GlobalSearch::new();
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('b'))));
    assert_eq!(search.query, "ab");

    let action = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Backspace)));
    assert_eq!(search.query, "a");
    assert_eq!(action, Some(SearchAction::QueryChanged));
}

#[test]
fn test_global_search_scan_state() {
    let mut search = GlobalSearch::new();
    assert!(!search.scanning);
    assert!(search.scan_results.is_empty());

    search.start_scan();
    assert!(search.scanning);
    assert!(search.results.is_empty());
    assert!(search.filtered.is_empty());
}

#[test]
fn test_global_search_drain_scan_batch() {
    let mut search = GlobalSearch::new();
    search.start_scan();

    search.drain_scan_batch(vec!["key1".to_string(), "key2".to_string()]);
    assert!(search.scanning); // still scanning
    assert_eq!(search.results.len(), 2);
    assert_eq!(search.filtered.len(), 2);

    search.drain_scan_batch(vec![]);
    assert!(!search.scanning); // done
    assert_eq!(search.results.len(), 2);
}

#[test]
fn test_global_search_filter_with_results() {
    let mut search = GlobalSearch::new();
    search.set_results(vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()]);

    // After set_results, filter matches all (query is "")
    assert_eq!(search.filtered.len(), 3);

    // Typing 'a' filters to items containing 'a'
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    assert_eq!(search.filtered, vec!["apple".to_string(), "banana".to_string()]);
}

#[test]
fn test_global_search_render_shows_scanning() {
    use ratatui::backend::TestBackend;

    let mut search = GlobalSearch::new();
    search.start_scan();
    search.scanning = true;

    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let _ = terminal.draw(|f| search.render(f, f.area()));
}

#[test]
fn test_pubsub_subscribe_action() {
    let mut widget = PubSubWidget::new();
    widget.input = "test_channel".to_string();
    widget.cursor = 12;

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(action, Some(PubSubAction::Subscribe("test_channel".to_string())));
    assert!(widget.channels.contains(&"test_channel".to_string()));
    assert_eq!(widget.active_channel, Some("test_channel".to_string()));
}

#[test]
fn test_pubsub_publish_action() {
    let mut widget = PubSubWidget::new();
    widget.active_channel = Some("ch1".to_string());
    widget.input_mode = PubSubInputMode::Message;
    widget.input = "hello".to_string();
    widget.cursor = 5;

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert_eq!(
        action,
        Some(PubSubAction::Publish {
            channel: "ch1".to_string(),
            message: "hello".to_string(),
        })
    );
}

#[test]
fn test_pubsub_unsubscribe_action() {
    let mut widget = PubSubWidget::new();
    widget.channels.push("ch1".to_string());
    widget.channels.push("ch2".to_string());
    widget.active_channel = Some("ch1".to_string());

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert_eq!(action, Some(PubSubAction::Unsubscribe("ch1".to_string())));
    assert_eq!(widget.channels.len(), 1);
    assert_eq!(widget.channels[0], "ch2");
}

#[test]
fn test_pubsub_unsubscribe_second_channel() {
    let mut widget = PubSubWidget::new();
    widget.channels.push("ch1".to_string());
    widget.channels.push("ch2".to_string());
    widget.channel_cursor = 1;
    widget.active_channel = Some("ch2".to_string());

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert_eq!(action, Some(PubSubAction::Unsubscribe("ch2".to_string())));
    assert_eq!(widget.channels.len(), 1);
    assert_eq!(widget.channels[0], "ch1");
}

#[test]
fn test_pubsub_clear_messages() {
    let mut widget = PubSubWidget::new();
    widget.push_message(PubSubMessage {
        channel: "ch".to_string(),
        pattern: None,
        payload: "msg1".to_string(),
        timestamp: std::time::Instant::now(),
    });
    widget.push_message(PubSubMessage {
        channel: "ch".to_string(),
        pattern: None,
        payload: "msg2".to_string(),
        timestamp: std::time::Instant::now(),
    });
    assert_eq!(widget.messages.len(), 2);

    let key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
    widget.handle_event(&Event::Key(key));
    assert!(widget.messages.is_empty());
}

#[test]
fn test_pubsub_channel_navigation() {
    let mut widget = PubSubWidget::new();
    widget.channels.push("ch1".to_string());
    widget.channels.push("ch2".to_string());
    widget.channels.push("ch3".to_string());

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(widget.channel_cursor, 1);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(widget.channel_cursor, 2);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
    assert_eq!(widget.channel_cursor, 2);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(widget.channel_cursor, 1);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(widget.channel_cursor, 0);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
    assert_eq!(widget.channel_cursor, 0);
}

#[test]
fn test_pubsub_render_with_messages() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut widget = PubSubWidget::new();
    widget.channels.push("test".to_string());
    widget.active_channel = Some("test".to_string());
    widget.push_message(PubSubMessage {
        channel: "test".to_string(),
        pattern: None,
        payload: "hello world".to_string(),
        timestamp: std::time::Instant::now(),
    });
    let _ = terminal.draw(|f| widget.render(f, f.area()));
}

#[test]
fn test_pubsub_render_empty() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let widget = PubSubWidget::new();
    let _ = terminal.draw(|f| widget.render(f, f.area()));
}

#[test]
fn test_pubsub_tab_toggles_mode() {
    let mut widget = PubSubWidget::new();
    assert_eq!(widget.input_mode, PubSubInputMode::Channel);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert_eq!(widget.input_mode, PubSubInputMode::Message);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert_eq!(widget.input_mode, PubSubInputMode::Channel);
}
