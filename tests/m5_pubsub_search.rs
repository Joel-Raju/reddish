use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::events::Event;
use reddish_tui::redis::client::RedisClientHandle;
use reddish_tui::redis::pubsub::PubSubMessage;
use reddish_tui::ui::pubsub::{
    PubSubAction, PubSubInputMode, PubSubKind, PubSubWidget, Subscription,
};
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
    search.set_results(vec![
        "apple".to_string(),
        "banana".to_string(),
        "cherry".to_string(),
    ]);

    // After set_results, filter matches all (query is "")
    assert_eq!(search.filtered.len(), 3);

    // Typing 'a' filters to items containing 'a'
    search.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('a'))));
    assert_eq!(
        search.filtered,
        vec!["apple".to_string(), "banana".to_string()]
    );
}

#[test]
fn test_global_search_render_shows_scanning() {
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
    assert_eq!(
        action,
        Some(PubSubAction::Subscribe("test_channel".to_string()))
    );
    assert!(
        widget
            .subscriptions
            .iter()
            .any(|s| s.kind == PubSubKind::Channel("test_channel".to_string()))
    );
    assert_eq!(
        widget.active_subscription,
        Some(PubSubKind::Channel("test_channel".to_string()))
    );
}

#[test]
fn test_pubsub_publish_action() {
    let mut widget = PubSubWidget::new();
    widget.active_subscription = Some(PubSubKind::Channel("ch1".to_string()));
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
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch1".to_string()),
        color: Color::Cyan,
    });
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch2".to_string()),
        color: Color::Magenta,
    });
    widget.active_subscription = Some(PubSubKind::Channel("ch1".to_string()));

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert_eq!(action, Some(PubSubAction::Unsubscribe("ch1".to_string())));
    assert_eq!(widget.subscriptions.len(), 1);
    assert_eq!(
        widget.subscriptions[0].kind,
        PubSubKind::Channel("ch2".to_string())
    );
}

#[test]
fn test_pubsub_unsubscribe_second_channel() {
    let mut widget = PubSubWidget::new();
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch1".to_string()),
        color: Color::Cyan,
    });
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch2".to_string()),
        color: Color::Magenta,
    });
    widget.channel_cursor = 1;
    widget.active_subscription = Some(PubSubKind::Channel("ch2".to_string()));

    let action = widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('D'))));
    assert_eq!(action, Some(PubSubAction::Unsubscribe("ch2".to_string())));
    assert_eq!(widget.subscriptions.len(), 1);
    assert_eq!(
        widget.subscriptions[0].kind,
        PubSubKind::Channel("ch1".to_string())
    );
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
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch1".to_string()),
        color: Color::Cyan,
    });
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch2".to_string()),
        color: Color::Magenta,
    });
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("ch3".to_string()),
        color: Color::Green,
    });

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
    widget.subscriptions.push(Subscription {
        kind: PubSubKind::Channel("test".to_string()),
        color: Color::Cyan,
    });
    widget.active_subscription = Some(PubSubKind::Channel("test".to_string()));
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
    assert_eq!(widget.input_mode, PubSubInputMode::Pattern);

    widget.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
    assert_eq!(widget.input_mode, PubSubInputMode::Channel);
}

// ---- Global search integration (5.19, 5.20, 5.21) ----

fn search_profile() -> ConnectionProfile {
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

async fn search_connection() -> redis::aio::MultiplexedConnection {
    let client = RedisClientHandle::connect(&search_profile()).await.unwrap();
    match &client.client {
        reddish_tui::redis::client::RedisClient::Standalone(c) => c.clone(),
        _ => unreachable!("tests always use standalone"),
    }
}

/// Insert keys, run `f`, then remove them again so the shared server stays tidy.
async fn with_keys<T, F, Fut>(pairs: &[(&str, &str)], f: F) -> T
where
    F: FnOnce(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let mut c = search_connection().await;
    let names: Vec<String> = pairs.iter().map(|(k, _)| k.to_string()).collect();
    for (k, v) in pairs {
        redis::cmd("SET")
            .arg(*k)
            .arg(*v)
            .query_async::<()>(&mut c)
            .await
            .unwrap();
    }
    let result = f(names).await;
    for k in pairs.iter().map(|(k, _)| *k) {
        let _ = redis::cmd("DEL").arg(k).query_async::<()>(&mut c).await;
    }
    result
}

#[tokio::test]
async fn test_global_search_finds_matching_keys() {
    let found = with_keys(
        &[
            ("test:search:alpha", "1"),
            ("test:search:beta", "2"),
            ("unrelated:key", "3"),
        ],
        |_names| async {
            let mut search = GlobalSearch::new();
            // Simulate the SCAN batch the app would receive for `*alpha*`.
            search.start_scan();
            search.query = "alpha".to_string();
            search.drain_scan_batch(vec![
                "test:search:alpha".to_string(),
                "test:search:beta".to_string(),
                "unrelated:key".to_string(),
            ]);
            search.filtered.clone()
        },
    )
    .await;

    assert!(
        found.contains(&"test:search:alpha".to_string()),
        "expected the matching key, got {found:?}"
    );
    assert!(
        !found.contains(&"unrelated:key".to_string()),
        "non-matching keys must be filtered out, got {found:?}"
    );
}

#[tokio::test]
async fn test_global_search_ranks_exact_match_first() {
    let found = with_keys(
        &[
            ("test:search:prefix_alpha_suffix", "1"),
            ("test:search:alpha", "2"),
        ],
        |_names| async {
            let mut search = GlobalSearch::new();
            search.set_results(vec![
                "test:search:prefix_alpha_suffix".to_string(),
                "test:search:alpha".to_string(),
            ]);
            search.query = "alpha".to_string();
            search.apply_filter();
            search.filtered.clone()
        },
    )
    .await;

    assert_eq!(
        found.first(),
        Some(&"test:search:alpha".to_string()),
        "the tighter match must rank first, got {found:?}"
    );
}

#[tokio::test]
async fn test_global_search_set_query_rescores() {
    let found = with_keys(
        &[
            ("test:search:alpha", "1"),
            ("test:search:alphabet", "2"),
            ("test:search:zulu", "3"),
        ],
        |_names| async {
            let mut search = GlobalSearch::new();
            search.set_results(vec![
                "test:search:alpha".to_string(),
                "test:search:alphabet".to_string(),
                "test:search:zulu".to_string(),
            ]);

            search.query = "alpha".to_string();
            search.apply_filter();
            let for_alpha = search.filtered.clone();

            // Re-querying must re-rank, not accumulate the previous filter.
            search.query = "zulu".to_string();
            search.apply_filter();
            let for_zulu = search.filtered.clone();

            (for_alpha, for_zulu)
        },
    )
    .await;

    let (for_alpha, for_zulu) = found;
    assert_eq!(for_alpha.len(), 2, "got {for_alpha:?}");
    assert_eq!(for_zulu, vec!["test:search:zulu".to_string()]);
}

#[tokio::test]
async fn test_global_search_progress_transitions() {
    let mut search = GlobalSearch::new();

    // Idle.
    assert!(!search.scanning);

    // A scan starts and stays in progress while batches arrive.
    search.start_scan();
    assert!(search.scanning);

    search.query = "user".to_string();
    search.drain_scan_batch(vec!["user:1".to_string(), "user:2".to_string()]);
    assert!(search.scanning, "still scanning after the first batch");
    assert_eq!(search.filtered.len(), 2);

    // A non-empty batch that matches nothing must not end the scan.
    search.drain_scan_batch(vec!["session:9".to_string()]);
    assert!(search.scanning, "a non-matching batch is not completion");

    // An empty batch is the completion sentinel.
    search.drain_scan_batch(vec![]);
    assert!(!search.scanning, "empty batch ends the scan");
    // Candidates are retained; only the scanning flag clears.
    assert_eq!(search.results.len(), 3);
}

#[tokio::test]
async fn test_global_search_regex_mode_over_real_keys() {
    let found = with_keys(
        &[("test:search:123", "1"), ("test:search:abc", "2")],
        |_names| async {
            let mut search = GlobalSearch::new();
            search.mode = reddish_tui::ui::search::SearchMode::Regex;
            search.set_results(vec![
                "test:search:123".to_string(),
                "test:search:abc".to_string(),
            ]);
            search.query = r"^test:search:\d+$".to_string();
            search.apply_filter();
            search.filtered.clone()
        },
    )
    .await;

    assert_eq!(found, vec!["test:search:123".to_string()]);
}

#[tokio::test]
async fn test_search_glob_pattern_matches_expected_keys() {
    // The server-side narrowing must be a superset of the client-side filter,
    // otherwise fuzzy matching can never surface extra candidates.
    let mut c = search_connection().await;
    let keys = [
        "test:glob:user:session:123",
        "test:glob:usr123",
        "other:glob:user",
    ];
    for k in keys {
        redis::cmd("SET")
            .arg(k)
            .arg("1")
            .query_async::<()>(&mut c)
            .await
            .unwrap();
    }

    let pattern = reddish_tui::ui::search::glob_pattern_for_query(
        "user 123",
        reddish_tui::ui::search::SearchMode::Fuzzy,
    );
    let (cursor, matched): (u64, Vec<String>) = redis::cmd("SCAN")
        .arg(0)
        .arg("MATCH")
        .arg(&pattern)
        .arg("COUNT")
        .arg(1000)
        .query_async(&mut c)
        .await
        .unwrap();
    assert_eq!(cursor, 0);

    assert!(
        matched.contains(&"test:glob:user:session:123".to_string()),
        "namespace-style query must match a nested key; pattern={pattern} matched={matched:?}"
    );
    assert!(
        !matched.contains(&"other:glob:user".to_string()),
        "keys lacking the second atom must not match; matched={matched:?}"
    );

    for k in keys {
        let _ = redis::cmd("DEL").arg(k).query_async::<()>(&mut c).await;
    }
}

// ---- PSUBSCRIBE (5.16) ----

#[tokio::test]
async fn test_pubsub_psubscribe_receives_matching_channels() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();

    let client2 = redis::Client::open("redis://127.0.0.1:16379/0").unwrap();
    let mut pubsub = client2.get_async_pubsub().await.unwrap();
    // Subscribe to a pattern that covers several channels.
    pubsub.psubscribe("test:psub:*").await.unwrap();

    // A matching channel is delivered.
    assert_eq!(client.publish("test:psub:one", "first").await.unwrap(), 1);
    let msg = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        pubsub.on_message().next(),
    )
    .await
    .expect("timed out waiting for a pattern match")
    .expect("stream ended");
    let payload: String = msg.get_payload().unwrap();
    let channel = msg.get_channel_name().to_string();
    assert_eq!(payload, "first");
    assert_eq!(channel, "test:psub:one");

    // A second matching channel on the same subscription also arrives.
    client.publish("test:psub:two", "second").await.unwrap();
    let msg = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        pubsub.on_message().next(),
    )
    .await
    .expect("timed out")
    .expect("stream ended");
    let payload: String = msg.get_payload().unwrap();
    assert_eq!(payload, "second");
    assert_eq!(msg.get_channel_name(), "test:psub:two");
}

#[tokio::test]
async fn test_pubsub_psubscribe_ignores_non_matching_channels() {
    let profile = test_profile();
    let client = RedisClientHandle::connect(&profile).await.unwrap();

    let client2 = redis::Client::open("redis://127.0.0.1:16379/0").unwrap();
    let mut pubsub = client2.get_async_pubsub().await.unwrap();
    pubsub.psubscribe("test:psub:only:*").await.unwrap();

    // Publishing outside the pattern must reach zero subscribers.
    assert_eq!(
        client
            .publish("test:other:channel", "ignored")
            .await
            .unwrap(),
        0,
        "a non-matching channel must not notify the subscription"
    );
}

#[tokio::test]
async fn test_pubsub_kind_reports_pattern_and_channel() {
    use reddish_tui::redis::pubsub::PubSubKind;

    let channel = PubSubKind::Channel("news".to_string());
    let pattern = PubSubKind::Pattern("news:*".to_string());

    assert!(!channel.is_pattern());
    assert!(pattern.is_pattern());
    assert_eq!(channel.name(), "news");
    assert_eq!(pattern.name(), "news:*");
    assert_ne!(channel, pattern);
}
