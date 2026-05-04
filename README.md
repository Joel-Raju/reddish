# Reddish TUI

A Redis terminal UI client built in Rust.

## Features

- **Connection Management** — Connect to standalone, cluster, sentinel, and TLS-enabled Redis instances
- **Key Browser** — Navigate Redis keys with namespace tree support using SCAN (never KEYS)
- **Value Inspector** — View and edit strings, hashes, lists, sets, sorted sets, and streams
- **REPL** — Execute raw Redis commands with history
- **Command Palette** — Quick access to all actions via fuzzy search
- **Stats Dashboard** — Real-time server stats and slow log viewer
- **Pub/Sub** — Subscribe to channels and view messages in real time
- **Global Search** — Fuzzy find across all keys
- **Themes** — Built-in themes: default, dracula, nord, solarized_dark, gruvbox_dark
- **Read-only Mode** — Prevent accidental writes with `--readonly`

## Quick Start

### Prerequisites

- Rust toolchain (1.80+)
- A running Redis server (for full functionality)

### Build

```bash
cargo build --release
```

### Run

```bash
# Connect to local Redis on default port
cargo run

# Connect via URL
cargo run -- --url redis://user:pass@localhost:6380/2

# Read-only mode
cargo run -- --readonly
```

### Test

```bash
# Start a test Redis server on port 16379, then:
cargo test
```

## Key Bindings

| Action | Key |
|--------|-----|
| Quit | `q` |
| Help | `?` |
| Navigate Up | `k` / `↑` |
| Navigate Down | `j` / `↓` |
| Enter / Expand | `Enter` |
| Delete | `d` |
| Refresh | `r` |
| Filter | `/` |
| Edit | `e` |
| Copy | `y` |
| Switch Tab | `1`–`4` |

## Configuration

Configuration is stored at:

- **Linux**: `~/.config/redis-tui/config.toml`
- **macOS**: `~/Library/Application Support/redis-tui/config.toml`

Example `config.toml`:

```toml
namespace_separator = ":"
scan_count = 200
refresh_interval_ms = 1000
log_level = "info"
color_scheme = "dracula"
mouse_enabled = true
confirm_deletes = true

[keybindings]
nav_up = { key = "k" }
nav_down = { key = "j" }
```

## Architecture

- `src/app.rs` — Main application loop and state machine
- `src/ui/` — TUI widgets (key browser, value viewer, REPL, command palette, dashboard, pubsub, search, help)
- `src/redis/` — Async Redis client wrapper and server info parsing
- `src/config/` — Configuration and connection profile management
- `src/events.rs` — Crossterm event handling
- `src/terminal.rs` — Terminal lifecycle management

## License

MIT
