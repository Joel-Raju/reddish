# v1 Execution Plan

> Derived from `spec/implementationv1.md` + `spec/PLAN.md`  
> Rule: **One atomic commit per task**. After each commit, update `spec/implementationv1.md` to mark the corresponding item(s) as ✅ Done.

---

## How to Use This Plan

1. Pick the next uncompleted task in order.
2. Implement it (including tests).
3. Run `cargo test` + `cargo clippy`.
4. Commit with the suggested message.
5. Edit `spec/implementationv1.md`: change `❌ Missing` / `⚠️ Partial` → `✅ Done` for the relevant line(s).
6. Continue to the next task.

---

## Execution Order

Tasks are ordered by **dependency** (lower-numbered tasks must complete first) and **risk** (low-risk additive changes before refactors).

---

### Phase A — Value Inspector Gaps (Complete Phase A)

#### Task A3.1 — Stream consumer groups keybinding
**Refs:** `implementationv1.md` 2.19, `PLAN.md` A3  
**Files:** `src/config/keybindings.rs`  
**What:** Add `inspector_stream_groups` keybinding (default `g` vim / `ctrl+shift+g` emacs).
```
feat: add stream consumer groups keybinding (A3.1)

Add inspector_stream_groups to Keymap with defaults:
- vim: g
- emacs: ctrl+shift+g
```

#### Task A3.2 — Stream consumer groups state + action
**Refs:** `implementationv1.md` 2.19, `PLAN.md` A3  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** Add `stream_show_groups: bool` field to `ValueInspector`, `PromptMode::StreamGroupsToggle`, `InspectorAction::StreamToggleGroups`. Handle `g` key in stream view to toggle.
```
feat: add stream consumer groups toggle state + action (A3.2)

Add stream_show_groups bool, StreamGroupsToggle prompt mode,
and InspectorAction::StreamToggleGroups. Pressing 'g' in stream
view toggles the groups panel on/off.
```

#### Task A3.3 — Stream consumer groups render (split pane)
**Refs:** `implementationv1.md` 2.19, `PLAN.md` A3  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** When `stream_show_groups == true`, split stream render into left (messages) + right (groups table). Call `client.xgroups()` in `app.rs` when toggle fires, populate groups data.
```
feat: render stream consumer groups as split pane (A3.3)

When stream_show_groups is active, split the stream view into
left panel (messages) and right panel (groups table with
name/consumers/pending/last-delivered-id).
```

#### Task A3.4 — Stream consumer groups tests
**Refs:** `implementationv1.md` 2.19, `PLAN.md` A3  
**Files:** `tests/value_inspector.rs` or `src/ui/value_inspector/mod.rs`  
**What:** Test toggle on/off, render with groups, empty state.
```
test: stream consumer groups toggle and render (A3.4)
```

**Phase A Complete.** ✅

---

### Phase B — PubSub Enhancements

#### Task B1.1 — PubSubKind enum + `psubscribe` client methods
**Refs:** `implementationv1.md` 5.3, `PLAN.md` B1  
**Files:** `src/redis/client.rs`, `src/ui/pubsub/mod.rs`  
**What:** Add `PubSubKind { Channel(String), Pattern(String) }` enum. Add `psubscribe()`/`punsubscribe()` on the pubsub connection. Extend `PubSubMessage` with `kind: PubSubKind`.
```
feat: add PSUBSCRIBE/PUNSUBSCRIBE client support (B1.1)

Add PubSubKind enum (Channel vs Pattern), psubscribe() and
punsubscribe() methods on RedisClientHandle, and update
PubSubMessage to carry the kind.
```

#### Task B1.2 — PubSub UI: pattern mode + color cycle
**Refs:** `implementationv1.md` 5.3, 5.7, `PLAN.md` B1  
**Files:** `src/ui/pubsub/mod.rs`, `src/app.rs`  
**What:** Add `PubSubInputMode::Pattern` (toggle with `Tab` cycle: channel → message → pattern). Color cycle (8 colors) assigned per channel/pattern. Add `PubSubAction::PSubscribe`/`PUnsubscribe`. Bottom bar hints update.
```
feat: add PSUBSCRIBE pattern mode + per-channel color cycle (B1.2)

Tab cycles through channel/message/pattern input modes.
Each subscription gets a unique color from an 8-color cycle.
Messages render with channel-colored tags.
```

#### Task B1.3 — PubSub PSUBSCRIBE tests
**Refs:** `implementationv1.md` 5.16, `PLAN.md` B1  
**Files:** `tests/pubsub.rs`  
**What:** Test pattern subscribe, pattern message receive, pattern unsubscribe, color assignment round-robin.
```
test: psubscribe flow, color round-robin, pattern unsubscribe (B1.3)
```

#### Task B2 — Extract PubSubClient module
**Refs:** `implementationv1.md` (architecture), `PLAN.md` B2  
**Files:** `src/redis/pubsub.rs` (new), `src/app.rs`, `src/ui/pubsub/mod.rs`  
**What:** Create `src/redis/pubsub.rs` with `PubSubClient` struct. Move inline pubsub logic (connection spawn, message routing) from `app.rs` into this module. Wire back in.
```
refactor: extract PubSubClient into dedicated module (B2)

Move pubsub connection management and message routing from
inline app.rs into src/redis/pubsub.rs::PubSubClient.
No behavioral changes.
```

**Phase B Complete.** ✅

---

### Phase C — Stats Dashboard Enhancements

#### Task C1.1 — SparklineBuffer module
**Refs:** `implementationv1.md` 4.3, 4.18-4.19, `PLAN.md` C1  
**Files:** `src/ui/stats/sparkline.rs` (new)  
**What:** Circular buffer (60 samples), `push()`, `as_slice()`, `latest()`, `delta()`. Delta returns `None` if < 2 samples.
```
feat: add SparklineBuffer circular buffer module (C1.1)

60-sample ring buffer with push, as_slice, latest, and delta.
Delta returns None when insufficient data.
```

#### Task C1.2 — SparklineBuffer tests
**Refs:** `implementationv1.md` 4.18-4.19, `PLAN.md` C1  
**Files:** `src/ui/stats/sparkline.rs`  
**What:** Test capacity, overflow wrap, delta with insufficient data, delta correctness.
```
test: SparklineBuffer capacity, overflow, delta (C1.2)
```

#### Task C2.1 — Stats dashboard: add sparkline buffers + diff computation
**Refs:** `implementationv1.md` 4.2-4.9, `PLAN.md` C2  
**Files:** `src/ui/info_dashboard/mod.rs`  
**What:** Add `prev_stats: Option<SystemStats>` and `SparklineBuffer` fields (memory, clients, ops, hits). On tick, compute diff (`+1,234 cmd/s`) and push to buffers.
```
feat: add sparkline buffers + delta computation to info dashboard (C2.1)

Track previous SystemStats, compute per-tick deltas, and feed
four SparklineBuffers: memory, clients, ops/sec, hits/misses.
```

#### Task C2.2 — Stats dashboard: 2×3 grid layout with sparklines + gauges
**Refs:** `implementationv1.md` 4.2-4.9, `PLAN.md` C2  
**Files:** `src/ui/info_dashboard/mod.rs`  
**What:** Split info dashboard render into 2×3 grid: Memory/Clients/Ops sparklines + Keyspace/Replication/CPU gauges. Use `ratatui::layout::Layout` with constraints.
```
feat: render stats dashboard as 2×3 grid with sparklines (C2.2)

Top row: Memory, Clients, Ops/sec sparklines.
Bottom row: Keyspace bar chart, Replication badge, CPU gauges.
```

#### Task C2.3 — Stats dashboard render tests
**Refs:** `implementationv1.md` 4.20-4.21, `PLAN.md` C2  
**Files:** `src/ui/info_dashboard/mod.rs` or `tests/info_dashboard.rs`  
**What:** Test render at various sizes, verify buffers populate after update, verify delta values.
```
test: stats dashboard render and sparkline population (C2.3)
```

#### Task C3.1 — Dedicated SlowLogPanel module
**Refs:** `implementationv1.md` 4.13-4.15, `PLAN.md` C3  
**Files:** `src/ui/slowlog.rs` (new)  
**What:** Create `SlowLogPanel` struct with `Table` rendering. Columns: id, timestamp, duration (µs), command, args.
```
feat: add dedicated SlowLogPanel with Table rendering (C3.1)

Extract slowlog display into src/ui/slowlog.rs::SlowLogPanel.
Renders as a ratatui Table with sortable columns.
```

#### Task C3.2 — SlowLogPanel: duration bar chart
**Refs:** `implementationv1.md` 4.13, `PLAN.md` C3  
**Files:** `src/ui/slowlog.rs`  
**What:** Render duration as horizontal bar relative to slowest entry in current view.
```
feat: add duration bar chart to SlowLogPanel (C3.2)

Each row shows a proportional bar relative to the slowest
entry currently visible.
```

#### Task C3.3 — SlowLogPanel: CSV export + SLOWLOG RESET
**Refs:** `implementationv1.md` 4.14-4.15, `PLAN.md` C3  
**Files:** `src/ui/slowlog.rs`, `src/redis/client.rs`, `src/config/keybindings.rs`  
**What:** Keybinding `x` exports visible entries to CSV. Keybinding `Ctrl+R` (when slowlog focused) triggers `SLOWLOG RESET`. Add `slowlog_reset()` client method.
```
feat: add SlowLog CSV export (x) and SLOWLOG RESET (Ctrl+R) (C3.3)

'x' writes visible slowlog entries to a CSV file.
Ctrl+R in slowlog panel sends SLOWLOG RESET to server.
```

#### Task C3.4 — SlowLogPanel: threshold filter
**Refs:** `implementationv1.md` 4.15, `PLAN.md` C3  
**Files:** `src/ui/slowlog.rs`  
**What:** Prompt for minimum duration (µs). Filter entries below threshold.
```
feat: add slowlog duration threshold filter (C3.4)

Prompt for min duration in µs; only entries ≥ threshold are
shown in the table.
```

#### Task C3.5 — SlowLogPanel tests
**Refs:** `implementationv1.md` 4.23-4.25, `PLAN.md` C3  
**Files:** `tests/info_dashboard.rs`, `src/ui/slowlog.rs`  
**What:** Test render with entries/empty, CSV export to tempfile, duration bars, threshold filter.
```
test: SlowLogPanel render, CSV export, duration bars, filter (C3.5)
```

**Phase C Complete.** ✅

---

### Phase D — REPL & Client Enhancements

#### Task D1.1 — `execute_raw` / `execute_pipeline` on RedisClientHandle
**Refs:** `implementationv1.md` 3.22-3.24, `PLAN.md` D1  
**Files:** `src/redis/client.rs`  
**What:** Add `execute_raw(cmd: &[String]) -> Result<redis::Value>` and `execute_pipeline(cmds: &[Vec<String>]) -> Result<Vec<redis::Value>>`.
```
feat: add execute_raw and execute_pipeline to RedisClientHandle (D1.1)

execute_raw sends a single raw command.
execute_pipeline sends multiple commands sequentially.
Both return redis::Value for flexible response handling.
```

#### Task D1.2 — Refactor REPL to use client methods
**Refs:** `implementationv1.md` 3.22-3.24, `PLAN.md` D1  
**Files:** `src/app.rs`  
**What:** Replace inline raw command dispatch in `handle_repl_command()` with calls to `client.execute_raw()` / `client.execute_pipeline()`.
```
refactor: migrate REPL raw dispatch to client methods (D1.2)

Replace inline redis::cmd dispatch in app.rs with calls to
new execute_raw / execute_pipeline on RedisClientHandle.
```

#### Task D1.3 — execute_raw / execute_pipeline tests
**Refs:** `implementationv1.md` 3.22-3.24, `PLAN.md` D1  
**Files:** `tests/repl.rs`  
**What:** Test GET/SET through execute_raw, pipeline round-trip, partial error handling.
```
test: execute_raw get/set and pipeline round-trip (D1.3)
```

#### Task D2.1 — Fuzzy REPL autocomplete: nucleo integration
**Refs:** `implementationv1.md` 3.6-3.8, `PLAN.md` D2  
**Files:** `Cargo.toml`, `src/ui/repl/autocomplete.rs`  
**What:** Add `nucleo` dependency. Create injector with Redis command names + current key list from browser.
```
feat: wire nucleo fuzzy matcher into REPL autocomplete (D2.1)

Add nucleo dependency. Build injector from 25 common Redis
commands + current key browser keys. Rank matches by score.
```

#### Task D2.2 — Fuzzy REPL autocomplete: match overlay + cycling
**Refs:** `implementationv1.md` 3.6-3.8, `PLAN.md` D2  
**Files:** `src/ui/repl/autocomplete.rs`, `src/ui/repl/mod.rs`  
**What:** Tab opens match overlay. Tab cycles matches. Esc closes. Enter accepts. Render overlay below input.
```
feat: REPL autocomplete match overlay with Tab cycling (D2.2)

Tab opens a dropdown of fuzzy matches. Tab/Shift+Tab cycles.
Enter accepts, Esc closes. Overlay renders below input line.
```

#### Task D2.3 — Fuzzy REPL autocomplete tests
**Refs:** `implementationv1.md` 3.19, `PLAN.md` D2  
**Files:** `tests/repl.rs`  
**What:** Test opens on Tab, Tab cycles, Esc closes, Enter accepts, key-name suggestions appear.
```
test: fuzzy autocomplete open, cycle, accept, close (D2.3)
```

**Phase D Complete.** ✅

---

### Phase E — Misc Features

#### Task E1.1 — MONITOR mode: CLI flag + client connection
**Refs:** `implementationv1.md` 4.5 (MONITOR), `PLAN.md` E1  
**Files:** `src/main.rs`, `src/cli.rs`, `src/redis/client.rs`  
**What:** Add `--allow-monitor` CLI flag (default false). Add `monitor()` method on `RedisClientHandle` that opens a MONITOR connection and returns a stream of raw command strings.
```
feat: add --allow-monitor flag and MONITOR connection (E1.1)

--allow-monitor defaults to false. When true, a separate
MONITOR connection streams all server commands as raw strings.
```

#### Task E1.2 — MONITOR mode: UI toggle + render
**Refs:** `implementationv1.md` 4.5 (MONITOR), `PLAN.md` E1  
**Files:** `src/ui/repl/mod.rs`, `src/app.rs`  
**What:** `Ctrl+M` in REPL tab toggles MONITOR overlay. Render as scrolling log of commands. Auto-scroll to latest.
```
feat: add MONITOR mode toggle (Ctrl+M) and render stream (E1.2)

Ctrl+M in REPL tab shows/hides MONITOR stream as a scrolling
log overlay. Auto-scrolls to latest command.
```

#### Task E1.3 — MONITOR mode tests
**Refs:** `implementationv1.md` (no specific test), `PLAN.md` E1  
**Files:** `tests/repl.rs`  
**What:** Test toggle, render stream, connection lifecycle.
```
test: MONITOR toggle, render, and connection lifecycle (E1.3)
```

#### Task E2.1 — String editor popup: full-screen overlay
**Refs:** `implementationv1.md` 2.7, `PLAN.md` E2  
**Files:** `src/ui/value_inspector/mod.rs`, `src/ui/widgets/editor_popup.rs`  
**What:** Add `edit_mode: EditorMode { BuiltIn, External }` to ValueInspector. `e` on string opens full-screen `TextAreaEditor` overlay. `Ctrl+S` saves, `Esc` cancels.
```
feat: add full-screen string editor overlay (E2.1)

'e' on a String key opens a full-screen TextAreaEditor overlay.
Ctrl+S emits WriteString, Esc cancels. Unsaved-changes warning
on Esc if modified.
```

#### Task E2.2 — `$EDITOR` integration
**Refs:** `implementationv1.md` 2.7, `PLAN.md` E2  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** If `$EDITOR` env var is set, spawn subprocess with temp file (`NamedTempFile`). On exit, read file back and emit `WriteString`. Fallback to built-in editor if `$EDITOR` unset.
```
feat: add $EDITOR env var integration for string editing (E2.2)

When $EDITOR is set, write value to NamedTempFile, spawn
subprocess, wait for exit, read back, and save. Falls back
to built-in editor if $EDITOR is unset.
```

#### Task E2.3 — String editor tests
**Refs:** `implementationv1.md` (no specific test), `PLAN.md` E2  
**Files:** `tests/value_inspector.rs`  
**What:** Test editor opens/closes, `$EDITOR` not set fallback, temp file cleanup.
```
test: string editor open/close, $EDITOR fallback, temp cleanup (E2.3)
```

**Phase E Complete.** ✅

---

### Phase F — Release Prep

#### Task F1.1 — GitHub Actions CI matrix
**Refs:** `implementationv1.md` 8.1, `PLAN.md` F1  
**Files:** `.github/workflows/ci.yml` (new)  
**What:** Matrix: ubuntu, macos, windows. Steps: checkout, rust-toolchain, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
```
ci: add GitHub Actions workflow for ubuntu/macos/windows (F1.1)

Matrix build with cargo test, clippy -D warnings, and fmt check.
```

#### Task F1.2 — `cargo audit` gate
**Refs:** `implementationv1.md` 8.4, `PLAN.md` F1  
**Files:** `.github/workflows/ci.yml`  
**What:** Add `cargo audit` step to CI. Fix or ignore any high-severity advisories.
```
ci: add cargo audit step to CI workflow (F1.2)

Zero high-severity advisories required for green build.
```

#### Task F2.1 — CHANGELOG.md
**Refs:** `implementationv1.md` 8.7, `PLAN.md` F2  
**Files:** `CHANGELOG.md` (new)  
**What:** Create CHANGELOG with sections for each milestone deliverable (M0–M8 + A1–F2).
```
docs: add CHANGELOG.md with milestone deliverables (F2.1)
```

#### Task F2.2 — Man page (`clap_mangen`)
**Refs:** `implementationv1.md` 8.8, `PLAN.md` F2  
**Files:** `Cargo.toml`, `build.rs` or `src/cli.rs`  
**What:** Add `clap_mangen` dev-dependency. Generate man page from clap CLI definition. Include in release assets.
```
docs: add man page generation via clap_mangen (F2.2)

Generate man page from clap CLI definition at build time.
```

**Phase F Complete.** ✅

---

### Additional Missing Items (Not in PLAN.md)

These are smaller gaps identified in `implementationv1.md` but not in `PLAN.md` phases. Slot them between major phases as needed.

#### Task X1 — JSON syntax highlighting in string view
**Refs:** `implementationv1.md` 2.5  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** In JSON view mode, color-code keys (yellow), strings (green), numbers (cyan), booleans (magenta) using `ratatui::text::Span`.
```
feat: add JSON syntax highlighting to string view (X1)

Color-coded spans: keys=yellow, strings=green, numbers=cyan,
booleans=magenta. Applied in JSON view mode only.
```

#### Task X2 — Hash bulk multi-select
**Refs:** `implementationv1.md` 2.11  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** `Space` toggles field selection. `Ctrl+D` deletes all selected fields. Visual indicator on selected rows.
```
feat: add hash field multi-select (Space toggle, Ctrl+D delete) (X2)

Space selects/deselects hash fields. Ctrl+D deletes all
selected fields in one HDEL call.
```

#### Task X3 — List insert before/after
**Refs:** `implementationv1.md` 2.9  
**Files:** `src/ui/value_inspector/mod.rs`, `src/redis/client.rs`  
**What:** `i` on list item opens prompt: "Insert [b]efore or [a]fter?" → value prompt → `LINSERT` command.
```
feat: add list insert before/after (X3)

'i' on a list item prompts for before/after, then value,
then emits LINSERT via client.
```

#### Task X4 — Hash field filter (`/`)
**Refs:** `implementationv1.md` 2.12  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** `/` in hash view opens filter prompt. Substring match on field names.
```
feat: add hash field name filter (X4)

'/' in hash view filters fields by substring match on field
names. Esc clears filter.
```

#### Task X5 — Stream follow mode (`f` auto-reload)
**Refs:** `implementationv1.md` 2.18  
**Files:** `src/ui/value_inspector/mod.rs`, `src/app.rs`  
**What:** `f` toggles stream follow mode. On `Event::Tick`, if follow is active and inspector holds a stream key, trigger background reload.
```
feat: add stream follow mode (f toggle) with tick-driven reload (X5)

'f' toggles follow mode. When active, each Tick event triggers
a background XADD fetch and appends new entries.
```

#### Task X6 — Stream XTRIM form
**Refs:** `implementationv1.md` 2.20  
**Files:** `src/ui/value_inspector/mod.rs`, `src/redis/client.rs`  
**What:** `Ctrl+T` in stream view opens XTRIM prompt (maxlen or minid). Execute XTRIM.
```
feat: add stream XTRIM form (Ctrl+T) (X6)

Ctrl+T opens prompt for XTRIM strategy (MAXLEN or MINID) and
value, then executes XTRIM on the stream key.
```

#### Task X7 — Search fuzzy matching (`nucleo`)
**Refs:** `implementationv1.md` 5.10  
**Files:** `src/ui/search/mod.rs`  
**What:** Replace `contains()` with `nucleo` fuzzy scorer. Rank results by match score.
```
feat: replace search substring with nucleo fuzzy ranking (X7)

Search results are ranked by nucleo fuzzy match score instead
of simple substring contains().
```

#### Task X8 — Search regex toggle
**Refs:** `implementationv1.md` 5.11  
**Files:** `src/ui/search/mod.rs`  
**What:** `Ctrl+R` in search overlay toggles regex mode. When on, treat query as regex pattern.
```
feat: add regex toggle (Ctrl+R) to search overlay (X8)

Ctrl+R switches between fuzzy and regex search modes.
Regex mode compiles the query as a regex pattern.
```

#### Task X9 — Search history persistence
**Refs:** `implementationv1.md` 5.12  
**Files:** `src/ui/search/mod.rs`  
**What:** Persist last N search queries to `~/.local/share/redis-tui/search_history`. Up/Down browses history in search input.
```
feat: persist search history and allow Up/Down browsing (X9)

Last 100 search queries saved to disk. Up/Down in search
input browses previous queries.
```

#### Task X10 — Value search (`Ctrl+Shift+F`)
**Refs:** `implementationv1.md` 5.14  
**Files:** `src/ui/value_inspector/mod.rs`  
**What:** `Ctrl+Shift+F` in value inspector opens search prompt. Substring search within current value's members (list/hash/zset).
```
feat: add value search (Ctrl+Shift+F) within inspector (X10)

Ctrl+Shift+F opens a prompt to search within the current
value's members (list/hash/zset). Matching items are
highlighted.
```

#### Task X11 — Mouse panel focus + click-to-select
**Refs:** `implementationv1.md` 7.14  
**Files:** `src/app.rs`, `src/ui/key_browser/mod.rs`, `src/ui/value_inspector/mod.rs`  
**What:** Store `Rect`s from last render. Map mouse clicks to panel focus and list index.
```
feat: add mouse click-to-focus and click-to-select (X11)

Mouse clicks on panels transfer focus. Clicks on list items
set the cursor to the clicked index.
```

#### Task X12 — `--bench` hidden flag
**Refs:** `implementationv1.md` 7.15  
**Files:** `src/cli.rs`, `src/app.rs`  
**What:** `--bench` runs 1000 renders and reports p99 render time. Assert p99 < 16ms.
```
feat: add --bench hidden flag for render performance (X12)

--bench runs 1000 render cycles on a populated app state and
reports p99 render time. Fails if p99 > 16ms.
```

#### Task X13 — `--profile-memory` hidden flag
**Refs:** `implementationv1.md` 7.16  
**Files:** `src/cli.rs`, `src/app.rs`  
**What:** `--profile-memory` loads 10K keys and reports RSS. Assert < 50MB.
```
feat: add --profile-memory hidden flag for RSS ceiling (X13)

--profile-memory loads 10K keys into the namespace tree and
reports RSS. Fails if RSS > 50MB.
```

#### Task X14 — History deduplication + max 1000
**Refs:** `implementationv1.md` 3.4  
**Files:** `src/ui/repl/history.rs`  
**What:** `push()` skips if identical to last entry. Cap input history at 1000 entries (evict oldest).
```
feat: deduplicate consecutive history entries and cap at 1000 (X14)

push() ignores duplicates of the most recent entry. Input
history is capped at 1000 entries, evicting oldest.
```

---

## Post-Commit Checklist (for every task)

After committing each task:

1. [ ] `cargo test` passes
2. [ ] `cargo clippy` clean
3. [ ] Task committed with suggested message
4. [ ] `spec/implementationv1.md` updated: change `❌ Missing` / `⚠️ Partial` → `✅ Done`
5. [ ] `spec/implementationv1.md` committed as a follow-up (or amend)

---

## Estimated Commit Count

| Phase | Tasks | Est. Commits |
|-------|-------|-------------|
| A (complete) | 4 | 4 |
| B | 4 | 4 |
| C | 5 | 5 |
| D | 3 | 3 |
| E | 3 | 3 |
| F | 2 | 2 |
| Additional (X) | 14 | 14 |
| **Total remaining** | **35** | **35** |

At ~2–3 tasks per day → **~2 weeks** to complete all remaining v1 work.

---

*End of execution plan.*
