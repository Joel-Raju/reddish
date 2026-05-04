use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use reddish_tui::config::keybindings::Keymap;
use reddish_tui::ui::help::{HelpContext, HelpOverlay};
use reddish_tui::ui::theme::{Theme, THEMES};

#[test]
fn test_all_themes_have_all_fields() {
    for theme in THEMES {
        assert!(!theme.name.is_empty());
        assert_ne!(theme.bg, ratatui::style::Color::Reset);
        assert_ne!(theme.fg, ratatui::style::Color::Reset);
        assert_ne!(theme.highlight_bg, ratatui::style::Color::Reset);
        assert_ne!(theme.highlight_fg, ratatui::style::Color::Reset);
    }

    let names: Vec<_> = THEMES.iter().map(|t| t.name).collect();
    assert_eq!(names.len(), std::collections::HashSet::<&&str>::from_iter(names.iter()).len());
}

#[test]
fn test_theme_by_name() {
    let t = Theme::by_name("dracula");
    assert!(t.is_some());
    assert_eq!(t.unwrap().name, "dracula");
    assert!(Theme::by_name("nonexistent").is_none());
}

#[test]
fn test_keymap_default_parses() {
    let km = Keymap::default();
    let ev = KeyEvent::from(KeyCode::Char('j'));
    assert!(km.matches("nav_down", &ev));
    assert!(!km.matches("nav_up", &ev));
}

#[test]
fn test_keymap_emacs_parses() {
    let km = Keymap::emacs();
    let ev = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert!(km.matches("nav_up", &ev));
}

#[test]
fn test_keymap_from_toml_override() {
    let toml = r#"
nav_up = { key = "w" }
"#;
    let km: Keymap = toml::from_str(toml).unwrap();
    let ev = KeyEvent::from(KeyCode::Char('w'));
    assert!(km.matches("nav_up", &ev));
    let ev_j = KeyEvent::from(KeyCode::Char('j'));
    assert!(!km.matches("nav_up", &ev_j));
}

#[test]
fn test_help_overlay_renders_without_panic() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    for ctx in [
        HelpContext::KeyBrowser,
        HelpContext::ValueInspector,
        HelpContext::Repl,
        HelpContext::Stats,
        HelpContext::PubSub,
        HelpContext::Global,
    ] {
        let help = HelpOverlay::new(ctx);
        let _ = terminal.draw(|f| help.render(f, f.area()));
    }
}

#[test]
fn test_cli_url_flag_parsed() {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(name = "redis-tui")]
    struct TestCli {
        #[arg(long)]
        url: Option<String>,
    }

    let cli = TestCli::parse_from(["redis-tui", "--url", "redis://user:pass@localhost:6380/2"]);
    let url = cli.url.unwrap();
    assert!(url.contains("localhost"));
    assert!(url.contains("6380"));
}

#[test]
fn test_cli_readonly_flag() {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(name = "redis-tui")]
    struct TestCli {
        #[arg(long)]
        readonly: bool,
    }

    let cli = TestCli::parse_from(["redis-tui", "--readonly"]);
    assert!(cli.readonly);
}
