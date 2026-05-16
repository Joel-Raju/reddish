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
use crate::ui::widgets::input::InputWidget;
use crate::ui::widgets::text_area_editor::TextAreaEditor;

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
    ListPush { key: String, value: String, head: bool },
    ListSet { key: String, index: i64, value: String },
    ListRemove { key: String, value: String },
    HashSet { key: String, field: String, value: String },
    HashDel { key: String, field: String },
    SetAdd { key: String, member: String },
    SetRem { key: String, member: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromptMode {
    Rpush,
    Lpush,
    Lset(usize),
    HashAddField,
    HashAddValue(String),
    HashEdit(String, usize),
    SetAddMember,
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
    pub text_editor: Option<TextAreaEditor>,
    pub list_cursor: usize,
    pub hash_cursor: usize,
    pub set_cursor: usize,
    pub list_prompt: Option<InputWidget>,
    pub prompt_mode: Option<PromptMode>,
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
            text_editor: None,
            list_cursor: 0,
            hash_cursor: 0,
            set_cursor: 0,
            list_prompt: None,
            prompt_mode: None,
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
        self.edit_mode = false;
        self.text_editor = None;
        self.list_cursor = 0;
        self.hash_cursor = 0;
        self.set_cursor = 0;
        self.list_prompt = None;
        self.prompt_mode = None;
    }

    pub fn set_error(&mut self, key: Option<String>, error: String) {
        self.key = key;
        self.value = None;
        self.loading = false;
        self.error = Some(error);
        self.encoding = None;
        self.memory_bytes = None;
        self.ttl = None;
        self.edit_mode = false;
        self.text_editor = None;
        self.list_cursor = 0;
        self.hash_cursor = 0;
        self.set_cursor = 0;
        self.list_prompt = None;
        self.prompt_mode = None;
    }

    pub fn set_value(&mut self, key: String, value: RedisValue) {
        self.key = Some(key);
        self.value = Some(value);
        self.loading = false;
        self.error = None;
        self.edit_mode = false;
        self.text_editor = None;
        self.list_cursor = 0;
        self.hash_cursor = 0;
        self.set_cursor = 0;
        self.list_prompt = None;
        self.prompt_mode = None;
    }

    pub fn set_metadata(&mut self, encoding: Option<String>, memory_bytes: Option<u64>, ttl: Option<Ttl>) {
        self.encoding = encoding;
        self.memory_bytes = memory_bytes;
        self.ttl = ttl;
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InspectorAction> {
        use crossterm::event::KeyCode;
        let Event::Key(key) = event else {
            return None;
        };

        if self.edit_mode {
            if let Some(ref mut editor) = self.text_editor {
                if key.code == KeyCode::Esc {
                    self.edit_mode = false;
                    self.text_editor = None;
                    return None;
                }
                if editor.handle_event(event) {
                    self.edit_mode = false;
                    let action = if editor.cancelled {
                        None
                    } else {
                        Some(InspectorAction::WriteString {
                            key: self.key.clone().unwrap_or_default(),
                            value: editor.text.clone(),
                        })
                    };
                    self.text_editor = None;
                    return action;
                }
            }
            return None;
        }

        // Handle inline input prompts (for list/hash a/p/e operations)
        if let Some(ref mut prompt) = self.list_prompt {
            prompt.handle_event(event);
            if prompt.submitted.is_some() {
                let val = prompt.submitted.take().unwrap_or_default();
                let mode = self.prompt_mode.take();
                self.list_prompt = None;
                let key = self.key.clone().unwrap_or_default();
                return match mode {
                    Some(PromptMode::Rpush) => Some(InspectorAction::ListPush {
                        key,
                        value: val,
                        head: false,
                    }),
                    Some(PromptMode::Lpush) => Some(InspectorAction::ListPush {
                        key,
                        value: val,
                        head: true,
                    }),
                    Some(PromptMode::Lset(idx)) => Some(InspectorAction::ListSet {
                        key,
                        index: idx as i64,
                        value: val,
                    }),
                    Some(PromptMode::HashAddField) => {
                        // First step done, now prompt for value
                        self.list_prompt = Some(InputWidget::new(format!("Value for '{}'", val)));
                        self.prompt_mode = Some(PromptMode::HashAddValue(val));
                        None
                    }
                    Some(PromptMode::HashAddValue(field)) => Some(InspectorAction::HashSet {
                        key,
                        field,
                        value: val,
                    }),
                    Some(PromptMode::HashEdit(field, _)) => Some(InspectorAction::HashSet {
                        key,
                        field,
                        value: val,
                    }),
                    Some(PromptMode::SetAddMember) => Some(InspectorAction::SetAdd {
                        key,
                        member: val,
                    }),
                    None => None,
                };
            }
            if prompt.cancelled {
                self.list_prompt = None;
                self.prompt_mode = None;
            }
            return None;
        }

        match key.code {
            KeyCode::Tab => {
                self.string_view = self.string_view.next();
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(RedisValue::String(ref s)) = self.value {
                    self.edit_mode = true;
                    self.text_editor = Some(TextAreaEditor::new(s.clone()));
                }
            }
            _ => {}
        }

        // List-specific key handling
        if let Some(RedisValue::List(ref items)) = self.value {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.list_cursor > 0 {
                        self.list_cursor -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.list_cursor + 1 < items.len() {
                        self.list_cursor += 1;
                    }
                }
                KeyCode::Char('a') => {
                    self.list_prompt = Some(InputWidget::new("Value to RPUSH"));
                    self.prompt_mode = Some(PromptMode::Rpush);
                }
                KeyCode::Char('p') => {
                    self.list_prompt = Some(InputWidget::new("Value to LPUSH"));
                    self.prompt_mode = Some(PromptMode::Lpush);
                }
                KeyCode::Char('D') => {
                    if let Some(val) = items.get(self.list_cursor) {
                        return Some(InspectorAction::ListRemove {
                            key: self.key.clone().unwrap_or_default(),
                            value: val.clone(),
                        });
                    }
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if let Some(val) = items.get(self.list_cursor) {
                        let mut prompt = InputWidget::new(format!("Edit [{}]", self.list_cursor));
                        prompt.value = val.clone();
                        prompt.cursor = val.len();
                        self.list_prompt = Some(prompt);
                        self.prompt_mode = Some(PromptMode::Lset(self.list_cursor));
                    }
                }
                _ => {}
            }
        }

        // Hash-specific key handling
        if let Some(RedisValue::Hash(ref entries)) = self.value {
            let fields: Vec<(&String, &String)> = entries.iter().collect();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.hash_cursor > 0 {
                        self.hash_cursor -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.hash_cursor + 1 < fields.len() {
                        self.hash_cursor += 1;
                    }
                }
                KeyCode::Char('a') => {
                    self.list_prompt = Some(InputWidget::new("Field name"));
                    self.prompt_mode = Some(PromptMode::HashAddField);
                }
                KeyCode::Char('D') => {
                    if let Some((field, _)) = fields.get(self.hash_cursor) {
                        return Some(InspectorAction::HashDel {
                            key: self.key.clone().unwrap_or_default(),
                            field: (*field).clone(),
                        });
                    }
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if let Some((field, val)) = fields.get(self.hash_cursor) {
                        let mut prompt = InputWidget::new(format!("Value for '{}'", field));
                        prompt.value = (*val).clone();
                        prompt.cursor = val.len();
                        self.list_prompt = Some(prompt);
                        self.prompt_mode = Some(PromptMode::HashEdit((*field).clone(), self.hash_cursor));
                    }
                }
                _ => {}
            }
        }

        // Set-specific key handling
        if let Some(RedisValue::Set(ref members)) = self.value {
            let items: Vec<&String> = members.iter().collect();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.set_cursor > 0 { self.set_cursor -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.set_cursor + 1 < items.len() { self.set_cursor += 1; }
                }
                KeyCode::Char('a') => {
                    self.list_prompt = Some(InputWidget::new("Member to SADD"));
                    self.prompt_mode = Some(PromptMode::SetAddMember);
                }
                KeyCode::Char('D') => {
                    if let Some(member) = items.get(self.set_cursor) {
                        return Some(InspectorAction::SetRem {
                            key: self.key.clone().unwrap_or_default(),
                            member: (*member).clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.edit_mode && let Some(ref editor) = self.text_editor {
            editor.render(frame, area);
            return;
        }

        // List-specific render
        if let Some(RedisValue::List(ref items)) = self.value {
            self.render_list(frame, area, items);
            return;
        }

        // Hash-specific render
        if let Some(RedisValue::Hash(ref entries)) = self.value {
            self.render_hash(frame, area, entries);
            return;
        }

        // Set-specific render
        if let Some(RedisValue::Set(ref members)) = self.value {
            self.render_set(frame, area, members);
            return;
        }

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

    fn render_list(&self, frame: &mut Frame, area: Rect, items: &[String]) {
        let title = self
            .key
            .as_deref()
            .map(|k| format!("List [{}] (len={})", k, items.len()))
            .unwrap_or_else(|| format!("List (len={})", items.len()));

        let mut text = String::new();
        for (i, item) in items.iter().enumerate() {
            let marker = if i == self.list_cursor { ">" } else { " " };
            text.push_str(&format!("{}[{}] {}\n", marker, i, item));
        }

        if let Some(ref prompt) = self.list_prompt {
            let prompt_label = match self.prompt_mode {
                Some(PromptMode::Rpush) => "RPUSH value:",
                Some(PromptMode::Lpush) => "LPUSH value:",
                Some(PromptMode::Lset(idx)) => &format!("LSET [{}] =", idx),
                _ => "Input:",
            };
            let prompt_text = format!(
                "{} {}{}",
                prompt_label,
                prompt.value,
                if prompt.cursor >= prompt.value.len() {
                    "_"
                } else {
                    " "
                }
            );
            text.push_str(&format!("\n{}", prompt_text));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_hash(&self, frame: &mut Frame, area: Rect, entries: &indexmap::IndexMap<String, String>) {
        let title = self
            .key
            .as_deref()
            .map(|k| format!("Hash [{}] (fields={})", k, entries.len()))
            .unwrap_or_else(|| format!("Hash (fields={})", entries.len()));

        let mut text = String::new();
        for (i, (field, val)) in entries.iter().enumerate() {
            let marker = if i == self.hash_cursor { ">" } else { " " };
            text.push_str(&format!("{}{}: {}\n", marker, field, val));
        }

        if let Some(ref prompt) = self.list_prompt {
            let prompt_label = match self.prompt_mode {
                Some(PromptMode::HashAddField) => "Field name: ",
                Some(PromptMode::HashAddValue(ref f)) => &format!("Value for '{}': ", f),
                Some(PromptMode::HashEdit(ref f, _)) => &format!("Value for '{}': ", f),
                _ => "Input: ",
            };
            text.push_str(&format!(
                "\n{}{}",
                prompt_label,
                if prompt.cursor >= prompt.value.len() {
                    format!("{}_", prompt.value)
                } else {
                    let before = &prompt.value[..prompt.cursor];
                    let after = &prompt.value[prompt.cursor + 1..];
                    format!("{}_{}{}", before, prompt.value.chars().nth(prompt.cursor).unwrap_or(' '), after)
                }
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_set(&self, frame: &mut Frame, area: Rect, members: &std::collections::BTreeSet<String>) {
        let title = self
            .key
            .as_deref()
            .map(|k| format!("Set [{}] (len={})", k, members.len()))
            .unwrap_or_else(|| format!("Set (len={})", members.len()));

        let mut text = String::new();
        let items: Vec<&String> = members.iter().collect();
        for (i, member) in items.iter().enumerate() {
            let marker = if i == self.set_cursor { ">" } else { " " };
            text.push_str(&format!("{}{}\n", marker, member));
        }

        if let Some(ref prompt) = self.list_prompt {
            text.push_str(&format!(
                "\nMember: {}{}",
                prompt.value,
                if prompt.cursor >= prompt.value.len() { "_" } else { "" }
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
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
