use std::io::{self, stdout, Stdout};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::cursor::Show,
    Terminal,
};

/// Initialize the terminal: enable raw mode and enter alternate screen.
pub fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore the terminal: disable raw mode, leave alternate screen, show cursor.
/// Idempotent — safe to call multiple times.
pub fn restore_terminal<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    let _ = terminal.show_cursor();
    Ok(())
}
