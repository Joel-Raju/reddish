use std::io::{self, Stdout, stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::cursor::Show,
};

/// Initialize the terminal: enable raw mode and enter alternate screen.
///
/// `mouse_enabled` honours the `mouse_enabled` config key, which was parsed
/// but never read before — the app captured the mouse unconditionally.
pub fn init_terminal(mouse_enabled: bool) -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    if mouse_enabled {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    } else {
        execute!(stdout, EnterAlternateScreen)?;
    }
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore the terminal: disable raw mode, leave alternate screen, show cursor.
/// Idempotent — safe to call multiple times.
pub fn restore_terminal<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    );
    let _ = terminal.show_cursor();
    Ok(())
}
