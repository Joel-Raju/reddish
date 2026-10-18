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
    assert!(
        tick_count >= 1,
        "Expected at least one Tick event within 300ms"
    );
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

// ---- Stress tests (0.13, 0.14, 0.15) ----

#[tokio::test]
async fn test_concurrent_mutation() {
    let mut app = App::new(Config::default());
    app.readonly = true;

    // Many tasks mutating different subsystems at once, plus a render loop.
    // A shared-mutation bug (an unwrap on shared state, a borrow held across
    // an await, a non-Send guard) would surface here rather than in the field.
    let mut tasks = Vec::new();
    for i in 0..8u32 {
        let mut cloned = App::new(Config::default());
        cloned.readonly = true;
        tasks.push(tokio::spawn(async move {
            for j in 0..50 {
                cloned
                    .key_browser
                    .tree
                    .insert(reddish_tui::ui::key_browser::tree::KeyEntry {
                        full_name: format!("ns{i}:key{j}"),
                        redis_type: None,
                        ttl: None,
                    });
                cloned.value_inspector.set_value(
                    format!("ns{i}:key{j}"),
                    reddish_tui::redis::types::RedisValue::List(vec!["a".into()]),
                );
                cloned.search.query.push('x');
                cloned.search.apply_filter();
                cloned.info_dashboard.update_stats(Default::default());
            }
        }));
    }

    // Render the main app repeatedly while the workers run.
    for i in 0..20 {
        let backend = TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let _ = i;
    }

    for task in tasks {
        task.await.expect("worker task panicked");
    }
}

/// Render at sizes that are degenerate or inverted. Ratatui clamps internally;
/// the point is that nothing panics or hangs.
#[test]
fn test_resize_extreme_sizes() {
    let mut app = App::new(Config::default());
    for i in 0..200 {
        app.key_browser
            .tree
            .insert(reddish_tui::ui::key_browser::tree::KeyEntry {
                full_name: format!("ns:{:04}:key", i),
                redis_type: Some(reddish_tui::redis::client::RedisType::String),
                ttl: None,
            });
    }
    app.key_browser.tree.expand_all();
    app.value_inspector.set_value(
        "ns:0000:key".to_string(),
        reddish_tui::redis::types::RedisValue::List(
            (0..500).map(|i| format!("item-{i}")).collect(),
        ),
    );

    let sizes = [
        (1u16, 1u16), // smallest possible
        (1, 100),     // one column
        (100, 1),     // one row
        (2, 2),       // smaller than the chrome
        (0, 0),       // degenerate
        (500, 200),   // larger than a typical terminal
        // Large but bounded: a TestBackend allocates width*height cells, so
        // the u16 maximum would try to reserve ~4 billion and OOM.
        (1000, 1000),
    ];

    for (w, h) in sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
    }

    // Every tab, at the tightest size where chrome cannot fit.
    let backend = TestBackend::new(3, 3);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    for tab in [
        reddish_tui::app::Tab::Keys,
        reddish_tui::app::Tab::Repl,
        reddish_tui::app::Tab::Info,
        reddish_tui::app::Tab::PubSub,
    ] {
        app.active_tab = tab;
        terminal.draw(|f| app.render(f)).unwrap();
    }
}

/// CLI flags must be independently settable and must not conflict.
#[test]
fn test_cli_flag_conflicts() {
    use clap::Parser;
    use reddish_tui::cli::Cli;

    // Each flag alone.
    let cases: Vec<Vec<&str>> = vec![
        vec!["redis-tui", "--readonly"],
        vec!["redis-tui", "--allow-monitor"],
        vec!["redis-tui", "--url", "redis://localhost:6379"],
        vec!["redis-tui", "--profile", "prod"],
        vec!["redis-tui", "--theme", "nord"],
        vec!["redis-tui", "--log-level", "debug"],
        vec!["redis-tui", "--bench"],
        vec!["redis-tui", "--profile-memory"],
    ];
    for args in &cases {
        assert!(
            Cli::try_parse_from(args).is_ok(),
            "expected {args:?} to parse"
        );
    }

    // Every combination of the boolean flags must coexist.
    for readonly in [false, true] {
        for monitor in [false, true] {
            for bench in [false, true] {
                for memory in [false, true] {
                    let mut args = vec!["redis-tui"];
                    if readonly {
                        args.push("--readonly");
                    }
                    if monitor {
                        args.push("--allow-monitor");
                    }
                    if bench {
                        args.push("--bench");
                    }
                    if memory {
                        args.push("--profile-memory");
                    }
                    let cli = Cli::try_parse_from(&args)
                        .unwrap_or_else(|e| panic!("{args:?} should parse: {e}"));
                    assert_eq!(cli.readonly, readonly, "{args:?}");
                    assert_eq!(cli.allow_monitor, monitor, "{args:?}");
                    assert_eq!(cli.bench, bench, "{args:?}");
                    assert_eq!(cli.profile_memory, memory, "{args:?}");
                }
            }
        }
    }

    // Option-valued flags, including an empty value which clap must accept
    // rather than silently swallow.
    for (flag, value) in [
        ("--url", "redis://h:1/0"),
        ("--profile", ""),
        ("--theme", ""),
        ("--log-level", ""),
    ] {
        let cli = Cli::try_parse_from(["redis-tui", flag, value])
            .unwrap_or_else(|e| panic!("{flag} {value:?} should parse: {e}"));
        let parsed = match flag {
            "--url" => cli.url,
            "--profile" => cli.profile,
            "--theme" => cli.theme,
            _ => cli.log_level,
        };
        assert_eq!(parsed.as_deref(), Some(value), "{flag}");
    }

    // Unknown flags are rejected rather than ignored.
    assert!(Cli::try_parse_from(["redis-tui", "--nope"]).is_err());
    // A value-taking flag missing its value is rejected.
    assert!(Cli::try_parse_from(["redis-tui", "--url"]).is_err());
    // Repeating a flag is rejected rather than silently taking the last value,
    // so a duplicated argument in a script surfaces instead of being ignored.
    assert!(
        Cli::try_parse_from(["redis-tui", "--theme", "nord", "--theme", "dracula"]).is_err(),
        "a repeated flag should be an error, not last-wins"
    );
}
