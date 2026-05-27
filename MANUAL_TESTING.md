# Manual Testing Guide

This document provides an end-to-end manual test plan for currently implemented Reddish TUI functionality.

## Scope

Use this checklist to validate:
- startup paths (`default`, `--url`, `--profile`, `--readonly`, `--log-level`, `--theme`)
- key browsing, multi-select, sorting, and typed value inspection
- set operations (union, inter, diff)
- zset score range filtering
- REPL execution, history, autocomplete, and overlay
- info dashboard refresh, slowlog scrolling, and colored stats
- pub/sub subscribe + publish, unsubscribe, and message clearing
- global search jump-to-key flow
- command palette, help, keymap shortcuts, and mouse interactions
- duplicate key, TTL editing, and clipboard copy

## Prerequisites

- Rust toolchain installed (`cargo`)
- Redis installed (`redis-server`, `redis-cli`)
- Terminal that supports mouse events (most modern terminals do)

## 1) Start Redis

```bash
redis-server --port 6379
```

All examples below assume `127.0.0.1:6379`.

## 2) Seed deterministic test data

Open a second terminal and run:

```bash
redis-cli <<'EOF'
FLUSHDB

# strings
SET user:1:name "Alice"
SET user:1:email "alice@example.com"
SET user:2:name "Bob"
SET counter 42

# hash
HSET user:1:profile name "Alice" age 30 city "NYC"

# list
LPUSH tasks "task-3" "task-2" "task-1"

# sets (for set operations)
SADD tags "rust" "redis" "tui"
SADD frameworks "rust" "actix" "tokio"
SADD langs "rust" "go" "python"

# zset (with varied scores for range query)
ZADD leaderboard 100 player1 200 player2 50 player3 75 player4 150 player5 250 player6

# stream
XADD events * sensor temp value 23.5
XADD events * sensor temp value 24.1

# ttl sample
SET session:abc "active" EX 300

SLOWLOG RESET
EOF
```

## 3) Prepare a profile for `--profile` testing

Create `connections.toml`:

- macOS: `~/Library/Application Support/redis-tui/connections.toml`
- Linux: `~/.config/redis-tui/connections.toml`

Example content:

```toml
[[profiles]]
name = "local-dev"
host = "127.0.0.1"
port = 6379
db = 0
mode = "standalone"
```

## 4) Startup/CLI test matrix

Run each command and verify expected behavior.

### 4.1 Default startup

```bash
cargo run --release
```

Expected:
- app opens in TUI mode
- you can connect and reach `Keys` tab

### 4.2 URL startup

```bash
cargo run --release -- --url redis://127.0.0.1:6379/0
```

Expected:
- app auto-connects without manual profile selection

### 4.3 Profile startup

```bash
cargo run --release -- --profile local-dev
```

Expected:
- app auto-connects using `local-dev`

### 4.4 Read-only startup

```bash
cargo run --release -- --readonly
```

Expected:
- app starts normally
- destructive operations (for example delete key) are blocked with a read-only error

### 4.5 Log level + theme overrides

```bash
cargo run --release -- --log-level debug --theme nord
```

Expected:
- app starts successfully
- theme override is applied for this run

## 5) In-app feature checks

### 5.1 Key browser + value inspector

1. Go to `Keys` tab (`1`).
2. Navigate with `j/k` or arrow keys.
3. Select `user:1:name` with `Enter`.

Expected:
- right panel shows `Alice`

Repeat selection for these keys and verify type previews render:
- `user:1:profile` (hash)
- `tasks` (list)
- `tags` (set)
- `leaderboard` (zset)
- `events` (stream)

### 5.2 Multi-select

1. In Keys tab, navigate to `user:1:name`.
2. Press `Space`.

Expected:
- key is marked with `*` prefix and dark background

3. Navigate to `user:1:email`.
4. Press `Space`.

Expected:
- both keys are selected

5. Press `Ctrl+A`.

Expected:
- all visible keys in current namespace are selected

### 5.3 Sort modes

1. In Keys tab, press `s` repeatedly.

Expected:
- title bar cycles through sort indicators: `sort: alpha`, `sort: type`, `sort: ttl`
- keys reorder accordingly

### 5.4 Key actions (keys tab)

1. Select key `counter`.
2. Press `d` (lowercase).

Expected:
- prompt opens for destination key name (pre-filled `counter_copy`)
- submit with `Enter` duplicates the key
- verify `counter_copy` appears after next refresh (`R`)

3. Select key `counter`.
4. Press `D` (uppercase).

Expected:
- non-readonly mode: key is deleted and key count updates
- readonly mode: operation blocked with error message

5. Press `r`.

Expected:
- prompt opens for new name
- rename operation works after submit

6. Press `t`.

Expected:
- prompt opens for TTL in seconds
- entering `60` sets 60-second TTL
- entering blank and submit persists the key (removes TTL)

7. Press `y`.

Expected:
- status bar shows "Copied: <key_name>"
- key name is on clipboard

8. Press `R`.

Expected:
- app shows informational message for refresh request
- key browser rescans and updates

### 5.5 Set operations

1. In Keys tab, navigate to `tags` (a set).
2. Press `Space` to select `tags`.
3. Navigate to `frameworks` and press `Space` to select it too.
4. With cursor on `tags`, press `u`.

Expected:
- prompt opens: "Destination key:" (pre-filled `tags_union`)
- submit opens second prompt: "Additional source keys (space-separated):"
- submit executes SUNIONSTORE
- result key `tags_union` appears after refresh (`R`)

5. Repeat with `i` (intersection) and `x` (difference).

Expected:
- SINTERSTORE and SDIFFSTORE work similarly
- selected keys are automatically included as sources

### 5.6 ZSet range query

1. Select `leaderboard` (zset) to open the value inspector.
2. Press `q`.

Expected:
- prompt opens: "Min score (empty=-inf):"

3. Type `75` and press `Enter`.

Expected:
- second prompt: "Max score (empty=+inf):"

4. Type `150` and press `Enter`.

Expected:
- zset view updates to show only members with scores between 75 and 150
- title shows `filter:75~150`
- cursor count updates to match filtered length

5. Press `q` again.

Expected:
- filter is cleared and all members are visible again

### 5.7 Value inspector editing

For each type, verify add/edit/delete/copy operations:

**String:**
- Select `user:1:name`, press `e` → edit value, press `Enter` to save
- Press `Tab` to cycle view modes (Raw → JSON → Base64 → Hex)
- Press `y` to copy string value

**List:**
- Select `tasks`, press `a` → RPUSH prompt, enter value, submit
- Press `p` → LPUSH prompt, enter value, submit
- Navigate list with `j/k`, press `e` to edit item at cursor
- Press `D` to delete item at cursor

**Hash:**
- Select `user:1:profile`, press `a` → add field prompt
- Enter field name, then value
- Navigate with `j/k`, press `e` to edit field value
- Press `D` to delete field

**Set:**
- Select `tags`, press `a` → add member prompt
- Press `D` to delete member at cursor

**ZSet:**
- Select `leaderboard`, press `a` → add member prompt, then score prompt
- Press `s` to toggle score sort asc/desc
- Press `q` to filter by score range (see 5.6)
- Press `e` to edit score at cursor

**Stream:**
- Select `events`, press `a` → entry ID prompt (`*` for auto), then fields prompt
- Press `f` to toggle compact/full view
- Press `g` to jump to last entry

### 5.8 REPL

1. Switch to REPL (`2`).
2. Enter `GET user:1:name` and press `Enter`.

Expected:
- output contains `Alice`

3. Verify autocomplete: type `ge`, press `Tab`.

Expected:
- input autocompletes to `GET`

4. Verify history:
- press `Up` to recall previous command
- press `Down` to move forward

5. Verify pipeline parsing:

```text
GET user:1:name | STRLEN
```

Expected:
- command executes successfully and shows output

6. Verify REPL overlay from Keys tab:
- Switch to Keys tab (`1`)
- Press `:`
- Type `GET counter`, press `Enter`

Expected:
- overlay executes command, shows result briefly, then closes
- you remain in Keys tab

### 5.9 Info dashboard

1. Switch to Info (`3`).
2. Wait a few seconds.

Expected:
- INFO-based metrics render with colored spans (version/uptime, clients/memory, hits/ratio/evictions)
- slowlog section renders (can be empty)
- data refreshes while tab stays active

3. Press `Space`.

Expected:
- slowlog toggles between compact and full display modes

4. Press `j` / `k` or `Down` / `Up`.

Expected:
- slowlog scrolls up/down when there are entries

### 5.10 Pub/Sub

1. Switch to Pub/Sub (`4`).
2. Keep input in `channel` mode (default), type `test-channel`, press `Enter`.
3. Press `Tab` to switch to `message` mode.
4. Type `hello-from-tui`, press `Enter`.

Expected:
- message appears in the feed on the right
- left panel shows `test-channel` with `▶` active indicator

5. Press `Tab` to switch back to channel mode.
6. Type `another-channel`, press `Enter`.

Expected:
- second channel appears in left panel
- you can switch between channels with `j/k` or Up/Down

7. With cursor on `test-channel`, press `D`.

Expected:
- channel is removed from subscription list

8. Press `Ctrl+L`.

Expected:
- all received messages are cleared from the feed

From another terminal:

```bash
redis-cli PUBLISH test-channel "hello from redis-cli"
```

Expected:
- new incoming message appears in the feed (if still subscribed)

### 5.11 Global search

1. Open search with `/`.
2. Type `user:`.
3. Move selection with `Up`/`Down`.
4. Press `Enter`.

Expected:
- overlay closes
- app switches to `Keys`
- cursor jumps to selected key and value inspector loads it

5. Re-open search and press `Esc`.

Expected:
- overlay closes without navigation

### 5.12 Command palette + help

1. Open palette with `Ctrl+P`.
2. Type `Quit`, press `Enter`.

Expected:
- app exits

Relaunch and verify:
1. Open palette with `Ctrl+P`, then press `Esc`.
2. Open help with `?`, close with `Esc`.

Expected:
- palette and help open/close correctly

### 5.13 App-level shortcuts

Verify these shortcuts from non-modal state:
- `1` / `2` / `3` / `4` switch tabs
- `Ctrl+\` opens connection screen
- `Ctrl+R` reconnects active connection
- `q` quits app

### 5.14 Mouse support

1. In `Keys` tab, use mouse wheel up/down.
2. Click tab bar labels near top row.

Expected:
- wheel scroll moves key cursor
- click switches tab (Keys/REPL/Info/PubSub)

## 6) Optional negative-path checks

- Start with invalid URL:

```bash
cargo run --release -- --url not-a-url
```

Expected:
- app surfaces invalid URL error state

- Start with unknown profile:

```bash
cargo run --release -- --profile does-not-exist
```

Expected:
- app surfaces profile-not-found error state

## 7) Cleanup

```bash
redis-cli FLUSHDB
```

Stop Redis if needed:

```bash
redis-cli SHUTDOWN
```

## Troubleshooting

| Issue | What to check |
|---|---|
| Connection refused | Confirm `redis-server` is running and port is correct |
| No keys listed | Re-run seed block and reconnect |
| Pub/Sub message missing | Confirm subscription step completed before publishing |
| Search shows no matches | Ensure keys were seeded and query matches key names |
| Slowlog empty | It may be empty on small local workloads; this is acceptable |
| Set operations fail | Confirm cursor is on a set key and selected keys are also sets |
| ZSet filter shows nothing | Confirm min/max scores are valid numbers or `-inf`/`+inf` |
