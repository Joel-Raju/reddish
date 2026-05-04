use std::collections::VecDeque;

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

#[derive(Debug, Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub pattern: Option<String>,
    pub payload: String,
    pub timestamp: std::time::Instant,
}

pub struct PubSubWidget {
    pub channels: Vec<String>,
    pub messages: VecDeque<PubSubMessage>,
    pub scroll: u16,
}

impl Default for PubSubWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSubWidget {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            messages: VecDeque::new(),
            scroll: 0,
        }
    }

    pub fn push_message(&mut self, msg: PubSubMessage) {
        if self.messages.len() > 1000 {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title("Pub/Sub");
        let text: String = self
            .messages
            .iter()
            .rev()
            .take(area.height.saturating_sub(2) as usize)
            .map(|m| {
                format!(
                    "[{}] {}: {}",
                    if let Some(ref p) = m.pattern {
                        format!("pattern {}", p)
                    } else {
                        m.channel.clone()
                    },
                    m.timestamp.elapsed().as_secs(),
                    m.payload
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, area);
    }
}
