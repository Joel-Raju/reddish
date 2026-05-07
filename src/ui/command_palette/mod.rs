use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::events::Event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Execute(String),
    Close,
}

pub struct CommandPalette {
    pub query: String,
    pub items: Vec<String>,
    pub filtered: Vec<String>,
    pub cursor: usize,
}

impl CommandPalette {
    pub fn new(items: Vec<String>) -> Self {
        let filtered = items.clone();
        Self {
            query: String::new(),
            items,
            filtered,
            cursor: 0,
        }
    }

    pub fn filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .filter(|s| s.to_lowercase().contains(&q))
            .cloned()
            .collect();
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<PaletteAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) => {
                    self.query.push(c);
                    self.filter();
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.filter();
                }
                KeyCode::Down if self.cursor + 1 < self.filtered.len() => {
                    self.cursor += 1;
                }
                KeyCode::Up if self.cursor > 0 => {
                    self.cursor -= 1;
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self.filtered.get(self.cursor) {
                        return Some(PaletteAction::Execute(cmd.clone()));
                    }
                }
                KeyCode::Esc => return Some(PaletteAction::Close),
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Command Palette (Ctrl+P)");
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let query_area = Rect { height: 1, ..inner };
        let list_area = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };

        let query_widget = Paragraph::new(format!("> {}", self.query));
        frame.render_widget(query_widget, query_area);

        let list_items: Vec<ListItem> = self
            .filtered
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let style = if i == self.cursor {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(s.as_str()).style(style)
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(self.cursor));
        let list = List::new(list_items);
        frame.render_stateful_widget(list, list_area, &mut state);
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
