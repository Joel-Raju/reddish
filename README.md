# Reddish TUI

Reddish is a terminal-first Redis client built with Rust, `tokio`, `crossterm`, and `ratatui`.

It is designed for day-to-day Redis operations with keyboard-centric workflows: key browsing, typed value inspection, REPL execution, stats/slowlog views, Pub/Sub, and global search.

## Status

The project is under active development and tracks a milestone-driven spec (`spec/reddish-tui-spec.md`).

Implemented highlights:
- Key browser with namespace tree and SCAN-based loading
- Typed value inspector foundation (`string`, `list`, `hash`, `set`, `zset`, `stream`)
- REPL with persistent history, command parsing, and pipeline execution
- Info dashboard refresh loop with INFO + SLOWLOG integration
- Pub/Sub publish + live subscription stream wiring
- Global search overlay and jump-to-key flow
- Keymap and mouse support foundations
- CLI startup overrides (`--url`, `--profile`, `--readonly`, `--log-level`, `--theme`)

## Features

- Connection and startup
  - Connection profiles (`standalone`, `cluster`, `sentinel`) with TLS/password resolution support
  - Startup overrides via CLI (`--url`, `--profile`, `--readonly`, `--log-level`, `--theme`)
- Data exploration and inspection
  - Key browser with namespace-aware navigation
  - Typed Redis value loading (`string`, `list`, `hash`, `set`, `zset`, `stream`)
  - Global search overlay with jump-to-key workflow
- Operational workflows
  - REPL with history, parsing, and pipeline execution
  - Info dashboard with periodic INFO and SLOWLOG refresh
  - Pub/Sub tab with publish flow and live subscription stream
- Interaction model
  - Vim/Emacs-style keymap foundations with configurable actions
  - Mouse capture and basic mouse interactions
  - Read-only mode to block destructive writes

## Installation

### Prerequisites

- Rust stable toolchain
- Redis server(s) for runtime usage

### Build from source

```bash
cargo build --release
```

Binary will be available at `target/release/reddish-tui`.

## Usage

```bash
# Start with default config and connection flow
cargo run

# Connect directly with URL
cargo run -- --url redis://localhost:6379/0

# Connect by saved profile name
cargo run -- --profile local-dev

# Force read-only mode
cargo run -- --readonly

# Override logging level and theme for this run
cargo run -- --log-level debug --theme nord
```

### CLI options

```text
redis-tui [OPTIONS]

--url <URL>          redis://[user:pass@]host:port/db
--profile <NAME>     Connect using a saved profile
--readonly           Disable all write operations
--log-level <LEVEL>  Override config log level
--theme <THEME>      Override config theme
```

## Configuration

### App config

Config file path:
- macOS: `~/Library/Application Support/redis-tui/config.toml`
- Linux: `~/.config/redis-tui/config.toml`

Example:

```toml
namespace_separator = ":"
scan_count = 200
refresh_interval_ms = 1000
log_level = "info"
color_scheme = "default"
mouse_enabled = true
confirm_deletes = true
```

### Connection profiles

Connection profiles are loaded from:
- macOS: `~/Library/Application Support/redis-tui/connections.toml`
- Linux: `~/.config/redis-tui/connections.toml`

## Keybindings

Reddish supports configurable keymaps and ships with Vim/Emacs-style defaults.

Keymap actions are configured under keybinding fields in your config and mapped to action IDs.

### Action groups

- Navigation: `nav_up`, `nav_down`, `nav_left`, `nav_right`
- Global: `quit`, `help`, `palette`, `filter`
- Tabs: `tab_keys`, `tab_repl`, `tab_info`, `tab_pubsub`
- Panel actions: `confirm`, `delete`, `refresh`, `edit`, `copy`

### Default keys (common)

```text
quit       q
help       ?
palette    :
filter     /

tab_keys   1
tab_repl   2
tab_info   3
tab_pubsub 4
```

### Navigation defaults by style

```text
Vim-style:
  nav_up    k
  nav_down  j
  nav_left  h
  nav_right l

Emacs-style:
  nav_up    Ctrl+p
  nav_down  Ctrl+n
  nav_left  Ctrl+b
  nav_right Ctrl+f
```

## Testing

Run the full suite:

```bash
cargo test -q
```

Some tests spin up local Redis instances and may print Redis startup warnings during execution.

## Project layout

- `src/app.rs` — app state and event routing
- `src/redis/` — Redis client, typed value model, server helpers
- `src/ui/` — UI components (keys, inspector, REPL, dashboard, pubsub, search)
- `src/config/` — config and keybinding models
- `src/events/` — input/tick event stream
- `src/terminal.rs` — terminal lifecycle + mouse capture setup
- `tests/` — milestone-oriented integration tests

## Roadmap

Near-term focus:
- deeper keymap coverage across all panels
- richer value editing workflows
- Pub/Sub UX refinements
- tighter spec-gate test coverage

## Contributing

Contributions are welcome.

Suggested workflow:
1. Open an issue describing the bug/feature.
2. Keep PRs small and milestone-scoped.
3. Run `cargo test -q` before submitting.
4. Include tests for behavior changes when possible.

## License

MIT
