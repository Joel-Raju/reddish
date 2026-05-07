use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::events::Event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    Execute(String),
    Close,
}

pub struct GlobalSearch {
    pub query: String,
    pub results: Vec<String>,
    pub filtered: Vec<String>,
    pub cursor: usize,
}

impl Default for GlobalSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
        }
    }

    pub fn set_results(&mut self, results: Vec<String>) {
        self.results = results;
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = if q.is_empty() {
            self.results.clone()
        } else {
            self.results
                .iter()
                .filter(|item| item.to_lowercase().contains(&q))
                .cloned()
                .collect()
        };
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<SearchAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) => {
                    self.query.push(c);
                    self.apply_filter();
                    None
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.apply_filter();
                    None
                }
                KeyCode::Down => {
                    if self.cursor + 1 < self.filtered.len() {
                        self.cursor += 1;
                    }
                    None
                }
                KeyCode::Up => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    None
                }
                KeyCode::Enter => {
                    if let Some(selected) = self.filtered.get(self.cursor) {
                        Some(SearchAction::Execute(selected.clone()))
                    } else {
                        Some(SearchAction::Execute(self.query.clone()))
                    }
                }
                KeyCode::Esc => Some(SearchAction::Close),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 50, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Global Search (Esc to close)");
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let query_widget = Paragraph::new(format!("> {}", self.query));
        frame.render_widget(query_widget, inner);

        if !self.filtered.is_empty() {
            let preview = self
                .filtered
                .iter()
                .take(8)
                .enumerate()
                .map(|(idx, item)| {
                    if idx == self.cursor {
                        format!("> {item}")
                    } else {
                        format!("  {item}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            let result_area = Rect {
                x: inner.x,
                y: inner.y.saturating_add(2),
                width: inner.width,
                height: inner.height.saturating_sub(2),
            };
            frame.render_widget(Paragraph::new(preview), result_area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
