use clap::Parser;
use color_eyre::Result;
use reddish_tui::{
    app::App,
    bench,
    cli::Cli,
    config::{
        Config,
        connections::{ConnectionProfile, ConnectionStore},
    },
    logging, terminal,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load()?;
    if let Some(level) = cli.log_level.clone() {
        config.log_level = Some(level);
    }
    if let Some(theme) = cli.theme.clone() {
        config.color_scheme = Some(theme);
    }

    logging::init_logging(&config)?;

    // A keybinding that no terminal can deliver is indistinguishable from a
    // missing one, and one bad `[keybindings]` entry used to fail silently.
    // Say it before the TUI takes over the screen. (All of this fires only
    // for user-written TOML: both shipped presets validate clean, which the
    // keybindings tests enforce.)
    for warning in config.keybindings.validation_warnings() {
        eprintln!("warning: keybinding {warning}");
    }

    // Install the theme before anything renders. An unknown name is an error
    // rather than a silent fallback: the user asked for something specific, and
    // quietly showing a different theme is worse than refusing to start.
    let theme_name = config.color_scheme();
    let selected =
        reddish_tui::ui::theme::resolve(theme_name).map_err(|e| color_eyre::eyre::eyre!(e))?;
    reddish_tui::ui::theme::set_current(selected);

    // Hidden performance gates. These run before any terminal setup so they
    // never touch raw mode or the alternate screen, making them safe in CI.
    // A budget violation exits with code 1 so the gates can fail a build.
    if cli.bench {
        std::process::exit(i32::from(!bench::run_bench(config)));
    }
    if cli.profile_memory {
        std::process::exit(i32::from(!bench::run_profile_memory(config)));
    }

    let mut term = terminal::init_terminal(config.mouse_enabled())?;

    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = crossterm::terminal::disable_raw_mode();
        panic_hook(info);
    }));

    color_eyre::install()?;

    let mut app = App::new(config);
    app.readonly = cli.readonly;
    app.allow_monitor = cli.allow_monitor;

    if let Some(profile_name) = cli.profile.as_deref() {
        if let Some(profile) = resolve_profile(profile_name)? {
            app.startup_profile = Some(profile);
        } else {
            app.error_message = Some(format!("Profile not found: {profile_name}"));
        }
    }

    if let Some(url) = cli.url.as_deref() {
        match profile_from_url(url) {
            Ok(profile) => app.startup_profile = Some(profile),
            Err(err) => app.error_message = Some(format!("Invalid --url: {err}")),
        }
    }

    let result = tokio::runtime::Runtime::new()?.block_on(async { app.run(&mut term).await });

    terminal::restore_terminal(&mut term)?;
    result
}

fn resolve_profile(name: &str) -> Result<Option<ConnectionProfile>> {
    let base = dirs::config_dir()
        .ok_or_else(|| color_eyre::eyre::eyre!("Could not determine config directory"))?;
    let path = base.join("redis-tui").join("connections.toml");
    let store = ConnectionStore::load(&path)?;
    Ok(store.get(name).cloned())
}

fn profile_from_url(url: &str) -> Result<ConnectionProfile> {
    // Delegates to the lib, where it is unit-tested: this function lived
    // in the binary with zero coverage for the README's headline flag.
    reddish_tui::config::connections::profile_from_url(url)
}
