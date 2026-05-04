use std::time::Duration;

use color_eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
    Frame, Terminal,
};
use std::io::Stdout;

use crate::config::Config;
use crate::events::{Event, EventHandler};
use crate::ui::key_browser::KeyBrowser;
use crate::ui::status_bar::StatusBar;
use crate::ui::tab_bar;

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
                match event {
                    Event::Key(key) => {
                        if key.code == KeyCode::Char('q') {
                            self.should_quit = true;
                        }
                    }
                    Event::Resize(_, _) => {}
                    Event::Tick => {}
                }
            }
        }

        Ok(())
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

        tab_bar::render_tab_bar(frame, main_layout[0], tab_bar::Tab::Keys);

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_layout[1]);

        self.key_browser.render(frame, content_layout[0]);

        frame.render_widget(
            Block::default().borders(Borders::ALL).title("Value"),
            content_layout[1],
        );

        self.status_bar.render(frame, main_layout[2]);
    }
}
