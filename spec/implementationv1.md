# Reddish TUI — v1 Implementation Gap Analysis

> Audit date: 2026-05-27  
> Audited against: `spec/reddish-tui-spec.md` + `spec/v1-spec.md`  
> Codebase commit: current working tree

---

## Summary

| Metric | Value |
|--------|-------|
| **Overall v1 Completion** | ~75% |
| **Milestones with Gate Tests** | 8 (M0–M8) |
| **Gate Tests Passing** | ~55% of specified tests exist and pass |
| **Major Architecture Pieces** | 90% implemented |
| **UI Polish / Advanced Features** | 50% implemented |
| **Performance / Benchmarks** | 10% implemented |

### Top 5 Gaps

1. **REPL lacks raw execution and pipeline APIs** — `execute_raw` / `execute_pipeline` not on `RedisClientHandle`; raw dispatch only in `app.rs`.
2. **REPL autocomplete is shallow** — only command-name prefix matching; no key-name fuzzy cache, no argument hints, no nucleo ranking.
3. **Search is substring-only** — no fuzzy matching, no regex toggle, no search history persistence.
4. **MONITOR mode not implemented** — no `--allow-monitor` flag, no `MONITOR` connection, no render stream.
5. **Performance gates are absent** — no `--bench` flag, no p99 render-time assertions, no RSS ceiling tests.

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
| 0.13 | `test_concurrent_mutation` | ❌ Missing | Not in codebase |
| 0.14 | `test_resize_extreme_sizes` | ❌ Missing | Not in codebase |
| 0.15 | `test_cli_flag_conflicts` | ❌ Missing | Not in codebase |

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
| 2.5 | JSON syntax highlighting with ANSI spans | ❌ Missing | Pretty-printed but no color-coded keys/values |
| 2.6 | Inline string edit (`e` → TextAreaEditor) | ✅ Done | `widgets/text_area_editor.rs` |
| 2.7 | `$EDITOR` integration (temp file + spawn) | ❌ Missing | Only built-in editor exists |
| 2.8 | List: `a` RPUSH, `p` LPUSH, `e` LSET, `D` LREM | ✅ Done | |
| 2.9 | List: `i` insert before/after | ❌ Missing | Not implemented |
| 2.10 | Hash: `a` add, `e` edit, `D` delete | ✅ Done | |
| 2.11 | Hash: bulk multi-select (`Space` + `Ctrl+D`) | ❌ Missing | Only single-field delete |
| 2.12 | Hash: field filter (`/`) | ❌ Missing | Not implemented |
| 2.13 | Set: `a` SADD, `D` SREM | ✅ Done | |
| 2.14 | Set: operations panel (union/inter/diff) | ⚠️ Partial | Exists in **Key Browser** (`u`/`i`/`x`), not in Value Inspector |
| 2.15 | ZSet: `a` ZADD, `e` edit score, `D` ZREM | ✅ Done | |
| 2.16 | ZSet: range query (`q` → min/max prompt) | ✅ Done | Score filter prompts implemented |
| 2.17 | Stream: `a` XADD, `D` XDEL | ✅ Done | |
| 2.18 | Stream: follow mode (`f` auto-reload on tick) | ❌ Missing | `stream_compact` toggle exists, but no tick-driven reload |
| 2.19 | Stream: consumer group panel (`g`) | ✅ Done | `G` toggles split-pane groups table |
| 2.20 | Stream: XTRIM form (`Ctrl+T`) | ❌ Missing | |
| 2.21 | Metadata display (encoding, memory, TTL) | ✅ Done | `set_metadata` on inspector |
| 2.22 | TTL colored badges (green→yellow→red) | ✅ Done | `render()` applies color by TTL |
| 2.23 | `test_get_set_string` | ✅ Done | Integration test |
| 2.24 | `test_string_view_json_format` | ❌ Missing | |
| 2.25 | `test_string_view_base64` | ❌ Missing | |
| 2.26 | `test_string_view_hex` | ❌ Missing | |
| 2.27 | `test_list_push_pop` | ✅ Done | |
| 2.28 | `test_hash_set_del` | ✅ Done | |
| 2.29 | `test_set_add_rem` | ✅ Done | |
| 2.30 | `test_zset_add_update_remove` | ✅ Done | |
| 2.31 | `test_stream_xadd_xdel` | ❌ Missing | No integration test for XADD/XDEL round-trip |
| 2.32 | `test_input_widget_typing` | ❌ Missing | |
| 2.33 | `test_input_widget_cancel` | ❌ Missing | |
| 2.34 | `test_value_inspector_renders_all_types` | ⚠️ Partial | Individual type render tests exist, no single loop over all variants |

**M2 Verdict:** ~85% complete. All six Redis types are viewable and editable. Missing: JSON syntax highlighting, `$EDITOR` integration, hash bulk ops, stream follow/groups, and a handful of unit tests.

---

### Milestone 3 — REPL & Command Palette

| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.1 | `:` triggers REPL overlay | ✅ Done | `AppMode::ReplOverlay` with `InputWidget` |
| 3.2 | Command history with Up/Down | ✅ Done | `ui/repl/history.rs` |
| 3.3 | History persisted to disk | ✅ Done | `CommandHistory::load/save` |
| 3.4 | History deduplicates consecutive duplicates | ❌ Missing | `push()` appends blindly |
| 3.5 | History max 1000 entries | ✅ Done | `ReplWidget` enforces `max_history: 1000` on output; input history unbounded |
| 3.6 | Autocomplete: command names | ✅ Done | `ReplAutocomplete` with nucleo fuzzy matching on 100+ commands |
| 3.7 | Autocomplete: key names (fuzzy) | ✅ Done | `suggest_keys()` with key cache, wired to `update_key_cache()` |
| 3.8 | Autocomplete: argument hints / subcommands | ❌ Missing | |
| 3.9 | Pipeline parsing with `;;` | ✅ Done | `parse_pipeline()` + `tokenize_shell_like()` |
| 3.10 | Quoted token handling | ✅ Done | Single + double quote support in tokenizer |
| 3.11 | Response type-aware formatting | ⚠️ Partial | `redis_value_to_string` handles all `redis::Value` variants but no rich formatting |
| 3.12 | Raw mode toggle (`Ctrl+R`) | ✅ Done | `repl.raw_mode` |
| 3.13 | `Ctrl+L` clears output | ✅ Done | |
| 3.14 | Command palette (`Ctrl+P`) | ✅ Done | `ui/command_palette/mod.rs` |
| 3.15 | `test_history_push_and_browse` | ❌ Missing | |
| 3.16 | `test_history_persistence` | ❌ Missing | |
| 3.17 | `test_history_max_1000` | ❌ Missing | |
| 3.18 | `test_autocomplete_command_prefix` | ✅ Done | `test_repl_tab_autocomplete` |
| 3.19 | `test_autocomplete_key_arg` | ✅ Done | `test_repl_key_cache_suggestions` |
| 3.20 | `test_pipeline_parsing` | ✅ Done | |
| 3.21 | `test_quoted_token_parsing` | ✅ Done | |
| 3.22 | `test_execute_raw_get_set` | ✅ Done | `test_execute_raw_get_set` in m1 |
| 3.23 | `test_execute_pipeline` | ✅ Done | `test_execute_pipeline_round_trip` in m1 |
| 3.24 | `test_execute_pipeline_partial_error` | ❌ Missing | |
| 3.25 | `test_repl_renders_without_panic` | ✅ Done | |
| 3.26 | `test_repl_ctrl_l_clears_output` | ❌ Missing | |

**M3 Verdict:** ~70% complete. REPL is functional for daily use. Autocomplete is the biggest gap — it needs a real fuzzy engine (`nucleo`) and key-name awareness. Several integration tests for raw execution are missing.

---

### Milestone 4 — Stats Dashboard & Slow Log

| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.1 | `StatsSnapshot` from INFO ALL | ✅ Done | `SystemStats::from_info_sections` |
| 4.2 | Memory panel (used, peak, fragmentation) | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.3 | Clients panel with sparkline | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.4 | Stats panel (ops/sec, hits/misses) | ✅ Done | Sparkline + delta in 2×3 grid |
| 4.5 | Keyspace bar chart | ❌ Missing |
| 4.6 | Replication panel | ❌ Missing | Only role inferred in cluster tests |
| 4.7 | CPU gauges | ❌ Missing |
| 4.8 | Persistence flags (RDB/AOF) | ❌ Missing |
| 4.9 | Delta from previous tick | ✅ Done | `update_stats()` computes diffs |
| 4.10 | SlowLog table (id/time/duration/command) | ✅ Done | `SlowLogPanel` with `Table` widget |
| 4.11 | SlowLog scroll | ✅ Done | `SlowLogPanel::scroll` with j/k/Up/Down |
| 4.12 | SlowLog compact toggle (`Space`) | ✅ Done | `SlowLogPanel::compact_mode` |
| 4.13 | SlowLog duration bar chart | ✅ Done | Color-coded bars in table cells |
| 4.14 | SlowLog CSV export (`e`) | ✅ Done | `SlowLogPanel::export_csv()` |
| 4.15 | SlowLog threshold filter (`t`) | ✅ Done | Cycles None→1ms→10ms→100ms→1s |
| 4.16 | `test_parse_info_valid` | ✅ Done | |
| 4.17 | `test_parse_info_missing_field` | ❌ Missing | |
| 4.18 | `test_sparkline_buffer_capacity` | ✅ Done | `ui::stats::sparkline` |
| 4.19 | `test_sparkline_buffer_delta_insufficient_data` | ✅ Done | `test_delta_requires_two_samples` |
| 4.20 | `test_stats_panel_update_populates_buffers` | ✅ Done | `test_info_dashboard_update_stats_seeds_buffers` |
| 4.21 | `test_stats_panel_renders_without_panic` | ✅ Done | `test_info_dashboard_render_with_sparklines` |
| 4.22 | `test_slowlog_get` | ✅ Done | |
| 4.23 | `test_slowlog_reset` | ❌ Missing | |
| 4.24 | `test_slowlog_csv_export` | ✅ Done | `test_slowlog_panel_export_csv` |
| 4.25 | `test_slowlog_renders_duration_bars` | ✅ Done | `test_slowlog_panel_render_with_entries_does_not_panic` |

**M4 Verdict:** ~65% complete. SparklineBuffer module is done. Info dashboard still needs integration (C2) and visual widgets (gauges, bar charts, deltas). Slow log is a scrolling list without analytics.

---

### Milestone 5 — Pub/Sub & Global Search

| # | Item | Status | Notes |
|---|------|--------|-------|
| 5.1 | PubSubClient with dedicated connection | ✅ Done | Spawns `get_async_pubsub()` in task |
| 5.2 | Subscribe to channels | ✅ Done | `PubSubAction::Subscribe` |
| 5.3 | PSUBSCRIBE (pattern subscribe) | ❌ Missing | UI only supports exact channels |
| 5.4 | Message stream with timestamps | ✅ Done | `relative_time()` formatting |
| 5.5 | Auto-scroll (`f` toggle) | ❌ Missing | `scroll` exists but no follow-mode logic |
| 5.6 | Publish form | ✅ Done | `PubSubInputMode::Message` |
| 5.7 | Color-coded per-channel tags | ⚠️ Partial | Channel list styled, messages use fixed blue tag |
| 5.8 | Message cap (1000) | ✅ Done | `pop_front()` when > 1000 |
| 5.9 | GlobalSearch with SCAN backend | ✅ Done | `spawn_search_scan` drives SCAN |
| 5.10 | Fuzzy matching (`nucleo`) | ❌ Missing | Substring filter only (`contains`) |
| 5.11 | Regex toggle | ❌ Missing | |
| 5.12 | Search history persisted | ❌ Missing | |
| 5.13 | Search progress indicator | ⚠️ Partial | `[scanning...]` label exists |
| 5.14 | Value search (`Ctrl+Shift+F`) | ❌ Missing | |
| 5.15 | `test_pubsub_subscribe_receive` | ✅ Done | |
| 5.16 | `test_pubsub_psubscribe` | ❌ Missing | |
| 5.17 | `test_pubsub_message_cap` | ✅ Done | |
| 5.18 | `test_pubsub_panel_renders_without_panic` | ✅ Done | |
| 5.19 | `test_global_search_finds_matching_keys` | ❌ Missing | |
| 5.20 | `test_global_search_progress_transitions` | ❌ Missing | |
| 5.21 | `test_search_set_query_rescores` | ❌ Missing | |
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
| 6.14 | `test_ssh_tunnel_opens_and_closes` | ❌ Missing | Requires live SSH; marked ignore in spec |
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
| 7.13 | Mouse click tab switching | ✅ Done | Row 0 click maps to tabs |
| 7.14 | Mouse panel focus + list index calc | ❌ Missing | No `Rect` tracking for click-to-focus |
| 7.15 | `--bench` hidden flag (1000 renders, p99 < 16ms) | ❌ Missing | |
| 7.16 | `--profile-memory` hidden flag | ❌ Missing | |
| 7.17 | `test_all_themes_have_all_fields` | ✅ Done | |
| 7.18 | `test_theme_by_name` | ✅ Done | |
| 7.19 | `test_keymap_default_parses` | ✅ Done | |
| 7.20 | `test_keymap_emacs_parses` | ✅ Done | |
| 7.21 | `test_keymap_from_toml_override` | ✅ Done | |
| 7.22 | `test_help_overlay_renders_without_panic` | ✅ Done | |
| 7.23 | `test_cli_url_flag_parsed` | ✅ Done | |
| 7.24 | `test_cli_readonly_flag` | ✅ Done | |
| 7.25 | `test_render_performance_10k_keys` | ❌ Missing | |
| 7.26 | `test_rss_under_50mb_at_10k_keys` | ❌ Missing | |

**M7 Verdict:** ~75% complete. Themes, keymaps, help, and CLI flags are done. Mouse is partially implemented. Performance benchmarking infrastructure is entirely absent.

---

### Milestone 8 — v1.0 Release

| # | Item | Status | Notes |
|---|------|--------|-------|
| 8.1 | GitHub Actions CI matrix (ubuntu, macos, windows) | ❌ Missing | No `.github/workflows` visible |
| 8.2 | Security: no plaintext passwords in logs | ⚠️ Partial | `resolve()` not logged, but no explicit test |
| 8.3 | Security: temp files via `NamedTempFile` | ❌ Missing | `$EDITOR` integration not implemented |
| 8.4 | `cargo audit` clean | ❌ Missing | |
| 8.5 | `cargo publish --dry-run` | ❌ Missing | |
| 8.6 | README with quick-start + keybindings | ❌ Missing | |
| 8.7 | CHANGELOG | ❌ Missing | |
| 8.8 | Man page (`clap_mangen`) | ❌ Missing | |
| 8.9 | `test_cargo_audit_clean` | ❌ Missing | |
| 8.10 | `test_no_plaintext_password_in_logs` | ❌ Missing | |
| 8.11 | `test_temp_file_deleted_on_drop` | ❌ Missing | |
| 8.12 | `test_binary_opens_and_exits_on_all_platforms` | ❌ Missing | |

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
| `?` help overlay | ❌ Missing | |
| String: Raw/JSON/Base64/Hex tabs | ✅ Done | |
| String: JSON syntax highlighting | ❌ Missing | |
| String: `$EDITOR` popup | ❌ Missing | |
| List: `a` RPUSH, `p` LPUSH, `e` LSET, `D` LREM | ✅ Done | |
| List: `i` insert before/after | ❌ Missing | |
| Hash: `a` add, `e` edit, `D` delete | ✅ Done | |
| Hash: bulk multi-select | ❌ Missing | |
| Set: `a` SADD, `D` SREM | ✅ Done | |
| Set: operations panel in inspector | ❌ Missing | Only in key browser |
| ZSet: `a` ZADD, `e` edit score, `D` ZREM | ✅ Done | |
| ZSet: range query | ✅ Done | |
| Stream: `a` XADD, `D` XDEL | ✅ Done | |
| Stream: follow mode | ❌ Missing | |
| Stream: consumer group panel | ❌ Missing | |
| Stream: XTRIM | ❌ Missing | |

### 4.4 REPL

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| `:` from anywhere | ✅ Done | `ReplOverlay` mode |
| Full Redis command execution | ✅ Done | `execute_repl_command` |
| History persisted | ✅ Done | |
| Autocomplete command names | ⚠️ Partial | Static list, prefix only |
| Autocomplete key names (fuzzy) | ❌ Missing | |
| Argument hints / subcommands | ❌ Missing | |
| Pipeline `;;` | ✅ Done | |
| Type-aware response formatting | ⚠️ Partial | Basic `redis_value_to_string` |
| Raw mode (`Ctrl+R`) | ✅ Done | |
| `Ctrl+L` clear | ✅ Done | |

### 4.5 Pub/Sub

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Subscribe exact channels | ✅ Done | |
| PSUBSCRIBE patterns | ✅ Done | |
| Timestamps + channel + payload | ✅ Done | |
| Auto-scroll `f` toggle | ❌ Missing | |
| Publish form | ✅ Done | |
| Color-coded per-channel tags | ✅ Done | 8-color cycle per subscription |
| MONITOR mode | ❌ Missing | |

### 4.6 Server Info & Stats

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Real-time refresh every 1s | ✅ Done | Tick-driven `refresh_info_dashboard` |
| Memory sparkline | ❌ Missing | |
| Clients sparkline | ❌ Missing | |
| Ops/sec sparkline | ❌ Missing | |
| Hits/misses sparkline | ❌ Missing | |
| Keyspace bar chart | ❌ Missing | |
| Replication panel | ❌ Missing | |
| CPU gauges | ❌ Missing | |
| Persistence flags | ❌ Missing | |
| Delta badges (`+1,234 cmd/s`) | ❌ Missing | |

### 4.7 Slow Log Viewer

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Table: id/time/duration/command | ✅ Done | |
| `SLOWLOG RESET` (`Ctrl+R`) | ❌ Missing | |
| `SLOWLOG LEN` in header | ❌ Missing | |
| Duration bar chart | ❌ Missing | |
| Filter by minimum duration | ❌ Missing | |
| Export to CSV (`x`) | ❌ Missing | |

### 4.8 Search & Filter

| Spec Requirement | Status | Gap |
|-----------------|--------|-----|
| Global key search (`Ctrl+F`) | ✅ Done | |
| SCAN-backed with progress | ⚠️ Partial | Progress label exists; no "scanned X of estimated Y" |
| Fuzzy matching (`nucleo`) | ❌ Missing | Substring only |
| Regex toggle | ❌ Missing | |
| Search history persisted | ❌ Missing | |
| Value search (`Ctrl+Shift+F`) | ❌ Missing | |

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
| **A** | A2 — ZSet range query (`q`) | ✅ **Complete** | `zrangebyscore()` client method, `q` keybinding, two-step min/max prompt, in-memory score filtering in `render_zset`, filter display in title | Tests |
| **A** | A3 — Stream consumer groups (`g`) | ✅ **Complete** | `inspector_stream_groups` keybinding (`G` vim / `ctrl+shift+m` emacs), `InspectorAction::StreamToggleGroups`, split-pane render with groups table (name/consumers/pending/last-delivered-id), wired to `client.xgroups()`. Tests: state toggle, reset on value change, render with/without groups. | — |
| **B** | B1 — PSUBSCRIBE + channel colors | ✅ **Complete** | `PubSubKind` enum, `PSubscribe`/`PUnsubscribe` actions, Pattern input mode, 8-color cycle round-robin per subscription, color-coded message tags, `start_pubsub_subscription` accepts `PubSubKind`. Tests: tab cycle, subscribe/psubscribe color, unsubscribe/punsubscribe, color wrap, render. | — |
| **B** | B2 — PubSubClient as module | ✅ **Complete** | `src/redis/pubsub.rs` with `PubSubClient::spawn_subscription`, `PubSubKind`/`PubSubMessage` moved from UI to shared redis module, `app.rs::start_pubsub_subscription` is thin wrapper | — |
| **C** | C1 — SparklineBuffer | ✅ **Complete** | `SparklineBuffer` circular buffer (default 60 samples), `push()`, `to_vec()`, `latest()`, `delta()`. Tests: capacity, overflow, wrap, delta, negative change. | — |
| **C** | C2 — 6-panel sparkline grid | ✅ **Complete** | `InfoDashboard` has `prev_stats` + 4 `SparklineBuffer`s, `update_stats()` computes deltas, renders 2×3 grid (Memory/Clients/Ops-sec/Hit Rate sparklines + Uptime/Version text). Tests: seeding, delta, missing fields, render. | — |
| **C** | C3 — Dedicated SlowLogPanel | ✅ **Complete** | `SlowLogPanel` extracted to `src/ui/slowlog_panel/mod.rs`, `Table` widget with duration bars (color-coded: green/yellow/red), CSV export (`e`), threshold filter (`t` cycles presets), compact mode (`Space`). 13 unit tests. | — |
| **D** | D1 — `execute_raw` / `execute_pipeline` | ✅ **Complete** | `RedisClientHandle::execute_raw()` and `execute_pipeline()` added. REPL refactored to use them. Tests: raw GET/SET/DEL, pipeline round-trip. | — |
| **D** | D2 — Fuzzy REPL autocomplete | ✅ **Complete** | `ReplAutocomplete` with nucleo `fuzzy_match_greedy`, 100+ Redis commands, key cache support. Tab opens overlay, Tab/Shift+Tab cycles, Enter accepts, Esc closes. 18 tests. | — |
| **E** | E1 — MONITOR mode | ❌ **Not started** | — | No `--allow-monitor` flag, no `MONITOR` connection, no render stream |
| **E** | E2 — String editor popup + `$EDITOR` | ❌ **Not started** | `TextAreaEditor` widget exists | No full-screen overlay, no `$EDITOR` subprocess integration |
| **F** | F1 — CI & Audit | ❌ **Not started** | — | No `.github/workflows`, no `cargo audit` gate |
| **F** | F2 — Documentation | ⚠️ **Partial** | README.md updated with current features, MANUAL_TESTING.md expanded | No CHANGELOG.md, no man page (`clap_mangen`) |

#### Implementation Notes / Deviations from PLAN.md

- **A2 (ZSet range query)**: PLAN.md specified an overlay approach with `InspectorAction::ZRangeQuery`. The actual implementation uses **in-memory filtering** (`zset_score_min`/`zset_score_max` state fields on `ValueInspector`) rather than re-fetching from Redis or rendering an overlay. The prompt sets state directly; no action is emitted to `app.rs`. This avoids an extra Redis round-trip and keeps the code simpler.
- **A3 (Stream consumer groups)**: Not yet implemented. PLAN.md's `InspectorAction::ShowGroups(Vec<StreamGroup>)` action will be used when this is built.

### PLAN.md Progress Summary

- **Completed**: 2 / 15 sub-milestones (A1, A2)
- **In progress**: 0 / 15
- **Not started**: 13 / 15

### Next Recommended Sub-Milestone

**A3 — Stream consumer groups (`g`)** is the natural next step because:
1. It completes Phase A (Value Inspector Gaps)
2. The client method `xgroups()` already exists — only UI wiring is needed
3. It follows the same split-pane pattern used in the Pub/Sub tab (two-panel layout)
4. Low risk: no new client methods, no new keybindings file changes, purely additive to `value_inspector`

---

## Recommendations

### P1 — Must Have for v1

1. **Sparkline buffers + stats dashboard graphs** (M4) — The spec calls this a "real-time dashboard." Without sparklines and bar charts, it is just a text dump.
2. **REPL autocomplete depth** (M3) — At minimum, feed the current key list into autocomplete so `GET <Tab>` suggests keys.
3. **Search fuzzy ranking** (M5) — Replace `contains()` with `nucleo` or a simple fuzzy scorer; the spec explicitly names `nucleo`.
4. **Slow Log CSV export + RESET** (M4) — Low-effort, high-utility features that exist in the spec gates.
5. **Performance benchmark gate** (M7) — Add the `--bench` hidden flag and the 10K-key render test. This is the spec's quality gate.

### P2 — Should Have for v1

1. **JSON syntax highlighting in string view** (M2) — Use `ratatui::text::Span` with colored segments.
2. **Hash bulk multi-select** (M2) — `Space` to toggle field selection, `Ctrl+D` to delete selected.
3. **Stream follow mode + consumer groups** (M2) — Follow mode is a small tick-driven reload; groups need a split-pane render.
4. **Help overlay context awareness** (M7) — `?` already opens help, but it shows a static string. Wire it to `HelpContext`.
5. **Mouse panel focus + click-to-select** (M7) — Store `Rect`s from last render and map clicks.

### P3 — Nice to Have / Post-v1

1. `$EDITOR` integration (M2) — Can be deferred; the built-in editor works.
2. MONITOR mode (M4) — Gated behind `--allow-monitor` flag; niche feature.
3. SSH tunnel live integration test (M6) — Acceptable to leave `#[ignore]`.
4. CI matrix + `cargo audit` (M8) — Required for release but not for feature completeness.

---

*End of gap analysis.*
