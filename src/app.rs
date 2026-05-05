use std::time::Duration;

use color_eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io::Stdout;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::backoff::backoff_sequence;
use crate::config::Config;
use crate::config::connections::{ConnectionMode, ConnectionProfile, ConnectionStore};
use crate::events::{Event, EventHandler};
use crate::redis::client::RedisClientHandle;
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::connection_screen::{ConnectionScreen, ConnectionScreenAction};
use crate::ui::key_browser::scanner_task;
use crate::ui::key_browser::{BrowserAction, KeyBrowser};
use crate::ui::repl::{ReplAction, ReplWidget};
use crate::ui::status_bar::{ConnectionState, StatusBar};
use crate::ui::tab_bar;
use crate::ui::value_inspector::ValueInspector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Repl,
    Help,
    Search,
    Confirm,
    ConnectionScreen,
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
    pub repl: ReplWidget,
    pub command_palette: Option<CommandPalette>,
    pub connection_screen: Option<ConnectionScreen>,
    pub readonly: bool,
    pub error_message: Option<String>,
    pub client: Option<RedisClientHandle>,
    last_profile: Option<ConnectionProfile>,
    pub scan_rx: Option<mpsc::Receiver<Vec<crate::ui::key_browser::tree::KeyEntry>>>,
    tick_count: u64,
    reconnect_attempt: u32,
}

impl App {
    pub fn new(config: Config) -> Self {
        let sep = config.namespace_separator().chars().next().unwrap_or(':');
        Self {
            should_quit: false,
            config,
            active_tab: Tab::Keys,
            mode_stack: vec![AppMode::Normal],
            key_browser: KeyBrowser::new(sep),
            status_bar: StatusBar::default(),
            value_inspector: ValueInspector::new(),
            repl: ReplWidget::new(),
            command_palette: None,
            connection_screen: None,
            readonly: false,
            error_message: None,
            client: None,
            last_profile: None,
            scan_rx: None,
            tick_count: 0,
            reconnect_attempt: 0,
        }
    }

    pub fn mode(&self) -> &AppMode {
        self.mode_stack.last().unwrap_or(&AppMode::Normal)
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
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
            Event::Tick => self.handle_tick(),
            Event::Resize(_, _) => {}
        }
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
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
                    ConnectionScreenAction::Save(_) | ConnectionScreenAction::Delete(_) => {}
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

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.mode_stack.push(AppMode::Help);
                return;
            }
            KeyCode::Char('1') => self.active_tab = Tab::Keys,
            KeyCode::Char('2') => self.active_tab = Tab::Repl,
            KeyCode::Char('3') => self.active_tab = Tab::Info,
            KeyCode::Char('4') => self.active_tab = Tab::PubSub,
            KeyCode::Char('\\')
                if key.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.open_connection_screen();
                return;
            }
            KeyCode::Char('r')
                if key.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.reconnect_active_connection().await;
                return;
            }
            KeyCode::Char('p')
                if key.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.command_palette = Some(CommandPalette::new(vec![
                    "Quit".to_string(),
                    "Switch:Keys".to_string(),
                    "Switch:REPL".to_string(),
                    "Switch:Info".to_string(),
                    "Switch:PubSub".to_string(),
                ]));
                return;
            }
            _ => {}
        }

        match self.active_tab {
            Tab::Keys => {
                if let Some(action) = self.key_browser.handle_event(&Event::Key(key)) {
                    self.handle_browser_action(action).await;
                }
            }
            Tab::Repl => {
                if let Some(ReplAction::Submit(_cmd)) = self.repl.handle_event(&Event::Key(key)) {
                    self.error_message = Some("REPL execution pipeline not wired yet".to_string());
                }
            }
            Tab::Info | Tab::PubSub => {}
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
                self.status_bar.connection_state = ConnectionState::Connected {
                    host,
                    port,
                    db,
                };
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
        self.key_browser = KeyBrowser::new(sep);
        self.status_bar.key_count = 0;
        self.scan_rx = None;
    }

    async fn start_scan_for_profile(&mut self, profile: &ConnectionProfile) {
        self.reset_key_browser_for_scan();
        match RedisClientHandle::connect(profile).await {
            Ok(scan_client) => {
                self.scan_rx = Some(scanner_task::start_scan(scan_client, &self.config));
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
                self.status_bar.connection_state = ConnectionState::Connected {
                    host,
                    port,
                    db,
                };
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

    fn handle_tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
        self.drain_scan_batches();
        self.status_bar.key_count = self.key_browser.tree.total_keys();
        self.update_status_bar_context();
    }

    async fn handle_browser_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::SelectKey(name, r_type) => {
                self.value_inspector.set_loading(name.clone());
                if let Some(client) = &self.client {
                    match client.get_value(&name, r_type).await {
                        Ok(value) => self.value_inspector.set_value(name, value),
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
                    self.error_message = Some("Read-only mode: delete blocked".to_string());
                    return;
                }

                if self.key_browser.tree.remove(&name) {
                    self.status_bar.key_count = self.key_browser.tree.total_keys();
                }
            }
            BrowserAction::RefreshRequested => {
                self.error_message = Some(
                    "Refresh requested but scanner wiring is not initialized".to_string(),
                );
            }
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
            _ => {
                frame.render_widget(
                    Block::default().borders(Borders::ALL).title("Placeholder"),
                    main_layout[1],
                );
            }
        }

        self.status_bar.render(frame, main_layout[2]);

        if self.mode() == &AppMode::Help {
            let help_text = "Keyboard Shortcuts:\n\n1-4: Switch tabs\nq: Quit\n?: Help\nEnter: Select key\nj/k or arrows: Navigate\nCtrl+P: Command palette\nD: Delete";
            let area = centered_rect(70, 60, frame.area());
            let block = Block::default().borders(Borders::ALL).title("Help (Esc to close)");
            let paragraph = Paragraph::new(help_text).block(block);
            frame.render_widget(paragraph, area);
        }

        if let Some(ref palette) = self.command_palette {
            palette.render(frame, frame.area());
        }

        if self.mode() == &AppMode::ConnectionScreen
            && let Some(ref screen) = self.connection_screen
        {
            let area = centered_rect(70, 70, frame.area());
            screen.render(frame, area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
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
        let mut child = Command::new("redis-server")
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
        assert!(matches!(app.status_bar.connection_state, ConnectionState::Disconnected));
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
