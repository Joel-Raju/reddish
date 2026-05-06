use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::events::Event;

pub struct InputWidget {
    pub value: String,
    pub cursor: usize,
    pub label: String,
    pub submitted: Option<String>,
    pub cancelled: bool,
}

impl InputWidget {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            label: label.into(),
            submitted: None,
            cancelled: false,
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            self.handle_key(key);
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.value.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.value.len();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.clear();
                self.cursor = 0;
            }
            KeyCode::Enter => {
                self.submitted = Some(self.value.clone());
            }
            KeyCode::Esc => {
                self.cancelled = true;
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.label.clone());

        let cursor_indicator = "_";
        let text = if self.cursor >= self.value.len() {
            format!("{}{}", self.value, cursor_indicator)
        } else {
            let before = &self.value[..self.cursor];
            let after = &self.value[self.cursor + 1..];
            format!("{}{}{}", before, cursor_indicator, after)
        };

        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
    }

    pub fn reset(&mut self) {
        self.value.clear();
        self.cursor = 0;
        self.submitted = None;
        self.cancelled = false;
    }
}
