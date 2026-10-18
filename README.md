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

### Man page

A pre-generated roff man page lives at `man/redis-tui.1`. Install it with:

```bash
sudo install -Dm644 man/redis-tui.1 /usr/local/share/man/man1/redis-tui.1
```

It is built from the same `Cli` definition the binary parses, so it cannot
drift. Regenerate after changing a flag:

```bash
cargo run --example generate-man > man/redis-tui.1
```

A unit test compares the committed page against a freshly generated one and
fails if they differ.

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

### Mouse

Mouse capture is enabled when the terminal starts.

| Action | Description |
|--------|-------------|
| Click a tab label | Switch tabs. Hit regions come from the rendered labels, so the target matches what is on screen. |
| Click a key row | Move the selection to that key. Accounts for the list's scroll offset, so clicking a scrolled row selects the key that row shows. |
| Scroll wheel | Move the key selection up or down on the Keys tab. |

Clicks below the last key, in the tab-bar gaps, or in an overlay are ignored.
While the search, help or connection overlays are open, clicks go to the overlay
rather than the panes behind it.

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
| inspector_list_insert | `I` | `Ctrl+shift+i` | Insert before/after the current list element |
| inspector_hash_add | `a` | `Ctrl+shift+h` | Add hash field |
| inspector_hash_filter | `Ctrl+f` | `Alt+f` | Filter hash fields by name (`Esc` clears) |
| inspector_value_search | `Ctrl+shift+f` | `Alt+/` | Search within the current value's members |
| delete_selected | `Ctrl+d` | `Ctrl+shift+d` | Delete all selected hash fields |
| select_all | `Ctrl+a` | `Ctrl+a` | Select all visible hash fields |
| toggle_select | `Space` | `Space` | Toggle selection of the field under the cursor |
| inspector_set_add | `a` | `Ctrl+shift+s` | Add set member |
| inspector_zset_add | `a` | `Ctrl+shift+z` | Add zset member |
| inspector_zset_range_query | `Q` | `Ctrl+shift+q` | Filter zset by score range |
| inspector_stream_add | `a` | `Ctrl+shift+x` | Add stream entry |
| inspector_stream_trim | `Ctrl+t` | `Ctrl+shift+t` | XTRIM the stream (MAXLEN / MINID) |
| inspector_stream_follow | `F` | `Ctrl+shift+v` | Follow the stream tail (reloads on tick) |
| inspector_stream_groups | `G` | `Ctrl+shift+m` | Toggle the consumer groups panel |
| inspector_toggle_view | `f` | `Ctrl+shift+f` | Toggle stream compact/full |
| inspector_goto_end | `g` | `Ctrl+shift+g` | Jump to last stream entry |
| cycle_sort | `s` | `Ctrl+o` | Toggle zset score sort asc/desc |
| Tab | `Tab` | `Tab` | Cycle string view mode (Raw / JSON / Base64 / Hex) |

> `q` quits the app from anywhere, so inspector bindings deliberately avoid it.
> The vim preset binds the hash filter to `Ctrl+f` because `/` is already the
> global search key.

### Search overlay shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| typing | — | — | Query is fuzzy-ranked by default |
| `search_toggle_mode` | `Ctrl+r` | `Ctrl+alt+r` | Toggle fuzzy / regex matching |
| `search_history_prev` | `Alt+Up` | `Alt+Up` | Recall an earlier query |
| `search_history_next` | `Alt+Down` | `Alt+Down` | Move forward through query history |
| `search_execute` | `Enter` | `Enter` | Open the highlighted key |
| `search_close` | `Esc` | `Ctrl+g` | Close the overlay |
| `Up` / `Down` | `Up` / `Down` | `Ctrl+p` / `Ctrl+n` | Move through results |

### Info dashboard shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| nav_up | `k` / `Up` | `Ctrl+p` | Scroll slowlog up |
| nav_down | `j` / `Down` | `Ctrl+n` | Scroll slowlog down |
| Space | `Space` | `Space` | Toggle slowlog compact mode |
| `t` | `t` | `t` | Cycle slowlog duration threshold |
| `e` | `e` | `e` | Export slowlog to CSV |

### Pub/Sub shortcuts

| Action | Vim | Emacs | Description |
|--------|-----|-------|-------------|
| Tab | `Tab` | `Tab` | Switch channel / message input mode |
| delete | `D` | `Ctrl+d` | Unsubscribe from selected channel |
| Ctrl+L | `Ctrl+l` | `Ctrl+l` | Clear message history |

## Testing

Run the full suite:

```bash
cargo test -q
```

Some tests spin up local Redis instances and may print Redis startup warnings
during execution. Integration tests expect a Redis on **port 16379**:

```bash
redis-server --port 16379 --daemonize yes --loglevel warning --save ''
```

## Performance gates

Two hidden flags enforce the spec's quality budgets. They run before any
terminal setup, open no Redis connection, and exit non-zero on a violation, so
they are safe in CI:

```bash
cargo run --release -- --bench            # 1000 render cycles, p99 must be <= 16ms
cargo run --release -- --profile-memory   # peak RSS at 10K keys must be <= 50MB
```

Measured on a release build with 10K keys:

| Scenario | p50 | p99 | Budget |
|----------|-----|-----|--------|
| Render, all namespaces expanded | 12.7ms | 13.5ms | 16ms ✅ |
| Render, default collapsed view | 0.14ms | 0.22ms | 16ms ✅ |
| Peak RSS (all six value types loaded) | — | 22.7MB | 50MB ✅ |

The fully-expanded case is the worst case: `KeyBrowser::render` builds one
`ListItem` per visible row, so expanding all 10K keys formats 10K rows per frame
even though only ~24 are drawn. The default view expands nothing.

Both gates assume a `--release` build; a debug build is roughly 3x slower and
will fail the render budget.

## Project layout

- `src/app.rs` — app state and event routing
- `src/redis/` — Redis client, typed value model, server helpers
- `src/ui/` — UI components (keys, inspector, REPL, dashboard, pubsub, search)
- `src/config/` — config and keybinding models
- `src/events/` — input/tick event stream
- `src/terminal.rs` — terminal lifecycle + mouse capture setup
- `src/bench.rs` — `--bench` / `--profile-memory` performance gates
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
