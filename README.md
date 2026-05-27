# Reddish TUI

Reddish is a terminal-first Redis client built with Rust, `tokio`, `crossterm`, and `ratatui`.

It is designed for day-to-day Redis operations with keyboard-centric workflows: key browsing, typed value inspection, REPL execution, stats/slowlog views, Pub/Sub, and global search.

## Status

The project is under active development and tracks a milestone-driven spec (`spec/reddish-tui-spec.md`).

Implemented highlights:
- Key browser with namespace tree, SCAN-based loading, multi-select, and sort modes
- Typed value inspector (`string`, `list`, `hash`, `set`, `zset`, `stream`) with editing and filtering
- Set operations: union/inter/diff with multi-source selection
- ZSet score range filtering
- REPL with persistent history, command parsing, pipeline execution, and autocomplete
- REPL overlay (`:`) for quick one-line commands without leaving the current tab
- Info dashboard refresh loop with INFO + SLOWLOG integration, colored stats, and scrolling
- Pub/Sub publish + live subscription stream with two-panel layout
- Global search overlay with real-time SCAN-backed filtering and jump-to-key flow
- Keymap and mouse support with Vim/Emacs presets
- CLI startup overrides (`--url`, `--profile`, `--readonly`, `--log-level`, `--theme`)

## Features

### Connection and startup
- Connection profiles (`standalone`, `cluster`, `sentinel`) with TLS/password resolution support
- Startup overrides via CLI (`--url`, `--profile`, `--readonly`, `--log-level`, `--theme`)

### Data exploration and inspection
- Key browser with namespace-aware navigation
- Multi-select with `Space` and select-all with `Ctrl+A`
- Sort modes: alphabetical, by Redis type, by TTL (`s` to cycle)
- Typed Redis value loading (`string`, `list`, `hash`, `set`, `zset`, `stream`)
- Value editing, TTL editing (`t`), and clipboard copy (`y`)
- Duplicate key via `DUMP`/`RESTORE` (`d`)
- Set operations: `SUNIONSTORE`, `SINTERSTORE`, `SDIFFSTORE` (`u`/`i`/`x`)
- ZSet score range query (`q`) with min/max filtering
- Global search overlay with jump-to-key workflow (`/`)

### Operational workflows
- REPL with history, parsing, pipeline execution (`;;`), and tab autocomplete
- REPL overlay (`:`) for quick commands from any tab
- Info dashboard with periodic INFO refresh, colored stats, and scrollable slowlog
- Pub/Sub tab with subscribe/unsubscribe, publish, and live message stream
- Read-only mode to block destructive writes

### Interaction model
- Vim/Emacs-style keymaps with configurable actions
- Mouse capture and basic mouse interactions
- Context-aware clipboard copy across all value types

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

### Global shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| quit | `q` | `Ctrl+c` | Quit application |
| help | `?` | `?` | Show help overlay |
| palette | `Ctrl+p` | `Ctrl+x` | Open command palette |
| filter | `/` | `Ctrl+s` | Open global search |
| repl_overlay | `:` | `Alt+;` | One-line REPL prompt |
| tab_keys | `1` | `Alt+1` | Keys tab |
| tab_repl | `2` | `Alt+2` | REPL tab |
| tab_info | `3` | `Alt+3` | Info dashboard tab |
| tab_pubsub | `4` | `Alt+4` | Pub/Sub tab |
| nav_up | `k` / `Up` | `Ctrl+p` | Move cursor up |
| nav_down | `j` / `Down` | `Ctrl+n` | Move cursor down |
| nav_left | `h` / `Left` | `Ctrl+b` | Collapse namespace / go back |
| nav_right | `l` / `Right` | `Ctrl+f` | Expand namespace / go forward |
| confirm | `Enter` | `Enter` | Confirm action |
| cancel | `Esc` | `Ctrl+g` | Cancel / close overlay |
| refresh | `R` | `Ctrl+l` | Refresh key browser scan |
| copy | `y` | `Ctrl+w` | Copy context-aware value |

### Key browser shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| confirm | `Enter` | `Enter` | Inspect selected key |
| delete | `D` | `Ctrl+d` | Delete selected key |
| new_key | `n` | `Ctrl+n` | Create a new key |
| rename_key | `r` | `Ctrl+r` | Rename selected key |
| expire_key | `e` | `Ctrl+e` | Persist key (remove TTL) |
| set_ttl | `t` | `Ctrl+t` | Set TTL on selected key |
| duplicate_key | `d` | `Ctrl+shift+d` | Duplicate selected key |
| set_union | `u` | `Ctrl+u` | SUNIONSTORE with multi-select |
| set_inter | `i` | `Ctrl+i` | SINTERSTORE with multi-select |
| set_diff | `x` | `Ctrl+d` | SDIFFSTORE with multi-select |
| toggle_select | `Space` | `Space` | Toggle key selection |
| select_all | `Ctrl+a` | `Ctrl+a` | Select all visible keys |
| cycle_sort | `s` | `Ctrl+o` | Cycle sort: alpha → type → TTL |

### Value inspector shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| edit | `e` | `Ctrl+e` | Edit current item |
| delete | `D` | `Ctrl+d` | Delete current item |
| copy | `y` | `Ctrl+w` | Copy current item |
| set_ttl | `t` | `Ctrl+t` | Set TTL on inspected key |
| inspector_list_push | `a` | `Ctrl+shift+a` | RPUSH to list |
| inspector_list_prepend | `p` | `Ctrl+shift+p` | LPUSH to list |
| inspector_hash_add | `a` | `Ctrl+shift+h` | Add hash field |
| inspector_set_add | `a` | `Ctrl+shift+s` | Add set member |
| inspector_zset_add | `a` | `Ctrl+shift+z` | Add zset member |
| inspector_zset_range_query | `q` | `Ctrl+shift+q` | Filter zset by score range |
| inspector_stream_add | `a` | `Ctrl+shift+x` | Add stream entry |
| inspector_toggle_view | `f` | `Ctrl+shift+f` | Toggle stream compact/full |
| inspector_goto_end | `g` | `Ctrl+shift+g` | Jump to last stream entry |
| cycle_sort | `s` | `Ctrl+o` | Toggle zset score sort asc/desc |
| Tab | `Tab` | `Tab` | Cycle string view mode |

### Info dashboard shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| nav_up | `k` / `Up` | `Ctrl+p` | Scroll slowlog up |
| nav_down | `j` / `Down` | `Ctrl+n` | Scroll slowlog down |
| Space | `Space` | `Space` | Toggle slowlog compact mode |

### Pub/Sub shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| Tab | `Tab` | `Tab` | Switch channel / message input mode |
| delete | `D` | `Ctrl+d` | Unsubscribe from selected channel |
| Ctrl+L | `Ctrl+l` | `Ctrl+l` | Clear message history |

### Search overlay shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| search_execute | `Enter` | `Enter` | Jump to selected key |
| search_close | `Esc` | `Ctrl+g` | Close search without action |

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
