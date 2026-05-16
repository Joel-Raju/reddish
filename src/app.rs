use std::time::Duration;

use color_eyre::Result;
use crossterm::event::KeyCode;
use futures::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
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
use crate::redis::client::{RedisClient, RedisClientHandle, RedisType};
use crate::redis::server::slowlog_get;
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};
use crate::ui::info_dashboard::{InfoDashboard, SystemStats};
use crate::ui::key_browser::scanner_task;
use crate::ui::key_browser::{BrowserAction, KeyBrowser};
use crate::ui::pubsub::{PubSubAction, PubSubMessage, PubSubWidget};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Keys,
    Repl,
    Info,
    PubSub,
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
    pub error_message: Option<String>,
    pub client: Option<RedisClientHandle>,
    pub keymap: Keymap,
    pub startup_profile: Option<ConnectionProfile>,
    last_profile: Option<ConnectionProfile>,
    pub scan_rx: Option<mpsc::Receiver<Vec<crate::ui::key_browser::tree::KeyEntry>>>,
    pub scan_cancel: Option<CancellationToken>,
    pub pubsub_rx: Option<mpsc::UnboundedReceiver<PubSubMessage>>,
    tick_count: u64,
    reconnect_attempt: u32,
    pub pending_delete_key: Option<String>,
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
            error_message: None,
            client: None,
            keymap: Keymap::default(),
            startup_profile: None,
            last_profile: None,
            scan_rx: None,
            scan_cancel: None,
            pubsub_rx: None,
            tick_count: 0,
            reconnect_attempt: 0,
            pending_delete_key: None,
        }
    }

    async fn handle_pubsub_action(&mut self, action: PubSubAction) {
        match action {
            PubSubAction::Subscribe(channel) => {
                match self.start_pubsub_subscription(channel.clone()) {
                    Ok(()) => {
                        self.error_message = Some(format!("Subscribed to channel '{channel}'"));
                    }
                    Err(err) => {
                        self.error_message = Some(format!("Subscribe failed: {err}"));
                    }
                }
            }
            PubSubAction::Publish { channel, message } => {
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
                            self.error_message = Some(format!("Publish failed: {err}"));
                        }
                    }
                } else {
                    self.error_message = Some("Not connected".to_string());
                }
            }
        }
    }

    async fn execute_repl_command(&mut self, input: String) {
        let stages = parse_pipeline(&input);
        if stages.is_empty() {
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

        let cmd_name = tokens[0].to_uppercase();
        let args = &tokens[1..];

        let mut conn = match &client.client {
            RedisClient::Standalone(conn) => conn.clone(),
        };

        let mut cmd = redis::cmd(&cmd_name);
        for arg in args {
            cmd.arg(arg);
        }

        let value: redis::Value = tokio::time::timeout(
            Duration::from_secs(5),
            cmd.query_async::<redis::Value>(&mut conn),
        )
        .await
        .map_err(|_| format!("{cmd_name} timeout"))
        .and_then(|res| res.map_err(|e| format!("{cmd_name} error: {e}")))?;

        Ok(redis_value_to_string(&value))
    }

    pub fn mode(&self) -> &AppMode {
        self.mode_stack.last().unwrap_or(&AppMode::Normal)
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
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
        }

        Ok(())
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
                if mouse.row == 0 {
                    self.active_tab = if mouse.column < 20 {
                        Tab::Keys
                    } else if mouse.column < 35 {
                        Tab::Repl
                    } else if mouse.column < 50 {
                        Tab::Info
                    } else {
                        Tab::PubSub
                    };
                }
            }
            _ => {}
        }
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        if self.mode() == &AppMode::Confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let key_name = self.pending_delete_key.take();
                    self.mode_stack.pop();
                    if let Some(name) = key_name {
                        if self.key_browser.tree.remove(&name) {
                            self.status_bar.key_count = self.key_browser.tree.total_keys();
                        }
                        if let Some(client) = &self.client {
                            if let Err(err) = client.delete(&name).await {
                                self.error_message =
                                    Some(format!("Failed to delete key '{name}': {err}"));
                            }
                        } else {
                            self.error_message = Some("Not connected".to_string());
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.pending_delete_key = None;
                    self.mode_stack.pop();
                }
                _ => {}
            }
            return;
        }

        if self.mode() == &AppMode::Search {
            if let Some(action) = self.search.handle_event(&Event::Key(key)) {
                match action {
                    SearchAction::Execute(key_name) => {
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
                        screen.cursor = screen.cursor.min(
                            screen.store.profiles.len().saturating_sub(1),
                        );
                    }
                }
            }
            return;
        }

        if self.mode() == &AppMode::Help && key.code == KeyCode::Esc {
            self.mode_stack.pop();
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
            let mut keys = self
                .key_browser
                .tree
                .all_keys()
                .into_iter()
                .map(|k| k.full_name)
                .collect::<Vec<_>>();
            keys.sort();
            self.search.query.clear();
            self.search.cursor = 0;
            self.search.set_results(keys);
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
                let inspector_action = self.value_inspector.handle_event(&Event::Key(key));
                if self.value_inspector.edit_mode {
                    if let Some(action) = inspector_action {
                        self.handle_inspector_action(action).await;
                    }
                } else if inspector_action.is_none()
                    && let Some(action) = self
                        .key_browser
                        .handle_event_with_keymap(&Event::Key(key), &self.keymap)
                {
                    self.handle_browser_action(action).await;
                }
            }
            Tab::Repl => {
                if let Some(ReplAction::Submit(cmd)) = self.repl.handle_event(&Event::Key(key)) {
                    self.execute_repl_command(cmd).await;
                }
            }
            Tab::Info => {}
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
        self.drain_scan_batches();
        self.drain_pubsub_messages();
        self.status_bar.key_count = self.key_browser.tree.total_keys();
        if self.active_tab == Tab::Info && self.tick_count.is_multiple_of(4) {
            self.refresh_info_dashboard().await;
        }
        self.update_status_bar_context();
    }

    fn start_pubsub_subscription(&mut self, channel: String) -> color_eyre::Result<()> {
        let profile = self
            .client
            .as_ref()
            .map(|c| c.profile.clone())
            .or_else(|| self.last_profile.clone())
            .ok_or_else(|| color_eyre::eyre::eyre!("Not connected"))?;

        let url = RedisClientHandle::connection_url(&profile)
            .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel::<PubSubMessage>();
        self.pubsub_rx = Some(rx);

        tokio::spawn(async move {
            let client = match redis::Client::open(url) {
                Ok(c) => c,
                Err(_) => return,
            };

            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(_) => return,
            };

            if pubsub.subscribe(channel.clone()).await.is_err() {
                return;
            }

            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let payload = msg
                    .get_payload::<String>()
                    .unwrap_or_else(|_| "<non-utf8 payload>".to_string());

                let _ = tx.send(PubSubMessage {
                    channel: msg.get_channel_name().to_string(),
                    pattern: msg.get_pattern::<String>().ok(),
                    payload,
                    timestamp: std::time::Instant::now(),
                });
            }
        });

        Ok(())
    }

    async fn refresh_info_dashboard(&mut self) {
        let Some(client) = self.client.as_ref() else {
            return;
        };

        match client.info("all").await {
            Ok(raw) => {
                self.info_dashboard.stats = SystemStats::from_info_sections(&raw);
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to refresh INFO: {err}"));
            }
        }

        match slowlog_get(client, 100).await {
            Ok(entries) => {
                self.info_dashboard.slowlog = entries;
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to refresh SLOWLOG: {err}"));
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

                    let (value_res, ttl_res, encoding_res, memory_res) = tokio::join!(
                        value_fut,
                        ttl_fut,
                        encoding_fut,
                        memory_fut,
                    );

                    match value_res {
                        Ok(value) => {
                            self.value_inspector.set_value(name, value);
                            let encoding = encoding_res.ok();
                            let memory_bytes = memory_res.ok();
                            let ttl = ttl_res.ok();
                            self.value_inspector.set_metadata(encoding, memory_bytes, ttl);
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
                    self.error_message = Some("Read-only mode: delete blocked (press Esc)".to_string());
                    return;
                }
                self.pending_delete_key = Some(name.clone());
                self.mode_stack.push(AppMode::Confirm);
            }
            BrowserAction::RefreshRequested => {
                self.error_message =
                    Some("Refresh requested but scanner wiring is not initialized".to_string());
            }
            BrowserAction::NewKey { name, key_type } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked (press Esc)".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    let result = match key_type {
                        RedisType::String => client.set_string(&name, "").await,
                        RedisType::List => client.list_push(&name, "", false).await,
                        RedisType::Hash => client.hash_set(&name, "", "").await,
                        RedisType::Set => client.set_add(&name, "").await,
                        RedisType::ZSet => client.zadd(&name, 0.0, "").await,
                        RedisType::Stream => {
                            client.xadd(&name, "*", &[]).await.map(|_| ())
                        }
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
                    self.error_message = Some("Read-only mode: write blocked (press Esc)".to_string());
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
                    self.error_message = Some("Read-only mode: write blocked (press Esc)".to_string());
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
                    self.error_message = Some("Read-only mode: write blocked (press Esc)".to_string());
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
        }
    }

    async fn handle_inspector_action(&mut self, action: InspectorAction) {
        match action {
            InspectorAction::WriteString { key, value } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked (press Esc)".to_string());
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
            InspectorAction::StreamAdd { key, entry_id, fields } => {
                if self.readonly {
                    self.error_message = Some("Read-only mode: write blocked".to_string());
                    return;
                }
                if let Some(ref client) = self.client {
                    let field_refs: Vec<(&str, &str)> = fields.iter().map(|(f, v)| (f.as_str(), v.as_str())).collect();
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

        let (value_res, ttl_res, encoding_res, memory_res) = tokio::join!(
            value_fut,
            ttl_fut,
            encoding_fut,
            memory_fut,
        );

        if let Ok(value) = value_res {
            self.value_inspector.set_value(key.to_string(), value);
            self.value_inspector.set_metadata(
                encoding_res.ok(),
                memory_res.ok(),
                ttl_res.ok(),
            );
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
            Tab::PubSub => vec![("4".to_string(), "pubsub".to_string())],
        };
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

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_layout[1]);

        match self.active_tab {
            Tab::Keys => {
                self.key_browser.render(frame, content_layout[0]);
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
            let key_name = self
                .pending_delete_key
                .as_deref()
                .unwrap_or("unknown");
            let dialog = ConfirmDialog::new(format!("Delete key '{}'?", key_name));
            let area = centered_rect(60, 20, frame.area());
            dialog.render(frame, area);
        }

        if self.mode() == &AppMode::Help {
            let help_text = "Keyboard Shortcuts:\n\n1-4: Switch tabs\nq: Quit\n?: Help\nEnter: Select key\nj/k or arrows: Navigate\nCtrl+P: Command palette\nD: Delete";
            let area = centered_rect(70, 60, frame.area());
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Help (Esc to close)");
            let paragraph = Paragraph::new(help_text).block(block);
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

#[cfg(test)]
mod tests {
    use super::*;
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
