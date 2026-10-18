//! Proves that `--theme` and `color_scheme` actually change what is drawn.
//!
//! Before this wiring, `theme.rs` was dead code: five themes were defined, a
//! `by_name` lookup existed, and nothing imported the module. `--theme nord`
//! exited successfully and changed nothing. These tests render the real widgets
//! and inspect the produced cells, so a regression to hardcoded colors is
//! caught rather than merely being described.

use std::sync::Mutex;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use reddish_tui::app::{App, Tab};
use reddish_tui::config::Config;
use reddish_tui::ui::theme::{self, Theme};

/// Serialises the tests in this file.
///
/// The active theme is process-wide, so two tests setting different themes
/// concurrently would render each other's expectations. Each test below
/// installs a theme, renders, then asserts, which only holds if nothing else
/// changes the theme in between.
static THEME_LOCK: Mutex<()> = Mutex::new(());

/// Render `App` and return every distinct foreground color in the frame.
fn rendered_colors(app: &mut App, w: u16, h: u16) -> Vec<Color> {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut seen: Vec<Color> = buffer
        .content()
        .iter()
        .filter(|cell| cell.symbol() != " ")
        .map(|cell| cell.fg)
        .collect();
    seen.sort_by_key(|c| format!("{c:?}"));
    seen.dedup();
    seen
}

fn app_with_keys() -> App {
    let mut app = App::new(Config::default());
    for i in 0..12 {
        app.key_browser
            .tree
            .insert(reddish_tui::ui::key_browser::tree::KeyEntry {
                full_name: format!("ns{i}:key"),
                redis_type: Some(reddish_tui::redis::client::RedisType::String),
                ttl: None,
            });
    }
    app.key_browser.tree.expand_all();
    app.active_tab = Tab::Keys;
    app
}

#[test]
fn default_theme_renders_without_panic() {
    let _lock = THEME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let default = Theme::by_name("default").unwrap();
    theme::set_current(default);
    let mut app = app_with_keys();
    let colors = rendered_colors(&mut app, 100, 30);
    assert!(!colors.is_empty());
    // The default theme uses plain ANSI colors, so they must show up verbatim.
    // Asserting the presence of specific theme colors is what proves the theme
    // reached the widgets; merely rendering without panicking would not.
    assert!(
        colors.contains(&default.title),
        "the theme title colour {:?} is missing from {colors:?}",
        default.title
    );
}

#[test]
fn every_theme_renders_every_tab() {
    let _lock = THEME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Themes are installed process-wide, so this checks each one in turn
    // rather than asserting a per-process value.
    for theme in reddish_tui::ui::theme::THEMES {
        theme::set_current(theme);
        let mut app = app_with_keys();
        for tab in [Tab::Keys, Tab::Repl, Tab::Info, Tab::PubSub] {
            app.active_tab = tab.clone();
            let label = format!("{tab:?}");
            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            terminal
                .draw(|f| app.render(f))
                .unwrap_or_else(|e| panic!("{} / {label} failed to render: {e}", theme.name));
        }
    }
}

/// The load-bearing test: a theme with distinctive colors must actually change
/// the output. If the widgets went back to hardcoded colors this fails.
#[test]
fn installing_a_different_theme_changes_the_rendered_colors() {
    let _lock = THEME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    theme::set_current(Theme::by_name("default").unwrap());
    let with_default = rendered_colors(&mut app_with_keys(), 100, 30);

    let gruvbox = Theme::by_name("gruvbox_dark").unwrap();
    theme::set_current(gruvbox);
    let with_gruvbox = rendered_colors(&mut app_with_keys(), 100, 30);

    assert_ne!(
        with_default, with_gruvbox,
        "gruvbox_dark uses truecolor backgrounds that the default theme cannot \
         produce, so identical output means the theme is not being applied"
    );
}

/// Truecolor themes use `Color::Rgb`, which no hardcoded ANSI color can produce.
#[test]
fn a_truecolor_theme_reaches_the_widgets() {
    let _lock = THEME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gruvbox = Theme::by_name("gruvbox_dark").unwrap();
    theme::set_current(gruvbox);
    let colors = rendered_colors(&mut app_with_keys(), 100, 30);
    assert!(
        colors.iter().any(|c| matches!(c, Color::Rgb(_, _, _))),
        "expected truecolor cells from gruvbox_dark, got {colors:?}"
    );
}

/// Each theme's own colors must appear in what it renders. A theme whose
/// palette never reaches the screen is the exact failure this whole change
/// exists to prevent.
#[test]
fn every_theme_actually_reaches_the_screen() {
    let _lock = THEME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for theme in reddish_tui::ui::theme::THEMES {
        theme::set_current(theme);
        let colors = rendered_colors(&mut app_with_keys(), 100, 30);
        assert!(
            colors.contains(&theme.title),
            "{}: title colour {:?} missing from {colors:?}",
            theme.name,
            theme.title
        );
        assert!(
            colors.contains(&theme.border),
            "{}: border colour {:?} missing from {colors:?}",
            theme.name,
            theme.border
        );
    }
}
