use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    style::{Color, Style},
};

use crate::events::Event;
use crate::redis::client::Ttl;
use crate::redis::types::RedisValue;

#[derive(Debug, Clone, PartialEq)]
pub enum StringViewMode {
    Raw,
    Json,
    Base64,
    Hex,
}

impl StringViewMode {
    pub fn next(&self) -> Self {
        match self {
            StringViewMode::Raw => StringViewMode::Json,
            StringViewMode::Json => StringViewMode::Base64,
            StringViewMode::Base64 => StringViewMode::Hex,
            StringViewMode::Hex => StringViewMode::Raw,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            StringViewMode::Raw => "Raw",
            StringViewMode::Json => "JSON",
            StringViewMode::Base64 => "Base64",
            StringViewMode::Hex => "Hex",
        }
    }

    pub fn render(&self, value: &str) -> String {
        match self {
            StringViewMode::Raw => value.to_string(),
            StringViewMode::Json => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| value.to_string())
                } else {
                    value.to_string()
                }
            }
            StringViewMode::Base64 => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
            }
            StringViewMode::Hex => value
                .as_bytes()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InspectorAction {
    WriteString { key: String, value: String },
    SetTtl { key: String, seconds: i64 },
}

pub struct ValueInspector {
    pub key: Option<String>,
    pub value: Option<RedisValue>,
    pub loading: bool,
    pub error: Option<String>,
    pub edit_mode: bool,
    pub string_view: StringViewMode,
    pub encoding: Option<String>,
    pub memory_bytes: Option<u64>,
    pub ttl: Option<Ttl>,
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
            edit_mode: false,
            string_view: StringViewMode::Raw,
            encoding: None,
            memory_bytes: None,
            ttl: None,
        }
    }

    pub fn set_loading(&mut self, key: String) {
        self.key = Some(key);
        self.value = None;
        self.loading = true;
        self.error = None;
        self.encoding = None;
        self.memory_bytes = None;
        self.ttl = None;
    }

    pub fn set_error(&mut self, key: Option<String>, error: String) {
        self.key = key;
        self.value = None;
        self.loading = false;
        self.error = Some(error);
        self.encoding = None;
        self.memory_bytes = None;
        self.ttl = None;
    }

    pub fn set_value(&mut self, key: String, value: RedisValue) {
        self.key = Some(key);
        self.value = Some(value);
        self.loading = false;
        self.error = None;
    }

    pub fn set_metadata(&mut self, encoding: Option<String>, memory_bytes: Option<u64>, ttl: Option<Ttl>) {
        self.encoding = encoding;
        self.memory_bytes = memory_bytes;
        self.ttl = ttl;
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InspectorAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event
            && key.code == KeyCode::Tab
        {
            self.string_view = self.string_view.next();
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .key
            .as_deref()
            .map(|k| format!("Value Inspector [{k}]"))
            .unwrap_or_else(|| "Value Inspector".to_string());

        // Build metadata line
        let mut meta_parts: Vec<Span> = Vec::new();
        if let Some(ref enc) = self.encoding {
            meta_parts.push(Span::styled(
                format!(" encoding:{} ", enc),
                Style::default().fg(Color::Cyan),
            ));
        }
        if let Some(ref ttl) = self.ttl {
            let ttl_str = ttl.display();
            if !ttl_str.is_empty() {
                let ttl_color = match ttl {
                    Ttl::NoExpiry => Color::Green,
                    Ttl::Expires(d) if d.as_secs() < 60 => Color::Red,
                    Ttl::Expires(d) if d.as_secs() < 600 => Color::Yellow,
                    _ => Color::Green,
                };
                meta_parts.push(Span::styled(
                    format!(" ttl:{} ", ttl_str),
                    Style::default().fg(ttl_color),
                ));
            }
        }
        if let Some(bytes) = self.memory_bytes {
            let mem_str = if bytes >= 1024 * 1024 {
                format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
            } else if bytes >= 1024 {
                format!("{:.1}KB", bytes as f64 / 1024.0)
            } else {
                format!("{}B", bytes)
            };
            meta_parts.push(Span::styled(
                format!(" mem:{} ", mem_str),
                Style::default().fg(Color::Magenta),
            ));
        }

        let body = if self.loading {
            "Loading value...".to_string()
        } else if let Some(err) = &self.error {
            format!("Error: {err}")
        } else if let Some(value) = &self.value {
            render_value_preview(value, &self.string_view)
        } else {
            "Select a key and press Enter to inspect its value".to_string()
        };

        let mut lines = Vec::new();
        if !meta_parts.is_empty() {
            lines.push(Line::from(meta_parts));
            lines.push(Line::from(""));
        }
        for line in body.lines() {
            lines.push(Line::from(line.to_string()));
        }

        let widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
    }
}

fn render_value_preview(value: &RedisValue, view_mode: &StringViewMode) -> String {
    match value {
        RedisValue::String(s) => {
            format!("[{}]\n{}", view_mode.label(), view_mode.render(s))
        }
        RedisValue::List(items) => {
            let preview = items
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
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
            let preview = items
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
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
