use color_eyre::Result;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;

pub fn init_logging(config: &Config) -> Result<()> {
    let log_dir = dirs::data_dir()
        .ok_or_else(|| color_eyre::eyre::eyre!("Could not determine data directory"))?
        .join("redis-tui");
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("tui.log");
    let file_appender = tracing_appender::rolling::never(&log_dir, "tui.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Keep guard alive for the lifetime of the app by leaking it
    // (or we can store it in App; for M0 we leak to keep API simple)
    let _leaked = Box::leak(Box::new(_guard));

    let filter = EnvFilter::try_new(config.log_level()).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    tracing::info!("Logging initialized to {:?}", log_file);
    Ok(())
}
