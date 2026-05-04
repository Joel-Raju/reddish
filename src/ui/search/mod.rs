use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
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
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<SearchAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) => {
                    self.query.push(c);
                    None
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    None
                }
                KeyCode::Enter => Some(SearchAction::Execute(self.query.clone())),
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
