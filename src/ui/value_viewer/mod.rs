pub mod formatters;

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::redis::client::RedisType;
use formatters::{detect_format, stringify, FormatHint};

pub struct ValueViewer {
    pub key: String,
    pub redis_type: RedisType,
    pub data: Vec<u8>,
    pub format_hint: FormatHint,
}

impl ValueViewer {
    pub fn new(key: &str, redis_type: RedisType, data: Vec<u8>) -> Self {
        let format_hint = detect_format(&data);
        Self {
            key: key.to_string(),
            redis_type,
            data,
            format_hint,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let text = stringify(&self.data, self.format_hint.clone());
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(self.key.clone()))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}
