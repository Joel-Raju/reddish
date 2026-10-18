use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn ctrl(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    #[test]
    fn new_widget_is_empty() {
        let widget = InputWidget::new("Label: ");
        assert_eq!(widget.value, "");
        assert_eq!(widget.cursor, 0);
        assert!(widget.submitted.is_none());
        assert!(!widget.cancelled);
    }

    #[test]
    fn typing_appends_characters() {
        let mut widget = InputWidget::new("");
        for c in "hello".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        assert_eq!(widget.value, "hello");
        assert_eq!(widget.cursor, 5);
    }

    #[test]
    fn typing_in_the_middle_inserts_at_the_cursor() {
        let mut widget = InputWidget::new("");
        for c in "ac".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Left));
        widget.handle_event(&key(KeyCode::Char('b')));
        assert_eq!(widget.value, "abc");
        assert_eq!(widget.cursor, 2);
    }

    #[test]
    fn backspace_removes_the_character_before_the_cursor() {
        let mut widget = InputWidget::new("");
        for c in "abc".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Backspace));
        assert_eq!(widget.value, "ab");
        assert_eq!(widget.cursor, 2);
    }

    #[test]
    fn backspace_at_the_start_is_a_noop() {
        let mut widget = InputWidget::new("");
        widget.handle_event(&key(KeyCode::Backspace));
        assert_eq!(widget.value, "");
        assert_eq!(widget.cursor, 0);
    }

    #[test]
    fn delete_removes_the_character_at_the_cursor() {
        let mut widget = InputWidget::new("");
        for c in "abc".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Home));
        widget.handle_event(&key(KeyCode::Delete));
        assert_eq!(widget.value, "bc");
        assert_eq!(widget.cursor, 0);
    }

    #[test]
    fn arrow_keys_move_the_cursor_within_bounds() {
        let mut widget = InputWidget::new("");
        for c in "ab".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Left));
        assert_eq!(widget.cursor, 1);
        widget.handle_event(&key(KeyCode::Left));
        widget.handle_event(&key(KeyCode::Left));
        assert_eq!(widget.cursor, 0, "clamped at the start");
        widget.handle_event(&key(KeyCode::Right));
        widget.handle_event(&key(KeyCode::Right));
        widget.handle_event(&key(KeyCode::Right));
        assert_eq!(widget.cursor, 2, "clamped at the end");
    }

    #[test]
    fn home_and_end_jump_to_the_boundaries() {
        let mut widget = InputWidget::new("");
        for c in "abc".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Home));
        assert_eq!(widget.cursor, 0);
        widget.handle_event(&key(KeyCode::End));
        assert_eq!(widget.cursor, 3);
    }

    #[test]
    fn ctrl_u_clears_the_whole_value() {
        let mut widget = InputWidget::new("");
        for c in "discard me".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&ctrl('u'));
        assert_eq!(widget.value, "");
        assert_eq!(widget.cursor, 0);
    }

    #[test]
    fn enter_submits_the_current_value() {
        let mut widget = InputWidget::new("");
        for c in "GET key".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Enter));
        assert_eq!(widget.submitted.as_deref(), Some("GET key"));
        assert!(!widget.cancelled);
    }

    #[test]
    fn escape_cancels_rather_than_submitting() {
        let mut widget = InputWidget::new("");
        for c in "abandoned".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Esc));
        assert!(widget.cancelled);
        assert!(
            widget.submitted.is_none(),
            "Esc must not produce a submission"
        );
    }

    #[test]
    fn reset_clears_submission_and_value() {
        let mut widget = InputWidget::new("");
        for c in "x".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        widget.handle_event(&key(KeyCode::Enter));
        widget.reset();
        assert_eq!(widget.value, "");
        assert_eq!(widget.cursor, 0);
        assert!(widget.submitted.is_none());
        assert!(!widget.cancelled);
    }

    #[test]
    fn render_shows_the_label_and_value() {
        // The widget draws inside a bordered Block, so it needs 3 rows.
        let backend = ratatui::backend::TestBackend::new(40, 3);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut widget = InputWidget::new("Name: ");
        for c in "ada".chars() {
            widget.handle_event(&key(KeyCode::Char(c)));
        }
        terminal
            .draw(|f| widget.render(f, f.area()))
            .expect("renders");
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Name: "), "got: {text}");
        assert!(text.contains("ada"), "got: {text}");
    }

    #[test]
    fn render_does_not_panic_when_empty() {
        let backend = ratatui::backend::TestBackend::new(10, 3);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let widget = InputWidget::new("");
        terminal
            .draw(|f| widget.render(f, f.area()))
            .expect("renders");
    }
}
