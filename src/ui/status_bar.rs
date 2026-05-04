use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected { host: String, port: u16, db: u8 },
    Reconnecting { attempt: u32 },
    Disconnected,
}

pub struct StatusBar {
    pub connection_state: ConnectionState,
    pub latency_ms: Option<u64>,
    pub key_count: usize,
    pub hints: Vec<(String, String)>,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            latency_ms: None,
            key_count: 0,
            hints: Vec::new(),
        }
    }
}

impl StatusBar {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let conn_text = match &self.connection_state {
            ConnectionState::Connected { host, port, db } => {
                let lat = self.latency_ms.map(|l| format!("{}ms", l)).unwrap_or_default();
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

        let text = format!("{} | Keys: {} | {}", conn_text, self.key_count, hint_text);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::NONE))
            .style(Style::default().fg(Color::White).bg(Color::Black));
        frame.render_widget(paragraph, area);
    }
}
