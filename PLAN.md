# Implementation Plan

## Phase A — Value Inspector Gaps

### A1. Set ops (`u`/`i`/`x`)
- **Client**: `sunionstore()`/`sinterstore()`/`sdiffstore()` on `RedisClientHandle`
- **UI**: Two-step prompts (destination key → additional source keys) in key_browser
- **Action**: `BrowserAction::SetOp { op, source_keys, dest }` wired in `app.rs`
- **Tests**: prompt opens, submit flow, two-step additional sources, empty reject

### A2. ZSet range query (`q`)
- **Client**: `zrangebyscore()` on `RedisClientHandle`
- **UI**: Prompt for min/max score → overlay results in value_inspector (zset view)
- **Action**: `InspectorAction::ZRangeQuery { key, min, max }`
- **Tests**: prompt flow, client method, render results overlay

### A3. Stream consumer groups (`g`)
- **UI**: Wire existing `xgroups()` into value_inspector stream view as split pane
- **Action**: `InspectorAction::ShowGroups(Vec<StreamGroup>)` toggle
- **Tests**: render groups panel, toggle on/off, empty state

---

## Phase B — PubSub Enhancements

### B1. PSUBSCRIBE + channel colors
- **Client**: `psubscribe()`/`punsubscribe()` on pubsub connection; `PubSubKind` enum
- **UI**: Color cycle per channel (8 colors), pattern subscription prompts (`s`/`p`), bottom bar hints
- **Actions**: `PubSubAction::PSubscribe`, `PubSubAction::PUnsubscribe`
- **Tests**: psubscribe flow, color assignment round-robin, pattern unsubscribe

### B2. PubSubClient as dedicated module
- **Refactor**: Extract `src/redis/pubsub.rs` module with `PubSubClient` struct
- **Wire**: Move inline pubsub logic from `app.rs` into dedicated module
- **Tests**: subscribe/receive through module API, message cap enforcement

---

## Phase C — Stats Dashboard Enhancements

### C1. SparklineBuffer
- **Module**: `src/ui/stats/sparkline.rs` — circular buffer (60 samples), `push()`, `as_slice()`, `latest()`, `delta()`
- **Tests**: capacity, overflow, delta with insufficient data

### C2. 6-panel sparkline grid
- **UI**: Split info dashboard into 2×3 grid: Memory/Client/Ops sparklines + Keyspace/Replication/CPU gauges
- **Data**: Diff computation from previous tick (`+1,234 cmd/s`), 4 sparkline buffers per snapshot
- **Tests**: render at various sizes, update populates buffers, delta values correct

### C3. Dedicated SlowLogPanel
- **Module**: `src/ui/slowlog.rs` — dedicated `SlowLogPanel` with `Table` rendering
- **Features**: Duration bar chart, CSV export (`x`), threshold filter, sort by column
- **Tests**: render with entries/empty, CSV export to file, duration bars, threshold filter

---

## Phase D — REPL & Client Enhancements

### D1. `execute_raw` / `execute_pipeline` on client
- **Client**: Add `execute_raw(&[String])` and `execute_pipeline(&[Vec<String>])` to `RedisClientHandle`
- **Refactor**: Migrate raw command dispatch from `app.rs` into client methods
- **Tests**: get/set through execute_raw, pipeline round-trip, partial error

### D2. Fuzzy REPL autocomplete
- **UI**: Wire `nucleo` injector into REPL overlay; `Tab` cycles matches; match list overlay
- **Integration**: Complete command name autocomplete + key name suggestions from browser
- **Tests**: autocomplete opens on Tab, Tab cycles, Esc closes, Enter accepts

---

## Phase E — Misc Features

### E1. MONITOR mode
- **Flag**: `--allow-monitor` CLI flag (default false)
- **Client**: `MONITOR` command connection, parse output stream
- **UI**: `Ctrl+M` toggle in REPL tab; render MONITOR stream as scrolling log
- **Tests**: monitor toggle, render stream, connection lifecycle

### E2. String editor popup + `$EDITOR` integration
- **UI**: Full-screen editor overlay using `TextAreaEditor`, `Ctrl+S` save, `Esc` cancel
- **Integration**: `$EDITOR` env var → spawn subprocess → read back on exit
- **Tests**: editor opens/closes, $EDITOR not set fallback, temp file cleanup

---

## Phase F — Release Prep

### F1. CI & Audit
- GitHub Actions matrix (ubuntu/macos/windows)
- `cargo audit` — zero high-severity advisories
- `cargo clippy -- -D warnings` gate

### F2. Documentation
- README.md: install, quick-start, keybinding reference, screenshot
- CHANGELOG.md: milestone deliverables
- Man page (`clap_mangen`)

---

## Ordering Rationale

1. **A1–A3 first**: Directly extend existing value_inspector and key_browser — minimal refactoring, immediately testable
2. **B1–B2**: PubSub is a well-isolated subsystem; enhancements don't touch other components
3. **C1–C3**: Stats dashboard is self-contained; sparkline/slowlog additions are purely additive
4. **D1–D2**: REPL autocomplete depends on D1 (client methods); D1 enables REPL overlay improvements
5. **E1–E2**: Self-contained features that depend on nothing else
6. **F1–F2**: End-of-project polish
