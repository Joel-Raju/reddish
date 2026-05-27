# Test Migration Guide

> Migrate from milestone-based test files (`m0_*.rs`, `m1_*.rs`, ...) to behavior/component-based test organization.

---

## 1. The Problem

Current structure organizes tests by **when** they were written (milestones), not **what** they test:

```
tests/
  m0_scaffolding.rs        ← config, terminal, events
  m1_connection_key_browser.rs  ← profiles, scan, tree, client ping/delete
  m2_value_inspector.rs    ← string/list/hash/set/zset/stream round-trips
  m3_repl_command_palette.rs   ← REPL, history, palette
  m4_info_slowlog.rs       ← INFO parsing, slowlog
  m5_pubsub_search.rs      ← pubsub, search
  m6_cluster_sentinel_tls.rs   ← cluster, sentinel, TLS, SSH
  m7_polish.rs             ← themes, keymaps, CLI flags
```

### Pain Points

| Pain Point | Example |
|-----------|---------|
| **Hunting** | A `NamespaceTree` bug requires checking `m0_scaffolding.rs` + `m1_connection_key_browser.rs` |
| **Refactoring friction** | Renaming `tree.rs` means updating tests scattered across milestone files |
| **No clear ownership** | New contributors can't tell where to add a test for hash field editing |
| **Temporal irrelevance** | Milestones are meaningless after v1; tests should describe behavior |
| **Slow feedback** | Running `m2_value_inspector.rs` spins up Redis even for pure render tests |

---

## 2. Target Architecture

Organize tests by **component** and **behavior**.

### 2.1 Unit Tests: In-Source `#[cfg(test)]`

Pure logic tests live in the same file as the code they test.

```rust
// src/ui/key_browser/tree.rs
impl NamespaceTree {
    pub fn insert(&mut self, key: KeyEntry) { ... }
    pub fn visible_rows(&self) -> Vec<TreeRow> { ... }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_creates_nested_namespaces() {
        let mut tree = NamespaceTree::new(':');
        tree.insert(key_entry("user:session:abc"));
        assert_eq!(tree.total_keys(), 1);
        let rows = tree.visible_rows();
        assert!(rows.iter().any(|r| r.label == "user" && r.is_namespace));
    }

    #[test]
    fn visible_rows_respects_expanded_state() {
        let mut tree = NamespaceTree::new(':');
        tree.insert(key_entry("user:session:abc"));
        tree.insert(key_entry("user:session:def"));
        tree.expand(&["user", "session"]);

        let rows = tree.visible_rows();
        assert_eq!(rows.len(), 4); // user(ns) → session(ns) → abc(key) → def(key)

        tree.collapse(&["user", "session"]);
        let rows = tree.visible_rows();
        assert_eq!(rows.len(), 2); // user(ns) → session(ns)
    }

    #[test]
    fn remove_deletes_leaf_and_prunes_empty_parents() {
        let mut tree = NamespaceTree::new(':');
        tree.insert(key_entry("user:session:abc"));
        tree.remove("user:session:abc");
        assert_eq!(tree.total_keys(), 0);
        let rows = tree.visible_rows();
        assert!(rows.is_empty());
    }
}
```

**Rule of thumb:** If the test doesn't touch Redis, disk, or the terminal, it belongs in `#[cfg(test)]`.

### 2.2 Integration Tests: By Feature

Integration tests (need Redis) live in `tests/*.rs` organized by **feature**:

```
tests/
  helpers/
    mod.rs                  # Shared utilities: redis_server(), mock_app()
    redis_server.rs         # Spawn redis-server --port 0, wait for ready, cleanup
  connection.rs             # Profile connect, TLS, cluster, sentinel, reconnect
  key_browser.rs            # SCAN integration, tree round-trip, multi-select
  value_inspector.rs        # All 6 Redis type round-trips, metadata
  repl.rs                   # Command parsing, history, pipelines, raw execution
  pubsub.rs                 # Subscribe, publish, PSUBSCRIBE, message flow
  info_dashboard.rs         # INFO refresh, slowlog integration
  search.rs                 # SCAN-backed search, fuzzy (future)
  app_integration.rs        # Tab switching, overlays, event routing end-to-end
```

### 2.3 Rendering Tests: In-Source or Snapshot

TUI rendering tests verify "does this render without panic at size X?"

```rust
// src/ui/value_inspector/mod.rs
#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, TestBackend};
    use super::*;

    fn render_to_buffer(inspector: &ValueInspector, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| inspector.render(f, f.size())).unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn renders_all_redis_types_at_80x24_without_panic() {
        for value in sample_values_all_types() {
            let mut inspector = ValueInspector::new();
            inspector.set_value("test".into(), value);
            let _buf = render_to_buffer(&inspector, 80, 24);
        }
    }

    #[test]
    fn zset_title_shows_filter_when_active() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("scores".into(), sample_zset());
        inspector.zset_score_min = Some("75".into());
        inspector.zset_score_max = Some("150".into());

        let buf = render_to_buffer(&inspector, 80, 24);
        let text = buf.content.iter().map(|c| c.symbol()).collect::<String>();
        assert!(text.contains("filter:75~150"));
    }
}
```

---

## 3. Test Mapping: Current → Target

### 3.1 `m0_scaffolding.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_config_defaults` | `src/config/mod.rs` → `#[cfg(test)]` | Unit |
| `test_config_from_toml` | `src/config/mod.rs` → `#[cfg(test)]` | Unit |
| `test_config_invalid_toml_returns_err` | `src/config/mod.rs` → `#[cfg(test)]` | Unit |
| `test_restore_terminal_is_idempotent` | `src/terminal.rs` → `#[cfg(test)]` | Unit |
| `test_event_tick_fires` | `src/events/mod.rs` → `#[cfg(test)]` | Unit |
| `test_app_quits_on_q` | `src/app.rs` → `#[cfg(test)]` | Unit |
| `test_render_does_not_panic` | `src/app.rs` → `#[cfg(test)]` | Unit |
| `test_concurrent_mutation` | `tests/app_integration.rs` | Integration |
| `test_resize_extreme_sizes` | `src/app.rs` → `#[cfg(test)]` | Unit |
| `test_cli_flag_conflicts` | `tests/app_integration.rs` | Integration |

### 3.2 `m1_connection_key_browser.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_connection_profile_serialization` | `src/config/connections.rs` → `#[cfg(test)]` | Unit |
| `test_connection_store_add_remove` | `src/config/connections.rs` → `#[cfg(test)]` | Unit |
| `test_scanner_scans_all_keys` | `tests/key_browser.rs` | Integration |
| `test_scanner_never_uses_keys_command` | `tests/key_browser.rs` | Integration |
| `test_namespace_tree_insert_and_visible_rows` | `src/ui/key_browser/tree.rs` → `#[cfg(test)]` | Unit |
| `test_namespace_tree_remove` | `src/ui/key_browser/tree.rs` → `#[cfg(test)]` | Unit |
| `test_confirm_dialog_keyboard` | `src/ui/widgets/confirm.rs` → `#[cfg(test)]` | Unit |
| `test_redis_client_ping` | `tests/connection.rs` | Integration |
| `test_redis_client_delete` | `tests/connection.rs` | Integration |
| `test_redis_client_ttl` | `tests/connection.rs` | Integration |
| `test_key_browser_renders_without_panic` | `src/ui/key_browser/mod.rs` → `#[cfg(test)]` | Unit |
| `test_scan_batch_updates_browser` | `tests/key_browser.rs` | Integration |
| `test_connection_screen_renders_without_panic` | `src/ui/connection_screen.rs` → `#[cfg(test)]` | Unit |

### 3.3 `m2_value_inspector.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_get_set_string` | `tests/value_inspector.rs` | Integration |
| `test_string_view_json_format` | `src/ui/value_inspector/mod.rs` → `#[cfg(test)]` | Unit |
| `test_string_view_base64` | `src/ui/value_inspector/mod.rs` → `#[cfg(test)]` | Unit |
| `test_string_view_hex` | `src/ui/value_inspector/mod.rs` → `#[cfg(test)]` | Unit |
| `test_list_push_pop` | `tests/value_inspector.rs` | Integration |
| `test_hash_set_del` | `tests/value_inspector.rs` | Integration |
| `test_set_add_rem` | `tests/value_inspector.rs` | Integration |
| `test_zset_add_update_remove` | `tests/value_inspector.rs` | Integration |
| `test_stream_xadd_xdel` | `tests/value_inspector.rs` | Integration |
| `test_input_widget_typing` | `src/ui/widgets/input.rs` → `#[cfg(test)]` | Unit |
| `test_input_widget_cancel` | `src/ui/widgets/input.rs` → `#[cfg(test)]` | Unit |
| `test_value_inspector_renders_all_types` | `src/ui/value_inspector/mod.rs` → `#[cfg(test)]` | Unit |

### 3.4 `m3_repl_command_palette.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_repl_command_palette` | `tests/repl.rs` | Integration |
| `test_repl_tab_autocomplete` | `src/ui/repl/autocomplete.rs` → `#[cfg(test)]` | Unit |
| `test_repl_pipeline_parsing` | `src/ui/repl/mod.rs` → `#[cfg(test)]` | Unit |
| `test_repl_quoted_tokens` | `src/ui/repl/mod.rs` → `#[cfg(test)]` | Unit |
| `test_repl_history_persistence` | `tests/repl.rs` | Integration |
| `test_repl_raw_mode_toggle` | `tests/repl.rs` | Integration |
| `test_repl_ctrl_l_clears_output` | `tests/repl.rs` | Integration |

### 3.5 `m4_info_slowlog.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_parse_info_valid` | `src/ui/info_dashboard/mod.rs` → `#[cfg(test)]` | Unit |
| `test_parse_info_missing_field` | `src/ui/info_dashboard/mod.rs` → `#[cfg(test)]` | Unit |
| `test_info_sections_present` | `tests/info_dashboard.rs` | Integration |
| `test_slowlog_get` | `tests/info_dashboard.rs` | Integration |
| `test_slowlog_parses_without_panic` | `src/ui/info_dashboard/mod.rs` → `#[cfg(test)]` | Unit |

### 3.6 `m5_pubsub_search.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_pubsub_subscribe_receive` | `tests/pubsub.rs` | Integration |
| `test_pubsub_message_cap` | `tests/pubsub.rs` | Integration |
| `test_pubsub_panel_renders_without_panic` | `src/ui/pubsub/mod.rs` → `#[cfg(test)]` | Unit |
| `test_search_overlay_renders_without_panic` | `src/ui/search/mod.rs` → `#[cfg(test)]` | Unit |
| `test_global_search_finds_matching_keys` | `tests/search.rs` | Integration |
| `test_global_search_progress_transitions` | `tests/search.rs` | Integration |

### 3.7 `m6_cluster_sentinel_tls.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_standalone_connect` | `tests/connection.rs` | Integration |
| `test_password_ref_env` | `src/config/connections.rs` → `#[cfg(test)]` | Unit |
| `test_parse_cluster_nodes` | `src/redis/client.rs` → `#[cfg(test)]` | Unit |
| `test_ssh_tunnel_opens_and_closes` | `tests/connection.rs` | Integration |
| `test_tls_config_roundtrip` | `src/config/connections.rs` → `#[cfg(test)]` | Unit |
| `test_reconnect_backoff_sequence` | `src/redis/client.rs` → `#[cfg(test)]` | Unit |
| `test_readonly_mode_blocks_writes` | `tests/app_integration.rs` | Integration |
| `test_connection_screen_renders_without_panic` | `src/ui/connection_screen.rs` → `#[cfg(test)]` | Unit |

### 3.8 `m7_polish.rs` → Dissolve

| Current Test | Target Location | Type |
|-------------|-----------------|------|
| `test_all_themes_have_all_fields` | `src/theme.rs` → `#[cfg(test)]` | Unit |
| `test_theme_by_name` | `src/theme.rs` → `#[cfg(test)]` | Unit |
| `test_keymap_default_parses` | `src/config/keybindings.rs` → `#[cfg(test)]` | Unit |
| `test_keymap_emacs_parses` | `src/config/keybindings.rs` → `#[cfg(test)]` | Unit |
| `test_keymap_from_toml_override` | `src/config/keybindings.rs` → `#[cfg(test)]` | Unit |
| `test_help_overlay_renders_without_panic` | `src/ui/widgets/help.rs` → `#[cfg(test)]` | Unit |
| `test_cli_url_flag_parsed` | `tests/app_integration.rs` | Integration |
| `test_cli_readonly_flag` | `tests/app_integration.rs` | Integration |
| `test_cli_profile_log_level_theme_flags` | `tests/app_integration.rs` | Integration |
| `test_render_performance_10k_keys` | `tests/app_integration.rs` | Integration |
| `test_rss_under_50mb_at_10k_keys` | `tests/app_integration.rs` | Integration |

---

## 4. New Directory Structure (After Migration)

```
src/
  config/
    mod.rs                  # #[cfg(test)] mod tests { config defaults, toml, validation }
    connections.rs          # #[cfg(test)] mod tests { profile serde, store add/remove, password ref }
    keybindings.rs          # #[cfg(test)] mod tests { vim/emacs/default parsing, matches(), override }
  redis/
    client.rs               # #[cfg(test)] mod tests { parse_cluster_nodes, backoff, reconnect }
    types.rs                # #[cfg(test)] mod tests { RedisValue formatting, ZSetEntry sort }
    scanner.rs              # #[cfg(test)] mod tests { (minimal — mostly integration) }
  ui/
    app_layout.rs           # #[cfg(test)] mod tests { layout splits, area calc }
    key_browser/
      mod.rs                # #[cfg(test)] mod tests { action dispatch, prompt handling, render }
      tree.rs               # #[cfg(test)] mod tests { insert, remove, visible_rows, expand/collapse }
    value_inspector/
      mod.rs                # #[cfg(test)] mod tests { string view modes, zset filter, render all types }
    info_dashboard/
      mod.rs                # #[cfg(test)] mod tests { INFO parsing, slowlog parse, render }
    pubsub/
      mod.rs                # #[cfg(test)] mod tests { render without panic }
    search/
      mod.rs                # #[cfg(test)] mod tests { render without panic }
    repl/
      mod.rs                # #[cfg(test)] mod tests { pipeline parse, tokenizer, render }
      autocomplete.rs       # #[cfg(test)] mod tests { prefix match, candidate list }
      history.rs            # #[cfg(test)] mod tests { push, browse, dedup, max entries }
    widgets/
      confirm.rs            # #[cfg(test)] mod tests { keyboard y/n/esc/enter }
      input.rs              # #[cfg(test)] mod tests { typing, backspace, cursor, submit, cancel }
      help.rs               # #[cfg(test)] mod tests { render without panic }
      text_area_editor.rs   # #[cfg(test)] mod tests { edit, cursor, save event }
  terminal.rs               # #[cfg(test)] mod tests { restore idempotent }
  events/
    mod.rs                  # #[cfg(test)] mod tests { tick fires, key event passthrough }
  app.rs                    # #[cfg(test)] mod tests { quit on q, render no panic, resize extreme }

tests/
  helpers/
    mod.rs                  # Shared: TestApp, assert_buffer_contains, sample data builders
    redis_server.rs         # RedisServerGuard: spawn, wait for ready, cleanup on drop
    mock_terminal.rs        # Helpers for TestBackend + Terminal setup
  connection.rs             # Connect standalone/cluster/sentinel, TLS, reconnect, ping, delete, TTL
  key_browser.rs            # SCAN integration, tree round-trip, multi-select, sort, refresh
  value_inspector.rs        # GET/SET round-trip for all 6 types, metadata fetch
  repl.rs                   # Raw execution, pipeline, history persistence, autocomplete integration
  pubsub.rs                 # Subscribe, publish, PSUBSCRIBE, message cap, unsubscribe
  info_dashboard.rs         # INFO refresh, slowlog fetch, sparkline update (future)
  search.rs                 # SCAN-backed search, fuzzy ranking (future), progress
  app_integration.rs        # Tab switching, overlay lifecycle, readonly enforcement, CLI flags
```

---

## 5. Naming Conventions

### 5.1 Unit Tests (in-source)

Use descriptive **behavioral** names. Prefer `snake_case` sentences.

```rust
// Good
#[test]
fn hash_field_can_be_added_and_deleted() {}

#[test]
fn zset_filter_hides_entries_outside_score_range() {}

#[test]
fn tree_remove_prunes_empty_namespace_nodes() {}

// Avoid
#[test]
fn test_m2_hash_set_del() {}          // milestone reference
#[test]
fn test_hash() {}                      // too vague
#[test]
fn hash_test_1() {}                    // numbered
```

### 5.2 Integration Tests (`tests/*.rs`)

Prefix with the component and describe the scenario:

```rust
// tests/key_browser.rs
#[tokio::test]
async fn browser_scan_populates_tree_without_keys_command() {}

#[tokio::test]
async fn browser_multi_select_includes_cursor_key_in_set_op_sources() {}

// tests/value_inspector.rs
#[tokio::test]
async fn inspector_zset_range_query_filters_visible_entries() {}

#[tokio::test]
async fn inspector_stream_xadd_creates_entry_with_auto_id() {}
```

### 5.3 Given-When-Then (optional but encouraged)

For complex behavior:

```rust
#[test]
fn given_a_hash_with_two_fields_when_hdel_removes_one_then_only_one_remains() {
    // Given
    let mut client = mock_client();
    client.hash_set("h", "f1", "v1").await.unwrap();
    client.hash_set("h", "f2", "v2").await.unwrap();

    // When
    client.hash_del("h", &["f1"]).await.unwrap();

    // Then
    let value = client.get_value("h", RedisType::Hash).await.unwrap();
    assert!(matches!(value, RedisValue::Hash(h) if h.len() == 1));
}
```

---

## 6. Migration Strategy (Phased)

Don't migrate everything at once. Use this incremental approach:

### Phase 1: Establish new rule (Week 1)

- Create `tests/helpers/mod.rs` with shared utilities
- **All new tests** go into:
  - `#[cfg(test)]` in source files (for unit tests)
  - `tests/<feature>.rs` (for integration tests)
- **No new tests in milestone files**

### Phase 2: Move tests when touching code (Ongoing)

Whenever you refactor or fix a module, move its tests:

```bash
# Example: you're fixing a bug in tree.rs
git mv tests/m1_connection_key_browser.rs tests/key_browser.rs  # only if moving ALL
cargo test --lib tree::tests  # run in-source tests
cargo test --test key_browser  # run integration tests
```

### Phase 3: Dissolve milestone files (Week by week)

As milestone files empty out, delete them:

1. `m7_polish.rs` → mostly unit tests → easiest to move first
2. `m0_scaffolding.rs` → config/terminal tests → move to source
3. `m2_value_inspector.rs` → split between source + `tests/value_inspector.rs`
4. `m1_connection_key_browser.rs` → split between source + `tests/key_browser.rs` + `tests/connection.rs`
5. `m3_repl_command_palette.rs` → split between source + `tests/repl.rs`
6. `m4_info_slowlog.rs` → split between source + `tests/info_dashboard.rs`
7. `m5_pubsub_search.rs` → split between source + `tests/pubsub.rs` + `tests/search.rs`
8. `m6_cluster_sentinel_tls.rs` → move to `tests/connection.rs`

### Phase 4: Clean up (Final)

- Delete empty milestone files
- Update `Cargo.toml` if any `[[test]]` entries reference milestone names
- Update CI to run `cargo test --lib` + `cargo test --test <feature>`
- Update `MANUAL_TESTING.md` if it references test file names

---

## 7. Test Type Decision Tree

When adding a new test, use this flow:

```
Does it need a running Redis server?
├── NO → Does it need the terminal / ratatui Frame?
│   ├── NO → #[cfg(test)] in source file (unit test)
│   └── YES → #[cfg(test)] in source file (render test with TestBackend)
└── YES → Is it testing a single component in isolation?
    ├── YES → tests/<component>.rs (integration test)
    └── NO → tests/app_integration.rs (end-to-end test)
```

### Speed Targets

| Test Type | Target Duration | Example |
|-----------|----------------|---------|
| Unit (`--lib`) | < 10ms each | Config parse, tree insert, keymap match |
| Render (`--lib`) | < 50ms each | Render at 80×24, buffer assertions |
| Integration (`--test`) | < 200ms each | Redis round-trip, SCAN batch |
| E2E (`--test app_integration`) | < 2s each | Full app lifecycle, tab switching |

Run `cargo test --lib` in pre-commit hooks (fast).
Run full suite (`cargo test`) in CI.

---

## 8. Example: Before & After

### Before (milestone-based)

```rust
// tests/m2_value_inspector.rs
#[tokio::test]
async fn test_get_set_string() {
    let server = redis_test_server().await;
    let client = RedisClientHandle::connect(&server.profile()).await.unwrap();
    client.set_string("k", "hello").await.unwrap();
    let value = client.get_value("k", RedisType::String).await.unwrap();
    assert!(matches!(value, RedisValue::String(s) if s == "hello"));
}

#[test]
fn test_string_view_json_format() {
    // Where does this go? m2 because it was built in M2?
    // But it doesn't need Redis...
}
```

### After (behavior-based)

```rust
// tests/value_inspector.rs — integration test
#[tokio::test]
async fn inspector_string_round_trip_get_and_set() {
    let server = helpers::redis_server().await;
    let client = RedisClientHandle::connect(&server.profile()).await.unwrap();
    client.set_string("k", "hello").await.unwrap();
    let value = client.get_value("k", RedisType::String).await.unwrap();
    assert!(matches!(value, RedisValue::String(s) if s == "hello"));
}
```

```rust
// src/ui/value_inspector/mod.rs — unit test
#[cfg(test)]
mod tests {
    #[test]
    fn string_json_view_pretty_prints_valid_json() {
        let mode = StringViewMode::Json;
        let rendered = mode.render(r#"{"a":1,"b":true}"#);
        assert!(rendered.contains("\"a\""));
        assert!(rendered.contains("\n")); // pretty-printed has newlines
    }
}
```

---

## 9. Acceptance Criteria

The migration is complete when:

- [ ] No `tests/m*.rs` files exist
- [ ] `cargo test --lib` runs in < 2 seconds
- [ ] `cargo test --test <feature>` runs per-component
- [ ] Every `src/**/*.rs` with logic has a `#[cfg(test)]` module
- [ ] All integration tests use `tests/helpers/redis_server.rs` (single spawn utility)
- [ ] CI runs `cargo test --lib` + `cargo test --test '*'` separately
- [ ] New contributors can find where to add a test in < 30 seconds

---

*End of migration guide.*
