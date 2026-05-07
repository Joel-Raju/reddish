use clap::Parser;
use color_eyre::Result;
use reddish_tui::{
    app::App,
    config::{
        Config,
        connections::{ConnectionMode, ConnectionProfile, ConnectionStore, PasswordRef},
    },
    logging, terminal,
};
use redis::IntoConnectionInfo;

#[derive(Parser, Debug)]
#[command(name = "redis-tui")]
#[command(about = "A Redis TUI client")]
struct Cli {
    #[arg(long, value_name = "URL")]
    url: Option<String>,
    #[arg(long, value_name = "NAME")]
    profile: Option<String>,
    #[arg(long)]
    readonly: bool,
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<String>,
    #[arg(long, value_name = "THEME")]
    theme: Option<String>,
}

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

    let mut term = terminal::init_terminal()?;

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
    let info = url.into_connection_info()?;

    let (host, port, tls_enabled) = match info.addr {
        redis::ConnectionAddr::Tcp(host, port) => (host, port, false),
        redis::ConnectionAddr::TcpTls { host, port, .. } => (host, port, true),
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "unsupported connection address in URL"
            ));
        }
    };

    let db = u8::try_from(info.redis.db)
        .map_err(|_| color_eyre::eyre::eyre!("URL db index out of range for u8"))?;

    Ok(ConnectionProfile {
        name: "cli-url".to_string(),
        host,
        port,
        db,
        username: info.redis.username,
        password: info.redis.password.map(PasswordRef::Plaintext),
        last_connected: None,
        mode: ConnectionMode::Standalone,
        tls: if tls_enabled {
            Some(reddish_tui::config::connections::TlsConfig {
                enabled: true,
                verify_certs: true,
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
        } else {
            None
        },
        ssh_tunnel: None,
    })
}
