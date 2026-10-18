//! Generate the man page from the shared [`Cli`] definition.
//!
//! Run with:
//! ```sh
//! cargo run --example generate-man > man/redis-tui.1
//! ```
//!
//! Building both from `reddish_tui::cli::Cli` means the man page cannot drift
//! from the binary's actual flags.

use std::io;

use clap::CommandFactory;
use clap_mangen::Man;

fn main() -> io::Result<()> {
    let cli = reddish_tui::cli::Cli::command();

    let man = Man::new(cli)
        .date("2026-10-17")
        .source("reddish-tui 0.1.0")
        .manual("Redis Terminal User Interface")
        .section("1");

    man.render(&mut io::stdout())?;

    // Append the sections clap_mangen does not generate: file locations and
    // keyboard shortcuts. Kept as literal roff so the page stays useful
    // without duplicating the README's full tables.
    let mut out = io::stdout();
    use std::io::Write;
    write!(out, "{}", FILES_SECTION)?;
    write!(out, "{}", SHORTCUTS_SECTION)?;
    Ok(())
}

const FILES_SECTION: &str = r#"
.SH FILES
.TP
.I ~/.config/redis\-tui/config.toml
Application settings: namespace separator, scan count, refresh interval,
theme, and per\-action keybinding overrides.
.TP
.I ~/.config/redis\-tui/connections.toml
Saved connection profiles, including password references (plaintext,
environment variable, or system keychain) and TLS / SSH tunnel settings.
.TP
.I ~/.local/share/redis\-tui/history
REPL command history, most recent last, capped at 1000 entries.
.TP
.I ~/.local/share/redis\-tui/search\_history
Recent global search queries, most recent first, capped at 100 entries.
.SH ENVIRONMENT
.TP
.B $EDITOR
When set, editing a string key (\fBe\fR in the value inspector) suspends the TUI
and opens this editor on the value. The value is written to a temporary file
that is removed when the editor exits, and the result is saved back to Redis
only if it differs. A leading program plus flags is supported, e.g.
\fBcode --wait\fR. Unset or blank means the built\-in editor is used instead.
.TP
.B REDIS\_URL, REDIS\_PASSWORD
Convenience values for a password reference of kind environment.
"#;

const SHORTCUTS_SECTION: &str = r#"
.SH KEYBOARD
.SS Global
.TP
.B 1 2 3 4
Switch to Keys, REPL, Info and Pub/Sub tabs.
.TP
.B :
Open the REPL command overlay.
.TP
.B Ctrl+P
Command palette.
.TP
.B Ctrl+\\
Open the connection screen.
.TP
.B Ctrl+R
Force reconnect.
.TP
.B /
Global fuzzy search (in the search overlay, Ctrl+R toggles regex mode).
.TP
.B ?
Help overlay.
.TP
.B q
Quit.
.SS Value inspector
The active binding depends on the value type: list, hash, set, zset or
stream. See the README for the complete table.
.TP
.B a / p / I
Append, prepend and insert relative to the element under the cursor (lists).
.TP
.B Ctrl+f / Ctrl+d
Filter hash fields; delete the selected fields.
.TP
.B Q
Filter a zset by score range.
.TP
.B F / Ctrl+T / G
Stream follow mode, XTRIM, and the consumer groups panel.
.TP
.B Tab
Cycle string view mode: Raw, JSON, Base64, Hex.
.SH EXIT STATUS
.TP
.B 0
Success.
.TP
.B 1
A performance gate failed (--bench or --profile-memory), or a runtime error
occurred.
.SH SEE ALSO
Full documentation: https://github.com/Joel\-Raju/reddish
"#;
