use std::collections::VecDeque;

use crossterm::event::KeyModifiers;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::events::Event;

#[derive(Debug, Clone, PartialEq)]
pub enum ReplLineStatus {
    Success,
    Error(String),
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplLine {
    pub input: String,
    pub output: String,
    pub status: ReplLineStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplAction {
    Submit(String),
}

pub struct ReplWidget {
    pub input: String,
    pub history: VecDeque<ReplLine>,
    pub cursor: usize,
    pub scroll: u16,
    pub max_history: usize,
}

impl Default for ReplWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplWidget {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: VecDeque::new(),
            cursor: 0,
            scroll: 0,
            max_history: 1000,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ReplAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' => {
                    self.input.clear();
                    self.cursor = 0;
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor > 0 {
                        self.input.remove(self.cursor - 1);
                        self.cursor -= 1;
                    }
                }
                KeyCode::Delete => {
                    if self.cursor < self.input.len() {
                        self.input.remove(self.cursor);
                    }
                }
                KeyCode::Left if self.cursor > 0 => self.cursor -= 1,
                KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.input.len(),
                KeyCode::Enter => {
                    let cmd = self.input.trim().to_string();
                    if !cmd.is_empty() {
                        self.history.push_back(ReplLine {
                            input: cmd.clone(),
                            output: String::new(),
                            status: ReplLineStatus::Info,
                        });
                        if self.history.len() > self.max_history {
                            self.history.pop_front();
                        }
                        self.input.clear();
                        self.cursor = 0;
                        return Some(ReplAction::Submit(cmd));
                    }
                }
                KeyCode::Up => {
                    if let Some(line) = self.history.iter().rev().nth(0) {
                        self.input = line.input.clone();
                        self.cursor = self.input.len();
                    }
                }
                KeyCode::Down => {
                    self.input.clear();
                    self.cursor = 0;
                }
                _ => {}
            }
        }
        None
    }

    pub fn add_result(&mut self, cmd: &str, output: String, status: ReplLineStatus) {
        if let Some(line) = self.history.iter_mut().rev().find(|l| l.input == cmd) {
            line.output = output;
            line.status = status;
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title("REPL");
        let history_text: String = self
            .history
            .iter()
            .rev()
            .take(area.height.saturating_sub(3) as usize)
            .map(|line| {
                let prefix = match line.status {
                    ReplLineStatus::Success => "✓ ".to_string(),
                    ReplLineStatus::Error(ref e) => format!("✗ {} ", e),
                    ReplLineStatus::Info => "› ".to_string(),
                };
                format!("{}{}\n{}", prefix, line.input, line.output)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!("{}\n> {}", history_text, self.input);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, area);
    }
}
