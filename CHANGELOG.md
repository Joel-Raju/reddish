# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Search fuzzy ranking** — global search scores candidates with `nucleo`
  instead of a substring test. The query is split into alphanumeric atoms;
  every atom must match as a subsequence and scores are summed, so fuzzy
  matching is a strict superset of the previous behaviour.
- **Search regex mode** — `Ctrl+R` toggles between fuzzy and regex
  interpretation of the query. Parse errors are surfaced in the overlay title
  rather than silently matching nothing.
- **Search history** — the last 100 executed queries persist to disk and are
  recalled with `Alt+Up` / `Alt+Down`.
- **Search progress** — the overlay title shows the live candidate count while
  a scan is in flight.
- **`$EDITOR` integration** — when set, editing a string key suspends the TUI
  and opens the editor on the value via a `NamedTempFile`. Supports editors
  with flags (`code --wait`); the file is deleted on exit, and the value is
  saved back only if it changed. Unset or blank falls back to the built-in editor.
- **JSON syntax highlighting** in the string view: keys yellow, strings green,
  numbers cyan, `true`/`false`/`null` magenta, punctuation dimmed. Invalid
  JSON falls back to plain text.
- **Hash field filter** (`Ctrl+f`) — narrows visible fields by substring;
  `Esc` clears.
- **Hash bulk operations** — `Space` toggles a field, `Ctrl+A` selects all
  visible fields, `Ctrl+D` deletes the selection in a single `HDEL`.
- **List insert** (`I`) — two-step BEFORE/AFTER prompt issuing `LINSERT`.
  A missing pivot is reported instead of silently doing nothing.
- **Stream follow mode** (`F`) — reloads the tail on tick, fetching only
  entries newer than the last seen id.
- **Stream XTRIM** (`Ctrl+T`) — two-step MAXLEN/MINID prompt with validation.
- **Stream consumer groups** (`G`) — split-pane panel backed by `XINFO GROUPS`.
- **PSUBSCRIBE / PUNSUBSCRIBE** with an 8-colour round-robin so each
  subscription and its messages are visually distinct.
- **Value member search** (`Ctrl+Shift+F`) — filters list, hash, set and zset
  members. Hashes match on either the field name or its value.
- **Stats dashboard bottom row** — keyspace bar chart per database, replication
  role/replicas/link state/offset, CPU gauges, and RDB/AOF persistence flags.
- **MONITOR mode** behind the `--allow-monitor` flag, toggled with `Ctrl+M`.
- **Performance gates** — hidden `--bench` (1000 render cycles, p99 ≤ 16 ms)
  and `--profile-memory` (RSS ≤ 50 MB at 10K keys). Both run before terminal
  setup and exit non-zero on violation, so they are safe in CI.
- **GitHub Actions CI** — six jobs covering formatting, lints, unit tests,
  Redis-backed integration tests on ubuntu/macos/windows, the performance
  gates, `cargo audit`, and a release build.
- **Connection topologies** — standalone, Redis Cluster (with per-node SCAN),
  and Sentinel with master resolution.
- **TLS** via `rediss://` URLs and **SSH tunnels** for both.
- **REPL** with `;;` pipelines, shell-style quoting, persisted history with
  deduplication, `Ctrl+L` clear, and fuzzy command/key autocomplete.
- **Command palette** (`Ctrl+P`), **help overlay**, **confirm dialogs**.
- **Five built-in themes** (default, dracula, nord, solarized, gruvbox) with
  vim and emacs keymap presets and TOML overrides.
- **Safety policy** — `--readonly` mode, plus blocking of `FLUSHDB`/`FLUSHALL`
  and `KEYS *` in the REPL.
- **Reconnect** with exponential backoff and jitter.

### Fixed

- Four keybindings were unreachable because a higher-priority binding claimed
  the same key first: ZSet range query was bound to `q` (which quits), hash
  filter and list insert collided with global search and set-inter
  intersection, and emacs `duplicate_key` collided with `delete`. A test now
  fails on any unlisted key collision between two actions.
- `KeyBrowser::sorted_rows` deep-cloned every tree row on each frame via a
  redundant `.to_vec()`. Removing it took render p99 from 15.1 ms to 13.5 ms
  at 10K keys.
- `LINSERT` returned `-1` for a missing pivot into a `query_async::<()>`,
  discarding the signal so the insert looked successful while doing nothing.
- `--profile-memory` reported 23168 MB for a 23 MB process because sysinfo
  already returns bytes and the value was scaled a second time.
- Two integration tests called `FLUSHDB` against the shared test server, wiping
  keys other tests were using concurrently. This made
  `test_execute_raw_get_set` flaky and forced `test_redis_client_ttl` to be
  permanently `#[ignore]`d; both are now namespace-scoped and always run.
- Redis 8 replies to `SET` with the RESP3 OK frame where Redis 7 replied
  `+OK`; two tests hardcoded the older reply.
- Redis glob metacharacters in a search query (`*`, `?`, `[`) altered the
  server-side `MATCH` pattern. Splitting the query on non-alphanumerics
  neutralises them.
- A stale test asserted Pub/Sub had a two-state Tab cycle after `Pattern` mode
  was added.
- Stream follow mode now caps its buffer at 5000 entries and adjusts the
  cursor when the oldest are dropped, so a long session cannot grow unbounded.

### Changed

- `cargo audit` reports zero vulnerabilities. `crossbeam-epoch` was bumped from
  0.9.18 to 0.9.21 to clear RUSTSEC-2026-0204 (invalid pointer dereference,
  reached via `nucleo → rayon`). The five remaining unmaintained/unsound
  warnings are documented with rationale in `audit.toml`.
- Search recall is bound to `Alt+Up` / `Alt+Down` rather than bare arrows, so
  result navigation keeps working.
- The ZSet range-query binding moved from `q` to `Q`, and the hash filter from
  `/` to `Ctrl+f`, because `q` quits from anywhere and `/` opens global search.
- `cargo fmt --check` and `clippy -D warnings` are now enforced in CI.

## [0.1.0]

Initial release: connection management, namespace key browser, value inspector
for all six Redis types, REPL, pub/sub, search, and the server info dashboard.

[Unreleased]: https://github.com/Joel-Raju/reddish/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Joel-Raju/reddish/releases/tag/v0.1.0