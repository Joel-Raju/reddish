use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use reddish_tui::events::Event;
use reddish_tui::ui::command_palette::{CommandPalette, PaletteAction};
use reddish_tui::ui::repl::{ReplLine, ReplLineStatus, ReplAction, ReplWidget};

#[test]
fn test_repl_line_status() {
    let line = ReplLine {
        input: "PING".to_string(),
        output: "PONG".to_string(),
        status: ReplLineStatus::Success,
    };
    assert_eq!(line.status, ReplLineStatus::Success);

    let err_line = ReplLine {
        input: "GET".to_string(),
        output: "Error".to_string(),
        status: ReplLineStatus::Error("ERR".to_string()),
    };
    assert!(matches!(err_line.status, ReplLineStatus::Error(ref s) if s == "ERR"));
}

#[test]
fn test_repl_widget_history_and_navigation() {
    let mut repl = ReplWidget::new();
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
    let mut repl = ReplWidget::new();
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('X'))));
    repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    let _ = terminal.draw(|f| repl.render(f, f.area()));
}

#[test]
fn test_palette_filters_items() {
    let mut palette = CommandPalette::new(vec![
        "Apple".to_string(),
        "Banana".to_string(),
        "Cherry".to_string(),
    ]);
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
    let mut palette = CommandPalette::new(vec![
        "Connect".to_string(),
        "Disconnect".to_string(),
    ]);
    let action = palette.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
    assert!(matches!(action, Some(PaletteAction::Execute(ref s)) if s == "Connect"));
}

#[test]
fn test_palette_closes_on_esc() {
    let mut palette = CommandPalette::new(vec!["Quit".to_string()]);
    let action = palette.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
    assert!(matches!(action, Some(PaletteAction::Close)));
}

#[test]
fn test_palette_renders_without_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let palette = CommandPalette::new(vec!["Cmd1".to_string(), "Cmd2".to_string()]);
    let _ = terminal.draw(|f| palette.render(f, f.area()));
}

#[test]
fn test_app_repl_tab_switch() {
    let mut app = reddish_tui::app::App::new(reddish_tui::config::Config::default());
    assert_eq!(app.active_tab, reddish_tui::app::Tab::Keys);
    // Simulate tab switch to REPL
    app.active_tab = reddish_tui::app::Tab::Repl;
    assert_eq!(app.active_tab, reddish_tui::app::Tab::Repl);
}
