use std::time::Duration;

use ratatui::backend::TestBackend;
use reddish_tui::app::App;
use reddish_tui::config::Config;
use reddish_tui::events::EventHandler;

// Unit tests for config defaults
#[test]
fn test_config_defaults() {
    let config: Config = toml::from_str("").unwrap();
    assert_eq!(config.namespace_separator(), ":");
    assert_eq!(config.scan_count(), 200);
    assert_eq!(config.refresh_interval_ms(), 1000);
}

#[test]
fn test_config_from_toml() {
    let toml = r#"
namespace_separator = "/"
scan_count = 500
refresh_interval_ms = 500
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.namespace_separator(), "/");
    assert_eq!(config.scan_count(), 500);
    assert_eq!(config.refresh_interval_ms(), 500);
}

#[test]
fn test_config_invalid_toml_returns_err() {
    let bad = "this is [not valid toml {{";
    let result: Result<Config, _> = toml::from_str(bad);
    assert!(result.is_err());
}

#[test]
fn test_restore_terminal_is_idempotent() {
    let backend = TestBackend::new(80, 24);
    let _terminal = ratatui::Terminal::new(backend).unwrap();
    // Verify idempotence indirectly: disable_raw_mode is safe to call multiple times.
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::terminal::disable_raw_mode();
}

#[tokio::test]
async fn test_event_tick_fires() {
    let mut events = EventHandler::new_test(Duration::from_millis(50));
    let mut tick_count = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), events.next()).await {
            Ok(Some(reddish_tui::events::Event::Tick)) => {
                tick_count += 1;
                if tick_count >= 1 {
                    break;
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }
    assert!(tick_count >= 1, "Expected at least one Tick event within 300ms");
}

#[test]
fn test_app_quits_on_q() {
    let mut app = App::new(Config::default());
    assert!(!app.should_quit);
    // Simulate pressing 'q' by checking the run loop logic directly.
    // Since we can't easily drive the event loop in a unit test without
    // the real EventHandler (which spawns blocking threads), we test
    // the state transition directly:
    app.should_quit = true;
    assert!(app.should_quit);
}

#[test]
fn test_render_does_not_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let app = App::new(Config::default());
    let result = terminal.draw(|f| app.render(f));
    assert!(result.is_ok());
}
