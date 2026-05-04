# Reddish TUI — Product & Implementation Specification

> A high-fidelity, high-performance terminal user interface for Redis, built with [Ratatui](https://ratatui.rs/).

---

## Table of Contents

1. [Vision & Goals](#1-vision--goals)
2. [Target Users](#2-target-users)
3. [Design Principles](#3-design-principles)
4. [Feature Specification](#4-feature-specification)
   - 4.1 Connection Management
   - 4.2 Key Browser
   - 4.3 Value Inspector & Editor
   - 4.4 Command Palette / REPL
   - 4.5 Pub/Sub Monitor
   - 4.6 Server Info & Stats
   - 4.7 Slow Log Viewer
   - 4.8 Search & Filter
   - 4.9 Keymap & Configuration
5. [UI Layout & Navigation](#5-ui-layout--navigation)
6. [Non-Functional Requirements](#6-non-functional-requirements)
7. [Implementation Specification](#7-implementation-specification)
   - 7.1 Technology Stack
   - 7.2 Architecture Overview
   - 7.3 Module Breakdown
   - 7.4 Data Flow
   - 7.5 Async Model
   - 7.6 Rendering Pipeline
8. [Milestones](#8-milestones)
9. [Open Questions & Future Work](#9-open-questions--future-work)

---

## 1. Vision & Goals

**Redis TUI** is a terminal-native Redis client that gives developers and operators complete visibility and control over a Redis instance without leaving the terminal. It targets the gap between `redis-cli` (powerful but minimal) and GUI tools like RedisInsight (feature-rich but heavy and mouse-dependent).

### Primary Goals

- **Fluent keyboard-first UX** — every action reachable in ≤3 keystrokes from any screen
- **Real-time awareness** — live key browser refresh, Pub/Sub streaming, latency graphs
- **Zero friction editing** — inline editing of all Redis types with type-aware widgets
- **Connection flexibility** — standalone, Sentinel, Cluster, TLS, SSH tunnel
- **Blazing performance** — responsive on databases with 10M+ keys; never blocks the UI thread

### Non-Goals (v1)

- Replacing `redis-cli` scripting pipelines
- GUI / web interface
- Redis Stack (RedisSearch, RedisGraph) advanced query UI (planned post-v1)

---

## 2. Target Users

| Persona | Pain today | What this solves |
|---|---|---|
| **Backend developer** | Switches between terminal and GUI to inspect keys | Single terminal workflow |
| **SRE / Ops** | `redis-cli INFO` output is hard to parse at a glance | Structured, live stats dashboard |
| **Data engineer** | Tedious to browse deeply-nested Hash/Stream keys | Collapsible tree, field search |
| **Security-conscious team** | GUI tools require opening ports or installing agents | Runs over existing SSH session |

---

## 3. Design Principles

1. **Terminal-native first** — designed for 80×24 minimum, beautiful at 220×50
2. **Progressive disclosure** — simple actions on the surface, power features discoverable via `?` help and command palette
3. **Never block** — all I/O is async; the UI always renders at target frame rate
4. **Respect the user's terminal** — honour `$TERM`, 256-color and true-color detection, restore terminal state on exit
5. **Keyboard sovereignty** — fully operable without mouse; mouse support is additive
6. **Fail loudly, recover gracefully** — connection errors surface inline, not as crashes; auto-reconnect with back-off

---

## 4. Feature Specification

### 4.1 Connection Management

#### Connection Profiles

- Stored in `~/.config/redis-tui/connections.toml` (XDG-compliant)
- Fields: `name`, `host`, `port`, `db`, `username`, `password` (keychain / env-var reference), `tls` (bool + cert paths), `sentinel` (master name + sentinel list), `cluster` (bool), `ssh_tunnel` (host, user, key_path)
- Passwords may reference `env:REDIS_PASS` or `keychain:redis-tui/prod` to avoid plaintext storage

#### Connection Screen

- Shown on launch if no default connection; otherwise connects immediately
- List of saved profiles with last-connected timestamp and latency badge
- `n` — new connection (inline form), `e` — edit, `d` — delete, `Enter` — connect
- Quick-connect bar: `redis://[user:pass@]host:port/db` URL paste

#### Connection Status Bar

- Persistent bottom-right indicator: `● PROD (us-east-1) 127.0.0.1:6379 db0 | 1.2ms`
- Color: green (connected), yellow (reconnecting), red (disconnected)
- `Ctrl+R` — force reconnect from anywhere

---

### 4.2 Key Browser

The central panel, taking ~60% of the horizontal real estate.

#### Key Tree

- Keys grouped by namespace separator (default `:`, configurable)
- Lazy-loaded subtrees — only scan children when a node is expanded
- Uses `SCAN` cursor iteration, never `KEYS` (safe for production)
- Node icons by type: `S` String, `L` List, `H` Hash, `Z` ZSet, `St` Stream, `X` unknown
- TTL badge on each key: `∞` (no TTL), `2h34m` (remaining), `exp` (already expired but not evicted yet), colored from green → yellow → red as TTL approaches zero

#### Key List Controls

| Key | Action |
|---|---|
| `j` / `k` or `↑↓` | Navigate keys |
| `h` / `l` or `←→` | Collapse / expand namespace node |
| `Enter` | Open key in Value Inspector |
| `/` | Filter keys (fuzzy, regex toggle with `Ctrl+R`) |
| `n` | New key (prompts for type and name) |
| `D` | Delete key (confirmation prompt) |
| `r` | Rename key |
| `t` | Edit TTL |
| `d` | Duplicate key |
| `c` | Copy key name to clipboard |
| `e` | Expire key now (set TTL to 1s) |
| `R` | Refresh / rescan current namespace |
| `Space` | Toggle selection (multi-select for bulk ops) |
| `Ctrl+A` | Select all in current namespace |

#### Sorting & Display

- Sort: alphabetical (default), by type, by TTL (ascending/descending), by memory usage
- Toggle: `s` cycles through sort modes
- Show/hide TTL column, memory column independently

#### Namespace Navigation

- Breadcrumb trail at top of key panel: `> prod > user > sessions`
- `Backspace` — go up one namespace level
- `g` — go to root

---

### 4.3 Value Inspector & Editor

Right panel, shown when a key is selected.

#### Shared Controls (all types)

- Key name (editable inline with `r`)
- TTL display with inline edit (`t`)
- Encoding badge (e.g., `ziplist`, `skiplist`, `embstr`)
- Memory usage in bytes / KB / MB
- `e` — enter edit mode, `Esc` — cancel, `Ctrl+S` — save
- `y` — yank (copy) value to clipboard as JSON
- `?` — type-specific help overlay

#### String

- Raw value in a scrollable text area
- Toggle between `raw`, `json` (pretty-printed), `base64`, `hex` views with `Tab`
- If value is valid JSON, display as syntax-highlighted tree
- Edit: full-screen editor popup (respects `$EDITOR` if configured, else built-in)

#### List

- Scrollable indexed list: `[0] value`, `[1] value`, …
- `a` — RPUSH (append), `p` — LPUSH (prepend)
- `e` on item — LSET (edit in place)
- `D` on item — LREM (delete element)
- `i` — insert before/after selected index
- Show list length in header

#### Hash

- Two-column table: `field │ value`
- Filterable with `/`
- `a` — add field, `e` on field — edit value, `D` — HDEL field
- Bulk: `Space` to multi-select fields, `Ctrl+D` to delete selected

#### Set

- Scrollable member list
- `a` — SADD, `D` — SREM
- Set operations panel: `u` union, `i` intersect, `d` diff with another key (prompts for key name)

#### Sorted Set (ZSet)

- Three-column table: `rank │ score │ member`
- Sort by score (default) or member name
- `a` — ZADD, `e` — edit score, `D` — ZREM
- Range query panel: `q` — opens ZRANGEBYSCORE / ZRANGEBYLEX form

#### Stream

- Message log view: `id │ timestamp │ field=value …`
- Auto-scroll toggle: `f` (follow mode, like `tail -f`)
- Consumer group panel: `g` — shows groups, consumers, pending count
- `a` — XADD, `D` — XDEL, `Ctrl+T` — XTRIM form

---

### 4.4 Command Palette / REPL

Accessed with `:` from anywhere (vim-style).

- Full Redis command REPL — type any command, see response
- Command history with `↑↓`, persisted to `~/.local/share/redis-tui/history`
- Autocomplete: command names, key names (fuzzy), argument hints (subcommands, option flags)
- Response rendered with type-aware formatting (same as Value Inspector)
- Pipeline mode: `|` separator to queue multiple commands, execute atomically
- `Ctrl+L` — clear REPL output
- Previous responses scrollable in output pane
- Toggle to "raw mode" (`Ctrl+R`) for unformatted wire-protocol output

---

### 4.5 Pub/Sub Monitor

Tab `4` or `:pubsub`

- Subscribe to channels or patterns (`SUBSCRIBE`, `PSUBSCRIBE`)
- Message stream with timestamps, channel names, message sizes
- Auto-scroll with `f` toggle
- Publish form: `p` — opens inline compose for `PUBLISH channel message`
- Multiple active subscriptions shown as tagged streams, color-coded per channel
- `MONITOR` mode (superuser only): stream all commands arriving at the server

---

### 4.6 Server Info & Stats Dashboard

Tab `3` or `:info`

Real-time dashboard, refreshes every 1s (configurable).

#### Panels

- **Memory**: `used_memory`, `peak_memory`, `fragmentation_ratio`, eviction policy, sparkline graph (last 60s)
- **Clients**: connected clients, blocked clients, `maxclients`, sparkline
- **Stats**: commands/sec, hits/misses ratio, keyspace hits/misses sparkline, total connections received
- **Keyspace**: per-db key count, expires count, avg TTL — bar chart
- **Replication**: role badge (master/slave/sentinel), replication offset, connected replicas, replication lag
- **CPU**: `used_cpu_sys`, `used_cpu_user` gauges
- **Persistence**: RDB last save time, AOF status, RDB/AOF save in progress indicator

All numeric values include delta from previous tick (e.g., `+1,234 cmd/s`).

---

### 4.7 Slow Log Viewer

Tab accessible via `:slowlog`

- Table: `id │ timestamp │ duration (µs) │ command │ args`
- `SLOWLOG RESET` with `Ctrl+R`
- `SLOWLOG LEN` shown in header
- Duration bar chart (relative to slowest entry)
- Filter by minimum duration threshold
- Export selected entries to CSV with `x`

---

### 4.8 Search & Filter

- Global key search: `Ctrl+F` — full fuzzy search across all keys (uses SCAN internally, shows progress)
- Regex search mode toggle
- Search history persisted
- Results panel replaces key list, `Esc` returns to normal browse
- Value search: `Ctrl+Shift+F` — search within the current key's value (List/Hash/ZSet members)

---

### 4.9 Keymap & Configuration

Config file: `~/.config/redis-tui/config.toml`

| Option | Default | Description |
|---|---|---|
| `namespace_separator` | `:` | Key namespace delimiter |
| `scan_count` | `200` | SCAN COUNT hint per batch |
| `refresh_interval_ms` | `1000` | Stats dashboard refresh rate |
| `editor` | `$EDITOR` | External editor for string values |
| `max_value_display_bytes` | `102400` | Truncate large values in inspector |
| `color_scheme` | `"default"` | Theme: `default`, `dracula`, `nord`, `solarized`, `gruvbox` |
| `mouse_enabled` | `true` | Enable mouse support |
| `confirm_deletes` | `true` | Require confirmation before DELETE |
| `keybindings` | — | Override any default keybinding |

All keybindings remappable. Vim, Emacs, and custom presets supported.

---

## 5. UI Layout & Navigation

### Default Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ redis-tui   [1:Keys]  [2:REPL]  [3:Info]  [4:PubSub]          ?:help  q:quit│
├───────────────────────────┬─────────────────────────────────────────────────┤
│ KEYS  (12,453)   /filter  │  KEY: user:sessions:abc123              [Hash]  │
│                           │  TTL: 2h 34m   Fields: 12   Mem: 1.2 KB        │
│ ▶ prod (4,211)            ├─────────────────────────────────────────────────┤
│ ▼ user (8,242)            │  field             │ value                      │
│   ▼ sessions (1,000)      │──────────────────────────────────────────────── │
│     abc123      [H] 2h34m │  user_id           │ 8842901                    │
│     def456      [H] 1h12m │  created_at        │ 2026-04-30T10:22:11Z       │
│     ...                   │  last_active       │ 2026-05-04T08:14:33Z       │
│   ▶ cache (7,242)         │  role              │ admin                      │
│ ▶ queue (5,112)           │  token             │ eyJhbGc...                 │
│                           │  ...               │                            │
│                           │                    │                            │
│                           │                    │                            │
│                           │                    │                            │
│                           │                    │                            │
├───────────────────────────┴─────────────────────────────────────────────────┤
│ ● PROD  127.0.0.1:6379  db0  |  1.2ms  |  Redis 7.2.4  |  j/k:nav  /:search│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Panels & Focus

- **Tab bar** — top row, `1-4` to switch tabs, `Shift+Tab` / `Tab` cycle panels within tab
- **Key Browser** — left panel, resizable with `<` / `>` (10%–80% width)
- **Value Inspector** — right panel, active when a key is focused
- **Status bar** — always visible, context-sensitive hints change per focused panel
- **Command Palette** — overlay triggered by `:`, floats over current layout
- **Help Overlay** — `?` — full keybinding reference, context-aware (shows bindings for focused panel)
- **Popup Dialogs** — confirmation, TTL edit, new key form — centered modal overlay

### Focus Model

Focus cycles: Key Browser → Value Inspector → (back). Indicated by colored border on focused panel. Mouse click also transfers focus.

---

## 6. Non-Functional Requirements

### Performance

| Metric | Target |
|---|---|
| Startup time (cold) | < 200ms |
| UI frame rate | 60 fps target, 30 fps minimum |
| Key browser render (10K visible nodes) | < 16ms per frame |
| SCAN batch (200 keys) | non-blocking; progress shown |
| Large value display (1MB string) | streamed; first bytes appear < 100ms |
| Memory footprint | < 50MB RSS at idle on 1M key database |

### Reliability

- All Redis commands wrapped with configurable timeout (default 5s)
- Auto-reconnect with exponential back-off (100ms → 30s) + jitter
- Read-only mode flag (`--readonly`) disables all write operations, enforced client-side
- Graceful terminal restore on panic (via `color-eyre` + `crossterm` cleanup hook)

### Compatibility

- Redis versions: 6.0+ (RESP2); 7.0+ enables RESP3 for richer type info
- Cluster: transparent key routing via `redis-rs` cluster client
- Sentinel: automatic master discovery and failover tracking
- Terminal: xterm-256color minimum; true-color detected via `$COLORTERM`
- OS: Linux, macOS; Windows (ConEmu / Windows Terminal)
- SSH tunnel: spawns `ssh -L` subprocess, tears down on exit

---

## 7. Implementation Specification

### 7.1 Technology Stack

| Layer | Crate | Rationale |
|---|---|---|
| TUI framework | `ratatui` (latest) | High-performance retained-mode TUI |
| Terminal backend | `crossterm` | Cross-platform terminal control |
| Async runtime | `tokio` (multi-thread) | Async I/O, timers, task management |
| Redis client | `redis-rs` (async feature) | Full Redis protocol, cluster, sentinel |
| Error handling | `color-eyre` | Rich error reports, panic hook |
| Config | `toml` + `serde` | Human-editable config files |
| Fuzzy search | `nucleo` | Fast, incremental fuzzy matching |
| Clipboard | `arboard` | Cross-platform clipboard access |
| Logging | `tracing` + `tracing-subscriber` | Async-aware, file-logged (never stdout) |
| Key bindings | Custom (via `crossterm` events) | Remappable, vim-style modal |

### 7.2 Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        main.rs                              │
│  - Init terminal, load config, parse args                   │
│  - Spawn App task                                           │
└───────────────────────┬─────────────────────────────────────┘
                        │
         ┌──────────────▼──────────────┐
         │         App (tokio task)    │
         │  - Event loop               │
         │  - Dispatch to components   │
         │  - Drive render             │
         └──┬───────────┬─────────────┘
            │           │
   ┌─────────▼──┐  ┌────▼──────────┐
   │  EventBus  │  │  Renderer     │
   │  (channel) │  │  (ratatui)    │
   └─────────┬──┘  └───────────────┘
             │
    ┌────────▼──────────────────────────┐
    │         Component Tree            │
    │  KeyBrowser │ ValueInspector      │
    │  REPL       │ StatsPanel          │
    │  PubSub     │ SlowLog             │
    │  StatusBar  │ CommandPalette      │
    └────────┬──────────────────────────┘
             │
    ┌────────▼──────────────────────────┐
    │       RedisClient (async)         │
    │  Connection pool (1 primary +     │
    │  1 subscribe-only connection)     │
    └───────────────────────────────────┘
```

### 7.3 Module Breakdown

```
src/
├── main.rs                  Entry point, terminal init/restore
├── app.rs                   App state, main event loop
├── config/
│   ├── mod.rs               Config loading, defaults, validation
│   ├── connections.rs       Connection profile structs
│   └── keybindings.rs       Keymap loading and lookup
├── redis/
│   ├── client.rs            Async Redis client wrapper
│   ├── scanner.rs           Cursor-based SCAN iterator
│   ├── types.rs             Typed Redis value enums
│   └── cluster.rs           Cluster-aware routing
├── ui/
│   ├── app_layout.rs        Root layout calculation
│   ├── tab_bar.rs           Tab rendering
│   ├── status_bar.rs        Status bar rendering
│   ├── key_browser/
│   │   ├── mod.rs           Key browser component
│   │   ├── tree.rs          Namespace tree data structure
│   │   └── scanner_task.rs  Background SCAN task
│   ├── value_inspector/
│   │   ├── mod.rs           Dispatcher by Redis type
│   │   ├── string.rs        String view/edit widget
│   │   ├── list.rs          List widget
│   │   ├── hash.rs          Hash table widget
│   │   ├── set.rs           Set widget
│   │   ├── zset.rs          Sorted set widget
│   │   └── stream.rs        Stream viewer
│   ├── repl/
│   │   ├── mod.rs           REPL component
│   │   ├── autocomplete.rs  Command + key autocomplete
│   │   └── history.rs       Persistent command history
│   ├── stats/
│   │   ├── mod.rs           Stats dashboard component
│   │   └── sparkline.rs     Rolling sparkline buffer
│   ├── pubsub.rs            Pub/Sub monitor component
│   ├── slowlog.rs           Slow log viewer
│   └── widgets/
│       ├── confirm.rs       Confirmation dialog
│       ├── input.rs         Single-line input widget
│       ├── help.rs          Help overlay
│       └── editor_popup.rs  Full-screen value editor
├── events/
│   ├── mod.rs               Event enum (Key, Tick, Redis, Resize)
│   └── handler.rs           Global event dispatch
└── clipboard.rs             Clipboard integration
```

### 7.4 Data Flow

#### Key Browser Population

```
App start
  → scanner_task spawned (tokio::spawn)
  → SCAN 0 COUNT 200 → batch of keys
  → sent via mpsc channel to KeyBrowser component
  → KeyBrowser inserts into NamespaceTree
  → next render tick displays updated tree
  → cursor continues until SCAN returns "0"
```

#### Key Selection → Value Load

```
User presses Enter on key
  → KeyBrowser sends SelectKey(name, type) event
  → ValueInspector receives event
  → spawns load_value task (redis::GET/HGETALL/etc.)
  → on response, updates component state
  → next render tick shows value
```

#### Stats Refresh

```
Tick event (every refresh_interval_ms)
  → StatsPanel fires INFO ALL command
  → response parsed into StatsSnapshot
  → sparkline buffers updated (circular buffer, 60 samples)
  → diff computed against previous snapshot
  → render shows updated gauges + sparklines
```

### 7.5 Async Model

Two dedicated Tokio tasks, always running:

**Event Task** — reads crossterm events from stdin, sends to `mpsc::Sender<Event>`. Never blocks the render loop.

**Tick Task** — fires `Event::Tick` at configurable interval (default 250ms for UI, 1000ms for stats). Drives sparkline updates, TTL countdown, reconnect heartbeat.

**Redis operations** — each command spawned as `tokio::spawn` with a timeout wrapper. Result sent back through `mpsc` channel. The UI never `await`s Redis directly inside the render loop.

**PubSub** — dedicated Redis connection (subscribe-only, per Redis protocol). Subscription events streamed through a separate `broadcast::Sender<PubSubMessage>`.

### 7.6 Rendering Pipeline

```
Event received
  → update_state(event) — pure state mutation, no I/O
  → terminal.draw(|frame| render(frame, &state))
      → app_layout (splits into regions)
      → each component.render(frame, area, &state)
          → ratatui widgets (List, Table, Paragraph, Gauge, Sparkline, …)
  → frame flushed to terminal
```

**Double-buffering** is handled by Ratatui internally — only changed cells are written to the terminal. Components must not hold `Frame` references across render calls.

**Conditional rendering** — components track a `dirty` flag; if state hasn't changed, they return the cached widget tree. This keeps 60fps achievable even with large key lists.

---

## 8. Milestones

Each milestone is self-contained. The **Gate** section lists the tests that must pass (zero failures, zero warnings) before work on the next milestone begins. Tests are the source of truth — not manual verification.

> **Test infrastructure convention:** Integration tests that need a live Redis process use a `redis_test_server()` helper (defined in `tests/helpers/mod.rs`) that spawns `redis-server --port 16379 --daemonize no` as a `Child` process, waits for it to accept connections, and drops it (killing the process) when the test ends. All integration tests are in `tests/` and tagged `#[cfg(test)]`. Unit tests live in `#[cfg(test)]` modules inside each source file.

---

### Milestone 0 — Scaffolding & Terminal Lifecycle

**Duration:** Week 1  
**Goal:** A runnable binary with correct terminal setup, teardown, event dispatch, config loading, and logging. Nothing Redis-specific yet.

#### What to Build

**`src/main.rs`**
- Call `crossterm::terminal::enable_raw_mode()` on entry
- Switch to alternate screen: `crossterm::execute!(stdout, EnterAlternateScreen)`
- Register a `color_eyre` panic hook that calls `restore_terminal()` before printing the panic — the terminal must never be left in raw mode on a crash
- Construct `App`, run its event loop, then call `restore_terminal()` on clean exit

**`src/terminal.rs`**
- `fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>>` — encapsulates backend creation
- `fn restore_terminal(terminal: &mut Terminal<...>) -> Result<()>` — disable raw mode, leave alternate screen, show cursor
- Both functions must be idempotent (safe to call twice)

**`src/app.rs` — `App` struct**
```rust
pub struct App {
    pub should_quit: bool,
    pub config: Config,
    pub active_tab: Tab,
}
```
- `App::new(config: Config) -> Self`
- `App::run(&mut self, terminal: &mut Terminal<...>) -> Result<()>` — the main loop:
  1. `terminal.draw(|f| self.render(f))`
  2. `crossterm::event::poll(Duration::from_millis(16))`
  3. On `KeyCode::Char('q')` → set `should_quit = true`
  4. Loop until `should_quit`
- `App::render(&self, frame: &mut Frame)` — draw a 3-panel placeholder layout using `ratatui::layout::Layout` with `Direction::Horizontal` / `Direction::Vertical` splits; each panel is a `Block::default().borders(Borders::ALL).title(...)`

**`src/config/mod.rs`**
```rust
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub namespace_separator: Option<String>, // defaults to ":"
    pub scan_count: Option<u32>,             // defaults to 200
    pub refresh_interval_ms: Option<u64>,    // defaults to 1000
    pub log_level: Option<String>,           // defaults to "info"
    pub color_scheme: Option<String>,        // defaults to "default"
    pub mouse_enabled: Option<bool>,         // defaults to true
    pub confirm_deletes: Option<bool>,       // defaults to true
}

impl Config {
    pub fn load() -> Result<Self>  // reads from XDG config path, falls back to defaults
    pub fn namespace_separator(&self) -> &str
    pub fn scan_count(&self) -> u32
    pub fn refresh_interval_ms(&self) -> u64
}
```
- Config file path: `$XDG_CONFIG_HOME/redis-tui/config.toml` with fallback to `~/.config/redis-tui/config.toml`
- Missing file → use all defaults (not an error)
- Invalid TOML → return `Err` with a user-readable message

**`src/logging.rs`**
- `fn init_logging(config: &Config) -> Result<()>`
- Uses `tracing_subscriber` with a `RollingFileAppender` writing to `$XDG_DATA_HOME/redis-tui/tui.log`
- **Never writes to stdout or stderr** — those belong to the terminal
- Log level from config (default `info`)

**`src/events/mod.rs`**
```rust
pub enum Event {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}
```
- `EventHandler` struct with an `mpsc::Receiver<Event>`
- Constructor spawns two `tokio::task`s:
  - **Input task:** `crossterm::event::read()` in a blocking thread (`tokio::task::spawn_blocking`), converts to `Event::Key` or `Event::Resize`, sends on channel
  - **Tick task:** `tokio::time::interval(Duration::from_millis(250))` → sends `Event::Tick`
- `EventHandler::next(&self) -> Result<Event>` — `recv().await` on the channel

#### Gate — Tests That Must Pass Before Milestone 1

```
tests/m0_scaffolding.rs
```

**`test_config_defaults`** *(unit)*
```rust
// Deserialize an empty TOML string into Config
// Assert: namespace_separator() == ":"
// Assert: scan_count() == 200
// Assert: refresh_interval_ms() == 1000
```

**`test_config_from_toml`** *(unit)*
```rust
// Deserialize a TOML string with explicit values
// Assert all getters return the overridden values
```

**`test_config_invalid_toml_returns_err`** *(unit)*
```rust
// Pass malformed TOML bytes to Config::from_str
// Assert result is Err
```

**`test_restore_terminal_is_idempotent`** *(unit, with mock terminal)*
```rust
// Call restore_terminal() twice on a TestBackend terminal
// Assert no panic, no error on second call
```

**`test_event_tick_fires`** *(async integration)*
```rust
// Construct EventHandler with tick interval 50ms
// Await next() twice
// Assert at least one Event::Tick received within 200ms
```

**`test_app_quits_on_q`** *(unit)*
```rust
// Construct App with a TestBackend
// Feed KeyEvent { code: KeyCode::Char('q'), ... } through event channel
// Call App::run for one iteration
// Assert app.should_quit == true
```

**`test_render_does_not_panic`** *(unit)*
```rust
// Construct App, call app.render() with a Frame from TestBackend (16x80 size)
// Assert no panic
// Assert terminal.draw() returns Ok
```

**CI gate:** `cargo clippy -- -D warnings`, `cargo fmt -- --check`, `cargo test` all pass.

---

### Milestone 1 — Connection Management & Key Browser

**Duration:** Weeks 2–3  
**Goal:** Connect to a real Redis instance, SCAN its keys into a namespace tree, render the tree, and support basic keyboard navigation and key deletion.

#### What to Build

**`src/config/connections.rs`**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db: u8,
    pub username: Option<String>,
    pub password: Option<PasswordRef>,  // see below
    pub last_connected: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PasswordRef {
    Plaintext(String),
    Env(String),    // e.g. "REDIS_PASS"  →  std::env::var()
}

impl PasswordRef {
    pub fn resolve(&self) -> Result<String>
}

pub struct ConnectionStore {
    path: PathBuf,
    pub profiles: Vec<ConnectionProfile>,
}

impl ConnectionStore {
    pub fn load(path: &Path) -> Result<Self>
    pub fn save(&self) -> Result<()>
    pub fn add(&mut self, profile: ConnectionProfile)
    pub fn remove(&mut self, name: &str) -> bool
    pub fn get(&self, name: &str) -> Option<&ConnectionProfile>
}
```
- File: `$XDG_CONFIG_HOME/redis-tui/connections.toml`
- Serialize/deserialize with `serde` + `toml`

**`src/redis/client.rs`**
```rust
pub struct RedisClient {
    conn: MultiplexedConnection,
    profile: ConnectionProfile,
}

impl RedisClient {
    pub async fn connect(profile: &ConnectionProfile) -> Result<Self>
    pub async fn ping(&self) -> Result<Duration>   // round-trip latency
    pub async fn dbsize(&self) -> Result<u64>
    pub async fn key_type(&self, key: &str) -> Result<RedisType>
    pub async fn ttl(&self, key: &str) -> Result<Ttl>
    pub async fn delete(&self, key: &str) -> Result<()>
    pub async fn rename(&self, from: &str, to: &str) -> Result<()>
    pub async fn set_ttl(&self, key: &str, seconds: i64) -> Result<()>
}

pub enum RedisType { String, List, Hash, Set, ZSet, Stream, Unknown }

pub enum Ttl {
    NoExpiry,
    Expires(Duration),
    KeyNotFound,
}
```
- All methods have a hard timeout of 5 seconds via `tokio::time::timeout`
- On connection failure, return a typed `RedisError::ConnectionFailed` (not a raw `redis::Error`)

**`src/redis/scanner.rs`**
```rust
pub struct Scanner {
    client: RedisClient,
    cursor: u64,
    pattern: Option<String>,
    count: u32,
    pub finished: bool,
}

impl Scanner {
    pub fn new(client: RedisClient, count: u32) -> Self
    pub async fn next_batch(&mut self) -> Result<Vec<String>>
    // Returns the next batch of keys from SCAN. Sets self.finished = true when cursor wraps to 0.
}
```
- Uses `SCAN cursor MATCH pattern COUNT count` internally
- Never uses `KEYS` command

**`src/ui/key_browser/tree.rs` — `NamespaceTree`**
```rust
pub struct NamespaceTree {
    separator: char,
    root: TreeNode,
}

pub struct TreeNode {
    pub segment: String,         // the namespace segment, e.g. "user"
    pub children: BTreeMap<String, TreeNode>,
    pub keys: Vec<KeyEntry>,     // leaf keys at this level
    pub expanded: bool,
}

pub struct KeyEntry {
    pub full_name: String,
    pub redis_type: RedisType,
    pub ttl: Ttl,
}

impl NamespaceTree {
    pub fn new(separator: char) -> Self
    pub fn insert(&mut self, key: KeyEntry)
    pub fn visible_rows(&self) -> Vec<TreeRow>   // flattened view of expanded nodes
    pub fn expand(&mut self, path: &[&str])
    pub fn collapse(&mut self, path: &[&str])
    pub fn remove(&mut self, full_name: &str) -> bool
    pub fn total_keys(&self) -> usize
}

pub struct TreeRow {
    pub depth: usize,
    pub label: String,
    pub is_namespace: bool,
    pub key: Option<KeyEntry>,
}
```

**`src/ui/key_browser/scanner_task.rs`**
- `fn start_scan(client: RedisClient, config: &Config) -> mpsc::Receiver<Vec<KeyEntry>>`
- Spawns a `tokio::task` that drives `Scanner::next_batch()` in a loop
- After each batch, sends `Vec<KeyEntry>` on the channel
- On completion or error, closes the sender (receiver sees `None`)
- Fetches type and TTL for each key using `client.key_type()` and `client.ttl()` (pipelined: use `redis::pipe()` for batches of 50)

**`src/ui/key_browser/mod.rs` — `KeyBrowser` component**
```rust
pub struct KeyBrowser {
    pub tree: NamespaceTree,
    pub cursor: usize,          // index into visible_rows()
    pub filter: Option<String>,
    pub state: BrowserState,
}

pub enum BrowserState {
    Scanning { keys_loaded: usize },
    Ready,
    Error(String),
}

impl KeyBrowser {
    pub fn handle_event(&mut self, event: &Event) -> Option<BrowserAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
    pub fn apply_scan_batch(&mut self, batch: Vec<KeyEntry>)
}

pub enum BrowserAction {
    SelectKey(String, RedisType),   // user pressed Enter on a key
    DeleteKey(String),              // user pressed D and confirmed
    RefreshRequested,
}
```

Key rendering rules:
- Each `TreeRow` is one line in a `ratatui::widgets::List`
- Namespace nodes: indented by `depth * 2` spaces, prefix `▶` (collapsed) or `▼` (expanded)
- Key nodes: indented by `depth * 2 + 2` spaces, type badge `[H]`, TTL badge colored green/yellow/red
- Highlight selected row with the theme's `highlight` style

**`src/ui/widgets/confirm.rs`**
```rust
pub struct ConfirmDialog {
    pub prompt: String,
    pub confirmed: Option<bool>,   // None = still open
}

impl ConfirmDialog {
    pub fn render(&self, frame: &mut Frame, area: Rect)
    pub fn handle_event(&mut self, event: &Event)
    // 'y' or Enter → confirmed = Some(true)
    // 'n' or Esc   → confirmed = Some(false)
}
```

**`src/ui/status_bar.rs`**
```rust
pub struct StatusBar {
    pub connection_state: ConnectionState,
    pub latency_ms: Option<u64>,
    pub key_count: usize,
    pub hints: Vec<(String, String)>,  // [("j/k", "nav"), ("D", "delete"), ...]
}

pub enum ConnectionState {
    Connected { host: String, port: u16, db: u8 },
    Reconnecting { attempt: u32 },
    Disconnected,
}

impl StatusBar {
    pub fn render(&self, frame: &mut Frame, area: Rect)
}
```

**`src/ui/tab_bar.rs`**
```rust
pub enum Tab { Keys, Repl, Info, PubSub }

pub fn render_tab_bar(frame: &mut Frame, area: Rect, active: Tab)
// Renders "redis-tui  [1:Keys]  [2:REPL]  [3:Info]  [4:PubSub]"
// Active tab is highlighted; others are dimmed
```

**`App` updates**
- Add `scan_rx: Option<mpsc::Receiver<Vec<KeyEntry>>>` to `App`
- On `Event::Tick`, drain all pending scan batches from `scan_rx` → call `key_browser.apply_scan_batch()`
- Ping Redis every 5 ticks → update `status_bar.latency_ms`
- Route key events to `key_browser.handle_event()`, process returned `BrowserAction`s (delete → confirm dialog → `client.delete()`)

#### Gate — Tests That Must Pass Before Milestone 2

```
tests/m1_connection_key_browser.rs
```

**`test_connection_profile_serialization`** *(unit)*
```rust
// Build a ConnectionProfile, serialize to TOML, deserialize back
// Assert all fields round-trip correctly
// Assert PasswordRef::Env("REDIS_PASS") resolves via std::env if var is set
```

**`test_connection_store_add_remove`** *(unit)*
```rust
// Create a ConnectionStore with a tempfile path
// Add two profiles, save, reload from disk
// Assert both profiles present
// Remove one, save, reload
// Assert only one profile remains
```

**`test_scanner_scans_all_keys`** *(async integration — requires redis_test_server())*
```rust
// SET 500 keys with prefix "test:m1:"
// Run Scanner with count=50 until finished
// Assert all 500 key names returned (across all batches, deduplicated)
// Assert Scanner.finished == true after last batch
```

**`test_scanner_never_uses_keys_command`** *(async integration)*
```rust
// Enable Redis command logging (CONFIG SET latency-monitor-threshold 0)
// Run Scanner to completion
// Fetch SLOWLOG / MONITOR output (or use a mock client that records commands)
// Assert no "KEYS" command appears in recorded commands
```

**`test_namespace_tree_insert_and_visible_rows`** *(unit)*
```rust
// Insert: "user:session:abc", "user:session:def", "user:profile:xyz", "queue:jobs"
// Assert tree.total_keys() == 4
// Expand "user", then "user:session"
// Call visible_rows()
// Assert order: [user(ns), user:session(ns), user:session:abc(key), user:session:def(key), user:profile(ns), queue(ns)]
// Collapse "user:session"
// Assert user:session:abc and user:session:def no longer in visible_rows()
```

**`test_namespace_tree_remove`** *(unit)*
```rust
// Insert 3 keys, remove one by full name
// Assert total_keys() == 2
// Assert removed key not present in any visible_rows() output
```

**`test_confirm_dialog_keyboard`** *(unit)*
```rust
// Create ConfirmDialog, feed KeyEvent 'y' → assert confirmed == Some(true)
// Create fresh ConfirmDialog, feed KeyEvent Esc → assert confirmed == Some(false)
// Create fresh ConfirmDialog, feed 'n' → assert confirmed == Some(false)
```

**`test_redis_client_ping`** *(async integration)*
```rust
// Connect to test server
// Call client.ping()
// Assert Ok(latency) where latency < Duration::from_millis(500)
```

**`test_redis_client_delete`** *(async integration)*
```rust
// SET "del_test" "value"
// Call client.delete("del_test")
// Assert EXISTS "del_test" returns 0
```

**`test_redis_client_ttl`** *(async integration)*
```rust
// SET "ttl_test" "v" EX 100
// Assert client.ttl("ttl_test") is Ttl::Expires(d) where d <= 100s
// PERSIST "ttl_test"
// Assert client.ttl("ttl_test") is Ttl::NoExpiry
```

**`test_key_browser_renders_without_panic`** *(unit)*
```rust
// Build a KeyBrowser with 3 inserted keys
// Call render() with a TestBackend Frame at 80x24
// Assert no panic
```

**`test_scan_batch_updates_browser`** *(unit)*
```rust
// Create KeyBrowser
// Call apply_scan_batch() with 50 KeyEntry items
// Assert tree.total_keys() == 50
// Assert BrowserState is Scanning { keys_loaded: 50 }
```

---

### Milestone 2 — Value Inspector & Editor

**Duration:** Weeks 4–5  
**Goal:** View and write all six Redis value types. Every operation round-trips through the real Redis client.

#### What to Build

**`src/redis/types.rs`**
```rust
pub enum RedisValue {
    String(String),
    List(Vec<String>),
    Hash(IndexMap<String, String>),   // preserves insertion order
    Set(BTreeSet<String>),
    ZSet(Vec<ZSetEntry>),
    Stream(Vec<StreamEntry>),
}

pub struct ZSetEntry { pub score: f64, pub member: String }
pub struct StreamEntry { pub id: String, pub fields: IndexMap<String, String> }
```

**`src/redis/client.rs` — additions**
```rust
impl RedisClient {
    pub async fn get_value(&self, key: &str, r_type: RedisType) -> Result<RedisValue>
    // Dispatches: GET / LRANGE 0 -1 / HGETALL / SMEMBERS / ZRANGE WITHSCORES / XRANGE - +

    // String
    pub async fn set_string(&self, key: &str, value: &str) -> Result<()>

    // List
    pub async fn list_push(&self, key: &str, value: &str, head: bool) -> Result<()>
    pub async fn list_set(&self, key: &str, index: i64, value: &str) -> Result<()>
    pub async fn list_remove(&self, key: &str, value: &str) -> Result<()>

    // Hash
    pub async fn hash_set(&self, key: &str, field: &str, value: &str) -> Result<()>
    pub async fn hash_del(&self, key: &str, fields: &[&str]) -> Result<()>

    // Set
    pub async fn set_add(&self, key: &str, member: &str) -> Result<()>
    pub async fn set_rem(&self, key: &str, member: &str) -> Result<()>

    // ZSet
    pub async fn zadd(&self, key: &str, score: f64, member: &str) -> Result<()>
    pub async fn zrem(&self, key: &str, member: &str) -> Result<()>
    pub async fn zscore_update(&self, key: &str, member: &str, score: f64) -> Result<()>

    // Stream
    pub async fn xadd(&self, key: &str, fields: &[(&str, &str)]) -> Result<String>
    pub async fn xdel(&self, key: &str, id: &str) -> Result<()>
    pub async fn xgroups(&self, key: &str) -> Result<Vec<StreamGroup>>

    // Common
    pub async fn memory_usage(&self, key: &str) -> Result<u64>
    pub async fn object_encoding(&self, key: &str) -> Result<String>
}
```

**`src/ui/value_inspector/mod.rs`**
```rust
pub struct ValueInspector {
    pub key: Option<String>,
    pub value: Option<RedisValue>,
    pub loading: bool,
    pub error: Option<String>,
    pub edit_mode: bool,
}

impl ValueInspector {
    pub fn handle_event(&mut self, event: &Event) -> Option<InspectorAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
    pub fn set_value(&mut self, key: String, value: RedisValue, meta: KeyMeta)
}

pub struct KeyMeta {
    pub ttl: Ttl,
    pub encoding: String,
    pub memory_bytes: u64,
}

pub enum InspectorAction {
    WriteString { key: String, value: String },
    ListPush { key: String, value: String, head: bool },
    ListSet { key: String, index: i64, value: String },
    ListRemove { key: String, value: String },
    HashSet { key: String, field: String, value: String },
    HashDel { key: String, fields: Vec<String> },
    SetAdd { key: String, member: String },
    SetRem { key: String, member: String },
    ZAdd { key: String, score: f64, member: String },
    ZRem { key: String, member: String },
    XAdd { key: String, fields: Vec<(String, String)> },
    XDel { key: String, id: String },
    SetTtl { key: String, seconds: i64 },
    CopyToClipboard(String),
    OpenInEditor { key: String, current_value: String },
}
```

**`src/ui/value_inspector/string.rs`**
- Renders value in a `Paragraph` with `Wrap::Word`
- View mode tabs (cycle with `Tab`): `Raw` | `JSON` | `Base64` | `Hex`
  - `JSON`: attempt `serde_json::from_str`, pretty-print with 2-space indent, syntax-highlighted using ANSI spans (strings=green, numbers=cyan, keys=yellow, booleans=magenta)
  - `Base64`: `base64::encode(value.as_bytes())`
  - `Hex`: `value.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")`
- Edit mode: inline text area using `tui-textarea` crate; `Ctrl+S` → emit `WriteString`
- `$EDITOR` integration: write value to a temp file, `std::process::Command::new(editor).arg(&path).status()`, read file back on exit, emit `WriteString`

**`src/ui/value_inspector/list.rs`**
- Renders as a `List` with items `[0] value`, `[1] value`, …
- `a` → emit `ListPush { head: false }` after inline input prompt
- `p` → emit `ListPush { head: true }`
- `e` on selected → inline edit, emit `ListSet { index: cursor }`
- `D` on selected → emit `ListRemove`

**`src/ui/value_inspector/hash.rs`**
- Renders as a `Table` with columns `Field` (40%) `Value` (60%)
- `/` → activates field filter (substring match)
- `a` → two-step input (field name, then value), emit `HashSet`
- `e` on row → edit value, emit `HashSet`
- `D` on row → emit `HashDel` (single); `Space` multi-select + `Ctrl+D` → `HashDel` (multi)

**`src/ui/value_inspector/set.rs`**
- Renders as a scrollable `List`
- `a` → inline input → `SetAdd`; `D` → `SetRem`

**`src/ui/value_inspector/zset.rs`**
- Three-column `Table`: `Rank` | `Score` | `Member`
- Sort toggle `s`: by score asc (default) / score desc / member alpha
- `a` → two-step input (member, score) → `ZAdd`
- `e` on row → edit score input → `ZAdd` (ZADD XX updates score)
- `D` → `ZRem`

**`src/ui/value_inspector/stream.rs`**
- Renders message list: `id` `│` `timestamp` `│` formatted `field=value` pairs (truncated to fit width)
- Follow mode toggle `f`: on `Event::Tick`, if follow is active and inspector holds a stream key, emit a background reload action
- `g` → opens consumer group panel as a split pane (groups table: name, consumers, pending, last-delivered-id)
- `a` → multi-field input form → `XAdd`; `D` → `XDel`

**`src/ui/widgets/input.rs`**
```rust
pub struct InputWidget {
    pub value: String,
    pub cursor: usize,
    pub label: String,
    pub submitted: Option<String>,   // Some(value) when Enter pressed
    pub cancelled: bool,             // true when Esc pressed
}

impl InputWidget {
    pub fn handle_event(&mut self, event: &Event)
    pub fn render(&self, frame: &mut Frame, area: Rect)
    // Handles: printable chars (insert at cursor), Backspace, Delete,
    //          Left/Right (move cursor), Home/End, Ctrl+U (clear), Enter, Esc
}
```

**`src/ui/widgets/editor_popup.rs`**
- Full-screen overlay using `tui-textarea` with line numbers
- `Ctrl+S` → save; `Esc` → cancel (with unsaved-changes warning)

**App integration**
- On `BrowserAction::SelectKey(name, type)` → spawn `tokio::task` to call `client.get_value()` + `memory_usage()` + `object_encoding()`, send result as `Event::ValueLoaded(RedisValue, KeyMeta)`
- On `InspectorAction` → dispatch to corresponding `client` method, on success re-load value

#### Gate — Tests That Must Pass Before Milestone 3

```
tests/m2_value_inspector.rs
```

**`test_get_set_string`** *(async integration)*
```rust
// client.set_string("k", "hello") → assert Ok
// client.get_value("k", RedisType::String) → assert RedisValue::String("hello")
```

**`test_string_view_json_format`** *(unit)*
```rust
// Construct StringViewer with value = r#"{"a":1,"b":true}"#
// Switch to JSON tab
// Assert rendered text contains "a" and "1" on separate formatted lines
// Assert no panic
```

**`test_string_view_base64`** *(unit)*
```rust
// Construct StringViewer with value = "hello"
// Switch to Base64 tab
// Assert rendered text contains "aGVsbG8="
```

**`test_string_view_hex`** *(unit)*
```rust
// value = "AB"  (bytes 0x41, 0x42)
// Switch to Hex tab
// Assert rendered text contains "41 42"
```

**`test_list_push_pop`** *(async integration)*
```rust
// client.list_push("l", "a", false)  // RPUSH
// client.list_push("l", "b", false)
// client.list_push("l", "z", true)   // LPUSH
// get_value → RedisValue::List(["z", "a", "b"])
// client.list_set("l", 0, "Z") → get_value → ["Z", "a", "b"]
// client.list_remove("l", "a") → get_value → ["Z", "b"]
```

**`test_hash_set_del`** *(async integration)*
```rust
// hash_set("h", "f1", "v1"), hash_set("h", "f2", "v2")
// get_value → RedisValue::Hash { "f1"→"v1", "f2"→"v2" }
// hash_del("h", &["f1"])
// get_value → RedisValue::Hash { "f2"→"v2" }
```

**`test_set_add_rem`** *(async integration)*
```rust
// set_add("s", "a"), set_add("s", "b")
// get_value → RedisValue::Set containing {"a","b"}
// set_rem("s", "a")
// get_value → Set containing only {"b"}
```

**`test_zset_add_update_remove`** *(async integration)*
```rust
// zadd("z", 1.0, "a"), zadd("z", 2.5, "b")
// get_value → ZSet with entries sorted by score
// zscore_update("z", "a", 99.0)
// get_value → "a" now has score 99.0, rank 1 (highest)
// zrem("z", "b")
// get_value → only "a" remains
```

**`test_stream_xadd_xdel`** *(async integration)*
```rust
// xadd("stream", &[("field", "value")]) → returns id like "12345-0"
// get_value → RedisValue::Stream with 1 entry
// xdel("stream", &id)
// get_value → RedisValue::Stream with 0 entries
```

**`test_input_widget_typing`** *(unit)*
```rust
// Create InputWidget, feed 'h', 'i' key events
// Assert widget.value == "hi"
// Feed Backspace → assert "h"
// Feed Enter → assert submitted == Some("h")
```

**`test_input_widget_cancel`** *(unit)*
```rust
// Feed Esc → assert widget.cancelled == true
```

**`test_value_inspector_renders_all_types`** *(unit)*
```rust
// For each RedisValue variant, call inspector.set_value() and then render()
// Assert no panic for any variant at terminal size 80x24 and 40x10 (constrained)
```

---

### Milestone 3 — REPL & Command Palette

**Duration:** Week 6  
**Goal:** Full Redis REPL accessible via `:`, with command history, autocomplete, and pipelined execution.

#### What to Build

**`src/redis/command_schema.rs`**
- Embed a static `&[CommandDef]` at compile time (`include_str!` a JSON file in `assets/`)
- Each `CommandDef` has: `name: &str`, `arity: i8`, `subcommands: &[&str]`, `flags: &[&str]`
- Covers all standard Redis commands from Redis 7 docs
- `fn commands_matching(prefix: &str) -> Vec<&'static CommandDef>`

**`src/ui/repl/history.rs`**
```rust
pub struct CommandHistory {
    entries: VecDeque<String>,   // max 1000 entries
    cursor: Option<usize>,       // None = current input; Some(i) = browsing history
    path: PathBuf,
}

impl CommandHistory {
    pub fn load(path: &Path) -> Self
    pub fn save(&self) -> Result<()>
    pub fn push(&mut self, cmd: String)
    pub fn prev(&mut self, current_input: &str) -> &str
    pub fn next(&mut self) -> &str
    pub fn reset_cursor(&mut self)
}
```
- Persists to `$XDG_DATA_HOME/redis-tui/history` (one command per line, newest last)
- Deduplicates consecutive identical commands

**`src/ui/repl/autocomplete.rs`**
```rust
pub struct Autocomplete {
    pub candidates: Vec<String>,
    pub selected: usize,
}

impl Autocomplete {
    pub fn compute(input: &str, key_cache: &[String]) -> Self
    // 1. If input is empty or first token: match command names from CommandSchema
    // 2. If first token is a known command and cursor is on arg 1: fuzzy-match key_cache
    // 3. If first token has subcommands (e.g. CLIENT): match subcommand names
    // Returns top 8 candidates sorted by score (nucleo fuzzy ranker)

    pub fn accept(&self) -> Option<&str>   // currently highlighted candidate
    pub fn next(&mut self)
    pub fn prev(&mut self)
}
```

**`src/ui/repl/mod.rs`**
```rust
pub struct Repl {
    pub input: String,
    pub cursor: usize,
    pub output: VecDeque<ReplEntry>,   // max 500 entries, oldest dropped
    pub history: CommandHistory,
    pub autocomplete: Option<Autocomplete>,
    pub raw_mode: bool,
}

pub struct ReplEntry {
    pub command: String,
    pub response: ReplResponse,
    pub timestamp: DateTime<Utc>,
}

pub enum ReplResponse {
    Ok(String),
    Array(Vec<String>),
    Error(String),
    Pipelined(Vec<ReplResponse>),
}

impl Repl {
    pub fn handle_event(&mut self, event: &Event) -> Option<ReplAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
    pub fn push_response(&mut self, cmd: String, resp: ReplResponse)
}

pub enum ReplAction {
    Execute(Vec<String>),        // tokenized args for single command
    ExecutePipeline(Vec<Vec<String>>),  // multiple commands separated by "|"
}
```

Input handling:
- Printable chars → insert at cursor; Backspace, Delete, Left, Right, Home, End — standard cursor ops
- `Tab` → if autocomplete open, cycle candidates; else open autocomplete
- `↑` / `↓` → history navigation (updates input)
- `Ctrl+U` → clear input
- `Ctrl+L` → clear output pane
- `Enter` → parse and emit `ReplAction`
- `Esc` → close autocomplete if open; clear input if autocomplete already closed

Pipeline parsing:
- Split input on ` | ` (space-pipe-space)
- Each segment tokenized via shell-like splitting (respect quoted strings: `"foo bar"` = one token)

Response rendering (when `raw_mode == false`):
- String response → `Paragraph`
- Array response → numbered `List` items
- Error response → red text
- Nested arrays → indented representation
- When `raw_mode == true` → raw RESP wire format representation

**`src/redis/client.rs` — additions**
```rust
impl RedisClient {
    pub async fn execute_raw(&self, args: &[String]) -> Result<redis::Value>
    // Constructs Cmd from args, executes, returns raw redis::Value
    // Apply 5s timeout

    pub async fn execute_pipeline(&self, cmds: &[Vec<String>]) -> Result<Vec<redis::Value>>
    // redis::pipe() with all commands, execute_async
}
```

**`src/redis/value_display.rs`**
```rust
pub fn redis_value_to_repl_response(v: redis::Value) -> ReplResponse
// Converts redis::Value enum to ReplResponse for display
```

**App integration**
- `:` key from any panel → push `AppMode::Repl` onto a mode stack
- `Esc` in Repl → pop mode, return to previous
- On `ReplAction::Execute` → `client.execute_raw()`, convert, call `repl.push_response()`
- On `ReplAction::ExecutePipeline` → `client.execute_pipeline()`, convert each, push as `Pipelined`

#### Gate — Tests That Must Pass Before Milestone 4

```
tests/m3_repl.rs
```

**`test_history_push_and_browse`** *(unit)*
```rust
// Push "GET foo", "SET bar baz", "GET foo" (duplicate consecutive — deduplicated)
// Assert entries.len() == 2
// prev() → "SET bar baz"; prev() → "GET foo"; next() → "SET bar baz"
```

**`test_history_persistence`** *(unit)*
```rust
// Write history to tempfile, reload from same path
// Assert same entries in same order
```

**`test_history_max_1000`** *(unit)*
```rust
// Push 1001 entries, assert entries.len() == 1000
// Assert oldest entry was dropped (first pushed is gone)
```

**`test_autocomplete_command_prefix`** *(unit)*
```rust
// Autocomplete::compute("GE", &[]) → candidates contains "GET", "GETSET", "GETRANGE", …
// Assert "GET" is first candidate (exact prefix match ranks highest)
```

**`test_autocomplete_key_arg`** *(unit)*
```rust
// key_cache = ["user:1", "user:2", "session:abc"]
// Autocomplete::compute("GET us", &key_cache) → candidates contains "user:1", "user:2"
// Assert "session:abc" is not in candidates (no match for "us")
```

**`test_pipeline_parsing`** *(unit)*
```rust
// Input: "SET foo bar | GET foo | DEL foo"
// Parse → Vec of 3 Vec<String>: [["SET","foo","bar"], ["GET","foo"], ["DEL","foo"]]
```

**`test_quoted_token_parsing`** *(unit)*
```rust
// Input: r#"SET key "hello world""#
// Tokenize → ["SET", "key", "hello world"]  (3 tokens, quotes stripped)
```

**`test_execute_raw_get_set`** *(async integration)*
```rust
// client.execute_raw(&["SET".into(), "repl_k".into(), "v".into()]) → Ok
// client.execute_raw(&["GET".into(), "repl_k".into()]) → redis::Value::Data("v")
```

**`test_execute_pipeline`** *(async integration)*
```rust
// Pipeline: [["SET","a","1"], ["SET","b","2"], ["MGET","a","b"]]
// Assert responses[2] is an array containing "1" and "2"
```

**`test_execute_pipeline_partial_error`** *(async integration)*
```rust
// Pipeline: [["SET","x","not_a_number"], ["INCR","x"]]
// Assert responses[0] is Ok
// Assert responses[1] is an error (not a panic)
```

**`test_repl_renders_without_panic`** *(unit)*
```rust
// Create Repl with 50 output entries of mixed types
// render() with TestBackend at 80x24
// Assert no panic
```

**`test_repl_ctrl_l_clears_output`** *(unit)*
```rust
// Push 10 entries to repl.output
// Feed Ctrl+L event
// Assert repl.output.is_empty()
```

---

### Milestone 4 — Stats Dashboard & Slow Log

**Duration:** Week 7  
**Goal:** Real-time server health dashboard driven by `INFO ALL`, sparkline graphs, and a slow log viewer.

#### What to Build

**`src/redis/info.rs`**
```rust
#[derive(Debug, Default)]
pub struct StatsSnapshot {
    pub timestamp: Instant,

    // Memory
    pub used_memory_bytes: u64,
    pub used_memory_peak_bytes: u64,
    pub mem_fragmentation_ratio: f64,
    pub maxmemory_bytes: Option<u64>,
    pub eviction_policy: String,

    // Clients
    pub connected_clients: u64,
    pub blocked_clients: u64,
    pub maxclients: u64,

    // Stats
    pub total_commands_processed: u64,
    pub instantaneous_ops_per_sec: u64,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub total_connections_received: u64,

    // Keyspace (per-db)
    pub databases: Vec<DbStats>,

    // Replication
    pub role: ReplicationRole,
    pub connected_replicas: u64,
    pub replication_offset: u64,

    // CPU
    pub used_cpu_sys: f64,
    pub used_cpu_user: f64,

    // Persistence
    pub rdb_last_save_time: u64,
    pub aof_enabled: bool,
    pub rdb_bgsave_in_progress: bool,
}

pub struct DbStats { pub id: u8, pub keys: u64, pub expires: u64 }
pub enum ReplicationRole { Master, Replica, Sentinel }

pub fn parse_info(raw: &str) -> Result<StatsSnapshot>
// Parses the flat key:value output of INFO ALL
// Returns Err if required fields are missing
```

**`src/redis/client.rs` — additions**
```rust
impl RedisClient {
    pub async fn info_all(&self) -> Result<StatsSnapshot>
    pub async fn slowlog_get(&self, count: u64) -> Result<Vec<SlowLogEntry>>
    pub async fn slowlog_reset(&self) -> Result<()>
    pub async fn slowlog_len(&self) -> Result<u64>
}

pub struct SlowLogEntry {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub duration_micros: u64,
    pub command: Vec<String>,
}
```

**`src/ui/stats/sparkline.rs`**
```rust
pub struct SparklineBuffer {
    data: VecDeque<u64>,
    capacity: usize,
}

impl SparklineBuffer {
    pub fn new(capacity: usize) -> Self          // capacity = 60
    pub fn push(&mut self, value: u64)
    pub fn as_slice(&self) -> &[u64]            // for ratatui Sparkline widget
    pub fn latest(&self) -> Option<u64>
    pub fn delta(&self) -> Option<i64>          // latest - previous, None if < 2 samples
}
```

**`src/ui/stats/mod.rs`**
```rust
pub struct StatsPanel {
    pub latest: Option<StatsSnapshot>,
    pub prev: Option<StatsSnapshot>,

    // Sparkline buffers
    pub memory_buf: SparklineBuffer,
    pub clients_buf: SparklineBuffer,
    pub ops_buf: SparklineBuffer,
    pub hits_buf: SparklineBuffer,
}

impl StatsPanel {
    pub fn update(&mut self, snapshot: StatsSnapshot)
    pub fn render(&self, frame: &mut Frame, area: Rect)
}
```

Layout (6-panel grid using `ratatui::layout::Layout` nested horizontal + vertical):
- Row 1: Memory gauge + sparkline | Client count + sparkline | Ops/sec + sparkline
- Row 2: Keyspace bar chart (one bar per db) | Replication info | CPU / Persistence flags

Each panel is a `Block` with title. Values display current reading + delta badge (`+123`, `-45`).

**`src/ui/slowlog.rs`**
```rust
pub struct SlowLogPanel {
    pub entries: Vec<SlowLogEntry>,
    pub cursor: usize,
    pub sort_col: SortCol,
}

pub enum SortCol { Id, Timestamp, Duration }

impl SlowLogPanel {
    pub fn handle_event(&mut self, event: &Event) -> Option<SlowLogAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
}

pub enum SlowLogAction {
    Reset,
    Export,   // triggers CSV export
}
```

Rendering:
- `Table` with columns: `ID` | `Time` | `Duration (µs)` | `Command`
- A thin bar below each row represents duration as a fraction of the slowest entry (relative bar chart)
- `Ctrl+R` → `SlowLogAction::Reset`
- `x` → `SlowLogAction::Export` → write CSV to `./redis-tui-slowlog-<timestamp>.csv`

**App integration**
- `Tab` 3 → switch to `StatsPanel`
- On `Event::Tick` when on Stats tab, spawn `client.info_all()` task → on result call `stats_panel.update()`
- Stats refresh independent of UI tick (uses `refresh_interval_ms` from config)
- Slow log loaded on entering Stats tab and on explicit `Ctrl+R`

#### Gate — Tests That Must Pass Before Milestone 5

```
tests/m4_stats.rs
```

**`test_parse_info_valid`** *(unit)*
```rust
// Provide a realistic INFO ALL string (include in test fixtures as a .txt file)
// parse_info(&raw) → Ok(snapshot)
// Assert: connected_clients > 0
// Assert: role == ReplicationRole::Master
// Assert: databases contains at least one DbStats
// Assert: used_memory_bytes > 0
```

**`test_parse_info_missing_field`** *(unit)*
```rust
// Provide INFO output with "used_memory" line removed
// Assert parse_info returns Err
```

**`test_sparkline_buffer_capacity`** *(unit)*
```rust
// SparklineBuffer::new(5)
// Push 7 values: [10,20,30,40,50,60,70]
// Assert as_slice() == [30,40,50,60,70]  (oldest dropped, capacity=5)
// Assert latest() == Some(70)
// Assert delta() == Some(10)
```

**`test_sparkline_buffer_delta_insufficient_data`** *(unit)*
```rust
// Empty buffer → delta() == None
// Push 1 value → delta() == None
// Push 2nd value → delta() == Some(second - first)
```

**`test_stats_panel_update_populates_buffers`** *(unit)*
```rust
// Create StatsPanel, call update() twice with different snapshots
// Assert memory_buf.as_slice().len() == 2
// Assert ops_buf.latest() reflects the second snapshot's ops value
```

**`test_stats_panel_renders_without_panic`** *(unit)*
```rust
// Call stats_panel.render() with a populated snapshot at 220x50, 80x24, and 80x10
// Assert no panic in any case
```

**`test_slowlog_get`** *(async integration)*
```rust
// CONFIG SET slowlog-log-slower-than 0   (log everything)
// Execute 5 commands via client.execute_raw
// client.slowlog_get(10) → assert entries.len() >= 5
// Assert each entry has duration_micros >= 0 and command non-empty
```

**`test_slowlog_reset`** *(async integration)*
```rust
// Ensure slowlog has entries
// client.slowlog_reset() → Ok
// client.slowlog_len() → 0
```

**`test_slowlog_csv_export`** *(unit)*
```rust
// Build a SlowLogPanel with 3 synthetic entries
// Trigger Export action, write to a tempfile path
// Read CSV back, assert 3 data rows + header row
// Assert duration column values match input
```

**`test_slowlog_renders_duration_bars`** *(unit)*
```rust
// Create SlowLogPanel with entries of durations [100, 500, 1000]
// render() at 120x30 — assert no panic
// (bar chart correctness validated visually / snapshot test)
```

---

### Milestone 5 — Pub/Sub & Global Search

**Duration:** Week 8  
**Goal:** Live Pub/Sub message streaming and full-database fuzzy key search.

#### What to Build

**`src/redis/pubsub.rs`**
```rust
pub struct PubSubClient {
    connection: PubSubConnection,   // dedicated redis-rs pubsub connection
    tx: broadcast::Sender<PubSubMessage>,
}

pub struct PubSubMessage {
    pub channel: String,
    pub payload: String,
    pub received_at: DateTime<Utc>,
    pub kind: PubSubKind,
}

pub enum PubSubKind { Message, PatternMessage { pattern: String }, Subscribe, Unsubscribe }

impl PubSubClient {
    pub async fn new(profile: &ConnectionProfile) -> Result<(Self, broadcast::Receiver<PubSubMessage>)>
    pub async fn subscribe(&mut self, channel: &str) -> Result<()>
    pub async fn psubscribe(&mut self, pattern: &str) -> Result<()>
    pub async fn unsubscribe(&mut self, channel: &str) -> Result<()>
    pub fn start_listening(self) -> tokio::task::JoinHandle<()>
    // Spawns a task: loop { pubsub.on_message() → tx.send() }
}
```
- Uses a **separate** `redis-rs` connection (not the multiplex connection from `RedisClient`) — required by Redis protocol for subscribe-mode connections
- `start_listening` task handles errors with tracing and continues the loop

**`src/ui/pubsub.rs`**
```rust
pub struct PubSubPanel {
    pub subscriptions: Vec<String>,     // active channel/pattern subscriptions
    pub messages: VecDeque<PubSubMessage>, // max 1000 messages
    pub follow: bool,                   // auto-scroll to latest
    pub cursor: usize,
    pub input: Option<InputWidget>,     // for subscribe/publish inputs
    pub input_mode: InputMode,
}

pub enum InputMode { None, Subscribe, PSubscribe, Publish }

impl PubSubPanel {
    pub fn handle_event(&mut self, event: &Event) -> Option<PubSubAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
    pub fn push_message(&mut self, msg: PubSubMessage)
}

pub enum PubSubAction {
    Subscribe(String),
    PSubscribe(String),
    Unsubscribe(String),
    Publish { channel: String, message: String },
}
```

Rendering:
- Top section: active subscriptions as colored tags (one color per channel, cycling 8 colors)
- Main section: message list — `time │ channel │ payload` — each channel name in its assigned color
- Bottom bar: `s`:subscribe `p`:psubscribe `u`:unsubscribe `P`:publish `f`:follow toggle

**`src/redis/search.rs`**
```rust
pub struct GlobalSearch {
    scanner: Scanner,
    matcher: nucleo::Matcher,
    pub results: Vec<KeyEntry>,
    pub query: String,
    pub progress: SearchProgress,
}

pub enum SearchProgress {
    Idle,
    Scanning { scanned: usize, total_estimate: u64 },
    Done { total_scanned: usize },
}

impl GlobalSearch {
    pub fn new(client: RedisClient, config: &Config) -> Self
    pub async fn next_batch(&mut self) -> bool
    // Scans next batch, filters matches against self.query using nucleo, appends to self.results
    // Returns true if more batches remain

    pub fn set_query(&mut self, q: String)
    // Updates query, re-scores existing results
}
```

**`src/ui/search.rs`**
```rust
pub struct SearchPanel {
    pub search: GlobalSearch,
    pub list_state: ListState,
    pub input: InputWidget,
    pub active: bool,
}

impl SearchPanel {
    pub fn handle_event(&mut self, event: &Event) -> Option<SearchAction>
    pub fn render_overlay(&self, frame: &mut Frame)
    // Renders as a centered floating panel (70% width, 80% height)
}

pub enum SearchAction {
    SelectKey(String, RedisType),
    Close,
}
```

**App integration**
- `Ctrl+F` → activate `SearchPanel`, begin scan, overlay rendered on top of current view
- `Esc` → deactivate search, scanner task cancelled (`JoinHandle::abort()`)
- On `Event::Tick` while search active → call `search.next_batch()` in a spawned task
- Tab 4 → `PubSubPanel`; on entering tab, start `PubSubClient` if not already started
- `broadcast::Receiver<PubSubMessage>` polled on `Event::Tick` → `pubsub_panel.push_message()`
- On `PubSubAction` → dispatch to `pubsub_client` methods

#### Gate — Tests That Must Pass Before Milestone 6

```
tests/m5_pubsub_search.rs
```

**`test_pubsub_subscribe_receive`** *(async integration)*
```rust
// Start PubSubClient, subscribe to channel "test-ch"
// From a second RedisClient, PUBLISH "test-ch" "hello"
// Await broadcast receiver with 1s timeout
// Assert received PubSubMessage { channel: "test-ch", payload: "hello" }
```

**`test_pubsub_psubscribe`** *(async integration)*
```rust
// psubscribe("test:*")
// Publish to "test:foo" and "test:bar"
// Assert 2 messages received with kind == PatternMessage { pattern: "test:*" }
```

**`test_pubsub_message_cap`** *(unit)*
```rust
// Push 1001 PubSubMessages into PubSubPanel
// Assert messages.len() == 1000  (oldest dropped)
```

**`test_pubsub_panel_renders_without_panic`** *(unit)*
```rust
// Populate panel with 50 messages across 3 channels
// render() at 80x24 — assert no panic
```

**`test_global_search_finds_matching_keys`** *(async integration)*
```rust
// SET "match:aaa" "1", "match:bbb" "2", "nomatch:zzz" "3"
// GlobalSearch with query "match"
// Drive next_batch() to completion
// Assert results contains "match:aaa" and "match:bbb"
// Assert "nomatch:zzz" not in results (fuzzy match against "match" should not score it)
```

**`test_global_search_progress_transitions`** *(async integration)*
```rust
// New search → assert progress == SearchProgress::Idle after construction
// Call next_batch() once → assert progress == Scanning
// Drive to completion → assert progress == Done
```

**`test_search_set_query_rescores`** *(unit)*
```rust
// Pre-populate search.results with ["alpha", "beta", "alphabet"]
// set_query("alpha")
// Assert "alpha" and "alphabet" are in results (score > 0)
// Assert "beta" is not in results or scored lower
```

**`test_search_overlay_renders_without_panic`** *(unit)*
```rust
// SearchPanel with 20 results, query="foo"
// render_overlay() at 120x40 — assert no panic
```

---

### Milestone 6 — Cluster, Sentinel & TLS

**Duration:** Week 9  
**Goal:** All production Redis deployment topologies supported. Connection switching without binary restart.

#### What to Build

**`src/config/connections.rs` — additions**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    // ... existing fields ...
    pub mode: ConnectionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionMode {
    Standalone,
    Cluster,
    Sentinel { master_name: String, sentinels: Vec<SentinelNode> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelNode { pub host: String, pub port: u16 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub verify_certs: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub client_cert_path: Option<PathBuf>,
    pub client_key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnel {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
    pub local_port: u16,   // auto-assigned if 0
}
```

**`src/redis/client.rs` — `RedisClient::connect` overhaul**
```rust
impl RedisClient {
    pub async fn connect(profile: &ConnectionProfile) -> Result<Self>
    // Dispatches based on profile.mode:
    //   Standalone → redis::Client::open(url)?.get_multiplexed_async_connection()
    //   Cluster    → redis::cluster::ClusterClient::new(nodes)?.get_async_connection()
    //   Sentinel   → redis::sentinel::SentinelClient::build(...)?.get_async_connection()
    // Applies TLS via native-tls if profile.tls.enabled
}
```

**`src/redis/ssh_tunnel.rs`**
```rust
pub struct SshTunnel {
    child: std::process::Child,
    pub local_port: u16,
}

impl SshTunnel {
    pub fn open(config: &SshTunnelConfig) -> Result<Self>
    // Spawns: ssh -N -L {local_port}:{remote_host}:{remote_port} {user}@{ssh_host} -i {key_path}
    // Probes local_port with TCP connect in a loop (100ms interval, 10s timeout)
    // Returns Err if tunnel doesn't become available within timeout
}

impl Drop for SshTunnel {
    fn drop(&mut self) { let _ = self.child.kill(); }
}
```

**`src/redis/cluster_info.rs`**
```rust
pub struct ClusterNode {
    pub id: String,
    pub addr: String,
    pub flags: Vec<String>,     // master, slave, myself, etc.
    pub slots: Vec<(u16, u16)>, // slot ranges this node handles
    pub ping_sent: u64,
    pub pong_recv: u64,
    pub link_state: String,
}

pub fn parse_cluster_nodes(raw: &str) -> Result<Vec<ClusterNode>>
// Parses CLUSTER NODES output (space-delimited, one node per line)
```

**`src/redis/client.rs` — cluster additions**
```rust
impl RedisClient {
    pub async fn cluster_nodes(&self) -> Result<Vec<ClusterNode>>
    pub async fn cluster_info(&self) -> Result<HashMap<String, String>>
}
```

**`src/ui/connection_screen.rs`**
```rust
pub struct ConnectionScreen {
    pub store: ConnectionStore,
    pub list_state: ListState,
    pub form: Option<ConnectionForm>,
    pub connecting: Option<String>,   // profile name being connected
    pub error: Option<String>,
}

pub struct ConnectionForm {
    pub fields: Vec<FormField>,
    pub active_field: usize,
    pub mode_select: ConnectionMode,
}

pub enum ConnectionScreenAction {
    Connect(ConnectionProfile),
    Save(ConnectionProfile),
    Delete(String),
    Cancel,
}

impl ConnectionScreen {
    pub fn handle_event(&mut self, event: &Event) -> Option<ConnectionScreenAction>
    pub fn render(&self, frame: &mut Frame, area: Rect)
}
```

**Connection switching (App-level)**
- `Ctrl+\` from any panel → push `AppMode::ConnectionScreen`
- On `ConnectionScreenAction::Connect` → drop existing `RedisClient` + `PubSubClient`, connect new client, pop mode
- Reconnect logic: on `RedisError::ConnectionFailed` → spawn reconnect task with back-off sequence `[100ms, 200ms, 400ms, 800ms, 1600ms, 5000ms, 30000ms]` + ±10% jitter; update `status_bar.connection_state = Reconnecting { attempt }`

**Read-only enforcement**
- `--readonly` CLI flag sets `App.readonly = true`
- All `InspectorAction`, `ReplAction`, `PubSubAction::Publish`, and delete/rename operations check `self.readonly` before emitting — return a user-visible "Read-only mode" inline error instead

#### Gate — Tests That Must Pass Before Milestone 7

```
tests/m6_cluster_sentinel_tls.rs
```

**`test_standalone_connect`** *(async integration)*
```rust
// ConnectionProfile { mode: Standalone, host: "127.0.0.1", port: 16379, ... }
// RedisClient::connect(&profile) → Ok
// client.ping() → Ok(latency)
```

**`test_password_ref_env`** *(unit)*
```rust
// std::env::set_var("TEST_REDIS_PASS", "secret")
// PasswordRef::Env("TEST_REDIS_PASS").resolve() → Ok("secret")
// std::env::remove_var("TEST_REDIS_PASS")
// PasswordRef::Env("TEST_REDIS_PASS").resolve() → Err
```

**`test_parse_cluster_nodes`** *(unit)*
```rust
// Provide a 3-node CLUSTER NODES output string (include as fixture)
// parse_cluster_nodes(&raw) → Ok(nodes) with len == 3
// Assert one node has flags containing "master" and "myself"
// Assert slot ranges are non-overlapping and together cover 0..=16383
```

**`test_ssh_tunnel_opens_and_closes`** *(integration — requires ssh + redis-server on localhost)*
```rust
// Mark as #[ignore] unless CI env var REDIS_TUI_SSH_TESTS=1 is set
// Open SSH tunnel to localhost
// Assert local_port accepts TCP connections
// Drop SshTunnel
// Assert local_port no longer accepts connections
```

**`test_tls_config_roundtrip`** *(unit)*
```rust
// Construct TlsConfig { enabled: true, verify_certs: false, ca_cert_path: Some(...), ... }
// Serialize to TOML, deserialize
// Assert all fields equal
```

**`test_reconnect_backoff_sequence`** *(unit — test the back-off generator, not the actual network)*
```rust
// backoff_sequence() → [100, 200, 400, 800, 1600, 5000, 30000] ms
// Each value should be within ±15% of the expected value (jitter)
// Assert sequence length == 7
// Assert max value == 30000 ± 15%
```

**`test_readonly_mode_blocks_writes`** *(unit)*
```rust
// Construct App with readonly = true
// Feed a 'D' (delete) key event on a selected key
// Assert no RedisClient write method was called (use a mock client)
// Assert inline error message is set in UI state
```

**`test_connection_screen_renders_without_panic`** *(unit)*
```rust
// Construct ConnectionScreen with 3 profiles
// render() at 80x24 — assert no panic
```

---

### Milestone 7 — Polish, Themes, Mouse & Performance

**Duration:** Week 10  
**Goal:** Production-ready quality. Themes, mouse, help overlay, performance benchmarks all pass.

#### What to Build

**`src/ui/theme.rs`**
```rust
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub border: Color,
    pub title: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub ttl_ok: Color,
    pub ttl_warn: Color,
    pub ttl_crit: Color,
    pub type_string: Color,
    pub type_list: Color,
    pub type_hash: Color,
    pub type_set: Color,
    pub type_zset: Color,
    pub type_stream: Color,
}

pub const THEMES: &[Theme] = &[DEFAULT, DRACULA, NORD, SOLARIZED_DARK, GRUVBOX_DARK];

impl Theme {
    pub fn by_name(name: &str) -> Option<&'static Theme>
}
```

Each theme defines all color fields as `ratatui::style::Color` values (exact RGB for true-color, nearest 256-color fallback computed at runtime by checking `$COLORTERM`).

**`src/ui/key_browser/mod.rs` — keybinding remapping**
```rust
// All handle_event() methods accept &Keymap instead of using hardcoded KeyCode matches
// Keymap is constructed from config at startup and passed through App state
```

**`src/config/keybindings.rs`**
```rust
#[derive(Debug, Deserialize)]
pub struct Keymap {
    pub nav_up: KeyDef,
    pub nav_down: KeyDef,
    pub nav_left: KeyDef,
    pub nav_right: KeyDef,
    pub confirm: KeyDef,
    pub cancel: KeyDef,
    pub delete: KeyDef,
    pub refresh: KeyDef,
    pub filter: KeyDef,
    pub edit: KeyDef,
    pub quit: KeyDef,
    pub copy: KeyDef,
    // ... all actions
}

#[derive(Debug, Deserialize)]
pub struct KeyDef {
    pub key: String,   // e.g. "j", "ctrl+r", "f5"
}

impl Keymap {
    pub fn default() -> Self          // vim-style defaults
    pub fn emacs() -> Self            // emacs-style alternative
    pub fn matches(&self, action: &str, event: &KeyEvent) -> bool
}
```

**Mouse support** — add to App event handling:
- `crossterm::event::EnableMouseCapture` on init when `config.mouse_enabled == true`
- On `Event::Mouse(MouseEvent::Down { column, row, .. })`:
  - Determine which panel contains `(column, row)` using stored `Rect` bounds from last render
  - Set focus to that panel
  - If click is within a `List` row → calculate item index from row offset, set cursor
- On `MouseEvent::ScrollUp` / `ScrollDown` → scroll focused panel

**`src/ui/widgets/help.rs`**
```rust
pub struct HelpOverlay {
    pub visible: bool,
    pub context: HelpContext,
}

pub enum HelpContext { KeyBrowser, ValueInspector, Repl, Stats, PubSub, Global }

impl HelpOverlay {
    pub fn render(&self, frame: &mut Frame, area: Rect)
    // Renders a centered modal with title "Keybindings — <context>"
    // Two-column layout: action name | key chord
    // Sourced from Keymap (shows user's actual bindings, not hardcoded strings)
}
```

**CLI argument parsing** (`clap` crate):
```
redis-tui [OPTIONS]
  --url <URL>          redis://[user:pass@]host:port/db  (skips connection screen)
  --profile <NAME>     Connect using a saved profile
  --readonly           Disable all write operations
  --log-level <LEVEL>  Override config log level [default: info]
  --theme <THEME>      Override config theme
```

**Performance requirements & instrumentation**

Add a `--bench` hidden flag that:
1. Connects to a Redis instance with 10,000 keys
2. Runs 1000 full render passes using `TestBackend`
3. Reports: min/max/p50/p95/p99 render time per frame
4. Exits with code 1 if p99 > 16ms (60fps budget)

Memory: add a `--profile-memory` hidden flag that prints RSS after 10s of idle via `/proc/self/status` (Linux) or `sysinfo` crate (cross-platform).

#### Gate — Tests That Must Pass Before Milestone 8

```
tests/m7_polish.rs
```

**`test_all_themes_have_all_fields`** *(unit)*
```rust
// For each theme in THEMES:
// Assert no Color field is Color::Reset (placeholder)
// Assert theme.name is non-empty and unique across THEMES
```

**`test_theme_by_name`** *(unit)*
```rust
// Theme::by_name("dracula") → Some(theme) with name == "dracula"
// Theme::by_name("nonexistent") → None
```

**`test_keymap_default_parses`** *(unit)*
```rust
// Keymap::default() → no panic
// keymap.matches("nav_up", &key_event('j')) → true
// keymap.matches("nav_up", &key_event('k')) → false
```

**`test_keymap_emacs_parses`** *(unit)*
```rust
// Keymap::emacs() → no panic
// keymap.matches("nav_up", &key_event_ctrl('p')) → true
```

**`test_keymap_from_toml_override`** *(unit)*
```rust
// Parse a TOML snippet: [keybindings]\n nav_up = { key = "w" }
// Assert keymap.matches("nav_up", &key_event('w')) → true
// Assert keymap.matches("nav_up", &key_event('j')) → false  (default overridden)
```

**`test_help_overlay_renders_without_panic`** *(unit)*
```rust
// For each HelpContext variant:
// help.render() at 80x24 — assert no panic
// Assert rendered output contains at least 5 keybinding rows
```

**`test_cli_url_flag_parsed`** *(unit)*
```rust
// Parse args: ["redis-tui", "--url", "redis://user:pass@localhost:6380/2"]
// Assert ConnectionProfile { host: "localhost", port: 6380, db: 2, username: Some("user") }
```

**`test_cli_readonly_flag`** *(unit)*
```rust
// Parse args: ["redis-tui", "--readonly"]
// Assert App would be constructed with readonly = true
```

**`test_render_performance_10k_keys`** *(benchmark, via cargo bench or hidden flag)*
```rust
// Insert 10,000 KeyEntry items into NamespaceTree (spread across 10 namespaces, all expanded)
// Render 100 times with TestBackend at 220x50
// Assert p99 render time < 16ms
```

**`test_rss_under_50mb_at_10k_keys`** *(integration — Linux/macOS only)*
```rust
// Mark #[ignore] unless CI env var REDIS_TUI_MEM_TESTS=1
// Load 10K keys into NamespaceTree, run 10 render passes
// Read RSS from /proc/self/status (Linux) or sysinfo
// Assert RSS < 50 * 1024 * 1024 bytes
```

---

### Milestone 8 — v1.0 Release

**Duration:** 3–5 days  
**Goal:** Cross-platform builds, security review, crate publication, documentation.

#### What to Build / Verify

- GitHub Actions matrix: `ubuntu-latest` (x86_64, aarch64 via cross), `macos-latest` (universal binary via `lipo`), `windows-latest`
- Security checklist:
  - `grep -rn "password\|secret\|token" src/ | grep -v "PasswordRef\|test"` — no plaintext credentials in non-config code
  - `tracing::debug!` calls must not log resolved passwords — verified by grep for `resolve()` call sites
  - Temp files for `$EDITOR` integration written to `tempfile::NamedTempFile` (auto-deleted on drop), never to a predictable path
  - `cargo audit` — zero high-severity advisories
- `cargo publish --dry-run` succeeds
- `README.md`: installation, quick-start, keybinding reference, screenshot/GIF
- `CHANGELOG.md`: all milestone deliverables listed
- Man page: `redis-tui.1` generated from CLI `--help` via `clap_mangen`

#### Gate — Tests That Must Pass for Release

**All previous milestone gates must pass on all three target platforms.**

**`test_cargo_audit_clean`** *(CI gate, not a Rust test)*
```bash
cargo install cargo-audit
cargo audit
# Exit code must be 0
```

**`test_no_plaintext_password_in_logs`** *(integration)*
```rust
// Construct a ConnectionProfile with PasswordRef::Plaintext("supersecret")
// Connect (will fail if no server, catch the error)
// Read the log file written during the attempt
// Assert log file contents do not contain "supersecret"
```

**`test_temp_file_deleted_on_drop`** *(unit)*
```rust
// Simulate $EDITOR integration: create NamedTempFile, write content, get path
// Drop the NamedTempFile handle
// Assert path no longer exists on disk
```

**`test_binary_opens_and_exits_on_all_platforms`** *(CI shell test)*
```bash
# Compile release binary
cargo build --release
# Run with --help (no Redis required)
./target/release/redis-tui --help
# Assert exit code 0
# Assert stdout contains "Usage:"
```

---

## 9. Open Questions & Future Work

### Open Questions (resolve before Milestone 3)

- Should MONITOR mode require a `--allow-monitor` flag to prevent accidental use on prod?
- Clipboard: fall back gracefully when no clipboard provider is available (headless servers)?
- How should cluster cross-slot operations be surfaced (warning vs. hard error)?

### Post-v1 Roadmap

- **Redis Stack support** — RedisSearch query UI, JSON type inspector, TimeSeries graphing
- **Diff view** — compare two keys side by side
- **Import / Export** — dump keys to JSON/RDB fragments, import from file
- **Scripting** — run Lua scripts via EVAL with syntax highlighting
- **Plugin system** — custom value renderers (e.g., Protobuf decoder via WASM plugin)
- **Multi-server** — split-pane comparing two Redis instances simultaneously
- **ACL editor** — visual editor for Redis ACL rules
