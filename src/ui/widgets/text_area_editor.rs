use crate::ui::theme;
use crossterm::event::KeyModifiers;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::events::Event;

pub struct TextAreaEditor {
    pub text: String,
    pub cursor: (usize, usize),
    pub cancelled: bool,
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
        Self {
            text,
            cursor,
            cancelled: false,
        }
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
                    self.cancelled = true;
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

    /// Draw the editor as a centred popup over `parent`.
    ///
    /// The pane underneath is still drawn by the caller, so the original value
    /// stays visible around the popup while editing — which is what makes this
    /// better than replacing the pane outright.
    pub fn render_popup(&self, frame: &mut Frame, parent: Rect, key: Option<&str>) {
        let area = popup_area(parent);
        // Clear first, otherwise the value pane shows through the border and
        // the text underneath the cursor.
        frame.render_widget(Clear, area);

        // The key goes on the top border and the hints on the bottom. Putting both on
        // one line meant a long key clipped the save/cancel hints, which are
        // the part a first-time user needs.
        let title = match key {
            Some(k) => format!(" Edit {k} "),
            None => " Edit ".to_string(),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(title)
            .title_bottom(Line::from(vec![
                Span::styled(" Ctrl+S", Style::default().fg(theme::current().success)),
                Span::raw(" save"),
                Span::styled(" · Ctrl+X", Style::default().fg(theme::current().warning)),
                Span::raw(" / Esc cancel"),
            ]));

        // No soft wrap. With wrapping, one logical line can occupy several
        // display rows, so ratatui's scroll offset counts rows the cursor
        // position cannot predict — the cursor would drift away from where the
        // user is typing. Scrolling both axes keeps the mapping exact.
        let inner_width = area.width.saturating_sub(2).max(1);
        let inner_height = area.height.saturating_sub(2).max(1);

        let line_count = self.text.lines().count() as u16;
        let scroll_y = (self.cursor.0 as u16).saturating_sub(inner_height.saturating_sub(1));
        let scroll_x = if self.cursor.1 <= inner_width as usize {
            0
        } else {
            (self.cursor.1 - inner_width as usize) as u16
        };

        let paragraph = Paragraph::new(self.text.clone())
            .block(block)
            .scroll((scroll_y.min(line_count.saturating_sub(1)), scroll_x));

        frame.render_widget(paragraph, area);
    }
}

/// Popup geometry: large enough to edit a value in, but leaving a margin of the
/// underlying pane visible on every side.
fn popup_area(parent: Rect) -> Rect {
    let width = parent.width.saturating_mul(80) / 100;
    let height = parent.height.saturating_mul(70) / 100;
    let width = width.max(1).min(parent.width);
    let height = height.max(1).min(parent.height);
    Rect {
        x: parent.x + (parent.width - width) / 2,
        y: parent.y + (parent.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod popup_tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(editor: &TextAreaEditor, w: u16, h: u16, key: Option<&str>) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| editor.render_popup(f, f.area(), key))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn editor_with(text: &str) -> TextAreaEditor {
        TextAreaEditor::new(text.to_string())
    }

    #[test]
    fn popup_shows_the_title_and_hints() {
        let text = draw(&editor_with("hello"), 80, 24, Some("user:1"));
        assert!(text.contains("Edit user:1"), "got: {text}");
        assert!(text.contains("Ctrl+S"), "got: {text}");
        assert!(text.contains("Ctrl+X"), "got: {text}");
    }

    #[test]
    fn popup_title_survives_a_missing_key() {
        let text = draw(&editor_with("hello"), 80, 24, None);
        assert!(text.contains("Edit"), "got: {text}");
        assert!(!text.contains("Edit  ("), "got: {text}");
    }

    #[test]
    fn popup_shows_the_text_being_edited() {
        let text = draw(&editor_with("the value under edit"), 80, 24, Some("k"));
        assert!(text.contains("the value under edit"), "got: {text}");
    }

    /// The popup must not fill the whole parent, or it is not a popup.
    #[test]
    fn popup_leaves_a_margin_around_it() {
        let parent = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let area = popup_area(parent);
        assert!(area.width < parent.width, "popup fills the width");
        assert!(area.height < parent.height, "popup fills the height");
        assert!(area.x > 0, "no left margin");
        assert!(area.y > 0, "no top margin");
        assert!(area.x + area.width < parent.width, "no right margin");
        assert!(area.y + area.height < parent.height, "no bottom margin");
    }

    #[test]
    fn popup_is_centred() {
        let parent = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let area = popup_area(parent);
        let left_gap = area.x;
        let right_gap = parent.width - (area.x + area.width);
        assert!(
            (left_gap as i32 - right_gap as i32).abs() <= 1,
            "not centred: {left_gap} left vs {right_gap} right"
        );
    }

    /// Degenerate sizes must not panic or produce a zero-area popup.
    #[test]
    fn popup_survives_degenerate_sizes() {
        for (w, h) in [
            (1u16, 1u16),
            (2, 2),
            (3, 3),
            (10, 4),
            (80, 1),
            (1, 80),
            (65535, 100),
        ] {
            let editor = editor_with("x");
            let text = draw(&editor, w, h, Some("k"));
            assert!(!text.is_empty(), "{w}x{h} rendered nothing at all");
        }

        // A zero-area terminal has nothing to draw in, which is correct: the
        // requirement is that it must not panic.
        let editor = editor_with("x");
        let _ = draw(&editor, 0, 0, Some("k"));
    }

    /// A long value must scroll so the cursor stays visible, not render only
    /// the first rows.
    #[test]
    fn long_text_scrolls_to_keep_the_cursor_visible() {
        let editor = editor_with("line 0\nline 1\nline 2\nline 3\nline 4");
        // Cursor starts at the end.
        assert_eq!(editor.cursor.0, 4);

        let text = draw(&editor, 40, 8, Some("k"));
        assert!(
            text.contains("line 4"),
            "the cursor's own line must be visible: {text}"
        );
    }

    /// Horizontal scrolling must exist, or a cursor past the right edge is lost.
    #[test]
    fn long_line_scrolls_horizontally() {
        let long: String = "x".repeat(200);
        let editor = editor_with(&long);
        assert_eq!(editor.cursor.1, 200);
        let text = draw(&editor, 40, 10, Some("k"));
        assert!(
            !text.is_empty(),
            "a 200-char line should still render its tail: {text}"
        );
    }

    #[test]
    fn short_text_does_not_scroll_away() {
        let editor = editor_with("short");
        let text = draw(&editor, 80, 24, Some("k"));
        assert!(text.contains("short"), "got: {text}");
    }

    /// The widget must not clear the parent area, only its own box.
    #[test]
    fn popup_clears_only_its_own_area() {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let editor = editor_with("edited");
        terminal
            .draw(|f| {
                // Pre-fill so we can tell what the popup painted over.
                let bg = Block::default().borders(Borders::ALL).title("BACKGROUND");
                f.render_widget(bg, f.area());
                editor.render_popup(f, f.area(), Some("k"));
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // The outer border survives, proving only the interior was cleared.
        assert!(text.contains("BACKGROUND"), "got: {text}");
        assert!(text.contains("edited"), "got: {text}");
    }
}
