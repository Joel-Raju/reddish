# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-23

A keyboard-driven Redis TUI: connection management, a namespace-grouped key
browser, a typed value inspector covering all six Redis types, a REPL with
fuzzy autocomplete, Pub/Sub, global key search, and a server dashboard with
per-second stats and slow-log analysis.

Five themes, vim and emacs keymap presets, cluster / sentinel / TLS support,
`$EDITOR` integration, and enforced performance budgets. `--readonly` blocks
every write, and the REPL refuses `FLUSHDB`, `FLUSHALL` and `KEYS *`.

### Added

- **Pub/Sub follow mode** — press `f` to toggle. The message pane tails the
  live feed by default; pausing freezes the viewport so a busy channel cannot
  move your reading position. Scrolling into history detaches automatically and
  scrolling back to the newest message re-attaches. The title always shows
  `[f] follow` or `[f] PAUSED`.
- **The built-in string editor is now a popup.** Pressing `e` used to replace
  the value pane with the editor, hiding the value you were editing. It now
  draws a centred overlay, so the original stays visible around it. The popup
  title carries the key being edited; `Ctrl+S` / `Ctrl+X` / `Esc` hints sit on
  the bottom border so a long key name cannot clip them. Long lines scroll
  horizontally and long values scroll vertically to keep the cursor on screen.

- **Themes now work.** Five themes ship (`default`, `dracula`, `nord`,
  `solarized_dark`, `gruvbox_dark`) and `--theme` / `color_scheme` actually
  apply them. The theme module had been dead code: nothing imported it, so
  `--theme nord` exited successfully and changed nothing. Theme colours now
  cover panel borders and titles, the key-list selection, type colours, TTL and
  memory metadata, severity ramps, prompts, JSON syntax highlighting, connection
  states and gauges. The per-subscription Pub/Sub colour cycle is deliberately
  left alone, since its job is to keep adjacent channels distinguishable.
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
- **Five built-in themes** (default, dracula, nord, solarized_dark,
  gruvbox_dark) with vim and emacs keymap presets and TOML overrides.
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

- **REPL subcommand hints** — `CONFIG <Tab>` now suggests `GET`, `SET`,
  `REWRITE` rather than every key in the database. Covers 11 commands whose
  second argument is a subcommand (`ACL`, `CLIENT`, `CLUSTER`, `CONFIG`,
  `FUNCTION`, `LATENCY`, `MEMORY`, `OBJECT`, `SCRIPT`, `SLOWLOG`, `XINFO`).
  Past the second token the argument is a key again, so key suggestions resume.
- **Pipeline errors no longer abort** — a failing command in a
  `cmd1 | cmd2 | cmd3` line records its own error and the remaining commands
  still run. Previously the first failure discarded every earlier result.
- **Honoured the documented `editor` and `max_value_display_bytes` config
  keys.** `editor` overrides `$EDITOR`; large values are truncated at render
  time with a visible marker instead of stalling the frame.
- **Help overlay is generated from the active keymap.** It previously showed the
  vim bindings unconditionally, so it told emacs users to press `q` to quit.

- **`confirm_deletes` is now honoured.** Setting `confirm_deletes = false` in
  `config.toml` skipped nothing — the key was parsed and exposed but never
  read, so the confirmation prompt appeared regardless. Read-only mode still
  overrides it.
- **The status bar shows a live latency.** `StatusBar::latency_ms` was never
  assigned anywhere, so the field it renders was permanently blank. A PING
  every ~4s at the default `refresh_interval_ms` of 1000 now fills it in
  (the probe runs every 4th tick); a failed probe keeps the previous reading
  rather than blanking the field.

- **`refresh_interval_ms` is now honoured.** It was documented as the refresh
  interval and appeared in the example config, but nothing read it — the event
  loop's tick rate was hardcoded to 250ms. It now drives the tick rate, and is
  clamped to 50–60000ms so a too-small value cannot turn the periodic tasks into
  a PING loop.

### Changed

- **Fixed a binding that could never fire.** The connection screen was bound to
  `Ctrl+\`, matched as `KeyCode::Char('\\')` with CONTROL — but terminals send
  `Ctrl+\` as byte `0x1C`, which crossterm decodes as `Ctrl+4`. The screen was
  unreachable from the keyboard. The binding now accepts the byte as it
  actually arrives (so `Ctrl+\` and `Ctrl+4` both work), and the connection
  screen is also reachable from the command palette as `Connect…`.
- **The command palette now performs the actions it lists.** It advertised
  `Switch:Keys`/`REPL`/`Info`/`PubSub` but only wired `Quit` — executing a
  switch item closed the overlay and did nothing. All items are wired, and an
  unwired one now reports itself instead of silently doing nothing.
- **The uppercase bindings finally fire.** Terminals deliver `D`, `R`, `Q`,
  `G`, `F`, `I` with a SHIFT modifier (crossterm decorates uppercase input),
  and `matches()` demanded exact modifiers — so the vim preset's refresh,
  element-delete, zset filter, stream follow/groups, and list-insert keys
  were dead on every real machine while their unit tests stayed green. The
  delete case was worse: the inspector ignored the key and the browser's
  modifier-blind fallback prompted to delete the **entire key** instead of
  the selected element. Key spellings now parse to the event the wire
  carries, and every preset binding is checked against the legacy decode
  table by a test plus startup validation for user config.
- **MONITOR is reachable.** It was toggled with `Ctrl+M` — byte `0x0D`, which
  every terminal delivers as Enter. The toggle is `Alt+M`; raw mode is
  `Alt+R` (the old `Ctrl+R` was shadowed by global reconnect); Shift+Tab
  cycles autocomplete backwards (legacy sends `ESC[Z` = BackTab); `Ctrl+U`
  clears the input line.
- **The help overlay is modal.** It consumed only Esc, so `q` quit the app
  through the overlay and `?` stacked a second help screen.
- **Errors are visible.** `App::error_message` was written in ~60 places and
  rendered by nothing — readonly blocks, "Copied:", CSV export paths,
  palette mis-wiring reports, all invisible. Every write now also pushes a
  status-bar toast.
- **The built-in string editor is modal.** Its advertised `Ctrl+S` save
  opened the global search overlay and `Ctrl+X` opened the palette, because
  global bindings ran before the editor saw the key.
- **The inspector honors the keymap's navigation bindings.** Emacs
  `Ctrl+N`/`Ctrl+P` previously fell through and moved the key browser
  behind the open value.
- **REPL pipelines record per-command errors and keep running**, as this
  changelog already claimed: a failing stage no longer aborts the line.
- **`[keybindings]` config finally loads.** The serde defaults existed on
  every field while `App` hardcoded the vim preset, so user keybinding TOML
  was silently ignored. The same fix wired `mouse_enabled`.
- **The emacs preset is deliverable.** All 14 `ctrl+shift+<letter>` chords
  are byte-identical to plain `ctrl+` chords in legacy encoding; the preset
  now uses vim's contextual inspector letters. `set_inter` (`Ctrl+I` is
  Tab), `set_diff` (shadowed by delete), `rename_key` (shadowed by
  reconnect) and `go_up` (shadowed by nav-left) rebound.
- **Mouse events no longer drive the panes behind an open overlay.**
- **Help text, status-bar hints and the palette title are generated from
  the loaded keymap.** The duplicated consumer-groups row and the false
  "Ctrl+C forces quit from any prompt" line are gone; the previously
  undocumented fixed chords (connection screen, reconnect, MONITOR) are
  listed.

- **Key list rendering is windowed.** With every namespace expanded the browser
  allocated a `ListItem` — a `String` and a `Style` — for all 10,001 rows on
  every frame, while an 80x24 terminal draws about 20 of them. Only the visible
  window is built now. Render p99 at 10K keys drops from 12.5ms to 8.0ms, which
  is what brought the performance gate back inside its 16ms budget on shared CI
  hardware.
- **Dropped four unused dependencies** (`openssh`, `anyhow`, `tui-textarea`,
  `xdg`). `openssh` was Unix-only and was the sole reason the crate did not
  compile on Windows at all.
- An unknown `--theme` is now rejected with the list of valid names instead of
  silently falling back to the default. `--help` previously advertised
  `solarized` and `gruvbox`, which were never real theme names.
- `PasswordRef` now has a hand-written `Debug` that redacts `Plaintext`
  instead of printing the secret, and `redis::client::redact_url` strips
  credentials from a connection URL before it can reach an error message.
  `Serialize` is unchanged, since it is what writes the value to
  `connections.toml`.
- Added `tests/binary_smoke.rs`, which drives the real executable to check
  `--help`, `--version`, flag rejection, and that the performance gates run
  without touching the terminal.
- `cargo audit` reports zero vulnerabilities. `crossbeam-epoch` was bumped from
  0.9.18 to 0.9.21 to clear RUSTSEC-2026-0204 (invalid pointer dereference,
  reached via `nucleo → rayon`). The five remaining unmaintained/unsound
  warnings are documented with rationale in `audit.toml`.
- Search recall is bound to `Alt+Up` / `Alt+Down` rather than bare arrows, so
  result navigation keeps working.
- The ZSet range-query binding moved from `q` to `Q`, and the hash filter from
  `/` to `Ctrl+f`, because `q` quits from anywhere and `/` opens global search.
- `cargo fmt --check` and `clippy -D warnings` are now enforced in CI.

[Unreleased]: https://github.com/Joel-Raju/reddish/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Joel-Raju/reddish/releases/tag/v0.1.0