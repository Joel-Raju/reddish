use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::events::Event;

pub struct ConfirmDialog {
    pub prompt: String,
    pub confirmed: Option<bool>,
}

impl ConfirmDialog {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            confirmed: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.confirmed = Some(true),
                KeyCode::Char('n') | KeyCode::Esc => self.confirmed = Some(false),
                _ => {}
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Confirm")
            .style(Style::default().bg(Color::Black).fg(Color::White));
        let text = format!("{}\n\n[y]es / [n]o", self.prompt);
        let paragraph = Paragraph::new(text).block(block);
        let popup_area = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(paragraph, popup_area);
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
