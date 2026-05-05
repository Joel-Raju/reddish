use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::redis::types::RedisValue;

pub struct ValueInspector {
    pub key: Option<String>,
    pub value: Option<RedisValue>,
    pub loading: bool,
    pub error: Option<String>,
}

impl Default for ValueInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueInspector {
    pub fn new() -> Self {
        Self {
            key: None,
            value: None,
            loading: false,
            error: None,
        }
    }

    pub fn set_loading(&mut self, key: String) {
        self.key = Some(key);
        self.value = None;
        self.loading = true;
        self.error = None;
    }

    pub fn set_error(&mut self, key: Option<String>, error: String) {
        self.key = key;
        self.value = None;
        self.loading = false;
        self.error = Some(error);
    }

    pub fn set_value(&mut self, key: String, value: RedisValue) {
        self.key = Some(key);
        self.value = Some(value);
        self.loading = false;
        self.error = None;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .key
            .as_deref()
            .map(|k| format!("Value Inspector [{k}]"))
            .unwrap_or_else(|| "Value Inspector".to_string());

        let body = if self.loading {
            "Loading value...".to_string()
        } else if let Some(err) = &self.error {
            format!("Error: {err}")
        } else if let Some(value) = &self.value {
            render_value_preview(value)
        } else {
            "Select a key and press Enter to inspect its value".to_string()
        };

        let widget = Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
    }
}

fn render_value_preview(value: &RedisValue) -> String {
    match value {
        RedisValue::String(s) => s.clone(),
        RedisValue::List(items) => {
            let preview = items.iter().take(10).cloned().collect::<Vec<_>>().join("\n");
            format!("List (len={}):\n{}", items.len(), preview)
        }
        RedisValue::Hash(entries) => {
            let preview = entries
                .iter()
                .take(10)
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("Hash (fields={}):\n{}", entries.len(), preview)
        }
        RedisValue::Set(items) => {
            let preview = items.iter().take(10).cloned().collect::<Vec<_>>().join("\n");
            format!("Set (len={}):\n{}", items.len(), preview)
        }
        RedisValue::ZSet(entries) => {
            let preview = entries
                .iter()
                .take(10)
                .map(|e| format!("{} {}", e.score, e.member))
                .collect::<Vec<_>>()
                .join("\n");
            format!("ZSet (len={}):\n{}", entries.len(), preview)
        }
        RedisValue::Stream(entries) => {
            let preview = entries
                .iter()
                .take(10)
                .map(|e| format!("{} {} fields", e.id, e.fields.len()))
                .collect::<Vec<_>>()
                .join("\n");
            format!("Stream (len={}):\n{}", entries.len(), preview)
        }
    }
}
