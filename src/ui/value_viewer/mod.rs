pub mod formatters;

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::events::Event;
use crate::redis::client::RedisType;
use crate::ui::value_viewer::formatters::{detect_format, FormatHint, preview};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueMode {
    Viewing,
    Editing,
    Readonly,
}

pub struct ValueViewer {
    pub key: String,
    pub redis_type: RedisType,
    pub raw: Vec<u8>,
    pub mode: ValueMode,
    pub formatted: String,
    pub dirty: bool,
    pub hint: FormatHint,
    pub scroll: u16,
}

impl ValueViewer {
    pub fn new(key: impl Into<String>, redis_type: RedisType, raw: Vec<u8>) -> Self {
        let hint = detect_format(&raw);
        let formatted = formatters::stringify(&raw, hint.clone());
        Self {
            key: key.into(),
            redis_type,
            raw,
            mode: ValueMode::Viewing,
            formatted,
            dirty: false,
            hint,
            scroll: 0,
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up if self.scroll > 0 => self.scroll -= 1,
                KeyCode::Down => self.scroll += 1,
                _ => {}
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Value ({:?}) [{}]", self.redis_type, self.key));
        let text = if self.mode == ValueMode::Editing {
            format!("[EDITING]\n{}", self.formatted)
        } else {
            self.formatted.clone()
        };
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, area);
    }

    pub fn preview(&self, max_chars: usize) -> String {
        preview(&self.raw, max_chars, self.hint.clone())
    }
}
