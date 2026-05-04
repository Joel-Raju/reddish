use clap::Parser;
use color_eyre::Result;
use reddish_tui::{app::App, config::Config, logging, terminal};

#[derive(Parser, Debug)]
#[command(name = "redis-tui")]
#[command(about = "A Redis TUI client")]
struct Cli {
    #[arg(long, value_name = "URL")]
    url: Option<String>,
    #[arg(long)]
    readonly: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
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
    let result = tokio::runtime::Runtime::new()?.block_on(async {
        app.run(&mut term).await
    });

    terminal::restore_terminal(&mut term)?;
    result
}
