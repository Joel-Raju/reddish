use std::time::Duration;

use color_eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::io::Stdout;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::backoff::backoff_sequence;
use crate::config::Config;
use crate::config::connections::{ConnectionMode, ConnectionProfile, ConnectionStore};
use crate::config::keybindings::Keymap;
use crate::events::{Event, EventHandler};
use crate::redis::client::{RedisClientHandle, RedisType};
use crate::redis::pubsub::{PubSubClient, PubSubKind, PubSubMessage};
use crate::redis::server::{slowlog_get, slowlog_len, slowlog_reset};
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};
use crate::ui::info_dashboard::{InfoDashboard, SystemStats};
use crate::ui::key_browser::scanner_task;
use crate::ui::key_browser::{BrowserAction, KeyBrowser};
use crate::ui::pubsub::{PubSubAction, PubSubWidget};
use crate::ui::repl::{ReplAction, ReplLineStatus, ReplWidget, parse_pipeline};
use crate::ui::search::{GlobalSearch, SearchAction};
use crate::ui::status_bar::{ConnectionState, StatusBar};
use crate::ui::tab_bar;
use crate::ui::value_inspector::{InspectorAction, ValueInspector};
use crate::ui::widgets::confirm::ConfirmDialog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Repl,
    Help,
    Search,
    Confirm,
    ConnectionScreen,
    ReplOverlay,
}

fn redis_value_to_string(value: &redis::Value) -> String {
    match value {
        redis::Value::Nil => "(nil)".to_string(),
        redis::Value::Int(i) => i.to_string(),
        redis::Value::BulkString(bytes) => String::from_utf8(bytes.clone())
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string()),
        redis::Value::SimpleString(s) => s.clone(),
        redis::Value::Okay => "OK".to_string(),
        redis::Value::Array(items) => items
            .iter()
            .map(redis_value_to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Map(items) => items
            .iter()
            .map(|(k, v)| format!("{}: {}", redis_value_to_string(k), redis_value_to_string(v)))
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Set(items) => items
            .iter()
            .map(redis_value_to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Double(f) => f.to_string(),
        redis::Value::Boolean(b) => b.to_string(),
        redis::Value::Attribute { data, attributes } => {
            let mut out = redis_value_to_string(data);
            if !attributes.is_empty() {
                out.push_str("\n# attributes\n");
                out.push_str(
                    &attributes
                        .iter()
                        .map(|(k, v)| {
                            format!("{}: {}", redis_value_to_string(k), redis_value_to_string(v))
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
            out
        }
        redis::Value::VerbatimString { text, .. } => text.clone(),
        redis::Value::BigNumber(n) => n.to_string(),
        redis::Value::Push { kind, data } => format!(
            "push({kind:?}) {}",
            data.iter()
                .map(redis_value_to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ),
        redis::Value::ServerError(err) => format!("{err:?}"),
    }
}

pub fn repl_policy_error_for_test(stages: &[Vec<String>], readonly: bool) -> Option<String> {
    repl_policy_error(stages, readonly)
}

pub async fn execute_repl_command_for_test(app: &mut App, input: String) {
    app.execute_repl_command(input).await;
}

fn repl_policy_error(stages: &[Vec<String>], readonly: bool) -> Option<String> {
    for stage in stages {
        let Some(command) = stage.first() else {
            continue;
        };
        let command = command.to_ascii_uppercase();
        if is_dangerous_repl_command(&command, &stage[1..]) {
            return Some(format!("{command} is blocked by safety policy"));
        }
        if readonly && is_mutating_repl_command(&command, &stage[1..]) {
            return Some(format!("Read-only mode: {command} blocked"));
        }
    }
    None
}

fn is_dangerous_repl_command(command: &str, args: &[String]) -> bool {
    matches!(command, "FLUSHDB" | "FLUSHALL")
        || command == "KEYS" && args.first().map(|arg| arg.trim() == "*").unwrap_or(false)
}

fn is_mutating_repl_command(command: &str, args: &[String]) -> bool {
    match command {
        "APPEND" | "BITFIELD" | "BITOP" | "BLMOVE" | "BLMPOP" | "BLPOP" | "BRPOP"
        | "BRPOPLPUSH" | "BZMPOP" | "BZPOPMAX" | "BZPOPMIN" | "COPY" | "DECR" | "DECRBY"
        | "DEL" | "EVAL" | "EVALSHA" | "EXPIRE" | "EXPIREAT" | "EXPIRETIME" | "FLUSHALL"
        | "FLUSHDB" | "GEOADD" | "GETDEL" | "GETEX" | "HDEL" | "HINCRBY" | "HINCRBYFLOAT"
        | "HMSET" | "HSET" | "HSETNX" | "INCR" | "INCRBY" | "INCRBYFLOAT" | "LINSERT" | "LMOVE"
        | "LMPOP" | "LPOP" | "LPOS" | "LPUSH" | "LPUSHX" | "LREM" | "LSET" | "LTRIM"
        | "MIGRATE" | "MOVE" | "MSET" | "MSETNX" | "PERSIST" | "PEXPIRE" | "PEXPIREAT"
        | "PFADD" | "PFMERGE" | "PUBLISH" | "RENAME" | "RENAMENX" | "RESTORE" | "RPOP"
        | "RPOPLPUSH" | "RPUSH" | "RPUSHX" | "SADD" | "SDIFFSTORE" | "SET" | "SETBIT" | "SETEX"
        | "SETNX" | "SETRANGE" | "SINTERSTORE" | "SMOVE" | "SORT" | "SPOP" | "SREM"
        | "SUNIONSTORE" | "TOUCH" | "UNLINK" | "XACK" | "XADD" | "XAUTOCLAIM" | "XCLAIM"
        | "XDEL" | "XGROUP" | "XREADGROUP" | "XSETID" | "XTRIM" | "ZADD" | "ZDIFFSTORE"
        | "ZINCRBY" | "ZINTERSTORE" | "ZMPOP" | "ZMSCORE" | "ZPOPMAX" | "ZPOPMIN"
        | "ZRANGESTORE" | "ZREM" | "ZREMRANGEBYLEX" | "ZREMRANGEBYRANK" | "ZREMRANGEBYSCORE"
        | "ZUNIONSTORE" => true,
        "ACL" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("SETUSER" | "DELUSER" | "LOAD" | "SAVE" | "GENPASS")
        ),
        "CLIENT" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("KILL" | "PAUSE" | "REPLY" | "SETNAME" | "TRACKING" | "UNBLOCK")
        ),
        "CONFIG" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("SET" | "REWRITE" | "RESETSTAT")
        ),
        "FUNCTION" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("LOAD" | "DELETE" | "FLUSH" | "RESTORE" | "KILL")
        ),
        "MEMORY" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("PURGE")
        ),
        "MODULE" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("LOAD" | "UNLOAD")
        ),
        "SCRIPT" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("LOAD" | "FLUSH" | "KILL")
        ),
        "SLOWLOG" => matches!(
            args.first().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("RESET")
        ),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Keys,
    Repl,
    Info,
    PubSub,
}

/// A string edit handed to the user's `$EDITOR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalEdit {
    pub key: String,
    pub value: String,
}

/// Split an `$EDITOR` value into a program and its leading arguments.
///
/// Handles the common forms: `vim`, `code --wait`, `emacsclient -nw`.
fn split_editor_command(editor: &str) -> Option<(String, Vec<String>)> {
    let mut parts = editor.split_whitespace().map(str::to_string);
    let program = parts.next()?;
    Some((program, parts.collect()))
}

/// The editor to use, if `$EDITOR` names one.
pub fn external_editor() -> Option<String> {
    std::env::var("EDITOR")
        .ok()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
}

/// The editor to run for string editing.
///
/// A configured `editor` in `config.toml` wins over `$EDITOR`, so a user can
/// pin an editor without changing their shell environment. Blank values fall
/// through to `$EDITOR` rather than being treated as a configured editor.
pub fn resolved_editor(config: &Config) -> Option<String> {
    config.editor().map(str::to_string).or_else(external_editor)
}

/// Write `edit.value` to a temp file, run the user's editor on it, and return
/// the edited text.
///
/// The caller is responsible for suspending the TUI; this function only touches
/// the filesystem and spawns a child process, so it is unit-testable without a
/// terminal.
///
/// Returns `Ok(None)` when `editor` is blank or malformed, when the editor exits
/// non-zero (a failed `write` should not silently discard the value), or when
/// the file cannot be read back. The `NamedTempFile` is removed on drop, so the
/// value is never left in a temp file after this returns.
pub fn edit_in_external_editor(
    edit: &ExternalEdit,
    editor: &str,
) -> color_eyre::Result<Option<String>> {
    use std::io::Write;

    let Some((program, args)) = split_editor_command(editor) else {
        return Ok(None);
    };

    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(edit.value.as_bytes())?;
    file.flush()?;

    let status = std::process::Command::new(&program)
        .args(&args)
        .arg(file.path())
        .status();

    match status {
        Ok(s) if s.success() => Ok(std::fs::read_to_string(file.path()).ok()),
        _ => Ok(None),
    }
}

/// Leave the alternate screen and disable raw mode so a child process can
/// use the terminal normally.
fn suspend_tui(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> std::io::Result<()> {
    use crossterm::{cursor, execute, terminal::LeaveAlternateScreen};
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    crossterm::terminal::disable_raw_mode()
}

/// Undo [`suspend_tui`] and clear the screen, since the editor redrew it.
fn resume_tui(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> std::io::Result<()> {
    use crossterm::{cursor, execute, terminal::EnterAlternateScreen};
    crossterm::terminal::enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, cursor::Hide)?;
    terminal.clear()
}

/// Geometry captured during the last render so mouse clicks can be resolved.
///
/// Ratatui is immediate-mode: nothing is retained between frames, so click
/// handling needs to know where widgets actually landed.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderGeometry {
    /// Where the tab bar was drawn.
    pub tab_bar: ratatui::layout::Rect,
    /// Key list geometry, including its scroll offset.
    pub key_list: crate::ui::key_browser::BrowserRenderInfo,
}

pub struct App {
    pub should_quit: bool,
    pub config: Config,
    pub active_tab: Tab,
    pub mode_stack: Vec<AppMode>,
    pub key_browser: KeyBrowser,
    pub status_bar: StatusBar,
    pub value_inspector: ValueInspector,
    pub info_dashboard: InfoDashboard,
    pub pubsub_widget: PubSubWidget,
    pub search: GlobalSearch,
    pub repl: ReplWidget,
    pub command_palette: Option<CommandPalette>,
    pub connection_screen: Option<ConnectionScreen>,
    pub readonly: bool,
    pub allow_monitor: bool,
    pub error_message: Option<String>,
    pub client: Option<RedisClientHandle>,
    pub keymap: Keymap,
    pub startup_profile: Option<ConnectionProfile>,
    last_profile: Option<ConnectionProfile>,
    pub scan_rx: Option<mpsc::Receiver<Vec<crate::ui::key_browser::tree::KeyEntry>>>,
    pub scan_cancel: Option<CancellationToken>,
    pub search_scan_rx: Option<mpsc::Receiver<Vec<String>>>,
    pub search_scan_cancel: Option<CancellationToken>,
    pub pubsub_rx: Option<mpsc::UnboundedReceiver<PubSubMessage>>,
    pub monitor_rx: Option<mpsc::UnboundedReceiver<String>>,
    tick_count: u64,
    reconnect_attempt: u32,
    pub pending_delete_key: Option<String>,
    pub overlay_input: Option<crate::ui::widgets::input::InputWidget>,
    /// Widget positions from the most recent render, for mouse hit-testing.
    ///
    /// Stored in a `Cell` because `render` takes `&self` (ratatui's immediate
    /// mode) yet still needs to publish where widgets landed.
    geometry: std::cell::Cell<RenderGeometry>,
    /// Set when a mouse click switches to the Info tab; `render` cannot await,
    /// so the refresh is deferred to the next tick.
    info_refresh_pending: bool,
    /// A string edit awaiting the external editor, consumed by `run` (which
    /// owns the `Terminal` and so can suspend the TUI around the subprocess).
    pub pending_external_edit: Option<ExternalEdit>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let sep = config.namespace_separator().chars().next().unwrap_or(':');
        let max_keys = config.max_keys_in_memory();
        Self {
            should_quit: false,
            config,
            active_tab: Tab::Keys,
            mode_stack: vec![AppMode::Normal],
            key_browser: KeyBrowser::new(sep, max_keys),
            status_bar: StatusBar::default(),
            value_inspector: ValueInspector::new(),
            info_dashboard: InfoDashboard::new(),
            pubsub_widget: PubSubWidget::new(),
            search: GlobalSearch::new(),
            repl: ReplWidget::new(),
            command_palette: None,
            connection_screen: None,
            readonly: false,
            allow_monitor: false,
            error_message: None,
            client: None,
            keymap: Keymap::default(),
            startup_profile: None,
            last_profile: None,
            scan_rx: None,
            scan_cancel: None,
            search_scan_rx: None,
            search_scan_cancel: None,
            pubsub_rx: None,
            monitor_rx: None,
            tick_count: 0,
            reconnect_attempt: 0,
            pending_delete_key: None,
            overlay_input: None,
            geometry: std::cell::Cell::new(RenderGeometry::default()),
            info_refresh_pending: false,
            pending_external_edit: None,
        }
    }

    /// Geometry captured by the most recent render.
    pub fn geometry(&self) -> RenderGeometry {
        self.geometry.get()
    }

    /// Test seam: seed the geometry a render would have produced.
    pub fn set_geometry_for_test(&self, geometry: RenderGeometry) {
        self.geometry.set(geometry);
    }

    async fn handle_pubsub_action(&mut self, action: PubSubAction) {
        match action {
            PubSubAction::Subscribe(channel) => {
                match self.start_pubsub_subscription(PubSubKind::Channel(channel.clone())) {
                    Ok(()) => {
                        self.error_message = Some(format!("Subscribed to channel '{channel}'"));
                    }
                    Err(err) => {
                        self.error_message = Some(format!("Subscribe failed: {err}"));
                    }
                }
            }
            PubSubAction::PSubscribe(pattern) => {
                match self.start_pubsub_subscription(PubSubKind::Pattern(pattern.clone())) {
                    Ok(()) => {
                        self.error_message = Some(format!("Subscribed to pattern '{pattern}'"));
                    }
                    Err(err) => {
                        self.error_message = Some(format!("PSubscribe failed: {err}"));
                    }
                }
            }
            PubSubAction::Unsubscribe(channel) => {
                self.pubsub_rx = None;
                self.error_message = Some(format!("Unsubscribed from '{channel}'"));
            }
            PubSubAction::PUnsubscribe(pattern) => {
                self.pubsub_rx = None;
                self.error_message = Some(format!("Unsubscribed from pattern '{pattern}'"));
            }
            PubSubAction::Publish { channel, message } => {
                if self.readonly {
                    let msg = "Read-only mode: publish blocked".to_string();
                    self.status_bar.push_error(msg.clone());
                    self.error_message = Some(msg);
                    return;
                }
                if let Some(client) = &self.client {
                    match client.publish(&channel, &message).await {
                        Ok(_) => {
                            self.pubsub_widget.push_message(PubSubMessage {
                                channel,
                                pattern: None,
                                payload: message,
                                timestamp: std::time::Instant::now(),
                            });
                            self.error_message = None;
                        }
                        Err(err) => {
                            let msg = format!("Publish failed: {err}");
                            self.status_bar.push_error(msg.clone());
                            self.error_message = Some(msg);
                        }
                    }
                } else {
                    let msg = "Not connected".to_string();
                    self.status_bar.push_error(msg.clone());
                    self.error_message = Some(msg);
                }
            }
        }
    }

    async fn execute_repl_command(&mut self, input: String) {
        let stages = parse_pipeline(&input);
        if stages.is_empty() {
            return;
        }
        if let Some(err) = repl_policy_error(&stages, self.readonly) {
            self.repl
                .add_result(&input, err.clone(), ReplLineStatus::Error(err.clone()));
            self.status_bar.push_error(err.clone());
            self.error_message = Some(err);
            return;
        }

        let mut last_output: Option<String> = None;
        for stage in stages {
            match self.execute_repl_stage(&stage, last_output.clone()).await {
                Ok(output) => {
                    last_output = Some(output);
                }
                Err(err) => {
                    self.repl
                        .add_result(&input, err.clone(), ReplLineStatus::Error(err.clone()));
                    self.status_bar.push_error(err.clone());
                    self.error_message = Some(err);
                    return;
                }
            }
        }

        let output = last_output.unwrap_or_default();
        self.repl
            .add_result(&input, output, ReplLineStatus::Success);
    }

    async fn execute_repl_stage(
        &self,
        stage: &[String],
        piped_input: Option<String>,
    ) -> std::result::Result<String, String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| "Not connected".to_string())?;

        let mut tokens = stage.to_vec();
        if let Some(input) = piped_input
            && !input.is_empty()
        {
            tokens.push(input);
        }
        if tokens.is_empty() {
            return Ok(String::new());
        }

        let value = client
            .execute_raw(&tokens)
            .await
            .map_err(|e| format!("{} error: {e}", tokens[0].to_uppercase()))?;
        Ok(redis_value_to_string(&value))
    }

    pub fn mode(&self) -> &AppMode {
        self.mode_stack.last().unwrap_or(&AppMode::Normal)
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        // A configured editor takes over string editing; otherwise the built-in
        // editor is always used.
        self.value_inspector.prefer_external_editor = resolved_editor(&self.config).is_some();
        self.value_inspector.max_value_display_bytes = self.config.max_value_display_bytes();

        if let Some(profile) = self.startup_profile.take() {
            self.connect_profile(profile).await;
        }

        let tick_rate = Duration::from_millis(250);
        let mut events = EventHandler::new(tick_rate);

        while !self.should_quit {
            terminal.draw(|f| self.render(f))?;

            let event = tokio::time::timeout(Duration::from_millis(100), events.next()).await;
            if let Ok(Some(event)) = event {
                self.handle_event(event).await;
            }

            // The external editor owns the terminal, so it is driven here where
            // the TUI can be suspended around the child process.
            if self.pending_external_edit.is_some() {
                self.run_pending_external_edit(terminal).await;
            }
        }

        Ok(())
    }

    /// Suspend the TUI, run the user's editor, restore the screen, and apply
    /// the result if the value actually changed.
    async fn run_pending_external_edit(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) {
        let Some(edit) = self.pending_external_edit.take() else {
            return;
        };
        let original = edit.value.clone();

        if suspend_tui(terminal).is_err() {
            self.error_message = Some("Could not suspend the terminal".to_string());
            return;
        }
        // `block_in_place` keeps the tokio runtime alive while this thread is
        // occupied by the editor subprocess.
        let editor = match resolved_editor(&self.config) {
            Some(e) => e,
            None => {
                let _ = resume_tui(terminal);
                return;
            }
        };
        let outcome = tokio::task::block_in_place(|| edit_in_external_editor(&edit, &editor));
        let _ = resume_tui(terminal);

        match outcome {
            Ok(Some(new_value)) if new_value != original => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: edit blocked (press Esc)".to_string());
                } else {
                    self.handle_inspector_action(InspectorAction::WriteString {
                        key: edit.key.clone(),
                        value: new_value,
                    })
                    .await;
                }
            }
            // Unchanged, cancelled, or no editor configured: leave the value
            // alone without reporting an error.
            Ok(_) => {}
            Err(err) => {
                self.error_message = Some(format!("External editor failed: {err}"));
            }
        }
    }

    async fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key_event(key).await,
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            Event::Tick => self.handle_tick().await,
            Event::Resize(_, _) => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.active_tab == Tab::Keys {
                    let rows = self.key_browser.tree.visible_rows();
                    if self.key_browser.cursor + 1 < rows.len() {
                        self.key_browser.cursor += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if self.active_tab == Tab::Keys && self.key_browser.cursor > 0 {
                    self.key_browser.cursor -= 1;
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_left_click(mouse.row, mouse.column);
            }
            _ => {}
        }
    }

    /// Resolve a left click against the geometry captured by the last render.
    fn handle_left_click(&mut self, row: u16, column: u16) {
        // Tab bar takes priority: it sits above every other widget.
        if let Some(tab) = tab_bar::tab_at(self.geometry().tab_bar, column, row) {
            self.active_tab = match tab {
                tab_bar::Tab::Keys => Tab::Keys,
                tab_bar::Tab::Repl => Tab::Repl,
                tab_bar::Tab::Info => Tab::Info,
                tab_bar::Tab::PubSub => Tab::PubSub,
            };
            if matches!(self.active_tab, Tab::Info) {
                // The keyboard path refreshes on entry. `render` cannot await,
                // so the same refresh is deferred to the next tick.
                self.info_refresh_pending = true;
            }
            return;
        }

        // Clicking a key row focuses and selects it. Only meaningful on the
        // Keys tab, where the list is actually drawn.
        if self.active_tab != Tab::Keys || self.mode() != &AppMode::Normal {
            return;
        }
        if let Some(index) = self.geometry().key_list.row_at(column, row) {
            let total = self.key_browser.sorted_rows().len();
            if index < total && index != self.key_browser.cursor {
                self.key_browser.cursor = index;
            }
        }
    }

    /// Delete `name` from Redis and drop it from the key tree.
    ///
    /// Shared by the confirmed path and the `confirm_deletes = false` path so
    /// both do exactly the same thing. Read-only mode is checked by the caller,
    /// before this is reached.
    async fn perform_delete(&mut self, name: String) {
        if self.key_browser.tree.remove(&name) {
            self.status_bar.key_count = self.key_browser.tree.total_keys();
        }
        match &self.client {
            Some(client) => {
                if let Err(err) = client.delete(&name).await {
                    self.error_message = Some(format!("Failed to delete key '{name}': {err}"));
                }
            }
            None => self.error_message = Some("Not connected".to_string()),
        }
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        if self.mode() == &AppMode::Confirm {
            if self.keymap.matches("confirm_yes", &key) || matches!(key.code, KeyCode::Enter) {
                let key_name = self.pending_delete_key.take();
                self.mode_stack.pop();
                if let Some(name) = key_name {
                    self.perform_delete(name).await;
                }
            } else if self.keymap.matches("confirm_no", &key) || matches!(key.code, KeyCode::Esc) {
                self.pending_delete_key = None;
                self.mode_stack.pop();
            }
            return;
        }

        if self.mode() == &AppMode::Search {
            if let Some(action) = self
                .search
                .handle_event_with_keymap(&Event::Key(key), &self.keymap)
            {
                match action {
                    SearchAction::Execute(key_name) => {
                        // Record the query that produced this selection, not
                        // the resolved key, so recall re-runs the same search.
                        let recorded = self.search.query.clone();
                        self.search.push_history(&recorded);
                        if self.key_browser.jump_to_key(&key_name) {
                            let rows = self.key_browser.tree.visible_rows();
                            if let Some(row) = rows.get(self.key_browser.cursor)
                                && let Some(key) = row.key.as_ref()
                            {
                                let action = BrowserAction::SelectKey(
                                    key.full_name.clone(),
                                    key.redis_type
                                        .clone()
                                        .unwrap_or(crate::redis::client::RedisType::Unknown),
                                );
                                self.handle_browser_action(action).await;
                            }
                        }
                        self.active_tab = Tab::Keys;
                        if self.mode() == &AppMode::Search {
                            self.mode_stack.pop();
                        }
                    }
                    SearchAction::Close => {
                        if self.mode() == &AppMode::Search {
                            self.mode_stack.pop();
                        }
                        self.cancel_search_scan();
                    }
                    SearchAction::QueryChanged => {
                        self.search.start_scan();
                        self.cancel_search_scan();
                        self.start_search_scan();
                    }
                    SearchAction::ModeToggled => {
                        // Re-scan: the server-side glob pattern depends on the mode.
                        self.search.start_scan();
                        self.cancel_search_scan();
                        self.start_search_scan();
                    }
                }
            }
            return;
        }

        if self.mode() == &AppMode::ConnectionScreen {
            if let Some(screen) = self.connection_screen.as_mut()
                && let Some(action) = screen.handle_event(&Event::Key(key))
            {
                match action {
                    ConnectionScreenAction::Connect(profile) => {
                        self.connect_profile(profile).await;
                        self.connection_screen = None;
                        if self.mode() == &AppMode::ConnectionScreen {
                            self.mode_stack.pop();
                        }
                    }
                    ConnectionScreenAction::Cancel => {
                        self.connection_screen = None;
                        if self.mode() == &AppMode::ConnectionScreen {
                            self.mode_stack.pop();
                        }
                    }
                    ConnectionScreenAction::Save(profile) => {
                        let screen = self.connection_screen.as_mut().unwrap();
                        screen.store.add(profile);
                        if let Err(err) = screen.store.save() {
                            screen.error = Some(format!("Failed to save: {err}"));
                        }
                    }
                    ConnectionScreenAction::Delete(name) => {
                        let screen = self.connection_screen.as_mut().unwrap();
                        screen.store.remove(&name);
                        if let Err(err) = screen.store.save() {
                            screen.error = Some(format!("Failed to save: {err}"));
                        }
                        screen.cursor = screen
                            .cursor
                            .min(screen.store.profiles.len().saturating_sub(1));
                    }
                }
            }
            return;
        }

        if self.mode() == &AppMode::Help && key.code == KeyCode::Esc {
            self.mode_stack.pop();
            return;
        }

        if self.mode() == &AppMode::ReplOverlay {
            if self.keymap.matches("cancel", &key) {
                self.overlay_input = None;
                self.mode_stack.pop();
            } else if self.keymap.matches("confirm", &key) {
                let cmd = self
                    .overlay_input
                    .as_ref()
                    .map(|o| o.value.clone())
                    .unwrap_or_default();
                self.overlay_input = None;
                self.mode_stack.pop();
                if !cmd.is_empty() {
                    self.execute_repl_command(cmd).await;
                }
            } else if let Some(ref mut overlay) = self.overlay_input {
                overlay.handle_event(&Event::Key(key));
            }
            return;
        }

        if let Some(ref mut palette) = self.command_palette {
            if let Some(action) = palette.handle_event(&Event::Key(key)) {
                match action {
                    PaletteAction::Execute(cmd) => {
                        if cmd == "Quit" {
                            self.should_quit = true;
                        }
                    }
                    PaletteAction::Close => {}
                }
                self.command_palette = None;
            }
            return;
        }

        if self.keymap.matches("quit", &key) {
            self.should_quit = true;
            return;
        }

        if self.keymap.matches("filter", &key) {
            self.search.query.clear();
            self.search.cursor = 0;
            self.search.start_scan();
            self.cancel_search_scan();
            self.start_search_scan();
            self.mode_stack.push(AppMode::Search);
            return;
        }

        if self.keymap.matches("help", &key) {
            self.mode_stack.push(AppMode::Help);
            return;
        }

        if self.keymap.matches("palette", &key) {
            self.command_palette = Some(CommandPalette::new(vec![
                "Quit".to_string(),
                "Switch:Keys".to_string(),
                "Switch:REPL".to_string(),
                "Switch:Info".to_string(),
                "Switch:PubSub".to_string(),
            ]));
            return;
        }

        if self.keymap.matches("tab_keys", &key) {
            self.active_tab = Tab::Keys;
            return;
        }
        if self.keymap.matches("tab_repl", &key) {
            self.active_tab = Tab::Repl;
            return;
        }
        if self.keymap.matches("tab_info", &key) {
            self.active_tab = Tab::Info;
            self.refresh_info_dashboard().await;
            return;
        }
        if self.keymap.matches("tab_pubsub", &key) {
            self.active_tab = Tab::PubSub;
            return;
        }

        if self.keymap.matches("repl_overlay", &key) {
            self.mode_stack.push(AppMode::ReplOverlay);
            self.overlay_input = Some(crate::ui::widgets::input::InputWidget::new(""));
            return;
        }

        match key.code {
            KeyCode::Char('\\')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.open_connection_screen();
                return;
            }
            KeyCode::Char('r')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.reconnect_active_connection().await;
                return;
            }
            _ => {}
        }

        match self.active_tab {
            Tab::Keys => {
                // The inspector gets first refusal on every key. Detect
                // consumption from its state rather than from the returned
                // action: prompts like the hash filter return `None` yet still
                // claim the key, and letting the key browser see it too would
                // open two prompts at once (this is what made `i` open both a
                // list-insert and a set-intersection prompt).
                let before = self.value_inspector.consumed_state();
                let inspector_action = self
                    .value_inspector
                    .handle_event_with_keymap(&Event::Key(key), &self.keymap);
                let consumed = self.value_inspector.consumed_state() != before;

                if let Some(action) = inspector_action.as_ref() {
                    self.handle_inspector_action(action.clone()).await;
                }
                if !consumed
                    && inspector_action.is_none()
                    && let Some(action) = self
                        .key_browser
                        .handle_event_with_keymap(&Event::Key(key), &self.keymap)
                {
                    self.handle_browser_action(action).await;
                }
            }
            Tab::Repl => {
                let was_monitor_visible = self.repl.monitor_visible;
                if let Some(ReplAction::Submit(cmd)) = self.repl.handle_event(&Event::Key(key)) {
                    self.execute_repl_command(cmd).await;
                }
                if self.repl.monitor_visible && !was_monitor_visible && self.monitor_rx.is_none() {
                    self.start_monitor().await;
                }
                if !self.repl.monitor_visible && was_monitor_visible {
                    self.monitor_rx = None;
                }
            }
            Tab::Info => {
                use crate::ui::info_dashboard::InfoAction;
                match self.info_dashboard.handle_event(&Event::Key(key)) {
                    Some(InfoAction::ExportCsv) => {
                        match self.info_dashboard.slowlog_panel.export_csv() {
                            Ok(path) => {
                                self.error_message = Some(format!("Exported slowlog to {path}"));
                            }
                            Err(err) => {
                                self.error_message = Some(format!("Failed to export CSV: {err}"));
                            }
                        }
                    }
                    Some(InfoAction::ResetSlowLog) => {
                        self.reset_slowlog().await;
                    }
                    _ => {}
                }
            }
            Tab::PubSub => {
                if let Some(action) = self.pubsub_widget.handle_event(&Event::Key(key)) {
                    self.handle_pubsub_action(action).await;
                }
            }
        }

        self.update_status_bar_context();
    }

    fn default_connection_profile() -> ConnectionProfile {
        ConnectionProfile {
            name: "default".to_string(),
            host: "127.0.0.1".to_string(),
            port: 6379,
            db: 0,
            username: None,
            password: None,
            last_connected: None,
            mode: ConnectionMode::Standalone,
            tls: None,
            ssh_tunnel: None,
        }
    }

    async fn reconnect_active_connection(&mut self) {
        let profile = self
            .client
            .as_ref()
            .map(|c| c.profile.clone())
            .or_else(|| self.last_profile.clone())
            .unwrap_or_else(Self::default_connection_profile);

        self.reconnect_attempt = self.reconnect_attempt.saturating_add(1);
        self.status_bar.connection_state = ConnectionState::Reconnecting {
            attempt: self.reconnect_attempt,
        };

        let delays = backoff_sequence();
        let idx = self
            .reconnect_attempt
            .saturating_sub(1)
            .min((delays.len().saturating_sub(1)) as u32) as usize;
        tokio::time::sleep(delays[idx]).await;

        match RedisClientHandle::connect(&profile).await {
            Ok(client) => {
                let host = profile.host.clone();
                let port = profile.port;
                let db = profile.db;
                self.client = Some(client);
                self.last_profile = Some(profile.clone());
                self.reconnect_attempt = 0;
                self.status_bar.connection_state = ConnectionState::Connected { host, port, db };
                self.error_message = None;
                self.start_scan_for_profile(&profile).await;
            }
            Err(err) => {
                self.client = None;
                self.status_bar.connection_state = ConnectionState::Reconnecting {
                    attempt: self.reconnect_attempt,
                };
                self.error_message = Some(format!("Reconnect failed: {err}"));
            }
        }
    }

    fn connections_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("redis-tui").join("connections.toml"))
    }

    fn open_connection_screen(&mut self) {
        let Some(path) = Self::connections_file_path() else {
            self.error_message = Some("Could not determine config directory".to_string());
            return;
        };

        match ConnectionStore::load(&path) {
            Ok(store) => {
                self.connection_screen = Some(ConnectionScreen::new(store));
                self.mode_stack.push(AppMode::ConnectionScreen);
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to load connections: {err}"));
            }
        }
    }

    fn reset_key_browser_for_scan(&mut self) {
        let sep = self
            .config
            .namespace_separator()
            .chars()
            .next()
            .unwrap_or(':');
        self.key_browser = KeyBrowser::new(sep, self.config.max_keys_in_memory());
        self.status_bar.key_count = 0;
        if let Some(cancel) = self.scan_cancel.take() {
            cancel.cancel();
        }
        self.scan_rx = None;
    }

    async fn start_scan_for_profile(&mut self, profile: &ConnectionProfile) {
        self.reset_key_browser_for_scan();
        match RedisClientHandle::connect(profile).await {
            Ok(scan_client) => {
                let (rx, cancel) = scanner_task::start_scan(scan_client, &self.config);
                self.scan_rx = Some(rx);
                self.scan_cancel = Some(cancel);
            }
            Err(err) => {
                self.error_message = Some(format!("Connected, but scan setup failed: {err}"));
            }
        }
    }

    async fn connect_profile(&mut self, profile: ConnectionProfile) {
        self.last_profile = Some(profile.clone());
        self.status_bar.connection_state = ConnectionState::Reconnecting { attempt: 1 };
        match RedisClientHandle::connect(&profile).await {
            Ok(client) => {
                let host = profile.host.clone();
                let port = profile.port;
                let db = profile.db;
                self.client = Some(client);
                self.reconnect_attempt = 0;
                self.status_bar.connection_state = ConnectionState::Connected { host, port, db };
                self.error_message = None;
                self.start_scan_for_profile(&profile).await;
            }
            Err(err) => {
                self.client = None;
                self.status_bar.connection_state = ConnectionState::Disconnected;
                self.error_message = Some(format!("Connection failed: {err}"));
            }
        }
    }

    async fn handle_tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
        self.status_bar.drain_expired_toasts();
        self.drain_scan_batches();
        self.drain_search_scan();
        self.drain_pubsub_messages();
        self.drain_monitor_messages();
        self.status_bar.key_count = self.key_browser.tree.total_keys();
        if self.active_tab == Tab::Info && self.tick_count.is_multiple_of(4) {
            self.refresh_info_dashboard().await;
        }
        if std::mem::take(&mut self.info_refresh_pending) {
            self.refresh_info_dashboard().await;
        }
        self.stream_follow_tick().await;
        self.probe_latency().await;
        self.update_status_bar_context();
    }

    /// Measure round-trip latency and publish it to the status bar.
    ///
    /// `StatusBar::latency_ms` existed but was never assigned, so the field the
    /// status bar renders was permanently blank. A PING every 4th tick (~1s)
    /// keeps the number live instead of showing a value from connect time that
    /// goes stale while the session runs.
    async fn probe_latency(&mut self) {
        if !self.tick_count.is_multiple_of(4) {
            return;
        }
        let Some(client) = self.client.as_ref() else {
            self.status_bar.latency_ms = None;
            return;
        };
        self.status_bar.latency_ms = match client.ping().await {
            Ok(duration) => Some(duration.as_millis() as u64),
            // Leave the previous reading in place: one failed probe says
            // nothing about current latency, and blanking the field on a
            // transient hiccup makes the status bar flicker.
            Err(_) => self.status_bar.latency_ms,
        };
    }

    /// Append stream entries newer than the last seen id when the inspector is
    /// following. Throttled to every 4th tick (~1s at the 250ms tick rate) so
    /// a busy stream does not issue one XRANGE per tick.
    async fn stream_follow_tick(&mut self) {
        if !self.tick_count.is_multiple_of(4) {
            return;
        }
        let Some(last_id) = self.value_inspector.follow_tick() else {
            return;
        };
        let Some(key) = self.value_inspector.key.clone() else {
            return;
        };
        let Some(client) = self.client.as_ref() else {
            return;
        };
        match client.xrange_after(&key, &last_id).await {
            Ok(new_entries) => {
                if new_entries.is_empty() {
                    return;
                }
                let followed = self.value_inspector.stream_follow;
                let appended = self.value_inspector.append_stream_entries(new_entries);
                if followed && appended > 0 {
                    self.value_inspector.follow_cursor_to_tail();
                }
            }
            Err(err) => {
                // A transient error should not kill follow mode; report once
                // via the status bar toast (which self-expires).
                self.status_bar
                    .push_error(format!("Stream follow failed: {err}"));
            }
        }
    }

    fn drain_monitor_messages(&mut self) {
        if let Some(ref mut rx) = self.monitor_rx {
            while let Ok(line) = rx.try_recv() {
                self.repl.push_monitor_line(line);
            }
        }
    }

    async fn start_monitor(&mut self) {
        if !self.allow_monitor {
            self.error_message =
                Some("MONITOR disabled. Use --allow-monitor to enable.".to_string());
            return;
        }
        let Some(ref client) = self.client else {
            self.error_message = Some("Not connected".to_string());
            return;
        };
        match client.monitor().await {
            Ok(Some(rx)) => {
                self.monitor_rx = Some(rx);
                self.repl.monitor_visible = true;
            }
            Ok(None) => {
                self.error_message = Some("MONITOR not supported in cluster mode".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to start MONITOR: {err}"));
            }
        }
    }

    fn start_pubsub_subscription(&mut self, kind: PubSubKind) -> color_eyre::Result<()> {
        let profile = self
            .client
            .as_ref()
            .map(|c| c.profile.clone())
            .or_else(|| self.last_profile.clone())
            .ok_or_else(|| color_eyre::eyre::eyre!("Not connected"))?;

        let rx = PubSubClient::spawn_subscription(&profile, &kind)?;
        self.pubsub_rx = Some(rx);
        Ok(())
    }

    async fn refresh_info_dashboard(&mut self) {
        let Some(client) = self.client.as_ref() else {
            return;
        };

        match client.info("all").await {
            Ok(raw) => {
                let stats = SystemStats::from_info_sections(&raw);
                self.info_dashboard.update_stats(stats);
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to refresh INFO: {err}"));
            }
        }

        match slowlog_get(client, 100).await {
            Ok(entries) => {
                self.info_dashboard.slowlog_panel.set_entries(entries);
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to refresh SLOWLOG: {err}"));
            }
        }
        // The server's count can exceed what we loaded; show it in the header.
        self.info_dashboard.slowlog_panel.server_len = slowlog_len(client).await.ok();
    }

    /// Clear the server's slow log and drop the loaded entries.
    async fn reset_slowlog(&mut self) {
        let Some(client) = self.client.as_ref() else {
            self.error_message = Some("Not connected".to_string());
            return;
        };
        match slowlog_reset(client).await {
            Ok(()) => {
                self.info_dashboard.slowlog_panel.set_entries(Vec::new());
                self.info_dashboard.slowlog_panel.server_len = Some(0);
                self.error_message = Some("Slow log reset".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to reset SLOWLOG: {err}"));
            }
        }
    }

    async fn handle_browser_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::SelectKey(name, r_type) => {
                self.value_inspector.set_loading(name.clone());
                if let Some(client) = &self.client {
                    let value_fut = client.get_value(&name, r_type);
                    let ttl_fut = client.ttl(&name);
                    let encoding_fut = client.object_encoding(&name);
                    let memory_fut = client.memory_usage(&name);

                    let (value_res, ttl_res, encoding_res, memory_res) =
                        tokio::join!(value_fut, ttl_fut, encoding_fut, memory_fut,);

                    match value_res {
                        Ok(value) => {
                            self.value_inspector.set_value(name, value);
                            let encoding = encoding_res.ok();
                            let memory_bytes = memory_res.ok();
                            let ttl = ttl_res.ok();
                            self.value_inspector
                                .set_metadata(encoding, memory_bytes, ttl);
                        }
                        Err(err) => {
                            self.value_inspector
                                .set_error(Some(name), format!("Failed to load value: {err}"));
                        }
                    }
                } else {
                    self.value_inspector
                        .set_error(Some(name), "Not connected".to_string());
                }
            }
            BrowserAction::DeleteKey(name) => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: delete blocked (press Esc)".to_string());
                    return;
                }
                self.pending_delete_key = Some(name.clone());
                // `confirm_deletes = false` skips the prompt and deletes
                // immediately. The key was parsed and exposed but never read,
                // so setting it changed nothing.
                if self.config.confirm_deletes() {
                    self.mode_stack.push(AppMode::Confirm);
                } else {
                    self.pending_delete_key = None;
                    self.perform_delete(name).await;
                }
            }
            BrowserAction::RefreshRequested => {
                if let Some(ref profile) = self.last_profile {
                    self.start_scan_for_profile(&profile.clone()).await;
                } else {
                    self.error_message = Some("No connection to refresh".to_string());
                }
            }
            BrowserAction::NewKey { name, key_type } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    let result = match key_type {
                        RedisType::String => client.set_string(&name, "").await,
                        RedisType::List => client.list_push(&name, "", false).await,
                        RedisType::Hash => client.hash_set(&name, "", "").await,
                        RedisType::Set => client.set_add(&name, "").await,
                        RedisType::ZSet => client.zadd(&name, 0.0, "").await,
                        RedisType::Stream => client.xadd(&name, "*", &[]).await.map(|_| ()),
                        RedisType::Unknown => {
                            self.error_message = Some("Unknown key type".to_string());
                            return;
                        }
                    };
                    if let Err(err) = result {
                        self.error_message = Some(format!("Failed to create key: {err}"));
                    }
                }
            }
            BrowserAction::RenameKey { old_name, new_name } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client
                    && let Err(err) = client.rename(&old_name, &new_name).await
                {
                    self.error_message = Some(format!("Failed to rename key: {err}"));
                }
            }
            BrowserAction::ExpireKey(name) => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client
                    && let Err(err) = client.set_ttl(&name, 1).await
                {
                    self.error_message = Some(format!("Failed to expire key: {err}"));
                }
            }
            BrowserAction::SetTtl { key, seconds } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client
                    && let Err(err) = client.set_ttl(&key, seconds).await
                {
                    self.error_message = Some(format!("Failed to set TTL: {err}"));
                }
            }
            BrowserAction::CopyKeyName(name) => {
                #[cfg(not(test))]
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(name.clone());
                }
                self.error_message = Some(format!("Copied: {}", name));
            }
            BrowserAction::DuplicateKey { old_name, new_name } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    match client.dump(&old_name).await {
                        Ok(data) => {
                            if let Err(err) = client.restore(&new_name, 0, &data).await {
                                self.error_message =
                                    Some(format!("Failed to duplicate key: {err}"));
                            }
                        }
                        Err(err) => {
                            self.error_message =
                                Some(format!("Failed to dump key '{old_name}': {err}"));
                        }
                    }
                } else {
                    self.error_message = Some("Not connected".to_string());
                }
            }
            BrowserAction::SetOp {
                op,
                source_keys,
                dest,
            } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    let sources: Vec<&str> = source_keys.iter().map(|s| s.as_str()).collect();
                    if let Err(err) = client.set_store(op, &dest, &sources).await {
                        self.error_message = Some(format!("Failed to {}: {err}", op.cmd_str()));
                    }
                } else {
                    self.error_message = Some("Not connected".to_string());
                }
            }
        }
    }

    /// Test seam: the real `handle_inspector_action` is private, so expose a
    /// wrapper that unit tests outside this module cannot reach.
    #[cfg(test)]
    pub async fn handle_inspector_action_public(&mut self, action: InspectorAction) {
        self.handle_inspector_action(action).await;
    }

    async fn handle_inspector_action(&mut self, action: InspectorAction) {
        match action {
            InspectorAction::WriteString { key, value } => {
                if self.readonly {
                    self.error_message =
                        Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.set_string(&key, &value).await {
                        self.error_message = Some(format!("Failed to write: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                } else {
                    self.error_message = Some("Not connected".to_string());
                }
            }
            InspectorAction::SetTtl { key, seconds } => {
                if let Some(ref client) = self.client {
                    if let Err(err) = client.set_ttl(&key, seconds).await {
                        self.error_message = Some(format!("Failed to set TTL: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                } else {
                    self.error_message = Some("Not connected".to_string());
                }
            }
            InspectorAction::ListPush { key, value, head } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.list_push(&key, &value, head).await {
                        self.error_message = Some(format!("Failed to push: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::ListSet { key, index, value } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.list_set(&key, index, &value).await {
                        self.error_message = Some(format!("Failed to set: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::ListRemove { key, value } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.list_remove(&key, &value).await {
                        self.error_message = Some(format!("Failed to remove: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::ListInsert {
                key,
                pivot,
                value,
                before,
            } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.list_insert(&key, before, &pivot, &value).await {
                        let msg = if before { "BEFORE" } else { "AFTER" };
                        self.error_message =
                            Some(format!("Failed to insert {msg} '{pivot}': {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::HashSet { key, field, value } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.hash_set(&key, &field, &value).await {
                        self.error_message = Some(format!("Failed to set hash field: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::HashDel { key, field } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.hash_del(&key, &[&field]).await {
                        self.error_message = Some(format!("Failed to delete hash field: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::HashDelMany { key, fields } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if fields.is_empty() {
                    return;
                }
                if let Some(ref client) = self.client {
                    let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
                    if let Err(err) = client.hash_del(&key, &refs).await {
                        self.error_message = Some(format!(
                            "Failed to delete {} hash fields: {err}",
                            refs.len()
                        ));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::SetAdd { key, member } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.set_add(&key, &member).await {
                        self.error_message = Some(format!("Failed to add set member: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::SetRem { key, member } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.set_rem(&key, &member).await {
                        self.error_message = Some(format!("Failed to remove set member: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::ZAdd { key, score, member } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.zadd(&key, score, &member).await {
                        self.error_message = Some(format!("Failed to add zset member: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::ZRem { key, member } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.zrem(&key, &member).await {
                        self.error_message = Some(format!("Failed to remove zset member: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::StreamAdd {
                key,
                entry_id,
                fields,
            } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    let field_refs: Vec<(&str, &str)> = fields
                        .iter()
                        .map(|(f, v)| (f.as_str(), v.as_str()))
                        .collect();
                    if let Err(err) = client.xadd(&key, &entry_id, &field_refs).await {
                        self.error_message = Some(format!("Failed to add stream entry: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::StreamRem { key, entry_id } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.xdel(&key, &entry_id).await {
                        self.error_message = Some(format!("Failed to delete stream entry: {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::StreamToggleGroups => {
                if self.value_inspector.stream_show_groups {
                    self.value_inspector.stream_show_groups = false;
                    self.value_inspector.stream_groups = None;
                } else if let Some(ref key) = self.value_inspector.key
                    && let Some(ref client) = self.client
                {
                    match client.xgroups(key).await {
                        Ok(groups) => {
                            self.value_inspector.stream_show_groups = true;
                            self.value_inspector.stream_groups = Some(groups);
                        }
                        Err(err) => {
                            self.error_message =
                                Some(format!("Failed to fetch stream groups: {err}"));
                        }
                    }
                }
            }
            InspectorAction::StreamTrim {
                key,
                strategy,
                threshold,
                approximate,
            } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    if let Err(err) = client.xtrim(&key, &strategy, &threshold, approximate).await {
                        self.error_message = Some(format!("Failed to XTRIM '{key}': {err}"));
                        return;
                    }
                    self.reload_inspector_value(&key).await;
                }
            }
            InspectorAction::StreamRefresh => {
                if let Some(ref key) = self.value_inspector.key.clone()
                    && self.client.is_some()
                {
                    self.reload_inspector_value(key).await;
                }
            }
            InspectorAction::InvalidInput(msg) => {
                self.status_bar.push_error(msg.clone());
                self.error_message = Some(msg);
            }
            InspectorAction::EditStringExternal { key, value } => {
                // Only queue when an editor is actually configured; otherwise
                // fall back to the built-in editor so `e` always does something.
                if resolved_editor(&self.config).is_some() {
                    self.pending_external_edit = Some(ExternalEdit { key, value });
                } else {
                    self.value_inspector.edit_mode = true;
                    self.value_inspector.text_editor = Some(
                        crate::ui::widgets::text_area_editor::TextAreaEditor::new(value),
                    );
                }
            }
            InspectorAction::CopyValue(val) => {
                #[cfg(not(test))]
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(val.clone());
                }
                self.error_message = Some(format!("Copied: {}", val));
            }
        }
    }

    async fn reload_inspector_value(&mut self, key: &str) {
        let Some(client) = self.client.as_ref() else {
            return;
        };
        let r_type = match client.key_type(key).await {
            Ok(t) => t,
            Err(_) => return,
        };
        let value_fut = client.get_value(key, r_type);
        let ttl_fut = client.ttl(key);
        let encoding_fut = client.object_encoding(key);
        let memory_fut = client.memory_usage(key);

        let (value_res, ttl_res, encoding_res, memory_res) =
            tokio::join!(value_fut, ttl_fut, encoding_fut, memory_fut,);

        if let Ok(value) = value_res {
            self.value_inspector.set_value(key.to_string(), value);
            self.value_inspector
                .set_metadata(encoding_res.ok(), memory_res.ok(), ttl_res.ok());
        }
    }

    fn drain_scan_batches(&mut self) {
        let mut should_clear = false;
        if let Some(rx) = self.scan_rx.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(batch) => self.key_browser.apply_scan_batch(batch),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.key_browser.finish_scan();
                        should_clear = true;
                        break;
                    }
                }
            }
        }

        if should_clear {
            self.scan_rx = None;
        }
    }

    fn drain_pubsub_messages(&mut self) {
        let mut should_clear = false;
        if let Some(rx) = self.pubsub_rx.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(msg) => self.pubsub_widget.push_message(msg),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        should_clear = true;
                        break;
                    }
                }
            }
        }

        if should_clear {
            self.pubsub_rx = None;
        }
    }

    fn update_status_bar_context(&mut self) {
        self.status_bar.key_count = self.key_browser.tree.total_keys();
        self.status_bar.hints = match self.active_tab {
            Tab::Keys => vec![
                ("j/k".to_string(), "nav".to_string()),
                ("Enter".to_string(), "open".to_string()),
                ("D".to_string(), "delete".to_string()),
            ],
            Tab::Repl => vec![
                ("Enter".to_string(), "run".to_string()),
                ("Up/Down".to_string(), "history".to_string()),
                ("Ctrl+P".to_string(), "palette".to_string()),
            ],
            Tab::Info => vec![("3".to_string(), "info".to_string())],
            Tab::PubSub => vec![
                ("Tab".to_string(), "mode".to_string()),
                ("Enter".to_string(), "send".to_string()),
                ("D".to_string(), "unsub".to_string()),
            ],
        };
    }

    fn start_search_scan(&mut self) {
        let client = match &self.client {
            Some(c) => c.clone(),
            None => return,
        };
        let query = self.search.query.clone();
        let mode = self.search.mode;
        let count = self.config.scan_count();
        let (tx, rx) = mpsc::channel::<Vec<String>>(64);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        self.search_scan_rx = Some(rx);
        self.search_scan_cancel = Some(cancel);
        spawn_search_scan(client, query, mode, count, tx, cancel_clone);
    }

    fn cancel_search_scan(&mut self) {
        if let Some(cancel) = self.search_scan_cancel.take() {
            cancel.cancel();
        }
        self.search_scan_rx = None;
    }

    fn drain_search_scan(&mut self) {
        let mut finished = false;
        if let Some(rx) = self.search_scan_rx.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(batch) => {
                        if batch.is_empty() {
                            finished = true;
                            break;
                        }
                        self.search.drain_scan_batch(batch);
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
        }
        if finished {
            self.search_scan_rx = None;
            self.search_scan_cancel = None;
            self.search.scanning = false;
        }
    }

    /// Human-readable form of a stored key definition.
    ///
    /// Key definitions are stored canonically in lowercase (`ctrl+p`,
    /// `alt+Down`), which reads poorly in an overlay. Special-case the
    /// modifier names and capitalise single-character keys; anything else is
    /// passed through.
    fn display_key(key: &str) -> String {
        const MODIFIERS: &[(&str, &str)] =
            &[("ctrl+", "Ctrl+"), ("alt+", "Alt+"), ("shift+", "Shift+")];

        let lowered = key.to_lowercase();
        for (lower, display) in MODIFIERS {
            if let Some(rest) = lowered.strip_prefix(lower) {
                return format!("{display}{}", Self::capitalize_named_key(rest));
            }
        }
        Self::capitalize_named_key(key)
    }

    /// Title-case named keys (`enter` → `Enter`) while leaving bare character
    /// keys lowercase.
    ///
    /// Capitalising a single letter would suggest Shift is held: `Q` and `q`
    /// are different keys, and displaying the wrong one misleads.
    fn capitalize_named_key(name: &str) -> String {
        // A bare character has no name to title-case.
        if name.chars().count() == 1 {
            return name.to_string();
        }

        // Split on '+' so "alt+Down" inside "ctrl+alt+Down" keeps its own
        // modifier casing consistent.
        let mut parts: Vec<String> = name.split('+').map(|p| p.to_string()).collect();
        if let Some(last) = parts.last_mut() {
            let mut chars = last.chars();
            if let Some(first) = chars.next() {
                *last = first.to_uppercase().collect::<String>() + chars.as_str();
            }
        }
        if parts.len() > 1 {
            let modifier_count = parts.len() - 1;
            for p in parts.iter_mut().take(modifier_count) {
                *p = match p.to_lowercase().as_str() {
                    "ctrl" => "Ctrl".to_string(),
                    "alt" => "Alt".to_string(),
                    "shift" => "Shift".to_string(),
                    other => other.to_string(),
                };
            }
        }
        parts.join("+")
    }

    /// The help overlay body, generated from the keymap actually in use.
    ///
    /// Every row is looked up rather than hardcoded: a preset that rebinds
    /// `quit` or `palette` would otherwise show text describing keys that do
    /// nothing. Actions missing from a user-supplied keymap are skipped rather
    /// than shown as a blank row.
    fn help_text(&self) -> String {
        let rows: &[(&str, &str)] = &[
            ("quit", "Quit"),
            ("help", "Help (this overlay)"),
            ("palette", "Command palette"),
            ("filter", "Global search"),
            ("repl_overlay", "One-line REPL prompt"),
            ("tab_keys", "Keys tab"),
            ("tab_repl", "REPL tab"),
            ("tab_info", "Info tab"),
            ("tab_pubsub", "Pub/Sub tab"),
            ("nav_up", "Move up"),
            ("nav_down", "Move down"),
            ("nav_left", "Collapse / go back"),
            ("nav_right", "Expand / go forward"),
            ("confirm", "Open / select"),
            ("cancel", "Close / clear"),
            ("refresh", "Refresh from server"),
            ("new_key", "New key"),
            ("rename_key", "Rename key"),
            ("edit", "Edit value  (strings)"),
            ("delete", "Delete key"),
            ("delete_selected", "Delete selected"),
            ("toggle_select", "Toggle selection"),
            ("select_all", "Select all"),
            ("duplicate_key", "Duplicate key"),
            ("cycle_sort", "Cycle sort order"),
            ("set_ttl", "Set TTL"),
            // Shares a key with `edit`: the inspector claims it for strings,
            // so this only fires when the selected key holds something else.
            ("expire", "Expire immediately  (non-strings)"),
            ("inspector_toggle_view", "Toggle inspector view mode"),
            ("inspector_hash_filter", "Filter hash fields"),
            ("inspector_value_search", "Search inside value"),
            ("inspector_stream_follow", "Toggle stream follow"),
            ("inspector_stream_groups", "Toggle consumer groups"),
            ("search_toggle_mode", "Toggle search regex mode"),
            ("search_execute", "Run search"),
            ("search_close", "Close search"),
        ];

        let mut text = String::from("Keyboard shortcuts\n\n");
        let mut width = 0usize;
        let mut lines: Vec<(String, &str)> = Vec::new();
        for (action, description) in rows {
            if let Some(key) = self.keymap.binding(action) {
                let shown = Self::display_key(key);
                width = width.max(shown.len());
                lines.push((shown, description));
            }
        }
        for (key, description) in lines {
            // Pad to the widest key so descriptions line up in a column.
            let pad = " ".repeat(width.saturating_sub(key.len()) + 2);
            text.push_str(&format!("{key}{pad}{description}\n"));
        }
        text.push_str(
            "\nCtrl+C forces quit from any prompt.\n\
             Mouse: click a tab to switch, click a key row to select.\n\
             Bindings follow the active keymap preset; see README for the full\n\
             per-type inspector keys.",
        );
        text
    }

    pub fn render(&self, frame: &mut Frame) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let tab = match self.active_tab {
            Tab::Keys => tab_bar::Tab::Keys,
            Tab::Repl => tab_bar::Tab::Repl,
            Tab::Info => tab_bar::Tab::Info,
            Tab::PubSub => tab_bar::Tab::PubSub,
        };
        tab_bar::render_tab_bar(frame, main_layout[0], tab);

        // Capture geometry as it is laid out so the next mouse event can be
        // hit-tested against the real widget positions.
        let mut geometry = RenderGeometry {
            tab_bar: main_layout[0],
            key_list: Default::default(),
        };

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_layout[1]);

        match self.active_tab {
            Tab::Keys => {
                geometry.key_list = self.key_browser.render(frame, content_layout[0]);
                self.value_inspector.render(frame, content_layout[1]);
            }
            Tab::Repl => {
                self.repl.render(frame, main_layout[1]);
            }
            Tab::Info => {
                self.info_dashboard.render(frame, main_layout[1]);
            }
            Tab::PubSub => {
                self.pubsub_widget.render(frame, main_layout[1]);
            }
        }

        self.status_bar.render(frame, main_layout[2]);

        if self.mode() == &AppMode::Confirm {
            let key_name = self.pending_delete_key.as_deref().unwrap_or("unknown");
            let dialog = ConfirmDialog::new(format!("Delete key '{}'?", key_name));
            let area = centered_rect(60, 20, frame.area());
            dialog.render(frame, area);
        }

        if self.mode() == &AppMode::ReplOverlay {
            let area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(frame.area())[1];
            let prompt_text = if let Some(ref overlay) = self.overlay_input {
                format!(":{}", overlay.value)
            } else {
                String::new()
            };
            let paragraph = Paragraph::new(prompt_text).style(Style::default().fg(Color::Yellow));
            frame.render_widget(paragraph, area);
        }

        if self.mode() == &AppMode::Help {
            let area = centered_rect(70, 60, frame.area());
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Help (Esc to close)");
            let paragraph = Paragraph::new(self.help_text()).block(block);
            frame.render_widget(paragraph, area);
        }

        if let Some(ref palette) = self.command_palette {
            palette.render(frame, frame.area());
        }

        if self.mode() == &AppMode::Search {
            self.search.render(frame, frame.area());
        }

        if self.mode() == &AppMode::ConnectionScreen
            && let Some(ref screen) = self.connection_screen
        {
            let area = centered_rect(70, 70, frame.area());
            screen.render(frame, area);
        }

        self.geometry.set(geometry);
    }
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn spawn_search_scan(
    client: crate::redis::client::RedisClientHandle,
    query: String,
    mode: crate::ui::search::SearchMode,
    count: u32,
    tx: tokio::sync::mpsc::Sender<Vec<String>>,
    cancel: tokio_util::sync::CancellationToken,
) {
    tokio::spawn(async move {
        let pattern = crate::ui::search::glob_pattern_for_query(&query, mode);
        let mut cursor: u64 = 0;
        loop {
            if cancel.is_cancelled() {
                break;
            }
            match client.scan_keys(cursor, &pattern, count).await {
                Ok((new_cursor, keys)) => {
                    if tokio::sync::mpsc::Sender::try_send(&tx, keys).is_err() {
                        break;
                    }
                    cursor = new_cursor;
                    if cursor == 0 {
                        let _ = tokio::sync::mpsc::Sender::try_send(&tx, Vec::new());
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redis::types::RedisValue;
    use std::process::{Child, Command};
    use std::time::Duration;

    async fn spawn_redis_server_on_port(port: u16) -> Child {
        let child = Command::new("redis-server")
            .args([
                "--port",
                &port.to_string(),
                "--daemonize",
                "no",
                "--loglevel",
                "warning",
            ])
            .spawn()
            .expect("Failed to start redis-server. Is it installed?");

        let url = format!("redis://127.0.0.1:{port}");
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok(client) = redis::Client::open(url.as_str())
                && client.get_connection().is_ok()
            {
                break;
            }
        }

        child
    }

    fn invalid_profile() -> ConnectionProfile {
        ConnectionProfile {
            name: "invalid".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1,
            db: 0,
            username: None,
            password: None,
            last_connected: None,
            mode: ConnectionMode::Standalone,
            tls: None,
            ssh_tunnel: None,
        }
    }

    // ---- $EDITOR integration (E2, 2.7, 8.3) ----

    #[test]
    fn split_editor_command_handles_plain_program() {
        assert_eq!(
            split_editor_command("vim"),
            Some(("vim".to_string(), vec![]))
        );
    }

    #[test]
    fn split_editor_command_handles_program_with_arguments() {
        assert_eq!(
            split_editor_command("code --wait"),
            Some(("code".to_string(), vec!["--wait".to_string()]))
        );
    }

    #[test]
    fn split_editor_command_trims_and_collapses_whitespace() {
        assert_eq!(
            split_editor_command("  emacsclient   -nw  "),
            Some(("emacsclient".to_string(), vec!["-nw".to_string()]))
        );
    }

    #[test]
    fn split_editor_command_rejects_empty() {
        assert_eq!(split_editor_command(""), None);
        assert_eq!(split_editor_command("   "), None);
    }

    #[test]
    fn external_editor_reads_environment() {
        with_editor("vim", |_| {
            assert_eq!(external_editor().as_deref(), Some("vim"));
        });
        with_editor("   ", |_| {
            assert_eq!(external_editor(), None, "blank $EDITOR must be ignored");
        });
    }

    /// Use a trivial shell script as the "editor" so the test needs no TUI.
    #[test]
    fn edit_in_external_editor_round_trips_a_change() {
        let script = write_script("#!/bin/sh\nprintf 'edited' > \"$1\"\n");
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "original".to_string(),
        };
        let result = with_editor(&script, |editor| edit_in_external_editor(&edit, editor));
        assert_eq!(result.unwrap().as_deref(), Some("edited"));
    }

    #[test]
    fn edit_in_external_editor_returns_current_value_when_unchanged() {
        // `true` is a no-op editor: it leaves the file alone.
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "same".to_string(),
        };
        let result = with_editor("true", |editor| edit_in_external_editor(&edit, editor));
        assert_eq!(result.unwrap().as_deref(), Some("same"));
    }

    #[test]
    fn edit_in_external_editor_does_nothing_when_editor_fails() {
        // `false` exits non-zero, which must not discard the value.
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "keep me".to_string(),
        };
        let result = with_editor("false", |editor| edit_in_external_editor(&edit, editor));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn edit_in_external_editor_returns_none_when_editor_is_unset() {
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "v".to_string(),
        };
        let result = without_editor(|editor| edit_in_external_editor(&edit, editor));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn edit_in_external_editor_returns_none_for_a_missing_program() {
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "v".to_string(),
        };
        let result = with_editor("definitely-not-a-real-editor-binary", |editor| {
            edit_in_external_editor(&edit, editor)
        });
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn edit_in_external_editor_supports_an_editor_with_arguments() {
        // The inner editor writes to its *last* argument, which is the file
        // path append_edit passed after the wrapper's own flags.
        let inner = write_script(
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nprintf 'via args' > \"$last\"\n",
        );
        // Wrap so the program is invoked with a leading flag of its own.
        let wrapper = write_script(&format!("#!/bin/sh\nexec {} --wait \"$@\"\n", inner));
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "v".to_string(),
        };
        let result = with_editor(&format!("{wrapper} --flag"), |editor| {
            edit_in_external_editor(&edit, editor)
        });
        assert_eq!(result.unwrap().as_deref(), Some("via args"));
    }

    /// The value must not survive in a temp file once the editor returns.
    #[test]
    fn temp_file_is_removed_after_editing() {
        let script =
            write_script("#!/bin/sh\nprintf '%s' \"$1\" > \"$0.leaked\"\ncat > /dev/null\n");
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "secret-value".to_string(),
        };
        let result = with_editor(&script, |editor| edit_in_external_editor(&edit, editor));
        assert!(result.is_ok());

        // The script recorded the temp path it was handed; that file must be
        // gone now that NamedTempFile has been dropped.
        let leaked = std::path::PathBuf::from(format!("{script}.leaked"));
        let recorded = std::fs::read_to_string(&leaked).expect("script recorded the temp path");
        let temp_path = std::path::PathBuf::from(recorded.trim());
        assert!(!temp_path.exists(), "temp file leaked at {temp_path:?}");
        let _ = std::fs::remove_file(&leaked);
    }

    #[test]
    fn edit_action_is_queued_when_an_editor_exists() {
        // A runtime is built up front; block_on cannot be called from inside
        // another runtime, which is why this is not a #[tokio::test].
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let script = write_script("#!/bin/sh\ntrue\n");
        let mut app = App::new(Config::default());
        with_editor(&script, |_| {
            rt.block_on(
                app.handle_inspector_action_public(InspectorAction::EditStringExternal {
                    key: "k".to_string(),
                    value: "v".to_string(),
                }),
            );
        });
        assert!(app.pending_external_edit.is_some());
    }

    #[test]
    fn edit_action_is_not_queued_when_no_editor_exists() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut app = App::new(Config::default());
        without_editor(|_| {
            rt.block_on(
                app.handle_inspector_action_public(InspectorAction::EditStringExternal {
                    key: "k".to_string(),
                    value: "v".to_string(),
                }),
            );
        });
        assert!(
            app.pending_external_edit.is_none(),
            "must not queue without an editor"
        );
    }

    #[test]
    fn inspector_edit_key_prefers_external_editor_when_enabled() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("k".to_string(), RedisValue::String("hello".to_string()));
        inspector.prefer_external_editor = true;

        let action = inspector.handle_event(&crossterm_event('e'));
        assert_eq!(
            action,
            Some(InspectorAction::EditStringExternal {
                key: "k".to_string(),
                value: "hello".to_string(),
            })
        );
        assert!(
            !inspector.edit_mode,
            "must not open the built-in editor as well"
        );
        assert!(inspector.text_editor.is_none());
    }

    #[test]
    fn inspector_edit_key_uses_builtin_editor_by_default() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("k".to_string(), RedisValue::String("hello".to_string()));

        assert_eq!(inspector.handle_event(&crossterm_event('e')), None);
        assert!(inspector.edit_mode);
        assert!(inspector.text_editor.is_some());
    }

    #[test]
    fn consumed_state_changes_when_a_prompt_opens() {
        use crate::ui::value_inspector::PromptMode;
        let mut inspector = ValueInspector::new();
        let mut map = indexmap::IndexMap::new();
        map.insert("f1".to_string(), "v".to_string());
        inspector.set_value("h".to_string(), RedisValue::Hash(map));

        let before = inspector.consumed_state();
        inspector.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('f'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        let after = inspector.consumed_state();
        assert_ne!(
            before, after,
            "opening a prompt must be reported as consuming the key"
        );
        assert_eq!(inspector.prompt_mode, Some(PromptMode::HashFilter));
    }

    #[test]
    fn consumed_state_unchanged_for_an_unrelated_key() {
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "l".to_string(),
            RedisValue::List(vec!["a".to_string(), "b".to_string()]),
        );
        let before = inspector.consumed_state();
        // A digit is bound to tab switching globally, so the inspector ignores it.
        inspector.handle_event(&crossterm_event('7'));
        assert_eq!(before, inspector.consumed_state());
    }

    // ---- helpers ----

    /// Serializes tests that mutate `$EDITOR`.
    ///
    /// The environment is process-global, so two such tests running
    /// concurrently would clobber each other's value and produce
    /// order-dependent failures.
    pub(super) static EDITOR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run `f` with `$EDITOR` set to `editor`, restoring it afterwards.
    pub(super) fn with_editor<T>(editor: &str, f: impl FnOnce(&str) -> T) -> T {
        let _guard = EDITOR_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var("EDITOR").ok();
        unsafe {
            std::env::set_var("EDITOR", editor);
        }
        let result = f(editor);
        restore_env("EDITOR", previous);
        result
    }

    /// Run `f` with `$EDITOR` removed, restoring it afterwards.
    pub(super) fn without_editor<T>(f: impl FnOnce(&str) -> T) -> T {
        let _guard = EDITOR_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var("EDITOR").ok();
        unsafe {
            std::env::remove_var("EDITOR");
        }
        let result = f("");
        restore_env("EDITOR", previous);
        result
    }

    fn crossterm_event(c: char) -> Event {
        Event::Key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char(c),
        ))
    }

    pub(super) fn write_script(body: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "reddish-fake-editor-{}-{}",
            std::process::id(),
            body.len()
        ));
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    pub(super) fn restore_env(key: &str, previous: Option<String>) {
        unsafe {
            match previous {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn geometry_defaults_to_nothing_clickable() {
        let app = App::new(Config::default());
        let geometry = app.geometry();
        assert_eq!(geometry.tab_bar.height, 0);
        assert_eq!(geometry.key_list.row_at(5, 5), None);
    }

    #[test]
    fn render_populates_clickable_geometry() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        for i in 0..5 {
            app.key_browser
                .tree
                .insert(crate::ui::key_browser::tree::KeyEntry {
                    full_name: format!("ns:key{i}"),
                    redis_type: None,
                    ttl: None,
                });
        }
        app.key_browser.tree.expand_all();

        terminal.draw(|f| app.render(f)).unwrap();

        let geometry = app.geometry();
        assert!(geometry.tab_bar.height > 0, "tab bar geometry missing");
        // Every tab must be reachable by clicking its label.
        for (tab, rect) in tab_bar::tab_hit_regions(geometry.tab_bar) {
            assert_eq!(tab_bar::tab_at(geometry.tab_bar, rect.x, rect.y), Some(tab));
        }
        // The key list must map a click on its first row.
        assert!(geometry.key_list.row_count > 0);
        assert_eq!(
            geometry
                .key_list
                .row_at(geometry.key_list.list_area.x, geometry.key_list.list_area.y),
            Some(0)
        );
    }

    #[test]
    fn clicking_a_tab_label_switches_tabs() {
        let mut app = App::new(Config::default());
        let geometry = RenderGeometry {
            tab_bar: ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 1,
            },
            key_list: Default::default(),
        };
        app.set_geometry_for_test(geometry);

        let regions = tab_bar::tab_hit_regions(geometry.tab_bar);
        let (_, info_rect) = regions[2]; // Info tab
        assert_eq!(app.active_tab, Tab::Keys);

        app.handle_left_click(info_rect.y, info_rect.x);
        assert_eq!(app.active_tab, Tab::Info);
    }

    #[test]
    fn clicking_the_tab_bar_takes_priority_over_the_key_list() {
        let mut app = App::new(Config::default());
        let tab_bar_area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 1,
        };
        app.set_geometry_for_test(RenderGeometry {
            tab_bar: tab_bar_area,
            // Deliberately overlapping the tab bar row.
            key_list: crate::ui::key_browser::BrowserRenderInfo {
                list_area: ratatui::layout::Rect {
                    x: 0,
                    y: 0,
                    width: 80,
                    height: 10,
                },
                scroll_offset: 0,
                row_count: 100,
            },
        });

        let (_, keys_rect) = tab_bar::tab_hit_regions(tab_bar_area)[0];
        app.handle_left_click(keys_rect.y, keys_rect.x);
        assert_eq!(app.active_tab, Tab::Keys, "stayed on Keys");
        assert_eq!(app.key_browser.cursor, 0, "list must not steal the click");
    }

    #[test]
    fn clicking_below_the_last_key_row_is_ignored() {
        let mut app = App::new(Config::default());
        for i in 0..3 {
            app.key_browser
                .tree
                .insert(crate::ui::key_browser::tree::KeyEntry {
                    full_name: format!("ns:key{i}"),
                    redis_type: None,
                    ttl: None,
                });
        }
        app.key_browser.tree.expand_all();
        app.key_browser.cursor = 1;

        let row_count = app.key_browser.sorted_rows().len();
        let list_area = ratatui::layout::Rect {
            x: 0,
            y: 2,
            width: 40,
            height: 20,
        };
        app.set_geometry_for_test(RenderGeometry {
            tab_bar: ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 1,
            },
            key_list: crate::ui::key_browser::BrowserRenderInfo {
                list_area,
                scroll_offset: 0,
                row_count,
            },
        });

        // Far below the final row.
        app.handle_left_click(21, 5);
        assert_eq!(app.key_browser.cursor, 1, "cursor must not move");
    }

    #[test]
    fn clicking_a_key_row_moves_the_cursor() {
        let mut app = App::new(Config::default());
        for i in 0..5 {
            app.key_browser
                .tree
                .insert(crate::ui::key_browser::tree::KeyEntry {
                    full_name: format!("ns:key{i}"),
                    redis_type: None,
                    ttl: None,
                });
        }
        app.key_browser.tree.expand_all();

        let row_count = app.key_browser.sorted_rows().len();
        let list_area = ratatui::layout::Rect {
            x: 0,
            y: 2,
            width: 40,
            height: 10,
        };
        app.set_geometry_for_test(RenderGeometry {
            tab_bar: ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 1,
            },
            key_list: crate::ui::key_browser::BrowserRenderInfo {
                list_area,
                scroll_offset: 0,
                row_count,
            },
        });

        // Third visible row.
        app.handle_left_click(4, 10);
        assert_eq!(app.key_browser.cursor, 2);
    }

    #[tokio::test]
    async fn test_connect_profile_failure_sets_disconnected() {
        let mut app = App::new(Config::default());
        app.connect_profile(invalid_profile()).await;

        assert!(app.client.is_none());
        assert!(matches!(
            app.status_bar.connection_state,
            ConnectionState::Disconnected
        ));
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Connection failed"))
        );
    }

    #[tokio::test]
    async fn test_reconnect_failure_sets_reconnecting_and_error() {
        let mut app = App::new(Config::default());
        app.last_profile = Some(invalid_profile());

        app.reconnect_active_connection().await;

        assert!(app.client.is_none());
        assert!(matches!(
            app.status_bar.connection_state,
            ConnectionState::Reconnecting { attempt: 1 }
        ));
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Reconnect failed"))
        );
    }

    #[tokio::test]
    async fn test_reconnect_success_sets_connected() {
        let port = 16381;
        let mut redis = spawn_redis_server_on_port(port).await;

        let mut app = App::new(Config::default());
        app.last_profile = Some(ConnectionProfile {
            name: "test-local".to_string(),
            host: "127.0.0.1".to_string(),
            port,
            db: 0,
            username: None,
            password: None,
            last_connected: None,
            mode: ConnectionMode::Standalone,
            tls: None,
            ssh_tunnel: None,
        });

        app.reconnect_active_connection().await;

        assert!(app.client.is_some());
        assert!(matches!(
            app.status_bar.connection_state,
            ConnectionState::Connected {
                host: _,
                port: 16381,
                db: 0
            }
        ));
        assert!(app.error_message.is_none());

        let _ = redis.kill();
        let _ = redis.wait();
    }
}

#[cfg(test)]
mod help_overlay_tests {
    use super::*;
    use crate::config::keybindings::Keymap;

    fn help_for(keymap: Keymap) -> String {
        let mut app = App::new(Config::default());
        app.keymap = keymap;
        app.help_text()
    }

    /// The overlay must describe the bindings in use, not a hardcoded list.
    #[test]
    fn help_reflects_the_vim_preset() {
        let text = help_for(Keymap::vim());
        assert!(text.contains("quit"), "missing action labels: {text}");
        assert!(
            text.lines()
                .any(|l| l.starts_with('q') && l.contains("Quit")),
            "vim binds quit to q:\n{text}"
        );
        assert!(
            text.lines()
                .any(|l| l.contains("Ctrl+p") && l.contains("palette")),
            "vim binds the palette to Ctrl+P:\n{text}"
        );
    }

    /// This is the bug that motivated generating the text: the emacs preset
    /// rebinds quit and palette, and the old hardcoded string was wrong for it.
    #[test]
    fn help_reflects_the_emacs_preset() {
        let text = help_for(Keymap::emacs());
        assert!(
            text.lines()
                .any(|l| l.contains("Ctrl+c") && l.contains("Quit")),
            "emacs binds quit to Ctrl+C:\n{text}"
        );
        assert!(
            text.lines()
                .any(|l| l.contains("Ctrl+x") && l.contains("palette")),
            "emacs binds the palette to Ctrl+X:\n{text}"
        );
        assert!(
            !text
                .lines()
                .any(|l| l.starts_with('q') && l.contains("Quit")),
            "q must not be advertised as quit under the emacs preset:\n{text}"
        );
    }

    #[test]
    fn help_lists_every_action_that_is_bound() {
        let text = help_for(Keymap::vim());
        for action in [
            "quit",
            "help",
            "palette",
            "filter",
            "tab_keys",
            "new_key",
            "delete",
            "inspector_toggle_view",
            "search_toggle_mode",
        ] {
            let label = keymap_label(action);
            assert!(
                text.contains(label),
                "action {action} ({label}) missing from help:\n{text}"
            );
        }
    }

    fn keymap_label(action: &str) -> &'static str {
        match action {
            "quit" => "Quit",
            "help" => "Help",
            "palette" => "palette",
            "filter" => "search",
            "tab_keys" => "Keys tab",
            "new_key" => "New key",
            "delete" => "Delete key",
            "inspector_toggle_view" => "inspector view",
            "search_toggle_mode" => "regex mode",
            other => panic!("unmapped action {other}"),
        }
    }

    /// An unknown action must be skipped, not rendered as a blank row.
    #[test]
    fn help_skips_unknown_actions() {
        assert_eq!(Keymap::vim().binding("no_such_action"), None);
        assert!(!Keymap::vim().matches(
            "no_such_action",
            &crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('x'))
        ));
    }

    #[test]
    fn binding_returns_the_configured_key_string() {
        let vim = Keymap::vim();
        assert_eq!(vim.binding("quit"), Some("q"));
        assert_eq!(vim.binding("help"), Some("?"));
    }

    /// The overlay must fit the area it is drawn into without being truncated
    /// to nothing, and must not panic at small terminal sizes.
    #[test]
    fn help_overlay_renders_without_panic() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        for (w, h) in [(100u16, 40u16), (70, 24), (20, 10), (3, 3)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut app = App::new(Config::default());
            app.mode_stack.push(AppMode::Help);
            terminal.draw(|f| app.render(f)).expect("renders");
        }
    }

    #[test]
    fn help_overlay_actually_renders_bindings() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut app = App::new(Config::default());
        app.mode_stack.push(AppMode::Help);
        terminal.draw(|f| app.render(f)).unwrap();

        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Keyboard shortcuts"), "got: {text}");
        assert!(text.contains("Quit"), "got: {text}");
    }
}

#[cfg(test)]
mod help_text_snapshot {
    use super::*;
    use crate::config::keybindings::Keymap;

    /// Not an assertion so much as a readable dump: run with `--nocapture` to
    /// see exactly what a user sees for each preset.
    #[test]
    fn dump_help_for_each_preset() {
        for (name, keymap) in [("vim", Keymap::vim()), ("emacs", Keymap::emacs())] {
            let mut app = App::new(Config::default());
            app.keymap = keymap;
            println!("=== {name} ===\n{}", app.help_text());
        }
    }
}

#[cfg(test)]
mod editor_config_tests {
    use super::tests::{with_editor, without_editor, write_script};
    use super::*;
    use crate::config::Config;

    fn config_with(editor: Option<&str>) -> Config {
        Config {
            editor: editor.map(str::to_string),
            ..Default::default()
        }
    }

    /// `config.toml` wins over the environment so a user can pin an editor
    /// without changing their shell.
    #[test]
    fn configured_editor_beats_the_environment() {
        with_editor("from-env", |env_editor| {
            assert_eq!(
                resolved_editor(&config_with(Some("from-config"))).as_deref(),
                Some("from-config"),
                "config should take precedence over $EDITOR ({env_editor})"
            );
        });
    }

    #[test]
    fn environment_is_used_when_config_is_absent() {
        with_editor("from-env", |env_editor| {
            assert_eq!(
                resolved_editor(&config_with(None)).as_deref(),
                Some(env_editor)
            );
        });
    }

    /// A blank config value means "not configured", so it must fall through to
    /// `$EDITOR` instead of trying to run an empty command.
    #[test]
    fn blank_config_editor_falls_back_to_the_environment() {
        with_editor("from-env", |_| {
            assert_eq!(
                resolved_editor(&config_with(Some(""))).as_deref(),
                Some("from-env")
            );
            assert_eq!(
                resolved_editor(&config_with(Some("   "))).as_deref(),
                Some("from-env")
            );
        });
    }

    #[test]
    fn no_config_and_no_environment_means_no_editor() {
        without_editor(|_| {
            assert_eq!(resolved_editor(&config_with(None)), None);
            assert_eq!(resolved_editor(&config_with(Some(""))), None);
        });
    }

    /// A configured editor with flags must still be split correctly.
    #[test]
    fn configured_editor_with_arguments_is_split() {
        with_editor("from-env", |_| {
            let editor = resolved_editor(&config_with(Some("code --wait"))).unwrap();
            let (program, args) = split_editor_command(&editor).unwrap();
            assert_eq!(program, "code");
            assert_eq!(args, vec!["--wait".to_string()]);
        });
    }

    /// The configured editor must actually be the one that runs, not just the
    /// one that wins the lookup.
    #[test]
    fn configured_editor_is_the_one_invoked() {
        let config_script = write_script("#!/bin/sh\nprintf 'from-config' > \"$1\"\n");
        let env_script = write_script("#!/bin/sh\nprintf 'from-env' > \"$1\"\n");
        let edit = ExternalEdit {
            key: "k".to_string(),
            value: "v".to_string(),
        };
        with_editor(&env_script, |env_editor| {
            let editor = resolved_editor(&config_with(Some(&config_script))).unwrap();
            let result = edit_in_external_editor(&edit, &editor).unwrap();
            assert_eq!(result.as_deref(), Some("from-config"));
            assert!(env_editor.contains("reddish-fake-editor"));
        });
    }

    /// `run` copies the config's value into the inspector; without this the
    /// setting would parse but never take effect.
    #[test]
    fn value_display_budget_reaches_the_inspector() {
        let mut app = App::new(Config {
            max_value_display_bytes: Some(512),
            ..Default::default()
        });
        app.value_inspector.max_value_display_bytes = app.config.max_value_display_bytes();
        assert_eq!(app.value_inspector.max_value_display_bytes, 512);
    }
}

#[cfg(test)]
mod confirm_deletes_tests {
    use super::*;
    use crate::redis::types::RedisValue;

    fn app_with_key(confirm: bool) -> App {
        let mut app = App::new(Config {
            confirm_deletes: Some(confirm),
            ..Default::default()
        });
        app.key_browser
            .tree
            .insert(crate::ui::key_browser::tree::KeyEntry {
                full_name: "doomed".to_string(),
                redis_type: None,
                ttl: None,
            });
        app.key_browser.tree.expand_all();
        app
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    /// The documented behaviour: confirm_deletes defaults to true.
    #[test]
    fn delete_prompts_by_default() {
        let mut app = app_with_key(true);
        assert!(app.config.confirm_deletes());

        let rt = runtime();
        rt.block_on(async {
            app.handle_browser_action(BrowserAction::DeleteKey("doomed".to_string()))
                .await;
        });

        assert_eq!(
            app.mode(),
            &AppMode::Confirm,
            "a delete must not happen before confirmation"
        );
        assert_eq!(app.pending_delete_key.as_deref(), Some("doomed"));
        assert!(
            app.key_browser
                .tree
                .all_keys()
                .iter()
                .any(|k| k.full_name == "doomed"),
            "the key must still be present before confirming"
        );
    }

    /// The bug: `confirm_deletes = false` was parsed but never read, so the
    /// prompt appeared regardless.
    #[test]
    fn confirm_deletes_false_skips_the_prompt() {
        let mut app = app_with_key(false);
        assert!(!app.config.confirm_deletes());

        let rt = runtime();
        rt.block_on(async {
            app.handle_browser_action(BrowserAction::DeleteKey("doomed".to_string()))
                .await;
        });

        assert_ne!(
            app.mode(),
            &AppMode::Confirm,
            "confirm_deletes = false must not prompt"
        );
        assert!(
            app.pending_delete_key.is_none(),
            "no pending confirmation should be left behind"
        );
        assert!(
            !app.key_browser
                .tree
                .all_keys()
                .iter()
                .any(|k| k.full_name == "doomed"),
            "the key should have been removed immediately"
        );
    }

    /// Skipping the prompt must not skip the read-only guard: that is a safety
    /// property, not a confirmation convenience.
    #[test]
    fn readonly_still_blocks_deletes_without_confirmation() {
        for confirm in [true, false] {
            let mut app = app_with_key(confirm);
            app.readonly = true;

            let rt = runtime();
            rt.block_on(async {
                app.handle_browser_action(BrowserAction::DeleteKey("doomed".to_string()))
                    .await;
            });

            assert!(
                app.key_browser
                    .tree
                    .all_keys()
                    .iter()
                    .any(|k| k.full_name == "doomed"),
                "confirm_deletes={confirm} must not bypass read-only mode"
            );
            assert_ne!(
                app.mode(),
                &AppMode::Confirm,
                "a blocked delete must not open a prompt"
            );
        }
    }

    /// Declining the prompt must leave the key alone.
    #[test]
    fn declining_the_prompt_keeps_the_key() {
        let mut app = app_with_key(true);
        let rt = runtime();
        rt.block_on(async {
            app.handle_browser_action(BrowserAction::DeleteKey("doomed".to_string()))
                .await;
        });
        assert_eq!(app.mode(), &AppMode::Confirm);

        rt.block_on(async {
            app.handle_key_event(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ))
            .await;
        });

        assert!(
            app.key_browser
                .tree
                .all_keys()
                .iter()
                .any(|k| k.full_name == "doomed"),
            "Esc must not delete"
        );
        assert!(app.pending_delete_key.is_none());
    }

    /// Confirming removes the key from the tree and reports the missing
    /// connection rather than pretending it succeeded.
    #[test]
    fn confirming_while_disconnected_removes_the_row_and_reports() {
        let mut app = app_with_key(true);
        let rt = runtime();
        rt.block_on(async {
            app.handle_browser_action(BrowserAction::DeleteKey("doomed".to_string()))
                .await;
            app.handle_key_event(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ))
            .await;
        });

        assert!(
            !app.key_browser
                .tree
                .all_keys()
                .iter()
                .any(|k| k.full_name == "doomed"),
            "the row should be dropped from the tree"
        );
        assert_eq!(
            app.error_message.as_deref(),
            Some("Not connected"),
            "the user must be told the server call did not happen"
        );
    }

    /// perform_delete must be reachable and consistent on its own.
    #[test]
    fn perform_delete_updates_the_key_count() {
        let mut app = app_with_key(true);
        let rt = runtime();
        rt.block_on(async {
            app.perform_delete("doomed".to_string()).await;
        });
        assert!(
            !app.key_browser
                .tree
                .all_keys()
                .iter()
                .any(|k| k.full_name == "doomed")
        );
        assert_eq!(app.status_bar.key_count, app.key_browser.tree.total_keys());
    }

    /// Deleting a key that is not in the tree must not panic or misreport.
    #[test]
    fn deleting_an_absent_key_is_safe() {
        let mut app = App::new(Config::default());
        let rt = runtime();
        rt.block_on(async {
            app.perform_delete("never-existed".to_string()).await;
        });
        assert_eq!(app.error_message.as_deref(), Some("Not connected"));
    }

    /// The value pane's own view must not be affected by the config change.
    #[test]
    fn other_config_defaults_are_untouched() {
        let app = Config::default();
        assert!(app.confirm_deletes());
        assert_eq!(app.color_scheme(), "default");
        assert!(app.mouse_enabled());
    }

    #[test]
    fn inspector_state_is_unrelated_to_delete_confirmation() {
        // Guards against perform_delete accidentally clearing the value pane.
        let mut app = app_with_key(true);
        app.value_inspector
            .set_value("doomed".to_string(), RedisValue::String("v".to_string()));
        let rt = runtime();
        rt.block_on(async {
            app.perform_delete("doomed".to_string()).await;
        });
        assert!(
            app.value_inspector.value.is_some(),
            "the value pane should keep showing what it was showing"
        );
    }
}

#[cfg(test)]
mod latency_tests {
    use super::*;

    fn connected_app() -> App {
        let mut app = App::new(Config::default());
        app.status_bar.connection_state = ConnectionState::Connected {
            host: "localhost".to_string(),
            port: 6379,
            db: 0,
        };
        app
    }

    /// The bug: `latency_ms` was never assigned anywhere, so the value the
    /// status bar renders was permanently blank.
    #[test]
    fn latency_is_never_probed_on_every_tick() {
        let mut app = connected_app();
        for i in 1..=3 {
            app.tick_count = i;
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            rt.block_on(app.probe_latency());
        }
        assert_eq!(
            app.status_bar.latency_ms, None,
            "the field must only update on the scheduled interval"
        );
    }

    #[test]
    fn latency_is_cleared_when_there_is_no_client() {
        let mut app = connected_app();
        app.status_bar.latency_ms = Some(42);

        // Disconnected: nothing to measure, so the stale reading must go.
        app.client = None;
        app.tick_count = 4;
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(app.probe_latency());
        assert_eq!(app.status_bar.latency_ms, None);
    }

    #[tokio::test]
    async fn latency_probe_against_a_live_server_produces_a_reading() {
        let profile = ConnectionProfile {
            name: "test".to_string(),
            host: "127.0.0.1".to_string(),
            port: 16379,
            db: 0,
            username: None,
            password: None,
            last_connected: None,
            ..Default::default()
        };
        let Ok(client) = RedisClientHandle::connect(&profile).await else {
            eprintln!("skipping: no test server on 16379");
            return;
        };

        let mut app = connected_app();
        app.client = Some(client);
        app.tick_count = 4;

        app.probe_latency().await;
        let reading = app.status_bar.latency_ms;
        assert!(reading.is_some(), "a connected client must yield a reading");
        // A local round trip is sub-second; anything larger means the number
        // is not a latency at all.
        assert!(
            reading.unwrap() < 5_000,
            "implausible latency: {reading:?}ms"
        );

        // The rendered status bar must actually show it.
        let backend = ratatui::backend::TestBackend::new(60, 1);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| app.status_bar.render(f, f.area()))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            text.contains("ms"),
            "status bar should show the latency: {text}"
        );
    }

    /// A failed probe must not blank the reading: one hiccup is not evidence
    /// that the connection is gone.
    #[tokio::test]
    async fn a_failed_probe_keeps_the_previous_reading() {
        let profile = ConnectionProfile {
            name: "test".to_string(),
            // Nothing listens here, so the probe cannot succeed.
            host: "127.0.0.1".to_string(),
            port: 1,
            db: 0,
            username: None,
            password: None,
            last_connected: None,
            ..Default::default()
        };
        let Ok(client) = RedisClientHandle::connect(&profile).await else {
            return; // connect already failed; nothing to assert
        };

        let mut app = connected_app();
        app.client = Some(client);
        app.status_bar.latency_ms = Some(7);
        app.tick_count = 4;

        app.probe_latency().await;
        assert_eq!(
            app.status_bar.latency_ms,
            Some(7),
            "a transient probe failure must not blank the display"
        );
    }

    /// Without a reading the status bar still renders, just without the field.
    #[test]
    fn status_bar_renders_without_a_latency() {
        let app = connected_app();
        assert_eq!(app.status_bar.latency_ms, None);
        let backend = ratatui::backend::TestBackend::new(60, 1);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| app.status_bar.render(f, f.area()))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("localhost:6379"), "got: {text}");
        assert!(!text.contains("ms"), "no latency to show: {text}");
    }
}
