use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected { host: String, port: u16, db: u8 },
    Reconnecting { attempt: u32 },
    Disconnected,
}

impl ConnectionState {
    pub fn color(&self) -> Color {
        match self {
            ConnectionState::Connected { .. } => Color::Green,
            ConnectionState::Reconnecting { .. } => Color::Yellow,
            ConnectionState::Disconnected => Color::Red,
        }
    }
}

pub struct StatusBar {
    pub connection_state: ConnectionState,
    pub latency_ms: Option<u64>,
    pub key_count: usize,
    pub hints: Vec<(String, String)>,
    pub error_notifications: VecDeque<(String, Instant)>,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            latency_ms: None,
            key_count: 0,
            hints: Vec::new(),
            error_notifications: VecDeque::new(),
        }
    }
}

impl StatusBar {
    pub fn push_error(&mut self, msg: String) {
        self.error_notifications.push_back((msg, Instant::now()));
    }

    pub fn drain_expired_toasts(&mut self) {
        let now = Instant::now();
        while let Some((_, timestamp)) = self.error_notifications.front() {
            if now.duration_since(*timestamp) > Duration::from_secs(3) {
                self.error_notifications.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let conn_text = match &self.connection_state {
            ConnectionState::Connected { host, port, db } => {
                let lat = self
                    .latency_ms
                    .map(|l| format!("{}ms", l))
                    .unwrap_or_default();
                format!("● {}:{} db{} | {}", host, port, db, lat)
            }
            ConnectionState::Reconnecting { attempt } => {
                format!("⟳ Reconnecting... (attempt {})", attempt)
            }
            ConnectionState::Disconnected => "○ Disconnected".to_string(),
        };

        let hint_text = self
            .hints
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect::<Vec<_>>()
            .join("  ");

        let mut text = format!("{} | Keys: {} | {}", conn_text, self.key_count, hint_text);

        // Show most recent non-expired error toast
        if let Some((msg, _)) = self.error_notifications.back() {
            if !text.is_empty() {
                text.push_str(" | ");
            }
            text.push_str(msg);
        }

        let conn_color = self.connection_state.color();
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::NONE))
            .style(Style::default().fg(conn_color).bg(Color::Black));
        frame.render_widget(paragraph, area);
    }
}
