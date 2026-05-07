use crossterm::event::KeyModifiers;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::events::Event;

pub struct TextAreaEditor {
    pub text: String,
    pub cursor: (usize, usize),
}

impl Default for TextAreaEditor {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextAreaEditor {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let lines: Vec<&str> = text.lines().collect();
        let cursor = if lines.is_empty() {
            (0, 0)
        } else {
            (lines.len() - 1, lines.last().unwrap().len())
        };
        Self { text, cursor }
    }

    pub fn cursor_line(&self) -> usize {
        self.cursor.0
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor.1
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            let mut lines: Vec<String> = self.text.lines().map(String::from).collect();
            if lines.is_empty() {
                lines.push(String::new());
            }
            match key.code {
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 's' => {
                    return true;
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'x' => {
                    return true;
                }
                KeyCode::Char(c) => {
                    let line_idx = self.cursor.0.min(lines.len().saturating_sub(1));
                    let col = self.cursor.1.min(lines[line_idx].len());
                    lines[line_idx].insert(col, c);
                    self.cursor.1 = col + 1;
                }
                KeyCode::Enter => {
                    let line_idx = self.cursor.0.min(lines.len().saturating_sub(1));
                    let col = self.cursor.1.min(lines[line_idx].len());
                    let remainder: String = lines[line_idx].split_off(col);
                    lines.insert(line_idx + 1, remainder);
                    self.cursor = (line_idx + 1, 0);
                }
                KeyCode::Backspace => {
                    let line_idx = self.cursor.0.min(lines.len().saturating_sub(1));
                    let col = self.cursor.1;
                    if col > 0 && col <= lines[line_idx].len() {
                        lines[line_idx].remove(col - 1);
                        self.cursor.1 = col - 1;
                    } else if line_idx > 0 {
                        let prev_len = lines[line_idx - 1].len();
                        let current = lines.remove(line_idx);
                        lines[line_idx - 1].push_str(&current);
                        self.cursor = (line_idx - 1, prev_len);
                    }
                }
                KeyCode::Left => {
                    if self.cursor.1 > 0 {
                        self.cursor.1 -= 1;
                    } else if self.cursor.0 > 0 {
                        self.cursor.0 -= 1;
                        self.cursor.1 = lines[self.cursor.0].len();
                    }
                }
                KeyCode::Right => {
                    let line_idx = self.cursor.0.min(lines.len().saturating_sub(1));
                    if self.cursor.1 < lines[line_idx].len() {
                        self.cursor.1 += 1;
                    } else if line_idx + 1 < lines.len() {
                        self.cursor.0 = line_idx + 1;
                        self.cursor.1 = 0;
                    }
                }
                KeyCode::Up if self.cursor.0 > 0 => {
                    self.cursor.0 -= 1;
                    self.cursor.1 = self.cursor.1.min(lines[self.cursor.0].len());
                }
                KeyCode::Down if self.cursor.0 + 1 < lines.len() => {
                    self.cursor.0 += 1;
                    self.cursor.1 = self.cursor.1.min(lines[self.cursor.0].len());
                }
                _ => {}
            }
            self.text = lines.join("\n");
        }
        false
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Edit (Ctrl+S save, Ctrl+X cancel)");
        let paragraph = Paragraph::new(self.text.clone())
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.cursor.0.saturating_sub(3) as u16, 0));
        frame.render_widget(paragraph, area);
    }
}
