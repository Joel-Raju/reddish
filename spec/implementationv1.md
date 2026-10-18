# Reddish TUI — v1 Implementation Gap Analysis

> Audit date: 2026-05-27  
> Re-audited: 2026-10-17 (worktree at `36adc08`)  
> Audited against: `spec/reddish-tui-spec.md` + `spec/v1-spec.md`

---

## Summary

| Metric | Value |
|--------|-------|
| **Overall v1 Completion** | ~95% |
| **Milestones with Gate Tests** | 8 (M0–M8) — all feature gates satisfied |
| **Gate Tests Passing** | ~80% of specified test rows exist and pass |
| **Major Architecture Pieces** | 100% implemented |
| **UI Polish / Advanced Features** | 90% implemented |
| **Performance / Benchmarks** | 100% implemented |

### Measured Performance (release build, 10K keys)

| Scenario | p50 | p99 | Budget |
|----------|-----|-----|--------|
| Render, all namespaces expanded | 11.1ms | 11.9ms | 16ms ✅ |
| Render, default collapsed view | 0.14ms | 0.15ms | 16ms ✅ |
| Peak RSS with all six value types loaded | — | 18.5MB | 50MB ✅ |

The fully-expanded case is the worst case: render builds one `ListItem` per
visible row, so 10K expanded keys means 10K formatted rows per frame even
though only ~24 are drawn. The default view expands nothing and is ~60x
cheaper. See `src/bench.rs`; run `reddish-tui --bench` and
`reddish-tui --profile-memory` to reproduce.

### Remaining Gaps

Every feature milestone (M0–M8) and every planned sub-milestone is
implemented. What remains is **specified test rows that do not exist yet**,
plus a handful of small spec features. Counted honestly rather than claimed
complete — see the tables above for the authoritative per-row status.

**Missing test rows (16)** — grouped by milestone:

| Milestone | Rows | Nature |
|-----------|------|--------|
| M0 | 0.13, 0.14, 0.15 | Stress tests: concurrent mutation, extreme resize, CLI flag conflicts |
| M2 | 2.31, 2.32, 2.33 | Stream XADD/XDEL round-trip; `InputWidget` typing and cancel |
| M3 | 3.15–3.17, 3.24, 3.26 | REPL history push/browse/persistence/cap; pipeline partial error; `Ctrl+L` clear |
| M4 | 4.23 | `SLOWLOG RESET` (the feature is also missing, not just the test) |
| M5 | 5.16, 5.19–5.21 | PSUBSCRIBE integration; search finds keys, progress transitions, query re-score |
| M6 | 6.14 | SSH tunnel live test — needs a real SSH daemon, correctly deferred |

Note 0.15 (CLI flag conflicts) is substantially covered by the keybinding
collision test added in this work; the remaining gap is CLI-level rather than
keymap-level.

**Missing small features (9)**

| Item | Ref | Nature |
|------|-----|---------|
| REPL argument/subcommand hints | 3.8 | `CONFIG <Tab>` suggests `GET`/`SET`/`REWRITE` |
| Pub/Sub auto-scroll follow | 5.5 | Tail the message pane as messages arrive |
| `SLOWLOG RESET` + `SLOWLOG LEN` | 4.7 | Client methods and keybinding; `LEN` in the panel header |
| Key browser sort by memory | 4.2 | `MEMORY USAGE` is per-key and lazily fetched, so this needs a design decision |
| Show/hide TTL and memory columns | 4.2 | Column toggles |
| Inline key rename in the inspector | 4.3 | `r` is rename in the key browser only |
| Set operations panel in the inspector | 4.3 | Union/intersect/diff exist in the browser only |
| Config `editor` key | 4.9 | `$EDITOR` is read from the environment but not overridable in `config.toml` |
| Config `max_value_display_bytes` | 4.9 | Parsed by neither the struct nor the renderers; large values are not truncated |

**Deviation from spec**

- `TextAreaEditor` renders inline in the value pane rather than as a full-screen
  popup. The external `$EDITOR` path is unaffected.

### Corrected Since Last Audit (2026-05-27)

The original audit predated Phases A–E. The following were reported as
❌ Missing / Not started but are **implemented and tested**:

- **A3** Stream consumer groups — keybinding, split-pane render, `client.xgroups()` wiring, 5 tests
- **B1** PSUBSCRIBE + per-subscription 8-color cycle — `PubSubKind`, pattern input mode, 6 tests
- **B2** `PubSubClient` extracted to `src/redis/pubsub.rs`
- **C1** `SparklineBuffer` — 60-sample ring buffer, 9 tests
- **C2** 2×3 sparkline grid with delta computation in `InfoDashboard`
- **C3** `SlowLogPanel` — table, duration bars, CSV export, threshold filter, compact mode, 13 tests
- **D1** `execute_raw` / `execute_pipeline` on `RedisClientHandle`; REPL refactored onto them
- **D2** Fuzzy REPL autocomplete via `nucleo` (commands + key cache), Tab cycling, overlay, 10 tests
- **E1** MONITOR mode — `--allow-monitor` flag, `client.monitor()`, `Ctrl+M` toggle, render stream
- **X14** Command history dedup + 1000-entry cap (`src/ui/repl/history.rs`)

---

## Detailed Checklist by Milestone

### Milestone 0 — Scaffolding & Terminal Lifecycle

| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.1 | Terminal raw mode + alternate screen | ✅ Done | `terminal.rs` |
| 0.2 | `restore_terminal` idempotent | ✅ Done | Safe to call multiple times |
| 0.3 | `App` struct with mode stack | ✅ Done | `app.rs` — `mode_stack: Vec<AppMode>` |
| 0.4 | Config loading with defaults | ✅ Done | `config/mod.rs` — all getters present |
| 0.5 | EventHandler with tick + input tasks | ✅ Done | `events/mod.rs` — uses `spawn_blocking` for input |
| 0.6 | `test_config_defaults` | ✅ Done | Checks `namespace_separator`, `scan_count`, `refresh_interval_ms` |
| 0.7 | `test_config_from_toml` | ✅ Done | |
| 0.8 | `test_config_invalid_toml_returns_err` | ✅ Done | |
| 0.9 | `test_restore_terminal_is_idempotent` | ⚠️ Partial | Exists but only tests `disable_raw_mode` twice |
| 0.10 | `test_event_tick_fires` | ✅ Done | |
| 0.11 | `test_app_quits_on_q` | ⚠️ Partial | Only asserts `should_quit = true` directly, no event loop test |
| 0.12 | `test_render_does_not_panic` | ✅ Done | |
| 0.13 | `test_concurrent_mutation` | ✅ Done | 8 workers x 50 mutations + interleaved renders |
| 0.14 | `test_resize_extreme_sizes` | ✅ Done | 7 sizes incl. 0x0, plus all four tabs at 3x3 |
| 0.15 | `test_cli_flag_conflicts` | ✅ Done | All 16 boolean combinations; duplicates rejected |

**M0 Verdict:** ~90% complete. Core scaffolding is solid; a few extra stress tests are missing.

---

### Milestone 1 — Connection Management & Key Browser

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1.1 | `ConnectionProfile` with TOML serde | ✅ Done | `config/connections.rs` |
| 1.2 | `PasswordRef` (Plaintext / Env / Keychain) | ✅ Done | `resolve()` implemented |
| 1.3 | `ConnectionStore` load/save/add/remove/get | ✅ Done | |
| 1.4 | `RedisClientHandle::connect` — Standalone | ✅ Done | |
| 1.5 | `RedisClientHandle::connect` — Cluster | ✅ Done | `redis::cluster::ClusterClient` |
| 1.6 | `RedisClientHandle::connect` — Sentinel | ✅ Done | `resolve_sentinel_master` implemented |
| 1.7 | 5-second timeout on all commands | ✅ Done | Macro + `tokio::time::timeout` throughout `client.rs` |
| 1.8 | `Scanner` with SCAN cursor iteration | ✅ Done | `redis/scanner.rs` — standalone + cluster per-node SCAN |
| 1.9 | Never uses KEYS command | ✅ Done | Verified by implementation inspection |
| 1.10 | `NamespaceTree` with max_keys cap | ✅ Done | `tree.rs` — `max_keys` enforced |
| 1.11 | Lazy type/ttl fetching on scan | ✅ Done | `KeyEntry` fields are `Option` |
| 1.12 | KeyBrowser prompts (new key, rename, TTL, duplicate) | ✅ Done | `key_browser/mod.rs` |
| 1.13 | StatusBar with connection state + hints | ✅ Done | `status_bar.rs` |
| 1.14 | Tab bar rendering | ✅ Done | `tab_bar.rs` |
| 1.15 | `test_connection_profile_serialization` | ✅ Done | |
| 1.16 | `test_connection_store_add_remove` | ✅ Done | |
| 1.17 | `test_scanner_scans_all_keys` | ✅ Done | Integration test against live Redis |
| 1.18 | `test_scanner_never_uses_keys_command` | ⚠️ Partial | Structural assertion only (no mock) |
| 1.19 | `test_namespace_tree_insert_and_visible_rows` | ✅ Done | |
| 1.20 | `test_namespace_tree_remove` | ✅ Done | |
| 1.21 | `test_confirm_dialog_keyboard` | ✅ Done | |
| 1.22 | `test_redis_client_ping` | ✅ Done | |
| 1.23 | `test_redis_client_delete` | ✅ Done | |
| 1.24 | `test_redis_client_ttl` | ⚠️ Partial | Exists but marked `#[ignore]` (flaky in CI) |
| 1.25 | `test_key_browser_renders_without_panic` | ✅ Done | |
| 1.26 | `test_scan_batch_updates_browser` | ✅ Done | |
| 1.27 | Connection screen (list / new / edit / delete) | ✅ Done | `connection_screen.rs` |
| 1.28 | Connection switching without restart (`Ctrl+\`) | ✅ Done | `app.rs` |

**M1 Verdict:** ~95% complete. Core connection and browser infrastructure is production-grade. A few tests need hardening.

---

### Milestone 2 — Value Inspector & Editor

| # | Item | Status | Notes |
|---|------|--------|-------|
| 2.1 | `RedisValue` enum (String/List/Hash/Set/ZSet/Stream) | ✅ Done | `redis/types.rs` |
| 2.2 | `get_value` dispatches by type | ✅ Done | `client.rs` |
| 2.3 | String view modes: Raw / JSON / Base64 / Hex | ✅ Done | `value_inspector/mod.rs` — `StringViewMode` |
| 2.4 | JSON pretty-print | ✅ Done | `serde_json::to_string_pretty` |
| 2.5 | JSON syntax highlighting with ANSI spans | ✅ Done | `highlight_json()` — keys yellow, strings green, numbers cyan, literals magenta |
| 2.6 | Inline string edit (`e` → TextAreaEditor) | ✅ Done | |
| 2.7 | `$EDITOR` integration (temp file + spawn) | ✅ Done | Suspends the TUI, NamedTempFile round-trip, saved only if changed |
| 2.8 | List: `a` RPUSH, `p` LPUSH, `e` LSET, `D` LREM | ✅ Done | |
| 2.9 | List: `i` insert before/after | ✅ Done | Two-step prompt → `LINSERT`; bound to `I` / `ctrl+shift+i` |
| 2.10 | Hash: `a` add, `e` edit, `D` delete | ✅ Done | |
| 2.11 | Hash: bulk multi-select (`Space` + `Ctrl+D`) | ✅ Done | `hash_selected` + `InspectorAction::HashDelMany` (single HDEL) |
| 2.12 | Hash: field filter (`Ctrl+f`) | ✅ Done | `hash_filter` substring match; Esc clears. Bound to `Ctrl+f` because `/` is the global search key |
| 2.13 | Set: `a` SADD, `D` SREM | ✅ Done | |
| 2.14 | Set: operations panel (union/inter/diff) | ✅ Done | Implemented in Key Browser (`u`/`i`/`x`) — inspector panel not ported |
| 2.15 | ZSet: `a` ZADD, `e` edit score, `D` ZREM | ✅ Done | |
| 2.16 | ZSet: range query (`Q` → min/max prompt) | ✅ Done | In-memory score filter (`zset_score_min/max`) |
| 2.17 | Stream: `a` XADD, `D` XDEL | ✅ Done | |
| 2.18 | Stream: follow mode (`F` auto-reload on tick) | ✅ Done | Tick-driven XRANGE from last id; `f` still toggles compact |
| 2.19 | Stream: consumer group panel (`g`) | ✅ Done | `G` toggles split-pane groups table via `client.xgroups()` |
| 2.20 | Stream: XTRIM form (`Ctrl+T`) | ✅ Done | Two-step MAXLEN/MINID prompt; strategy validated |
| 2.21 | Metadata display (encoding, memory, TTL) | ✅ Done | `set_metadata` on inspector |
| 2.22 | TTL colored badges (green→yellow→red) | ✅ Done | `render()` applies color by TTL |
| 2.23 | `test_get_set_string` | ✅ Done | Integration test |
| 2.24 | `test_string_view_json_format` | ✅ Done | `string_view_json_pretty_prints` + invalid-JSON fallback |
| 2.25 | `test_string_view_base64` | ✅ Done | `string_view_base64_encodes` |
| 2.26 | `test_string_view_hex` | ✅ Done | `string_view_hex_emits_spaced_bytes` |
| 2.27 | `test_list_push_pop` | ✅ Done | |
| 2.28 | `test_hash_set_del` | ✅ Done | |
| 2.29 | `test_set_add_rem` | ✅ Done | |
| 2.30 | `test_zset_add_update_remove` | ✅ Done | |
| 2.31 | `test_stream_xadd_xdel` | ✅ Done | Round-trip, multi-field, selective delete, XDEL idempotency |
| 2.32 | `test_input_widget_typing` | ✅ Done | Insert-at-cursor, delete, arrows, Home/End, Ctrl+U |
| 2.33 | `test_input_widget_cancel` | ✅ Done | Esc cancels without submitting; reset clears state |
| 2.34 | `test_value_inspector_renders_all_types` | ⚠️ Partial | Individual type render tests exist, no single loop over all variants |

**M2 Verdict:** ~85% complete. All six Redis types are viewable and editable. Missing: JSON syntax highlighting, `$EDITOR` integration, hash bulk ops, stream follow/groups, and a handful of unit tests.

---

### Milestone 3 — REPL & Command Palette

| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.1 | `:` triggers REPL overlay | ✅ Done | `AppMode::ReplOverlay` with `InputWidget` |
| 3.2 | Command history with Up/Down | ✅ Done | `ui/repl/history.rs` |
| 3.3 | History persisted to disk | ✅ Done | `CommandHistory::load/save` |
| 3.4 | History deduplicates consecutive duplicates | ✅ Done | `history.rs::push` skips if equal to last |
| 3.5 | History max 1000 entries | ✅ Done | `history.rs::push` evicts oldest at 1000; `ReplWidget.max_history = 1000` |
| 3.6 | Autocomplete: command names | ✅ Done | `ReplAutocomplete` with nucleo fuzzy matching on 100+ commands |
| 3.7 | Autocomplete: key names (fuzzy) | ✅ Done | `suggest_keys()` with key cache, wired to `update_key_cache()` |
| 3.8 | Autocomplete: argument hints / subcommands | ❌ Missing | |
| 3.9 | Pipeline parsing with `;;` | ✅ Done | `parse_pipeline()` + `tokenize_shell_like()` |
| 3.10 | Quoted token handling | ✅ Done | Single + double quote support in tokenizer |
| 3.11 | Response type-aware formatting | ⚠️ Partial | `redis_value_to_string` handles all `redis::Value` variants but no rich formatting |
| 3.12 | Raw mode toggle (`Ctrl+R`) | ✅ Done | `repl.raw_mode` |
| 3.13 | `Ctrl+L` clears output | ✅ Done | |
| 3.14 | Command palette (`Ctrl+P`) | ✅ Done | `ui/command_palette/mod.rs` |
| 3.15 | `test_history_push_and_browse` | ✅ Done | Order, dedup, prev/next, clamping, cursor reset |
| 3.16 | `test_history_persistence` | ✅ Done | Save/load round-trip, whitespace handling, mkdir |
| 3.17 | `test_history_max_1000` | ✅ Done | 1100 pushes cap at 1000, newest survives |
| 3.18 | `test_autocomplete_command_prefix` | ✅ Done | `test_repl_tab_autocomplete` |
| 3.19 | `test_autocomplete_key_arg` | ✅ Done | `test_repl_key_cache_suggestions` |
| 3.20 | `test_pipeline_parsing` | ✅ Done | |
| 3.21 | `test_quoted_token_parsing` | ✅ Done | |
| 3.22 | `test_execute_raw_get_set` | ✅ Done | `test_execute_raw_get_set` in m1 |
| 3.23 | `test_execute_pipeline` | ✅ Done | `test_execute_pipeline_round_trip` in m1 |
| 3.24 | `test_execute_pipeline_partial_error` | ❌ Missing | |
| 3.25 | `test_repl_renders_without_panic` | ✅ Done | |
| 3.26 | `test_repl_ctrl_l_clears_output` | ✅ Done | Clears output pane, preserves command history |

**M3 Verdict:** ~70% complete. REPL is functional for daily use. Autocomplete is the biggest gap — it needs a real fuzzy engine (`nucleo`) and key-name awareness. Several integration tests for raw execution are missing.

---

### Milestone 4 — Stats Dashboard & Slow Log

| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.1 | `StatsSnapshot` from INFO ALL | ✅ Done | `SystemStats::from_info_sections` |
| 4.2 | Memory panel (used, peak, fragmentation) | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.3 | Clients panel with sparkline | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.4 | Stats panel (ops/sec, hits/misses) | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.5 | Keyspace bar chart | ✅ Done | One proportional bar per dbN + total |
| 4.6 | Replication panel | ✅ Done | Role, replicas, link state, offset |
| 4.7 | CPU gauges | ✅ Done | sys+usr side by side, green/yellow/red by load |
| 4.8 | Persistence flags (RDB/AOF) | ✅ Done | RDB bgsave status, AOF on/off, rewrite, pending changes |
| 4.9 | Delta from previous tick | ✅ Done | `update_stats()` computes diffs, `delta_label()` renders `(+N)` |
| 4.10 | SlowLog table (id/time/duration/command) | ✅ Done | `SlowLogPanel` with `Table` widget |
| 4.11 | SlowLog scroll | ✅ Done | `SlowLogPanel::handle_event` with j/k/Up/Down |
| 4.12 | SlowLog compact toggle (`Space`) | ✅ Done | `SlowLogPanel::compact_mode` |
| 4.13 | SlowLog duration bar chart | ✅ Done | `duration_bar()` — color-coded, relative to slowest visible entry |
| 4.14 | SlowLog CSV export (`e`) | ✅ Done | `SlowLogPanel::export_csv()` |
| 4.15 | SlowLog threshold filter (`t`) | ✅ Done | Cycles None→1ms→10ms→100ms→1s |
| 4.16 | `test_parse_info_valid` | ✅ Done | |
| 4.17 | `test_parse_info_missing_field` | ✅ Done | Section-present/field-absent plus missing-section and empty input |
| 4.18 | `test_sparkline_buffer_capacity` | ✅ Done | `ui::stats::sparkline` |
| 4.19 | `test_sparkline_buffer_delta_insufficient_data` | ✅ Done | `test_delta_requires_two_samples` |
| 4.20 | `test_stats_panel_update_populates_buffers` | ✅ Done | `test_info_dashboard_update_stats_seeds_buffers` |
| 4.21 | `test_stats_panel_renders_without_panic` | ✅ Done | `test_info_dashboard_render_with_sparklines` |
| 4.22 | `test_slowlog_get` | ✅ Done | |
| 4.23 | `test_slowlog_reset` | ✅ Done | RESET empties the log; idempotent; `SLOWLOG LEN` reported |
| 4.24 | `test_slowlog_csv_export` | ✅ Done | `test_slowlog_panel_export_csv` |
| 4.25 | `test_slowlog_renders_duration_bars` | ✅ Done | `test_slowlog_panel_render_with_entries_does_not_panic` |

**M4 Verdict:** ~65% complete. SparklineBuffer module is done. Info dashboard still needs integration (C2) and visual widgets (gauges, bar charts, deltas). Slow log is a scrolling list without analytics.

---

### Milestone 5 — Pub/Sub & Global Search

| # | Item | Status | Notes |
|---|------|--------|-------|
| 5.1 | PubSubClient with dedicated connection | ✅ Done | Spawns `get_async_pubsub()` in task |
| 5.2 | Subscribe to channels | ✅ Done | `PubSubAction::Subscribe` |
| 5.3 | PSUBSCRIBE (pattern subscribe) | ✅ Done | `PubSubKind::Pattern` + `PSubscribe`/`PUnsubscribe` actions, pattern input mode |
| 5.4 | Message stream with timestamps | ✅ Done | `relative_time()` formatting |
| 5.5 | Auto-scroll (`f` toggle) | ❌ Missing | `scroll` exists but no follow-mode logic |
| 5.6 | Publish form | ✅ Done | `PubSubInputMode::Message` |
| 5.7 | Color-coded per-channel tags | ✅ Done | 8-color round-robin per subscription; message tags resolve to subscription color |
| 5.8 | Message cap (1000) | ✅ Done | `pop_front()` when > 1000 |
| 5.9 | GlobalSearch with SCAN backend | ✅ Done | `spawn_search_scan` drives SCAN |
| 5.10 | Fuzzy matching (`nucleo`) | ✅ Done | Per-atom subsequence scoring, summed and rank-sorted |
| 5.11 | Regex toggle | ✅ Done | `Ctrl+R` / `SearchMode::Regex`, parse errors surfaced in title |
| 5.12 | Search history persisted | ✅ Done | `Alt+Up`/`Alt+Down` recall; 100 queries to `search_history` |
| 5.13 | Search progress indicator | ✅ Done | Title shows `[scanning… N candidates]` |
| 5.14 | Value search (`Ctrl+Shift+F`) | ✅ Done | Filters members of list/hash/set/zset; second press clears |
| 5.15 | `test_pubsub_subscribe_receive` | ✅ Done | |
| 5.16 | `test_pubsub_psubscribe` | ✅ Done | Pattern receives matches, ignores non-matching channels |
| 5.17 | `test_pubsub_message_cap` | ✅ Done | |
| 5.18 | `test_pubsub_panel_renders_without_panic` | ✅ Done | |
| 5.19 | `test_global_search_finds_matching_keys` | ✅ Done | Live keys, ranking, regex mode, glob superset check |
| 5.20 | `test_global_search_progress_transitions` | ✅ Done | Idle -> scanning -> done; empty batch is the sentinel |
| 5.21 | `test_search_set_query_rescores` | ✅ Done | Re-query re-ranks instead of accumulating |
| 5.22 | `test_search_overlay_renders_without_panic` | ✅ Done | |

**M5 Verdict:** ~75% complete. Pub/Sub is usable for channel subscribe/publish. Search works but is a simple substring scan rather than a fuzzy ranked experience.

---

### Milestone 6 — Cluster, Sentinel, TLS & Safety

| # | Item | Status | Notes |
|---|------|--------|-------|
| 6.1 | Cluster connection mode | ✅ Done | `ConnectionMode::Cluster` |
| 6.2 | Sentinel connection mode | ✅ Done | `ConnectionMode::Sentinel` + master resolution |
| 6.3 | TLS config + `rediss://` URLs | ✅ Done | `TlsConfig` + scheme selection in `build_redis_url` |
| 6.4 | SSH tunnel config + spawn | ✅ Done | `SshTunnelConfig` + `SshTunnel::open` |
| 6.5 | `CLUSTER NODES` parsing | ✅ Done | `parse_cluster_nodes` in `client.rs` |
| 6.6 | Per-node SCAN in cluster | ✅ Done | `Scanner::next_batch_cluster` |
| 6.7 | Reconnect with exponential backoff | ✅ Done | `backoff_sequence()` + jitter |
| 6.8 | Read-only mode (`--readonly`) | ✅ Done | `App::readonly` blocks mutating REPL + inspector |
| 6.9 | Danger policy (FLUSHDB/FLUSHALL/KEYS *) | ✅ Done | `is_dangerous_repl_command` + readonly mutating list |
| 6.10 | Connection screen form | ✅ Done | `connection_screen.rs` |
| 6.11 | `test_standalone_connect` | ✅ Done | |
| 6.12 | `test_password_ref_env` | ✅ Done | |
| 6.13 | `test_parse_cluster_nodes` | ✅ Done | |
| 6.14 | `test_ssh_tunnel_opens_and_closes` | ⚠️ Deferred | Needs a live SSH daemon; correctly not written |
| 6.15 | `test_tls_config_roundtrip` | ✅ Done | |
| 6.16 | `test_reconnect_backoff_sequence` | ✅ Done | |
| 6.17 | `test_readonly_mode_blocks_writes` | ✅ Done | |
| 6.18 | `test_connection_screen_renders_without_panic` | ✅ Done | |

**M6 Verdict:** ~90% complete. All production topology modes are implemented and tested. SSH tunnel integration test is the only missing gate (expected, as it requires a live SSH daemon).

---

### Milestone 7 — Polish, Themes, Mouse & Performance

| # | Item | Status | Notes |
|---|------|--------|-------|
| 7.1 | Theme struct with all color fields | ✅ Done | `theme.rs` |
| 7.2 | 5 built-in themes (default, dracula, nord, solarized, gruvbox) | ✅ Done | `THEMES` static |
| 7.3 | Theme lookup by name | ✅ Done | `Theme::by_name` |
| 7.4 | Vim preset keymap | ✅ Done | `Keymap::vim()` |
| 7.5 | Emacs preset keymap | ✅ Done | `Keymap::emacs()` |
| 7.6 | TOML keybinding override | ✅ Done | `serde::Deserialize` on `Keymap` |
| 7.7 | Help overlay with context variants | ✅ Done | `help.rs` with 6 contexts |
| 7.8 | CLI `--url` parsing | ✅ Done | Tested in `m7_polish.rs` |
| 7.9 | CLI `--readonly` parsing | ✅ Done | |
| 7.10 | CLI `--profile` / `--log-level` / `--theme` | ✅ Done | |
| 7.11 | Mouse capture enabled | ✅ Done | `init_terminal` calls `EnableMouseCapture` |
| 7.12 | Mouse scroll in key browser | ✅ Done | `handle_mouse_event` |
| 7.13 | Mouse click tab switching | ✅ Done | Hit regions derived from the rendered tab labels |
| 7.14 | Mouse panel focus + list index calc | ✅ Done | Geometry captured during render; click maps to a row via the list scroll offset |
| 7.15 | `--bench` hidden flag (1000 renders, p99 < 16ms) | ✅ Done | `src/bench.rs`; p99 13.5ms fully expanded, 0.22ms collapsed |
| 7.16 | `--profile-memory` hidden flag | ✅ Done | Peak RSS 22.7MB at 10K keys, budget 50MB |
| 7.17 | `test_all_themes_have_all_fields` | ✅ Done | |
| 7.18 | `test_theme_by_name` | ✅ Done | |
| 7.19 | `test_keymap_default_parses` | ✅ Done | |
| 7.20 | `test_keymap_emacs_parses` | ✅ Done | |
| 7.21 | `test_keymap_from_toml_override` | ✅ Done | |
| 7.22 | `test_help_overlay_renders_without_panic` | ✅ Done | |
| 7.23 | `test_cli_url_flag_parsed` | ✅ Done | |
| 7.24 | `test_cli_readonly_flag` | ✅ Done | |
| 7.25 | `test_render_performance_10k_keys` | ✅ Done | `bench::tests` render every tab at 10K keys + extreme sizes |
| 7.26 | `test_rss_under_50mb_at_10k_keys` | ✅ Done | `--profile-memory` gate; `report_rss` budget logic unit-tested |

**M7 Verdict:** ~75% complete. Themes, keymaps, help, and CLI flags are done. Mouse is partially implemented. Performance benchmarking infrastructure is entirely absent.

---

### Milestone 8 — v1.0 Release

| # | Item | Status | Notes |
|---|------|--------|-------|
| 8.1 | GitHub Actions CI matrix (ubuntu, macos, windows) | ✅ Done | `.github/workflows/ci.yml`: lint, unit, integration (3 OS), perf, security, build |
| 8.2 | Security: no plaintext passwords in logs | ✅ Done | Hand-written `Debug` for `PasswordRef`; `redact_url()` for URLs |
| 8.3 | Security: temp files via `NamedTempFile` | ✅ Done | `edit_in_external_editor` uses NamedTempFile; removed on drop |
| 8.4 | `cargo audit` clean | ✅ Done | 0 vulnerabilities; 5 documented warnings in `audit.toml` |
| 8.5 | `cargo publish --dry-run` | ✅ Done | `cargo package` clean; metadata + LICENSE added |
| 8.6 | README with quick-start + keybindings | ✅ Done | Quick start, usage, full vim/emacs tables, `$EDITOR` |
| 8.7 | CHANGELOG | ✅ Done | `CHANGELOG.md` follows Keep a Changelog |
| 8.8 | Man page (`clap_mangen`) | ✅ Done | `man/redis-tui.1`; generated from `cli::Cli`, staleness-tested |
| 8.9 | `test_cargo_audit_clean` | ✅ Done | Verified locally; enforced by the `security` CI job |
| 8.10 | `test_no_plaintext_password_in_logs` | ✅ Done | 7 `secret_tests` + 8 `redaction_tests` |
| 8.11 | `test_temp_file_deleted_on_drop` | ✅ Done | `temp_file_is_removed_after_editing` records the path and asserts it is gone |
| 8.12 | `test_binary_opens_and_exits_on_all_platforms` | ✅ Done | `tests/binary_smoke.rs`, 9 tests driving the real executable |

**M8 Verdict:** ~20% complete. Release engineering (CI, docs, security audit) has not been started.

---

## Feature-Specific Deep Dives

### 4.1 Connection Management

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| `~/.config/redis-tui/connections.toml` | ✅ Done | |
| Password env/keychain refs | ✅ Done | |
| Quick-connect bar (`redis://` URL paste) | ⚠️ Partial | Text label exists, no actual URL parsing + connect |
| Connection status bar with latency | ⚠️ Partial | State shown; latency updated on reconnect only, not periodic ping |
| `Ctrl+R` force reconnect | ✅ Done | |

### 4.2 Key Browser

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Lazy-loaded namespace tree | ✅ Done | |
| SCAN never KEYS | ✅ Done | |
| Type badges (`[S]`, `[H]`, etc.) | ✅ Done | |
| TTL badges with color gradient | ✅ Done | |
| Sort: alpha / type / TTL | ✅ Done | |
| Sort by memory usage | ❌ Missing | |
| Show/hide TTL & memory columns | ❌ Missing | |
| Multi-select (`Space`, `Ctrl+A`) | ✅ Done | |
| Breadcrumb trail | ✅ Done | |
| `Backspace` go up / `g` root | ✅ Done | |
| Filter (`/`) | ⚠️ Partial | Global search overlay exists; in-browser filter not implemented |

### 4.3 Value Inspector

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Key name editable inline (`r`) | ❌ Missing | `r` is rename in key browser |
| TTL inline edit (`t`) | ✅ Done | |
| Encoding badge | ✅ Done | |
| Memory usage | ✅ Done | |
| `e` edit / `Esc` cancel / `Ctrl+S` save | ✅ Done | |
| `y` yank as JSON | ⚠️ Partial | Copies raw value; not serialized JSON for complex types |
| `?` help overlay | ✅ Done | Generated from the active keymap, not hardcoded |
| String: Raw/JSON/Base64/Hex tabs | ✅ Done | |
| String: JSON syntax highlighting | ✅ Done | Per-token spans; invalid JSON falls back to plain text |
| String: `$EDITOR` popup | ✅ Done | Suspends the TUI around the subprocess |
| List: `a` RPUSH, `p` LPUSH, `e` LSET, `D` LREM | ✅ Done | |
| List: `i` insert before/after (`I`) | ✅ Done | `LINSERT`; missing pivot reported instead of silently ignored |
| Hash: `a` add, `e` edit, `D` delete | ✅ Done | `D` deletes the whole selection when one is active |
| Hash: bulk multi-select | ✅ Done | `Space` toggles, `Ctrl+A` selects visible, `Ctrl+D` deletes in one HDEL |
| Hash: field filter (`Ctrl+f`) | ✅ Done | Case-insensitive substring; empty/Esc clears |
| Set: `a` SADD, `D` SREM | ✅ Done | |
| Set: operations panel in inspector | ❌ Missing | Only in key browser |
| ZSet: `a` ZADD, `e` edit score, `D` ZREM | ✅ Done | |
| ZSet: range query (`Q`) | ✅ Done | |
| Stream: `a` XADD, `D` XDEL | ✅ Done | |
| Stream: follow mode | ✅ Done | `F` toggles; tick reload throttled to ~1s; buffer capped at 5000 |
| Stream: consumer group panel | ✅ Done | Rendered beside the entries; toggled with `G` |
| Stream: XTRIM | ✅ Done | `Ctrl+T` two-step MAXLEN/MINID prompt |

### 4.4 REPL

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| `:` from anywhere | ✅ Done | `ReplOverlay` mode |
| Full Redis command execution | ✅ Done | `execute_raw` on `RedisClientHandle` |
| History persisted | ✅ Done | |
| Autocomplete command names | ✅ Done | `nucleo` fuzzy scoring over 100+ commands |
| Autocomplete key names (fuzzy) | ✅ Done | `suggest_keys()` fed by `update_key_cache()` |
| Argument hints / subcommands | ❌ Missing | |
| Pipeline `;;` | ✅ Done | |
| Type-aware response formatting | ⚠️ Partial | `redis_value_to_string` covers all `redis::Value` variants; no nested indent |
| Raw mode (`Ctrl+R`) | ✅ Done | |
| `Ctrl+L` clear | ✅ Done | |
| MONITOR (`Ctrl+M`) | ✅ Done | Gated behind `--allow-monitor`; `Ctrl+R` reclaims the raw-mode binding |

### 4.5 Pub/Sub

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Subscribe exact channels | ✅ Done | |
| PSUBSCRIBE patterns | ✅ Done | |
| Timestamps + channel + payload | ✅ Done | |
| Auto-scroll `f` toggle | ❌ Missing | |
| Publish form | ✅ Done | |
| Color-coded per-channel tags | ✅ Done | 8-color cycle per subscription |
| MONITOR mode | ✅ Done | Separate connection, render stream in REPL |

### 4.6 Server Info & Stats

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Real-time refresh every 1s | ✅ Done | Tick-driven `refresh_info_dashboard` (every 4th tick) |
| Memory sparkline | ✅ Done | |
| Clients sparkline | ✅ Done | |
| Ops/sec sparkline | ✅ Done | |
| Hits/misses sparkline | ✅ Done | Rendered as hit-rate ratio + delta |
| Keyspace bar chart | ✅ Done | Per-db proportional bars + total |
| Replication panel | ✅ Done | Shares a cell with CPU gauges and persistence flags |
| CPU gauges | ✅ Done | Two compact gauges on one line |
| Persistence flags | ✅ Done | RDB/AOF status, rewrite in progress, pending changes |
| Delta badges (`+1,234 cmd/s`) | ✅ Done | |

### 4.7 Slow Log Viewer

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Table: id/time/duration/command | ✅ Done | |
| `SLOWLOG RESET` (`R`) | ✅ Done | `slowlog_reset` + `InfoAction::ResetSlowLog` |
| `SLOWLOG LEN` in header | ✅ Done | `server_len` shown as `[N on server]` |
| Duration bar chart | ✅ Done | `SlowLogPanel::duration_bar`, relative to the longest entry |
| Filter by minimum duration | ✅ Done | `threshold_us`, cycled with `T` |
| Export to CSV (`E`) | ✅ Done | `SlowLogPanel::export_csv` |

### 4.8 Search & Filter

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Global key search (`Ctrl+F`) | ✅ Done | |
| SCAN-backed with progress | ✅ Done | Title shows live candidate count while scanning |
| Fuzzy matching (`nucleo`) | ✅ Done | Per-atom subsequence scoring via `nucleo::Matcher` |
| Regex toggle | ✅ Done | `Ctrl+R` toggles `SearchMode::Fuzzy` / `SearchMode::Regex` |
| Search history persisted | ✅ Done | `Alt+Up`/`Alt+Down`, persisted most-recent-first |
| Value search (`Ctrl+Shift+F`) | ✅ Done | Filters members; hashes match on field name *or* value |

### 4.9 Keymap & Configuration

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| `config.toml` with defaults | ✅ Done | |
| `namespace_separator` | ✅ Done | |
| `scan_count` | ✅ Done | |
| `max_keys_in_memory` | ✅ Done | |
| `refresh_interval_ms` | ✅ Done | |
| `color_scheme` | ⚠️ Partial | Parsed but not dynamically applied to all widgets |
| `mouse_enabled` | ✅ Done | |
| `confirm_deletes` | ⚠️ Partial | Field exists but confirmation logic is hardcoded |
| `editor` | ❌ Missing | `$EDITOR` not used |
| `max_value_display_bytes` | ❌ Missing | |
| Vim/Emacs presets | ✅ Done | |
| TOML keybinding override | ✅ Done | |

---

## Cross-Reference: PLAN.md Phases vs. Current State

The `PLAN.md` defined 6 phases (A–F) with 15 sub-milestones. Below is the current status of each.

| Phase | Milestone | Status | What's Done | What's Missing |
|-------|-----------|--------|-------------|----------------|
| **A** | A1 — Set ops (`u`/`i`/`x`) | ✅ **Complete** | `SetOpType` enum, `set_store()` client method, two-step prompt in key_browser, keybindings, `BrowserAction::SetOp` wired in `app.rs` | Tests (prompt flow, integration) |
| **A** | A2 — ZSet range query (`Q`) | ✅ **Complete** | `zrangebyscore()` client method, `Q` keybinding, two-step min/max prompt, in-memory score filtering in `render_zset`, filter display in title | Key was `q`; moved to `Q` because `q` quits from anywhere |
| **A** | A3 — Stream consumer groups (`g`) | ✅ **Complete** | `inspector_stream_groups` keybinding (`G` / `ctrl+shift+m`), `InspectorAction::StreamToggleGroups`, split-pane render with groups table, `client.xgroups()` wiring, 5 unit tests | Keybinding is `G` not `g` (`g` taken by `inspector_goto_end`) |
| **B** | B1 — PSUBSCRIBE + channel colors | ✅ **Complete** | `PubSubKind` enum, `PSubscribe`/`PUnsubscribe` actions, Pattern input mode, 8-color cycle round-robin per subscription, color-coded message tags, 6 unit tests | — |
| **B** | B2 — PubSubClient as module | ✅ **Complete** | `src/redis/pubsub.rs` with `PubSubClient::spawn_subscription`, `PubSubKind`/`PubSubMessage` moved from UI to shared redis module | Only one active subscription stream supported at a time |
| **C** | C1 — SparklineBuffer | ✅ **Complete** | `SparklineBuffer` circular buffer (default 60 samples), `push()`, `to_vec()`, `latest()`, `delta()`. 9 tests | — |
| **C** | C2 — 6-panel sparkline grid | ✅ **Complete** | `InfoDashboard` has `prev_stats` + 4 `SparklineBuffer`s, `update_stats()` computes deltas, renders 2×3 grid | — |
| **C** | C3 — Dedicated SlowLogPanel | ✅ **Complete** | `SlowLogPanel` in `src/ui/slowlog_panel/mod.rs`, `Table` widget with duration bars, CSV export (`e`), threshold filter (`t`), compact mode (`Space`). 13 unit tests | `SLOWLOG RESET` not wired |
| **D** | D1 — `execute_raw` / `execute_pipeline` | ✅ **Complete** | Both on `RedisClientHandle`; REPL refactored onto them via `execute_repl_stage`. Tests: raw GET/SET/DEL, pipeline round-trip | — |
| **D** | D2 — Fuzzy REPL autocomplete | ✅ **Complete** | `ReplAutocomplete` with nucleo `fuzzy_match_greedy`, 100+ Redis commands, key cache, Tab cycling, Enter accepts, Esc closes. 10 tests | Subcommand/argument hints |
| **E** | E1 — MONITOR mode | ✅ **Complete** | `--allow-monitor` flag, `client.monitor()` returns unbounded receiver, `Ctrl+M` toggles overlay, `drain_monitor_messages()` on tick, 1000-line cap. 3 tests | Unavailable in cluster mode (returns `None`) |
| **E** | E2 — String editor + `$EDITOR` | ✅ **Complete** | `edit_in_external_editor` + TUI suspend/resume; built-in editor remains the fallback | The built-in editor is not a separate full-screen popup |
| **F** | F1 — CI & Audit | ✅ **Complete** | 6-job matrix; `cargo audit` clean after bumping crossbeam-epoch | — |
| **F** | F2 — Documentation | ✅ **Complete** | README quick-start + keybindings, MANUAL_TESTING.md, CHANGELOG.md, `man/redis-tui.1` | — |

#### Implementation Notes / Deviations from PLAN.md

- **A2 (ZSet range query)**: PLAN.md specified an overlay approach with `InspectorAction::ZRangeQuery`. The actual implementation uses **in-memory filtering** (`zset_score_min`/`zset_score_max` state fields on `ValueInspector`) rather than re-fetching from Redis or rendering an overlay. The prompt sets state directly; no action is emitted to `app.rs`. This avoids an extra Redis round-trip and keeps the code simpler. The key was moved from `q` to `Q` because `app.rs` checks `quit` before dispatching to the tab, so a `q`-bound inspector action was unreachable.
- **A3 (Stream groups)**: Bound to `G` (vim) / `ctrl+shift+m` (emacs) rather than `g`, because `g` was already bound to `inspector_goto_end`.
- **E1 (MONITOR)**: Bound to `Ctrl+M` as planned. `Ctrl+R` is taken by REPL raw mode, so the `--allow-monitor` gate is the only protection against accidental MONITOR connections.

### PLAN.md Progress Summary

- **Completed**: 24 / 24 tracked sub-milestones (A1–A3, B1–B2, C1–C3, D1–D2, E1–E2, F1–F2, X1–X14)
- **In progress**: 0
- **Not started**: none

### Next Recommended Sub-Milestone

**The 16 missing test rows** (see Remaining Gaps above). They are the highest
value remaining work because each pins behaviour that currently has no
regression cover, and several exercise code paths that turned out to hold real
bugs when first tested — the FLUSHDB interference and the unreachable key
bindings were both found this way.

---

## Recommendations

### P1 — Must Have for v1

All milestone gates (M0–M8) are now satisfied. The remaining items are the two
optional features below plus the residual M8 test rows.

### P2 — Should Have for v1

1. **The 16 missing test rows listed above** — highest value, since each pins
   behaviour that currently has no regression cover.
2. **`SLOWLOG RESET` + `SLOWLOG LEN`** (4.7) — the only missing slowlog feature.
3. **Config `editor` and `max_value_display_bytes`** (4.9) — both are documented
   config keys that are silently ignored today.
4. **Full-screen editor popup** (E2) — `TextAreaEditor` renders inline in the value pane; the only remaining deviation from the spec.

### P3 — Nice to Have / Post-v1

1. **REPL subcommand/argument hints** (M3 3.8) — `CONFIG <Tab>` suggests `GET`, `SET`, `REWRITE`.
2. **Pub/Sub auto-scroll `f` toggle** (M5 5.5) — Follow-tail behavior for the message pane.
3. **Key browser memory sort and column toggles** (4.2) — needs a decision on lazily-fetched `MEMORY USAGE`.
4. **Inspector-side set operations, inline rename** (4.3) — both exist in the key browser.
5. **SSH tunnel live integration test** (M6) — Correctly deferred; needs a real SSH daemon.
6. **Nested REPL response indentation** (M3 3.11) — `redis_value_to_string` is flat; nested arrays would read better indented.

---

*End of gap analysis.*
