use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::TestBackend;
use reddish_tui::events::Event;
use reddish_tui::ui::command_palette::{CommandPalette, PaletteAction};
use reddish_tui::ui::repl::{ReplAction, ReplLineStatus, ReplWidget};

/// Isolated history path so tests never touch the developer's real REPL
/// history file.
fn temp_history_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("reddish-repl-it-{}-{name}", std::process::id()))
        .join("history")
}

#[test]
fn test_repl_widget_history_and_navigation() {
    let mut repl = ReplWidget::with_history_path(temp_history_path("nav"));
    // Type a command
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('P'))));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('I'))));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('N'))));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('G'))));
    assert_eq!(repl.input, "PING");
    assert_eq!(repl.cursor, 4);

    // Submit
    let action = repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(action, Some(ReplAction::Submit(ref s)) if s == "PING"));
    assert_eq!(repl.input, "");
    assert_eq!(repl.history.len(), 1);

    // Add result
    repl.add_result("PING", "PONG".to_string(), ReplLineStatus::Success);
    let last = repl.history.back().unwrap();
    assert_eq!(last.output, "PONG");
    assert_eq!(last.status, ReplLineStatus::Success);
}

#[test]
fn test_repl_widget_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut repl = ReplWidget::with_history_path(temp_history_path("render"));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('X'))));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    let _ = terminal.draw(|f| repl.render(f, f.area()));
}

#[test]
fn test_palette_filters_items() {
    let mut palette = CommandPalette::new(
        vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
        ],
        "Ctrl+P".to_string(),
    );
    palette.query.push('a');
    palette.filter();
    assert_eq!(palette.filtered.len(), 2); // Apple, Banana

    palette.query.push('p');
    palette.filter();
    assert_eq!(palette.filtered.len(), 1); // Apple
    assert_eq!(palette.filtered[0], "Apple");
}

#[test]
fn test_palette_selects_item_on_enter() {
    let mut palette = CommandPalette::new(
        vec!["Connect".to_string(), "Disconnect".to_string()],
        "Ctrl+P".to_string(),
    );
    let action = palette.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(action, Some(PaletteAction::Execute(ref s)) if s == "Connect"));
}

#[test]
fn test_palette_closes_on_esc() {
    let mut palette = CommandPalette::new(vec!["Quit".to_string()], "Ctrl+P".to_string());
    let action = palette.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(matches!(action, Some(PaletteAction::Close)));
}

#[test]
fn test_palette_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let palette = CommandPalette::new(
        vec!["Cmd1".to_string(), "Cmd2".to_string()],
        "Ctrl+P".to_string(),
    );
    let _ = terminal.draw(|f| palette.render(f, f.area()));
}

#[tokio::test]
async fn test_app_repl_tab_switch() {
    use crossterm::event::{KeyCode, KeyEvent};

    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    assert_eq!(app.active_tab, reddish_tui::app::Tab::Keys);
    // The real dispatch: the tab keys themselves. The previous version
    // assigned `active_tab` and asserted the assignment.
    for (key, tab) in [
        ('2', reddish_tui::app::Tab::Repl),
        ('3', reddish_tui::app::Tab::Info),
        ('4', reddish_tui::app::Tab::PubSub),
        ('1', reddish_tui::app::Tab::Keys),
    ] {
        reddish_tui::app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Char(key)))
            .await;
        assert_eq!(app.active_tab, tab, "pressing {key} must switch tabs");
    }
}

#[tokio::test]
async fn test_app_repl_overlay_press_colon() {
    use crossterm::event::{KeyCode, KeyEvent};
    use reddish_tui::app::AppMode;

    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    assert_eq!(app.mode(), &AppMode::Normal);
    assert!(app.overlay_input.is_none());
    // The real dispatch: the previous version pressed nothing.
    reddish_tui::app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Char(':'))).await;
    assert_eq!(
        app.mode(),
        &AppMode::ReplOverlay,
        "pressing : opens the overlay"
    );
    assert!(
        app.overlay_input.is_some(),
        "the overlay owns an input line"
    );
}

#[tokio::test]
async fn test_app_repl_overlay_esc_closes() {
    use crossterm::event::{KeyCode, KeyEvent};
    use reddish_tui::app::{self, AppMode};

    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    // Open the overlay the way a user does.
    app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Char(':'))).await;
    assert_eq!(app.mode(), &AppMode::ReplOverlay);

    app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Esc)).await;
    assert_eq!(app.mode(), &AppMode::Normal, "Esc must close the overlay");
    assert!(app.overlay_input.is_none(), "Esc must drop the input line");
}

#[tokio::test]
async fn test_app_repl_overlay_enter_executes() {
    use crossterm::event::{KeyCode, KeyEvent};
    use reddish_tui::app::{self, AppMode};

    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    // Open the overlay and type the command key by key — the real user
    // path. The old test constructed `InputWidget::new("PING")`, but that
    // constructor takes a *label*, so the value was always empty, its
    // Enter executed nothing, and the test merely popped the mode stack
    // itself and asserted the pop.
    app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Char(':'))).await;
    assert_eq!(app.mode(), &AppMode::ReplOverlay);
    for c in "PING".chars() {
        app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Char(c))).await;
    }

    app::handle_key_event_for_test(&mut app, KeyEvent::from(KeyCode::Enter)).await;
    assert_eq!(app.mode(), &AppMode::Normal, "Enter must close the overlay");
    assert!(app.overlay_input.is_none());
    assert_eq!(
        app.error_message.as_deref(),
        Some("Not connected"),
        "Enter must have executed PING, which fails with no client — the failure is the proof it ran"
    );
}

#[test]
fn test_app_repl_overlay_typing() {
    use crossterm::event::KeyCode;
    use reddish_tui::app::AppMode;
    use reddish_tui::events::Event;
    use reddish_tui::ui::widgets::input::InputWidget;

    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    app.mode_stack.push(AppMode::ReplOverlay);
    app.overlay_input = Some(InputWidget::new(""));

    if let Some(ref mut overlay) = app.overlay_input {
        overlay.handle_event(&Event::Key(crossterm::event::KeyEvent::from(
            KeyCode::Char('P'),
        )));
        overlay.handle_event(&Event::Key(crossterm::event::KeyEvent::from(
            KeyCode::Char('I'),
        )));
        overlay.handle_event(&Event::Key(crossterm::event::KeyEvent::from(
            KeyCode::Char('N'),
        )));
        overlay.handle_event(&Event::Key(crossterm::event::KeyEvent::from(
            KeyCode::Char('G'),
        )));
        assert_eq!(overlay.value, "PING");
    } else {
        panic!("overlay_input should be Some");
    }
}
