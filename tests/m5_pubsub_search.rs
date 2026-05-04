use futures::StreamExt;
use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::redis::client::RedisClientHandle;
use reddish_tui::ui::pubsub::{PubSubMessage, PubSubWidget};
use reddish_tui::ui::search::{GlobalSearch, SearchAction};
use crossterm::event::{KeyCode, KeyEvent};
use reddish_tui::events::Event;
use ratatui::backend::TestBackend;

fn test_profile() -> ConnectionProfile {
    ConnectionProfile {
        name: "test".to_string(),
        host: "127.0.0.1".to_string(),
        port: 16379,
        db: 0,
        username: None,
        password: None,
        last_connected: None,
    }
}

#[tokio::test]
async fn test_publish_and_subscribe() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();

    let _conn = match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
    };

    // Use a PubSub connection via redis crate
    let client2 = redis::Client::open("redis://127.0.0.1:16379/0").unwrap();
    let mut pubsub = client2.get_async_pubsub().await.unwrap();
    pubsub.subscribe("test_channel").await.unwrap();

    // Publish via the regular client
    let receivers = client.publish("test_channel", "hello").await.unwrap();
    assert_eq!(receivers, 1);

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), pubsub.on_message().next()).await;
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
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('b'))));
    assert_eq!(search.query, "ab");

    let action = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(action, Some(SearchAction::Execute(ref s)) if s == "ab"));

    let action2 = search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(matches!(action2, Some(SearchAction::Close)));
}
