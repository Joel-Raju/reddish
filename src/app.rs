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
use tokio::sync::mpsc;

use crate::config::Config;
use crate::events::{Event, EventHandler};
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::key_browser::{BrowserAction, KeyBrowser};
use crate::ui::repl::{ReplAction, ReplWidget};
use crate::ui::status_bar::StatusBar;
use crate::ui::tab_bar;

#[derive(Debug)]
enum AppTaskResult {
    ScanBatch(Vec<crate::ui::key_browser::tree::KeyEntry>),
    ScanFinished,
    Error(String),
}

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
    pub repl: ReplWidget,
    pub command_palette: Option<CommandPalette>,
    pub readonly: bool,
    pub error_message: Option<String>,
    pub scan_rx: Option<mpsc::Receiver<Vec<crate::ui::key_browser::tree::KeyEntry>>>,
    task_tx: mpsc::UnboundedSender<AppTaskResult>,
    task_rx: mpsc::UnboundedReceiver<AppTaskResult>,
    tick_count: u64,
}

impl App {
    pub fn new(config: Config) -> Self {
        let sep = config.namespace_separator().chars().next().unwrap_or(':');
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        Self {
            should_quit: false,
            config,
            active_tab: Tab::Keys,
            mode_stack: vec![AppMode::Normal],
            key_browser: KeyBrowser::new(sep),
            status_bar: StatusBar::default(),
            repl: ReplWidget::new(),
            command_palette: None,
            readonly: false,
            error_message: None,
            scan_rx: None,
            task_tx,
            task_rx,
            tick_count: 0,
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

            self.drain_task_results();

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
                    self.handle_browser_action(action);
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

    fn handle_tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
        self.drain_scan_batches();
        self.status_bar.key_count = self.key_browser.tree.total_keys();
        self.update_status_bar_context();
    }

    fn handle_browser_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::SelectKey(name, _r_type) => {
                self.error_message = Some(format!("Selected key: {name}"));
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
                let _ = self.task_tx.send(AppTaskResult::Error(
                    "Refresh requested but scanner wiring is not initialized".to_string(),
                ));
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

    fn drain_task_results(&mut self) {
        loop {
            match self.task_rx.try_recv() {
                Ok(AppTaskResult::ScanBatch(batch)) => self.key_browser.apply_scan_batch(batch),
                Ok(AppTaskResult::ScanFinished) => self.key_browser.finish_scan(),
                Ok(AppTaskResult::Error(msg)) => self.error_message = Some(msg),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
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
                frame.render_widget(
                    Block::default().borders(Borders::ALL).title("Value"),
                    content_layout[1],
                );
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
