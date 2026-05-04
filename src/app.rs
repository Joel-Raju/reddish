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

use crate::config::Config;
use crate::events::{Event, EventHandler};
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::key_browser::KeyBrowser;
use crate::ui::repl::{ReplAction, ReplWidget};
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
    pub repl: ReplWidget,
    pub command_palette: Option<CommandPalette>,
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
            repl: ReplWidget::new(),
            command_palette: None,
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
                        if self.mode() == &AppMode::Help && key.code == KeyCode::Esc {
                            self.mode_stack.pop();
                        } else if let Some(ref mut palette) = self.command_palette {
                            if let Some(action) = palette.handle_event(&event) {
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
                        } else if self.active_tab == Tab::Repl {
                            if let Some(ReplAction::Submit(_cmd)) = self.repl.handle_event(&event) {
                                // command execution deferred
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => self.should_quit = true,
                                KeyCode::Char('?') => self.mode_stack.push(AppMode::Help),
                                KeyCode::Char('1') => self.active_tab = Tab::Keys,
                                KeyCode::Char('2') => self.active_tab = Tab::Repl,
                                KeyCode::Char('3') => self.active_tab = Tab::Info,
                                KeyCode::Char('4') => self.active_tab = Tab::PubSub,
                                _ => {}
                            }
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
