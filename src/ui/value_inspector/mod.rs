use ratatui::{
    Frame,
    layout::{Direction, Layout, Rect, Constraint},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Table, Row as TableRow, Cell},
    style::{Color, Style},
};

use crate::events::Event;
use crate::redis::client::Ttl;
use crate::redis::types::{RedisValue, StreamEntry, StreamGroup, ZSetEntry};
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
    /// Render as styled lines. Only `Json` produces color; every other mode
    /// yields unstyled lines.
    pub fn render_lines(&self, value: &str) -> Vec<Line<'static>> {
        match self {
            StringViewMode::Json => match serde_json::from_str::<serde_json::Value>(value) {
                Ok(parsed) => highlight_json(&parsed),
                // Not valid JSON — fall back to plain text so the value is
                // still readable rather than replaced with an error.
                Err(_) => vec![Line::raw(value.to_string())],
            },
            other => {
                let rendered = other.render(value);
                rendered.lines().map(|l| Line::raw(l.to_string())).collect()
            }
        }
    }

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

/// Colors used for JSON syntax highlighting.
const JSON_KEY_COLOR: Color = Color::Yellow;
const JSON_STRING_COLOR: Color = Color::Green;
const JSON_NUMBER_COLOR: Color = Color::Cyan;
const JSON_LITERAL_COLOR: Color = Color::Magenta;
const JSON_PUNCT_COLOR: Color = Color::DarkGray;

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

/// True when a value is a container that should be expanded across lines.
/// Empty containers stay inline as `{}` / `[]`.
fn json_is_expanded(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(m) => !m.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        _ => false,
    }
}

fn json_open_token(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Array(_) => "[",
        _ => "{",
    }
}

fn json_close_token(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Array(_) => "]",
        _ => "}",
    }
}

/// Color a leaf value. Empty containers fall through to the punctuation branch
/// and render inline as `{}` / `[]`.
fn json_scalar_spans(value: &serde_json::Value) -> Vec<Span<'static>> {
    match value {
        serde_json::Value::String(s) => vec![styled(
            serde_json::to_string(s).unwrap_or_default(),
            JSON_STRING_COLOR,
        )],
        serde_json::Value::Number(n) => vec![styled(n.to_string(), JSON_NUMBER_COLOR)],
        serde_json::Value::Bool(_) | serde_json::Value::Null => {
            vec![styled(value.to_string(), JSON_LITERAL_COLOR)]
        }
        _ => vec![
            styled(json_open_token(value), JSON_PUNCT_COLOR),
            styled(json_close_token(value), JSON_PUNCT_COLOR),
        ],
    }
}

/// Pretty-print a parsed JSON value with per-token coloring.
///
/// Keys are yellow, strings green, numbers cyan, `true`/`false`/`null`
/// magenta, and structural punctuation dark gray so the shape of the
/// document stays readable without competing with the values.
///
/// Returns real `Line`s rather than spans containing `\n` — ratatui does not
/// break lines on embedded newlines within a span.
fn highlight_json(value: &serde_json::Value) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    if json_is_expanded(value) {
        out.push(Line::from(vec![
            styled(json_open_token(value), JSON_PUNCT_COLOR),
        ]));
        write_json_entries(&mut out, value, 1);
        out.push(Line::from(vec![styled(json_close_token(value), JSON_PUNCT_COLOR)]));
    } else {
        out.push(Line::from(json_scalar_spans(value)));
    }
    out
}

/// Emit every child of an expanded container at `indent`, comma-separated.
fn write_json_entries(out: &mut Vec<Line<'static>>, container: &serde_json::Value, indent: usize) {
    let entries: Vec<(Option<&str>, &serde_json::Value)> = match container {
        serde_json::Value::Object(m) => m.iter().map(|(k, v)| (Some(k.as_str()), v)).collect(),
        serde_json::Value::Array(a) => a.iter().map(|v| (None, v)).collect(),
        _ => return,
    };
    let last = entries.len().saturating_sub(1);
    for (i, (key, value)) in entries.into_iter().enumerate() {
        write_json_entry(out, key, value, indent);
        if i != last {
            append_comma(out);
        }
    }
}

/// Emit one child. `indent` is the depth of the child's own line; a nested
/// container's closing bracket is drawn at `indent - 1` to align under it.
fn write_json_entry(
    out: &mut Vec<Line<'static>>,
    key: Option<&str>,
    value: &serde_json::Value,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let mut spans = vec![Span::raw(pad.clone())];
    if let Some(k) = key {
        spans.push(styled(format!("\"{k}\""), JSON_KEY_COLOR));
        spans.push(Span::raw(": ".to_string()));
    }

    if json_is_expanded(value) {
        spans.push(styled(json_open_token(value), JSON_PUNCT_COLOR));
        out.push(Line::from(spans));
        write_json_entries(out, value, indent + 1);
        out.push(Line::from(vec![
            Span::raw("  ".repeat(indent.saturating_sub(1))),
            styled(json_close_token(value), JSON_PUNCT_COLOR),
        ]));
    } else {
        spans.extend(json_scalar_spans(value));
        out.push(Line::from(spans));
    }
}

/// Append a separator to the most recently emitted line.
fn append_comma(out: &mut Vec<Line<'static>>) {
    if let Some(line) = out.last_mut() {
        line.spans.push(Span::raw(",".to_string()));
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
    ZAdd { key: String, score: f64, member: String },
    ZRem { key: String, member: String },
    StreamAdd { key: String, entry_id: String, fields: Vec<(String, String)> },
    StreamRem { key: String, entry_id: String },
    StreamToggleGroups,
    CopyValue(String),
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
    ZSetAddMember,
    ZSetAddScore(String),
    ZSetEditScore(String, usize),
    ZSetScoreMin,
    ZSetScoreMax(String),
    StreamAddId,
    StreamAddFields(String),
    SetTtlSeconds,
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
    pub zset_cursor: usize,
    pub zset_sort_score_asc: bool,
    pub zset_score_min: Option<String>,
    pub zset_score_max: Option<String>,
    pub stream_cursor: usize,
    pub stream_compact: bool,
    pub stream_show_groups: bool,
    pub stream_groups: Option<Vec<StreamGroup>>,
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
            zset_cursor: 0,
            zset_sort_score_asc: true,
            zset_score_min: None,
            zset_score_max: None,
            stream_cursor: 0,
            stream_compact: true,
            stream_show_groups: false,
            stream_groups: None,
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
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_show_groups = false;
        self.stream_groups = None;
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
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_show_groups = false;
        self.stream_groups = None;
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
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_show_groups = false;
        self.stream_groups = None;
        self.list_prompt = None;
        self.prompt_mode = None;
    }

    pub fn set_metadata(&mut self, encoding: Option<String>, memory_bytes: Option<u64>, ttl: Option<Ttl>) {
        self.encoding = encoding;
        self.memory_bytes = memory_bytes;
        self.ttl = ttl;
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InspectorAction> {
        self.handle_event_with_keymap(event, &crate::config::keybindings::Keymap::default())
    }

    pub fn handle_event_with_keymap(&mut self, event: &Event, keymap: &crate::config::keybindings::Keymap) -> Option<InspectorAction> {
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
                    Some(PromptMode::ZSetAddMember) => {
                        self.list_prompt = Some(InputWidget::new("Score for member"));
                        self.prompt_mode = Some(PromptMode::ZSetAddScore(val));
                        None
                    }
                    Some(PromptMode::ZSetAddScore(member)) => {
                        let score = val.parse::<f64>().ok()?;
                        Some(InspectorAction::ZAdd { key, score, member })
                    }
                    Some(PromptMode::ZSetEditScore(member, _)) => {
                        let score = val.parse::<f64>().ok()?;
                        Some(InspectorAction::ZAdd { key, score, member })
                    }
                    Some(PromptMode::StreamAddId) => {
                        let entry_id = if val.is_empty() { "*".to_string() } else { val };
                        self.list_prompt = Some(InputWidget::new("Fields (key=val key=val ...)"));
                        self.prompt_mode = Some(PromptMode::StreamAddFields(entry_id));
                        None
                    }
                    Some(PromptMode::StreamAddFields(entry_id)) => {
                        let mut fields = Vec::new();
                        for part in val.split_whitespace() {
                            if let Some(eq_pos) = part.find('=') {
                                let f = part[..eq_pos].to_string();
                                let v = part[eq_pos + 1..].to_string();
                                fields.push((f, v));
                            }
                        }
                        Some(InspectorAction::StreamAdd { key, entry_id, fields })
                    }
                    Some(PromptMode::SetTtlSeconds) => {
                        if val.trim().is_empty() {
                            Some(InspectorAction::SetTtl { key, seconds: -1 })
                        } else {
                            let seconds = val.parse::<i64>().ok()?;
                            Some(InspectorAction::SetTtl { key, seconds })
                        }
                    }
                    Some(PromptMode::ZSetScoreMin) => {
                        let min = if val.trim().is_empty() { "-inf".to_string() } else { val.trim().to_string() };
                        self.list_prompt = Some(InputWidget::new("Max score (empty=+inf):"));
                        self.prompt_mode = Some(PromptMode::ZSetScoreMax(min));
                        None
                    }
                    Some(PromptMode::ZSetScoreMax(min)) => {
                        let max = if val.trim().is_empty() { "+inf".to_string() } else { val.trim().to_string() };
                        self.zset_score_min = Some(min);
                        self.zset_score_max = Some(max);
                        None
                    }
                    None => None,
                };
            }
            if prompt.cancelled {
                self.list_prompt = None;
                self.prompt_mode = None;
            }
            return None;
        }

        if key.code == KeyCode::Tab {
            self.string_view = self.string_view.next();
        }
        if keymap.matches("edit", key)
            && let Some(RedisValue::String(ref s)) = self.value
        {
            self.edit_mode = true;
            self.text_editor = Some(TextAreaEditor::new(s.clone()));
        }
        if keymap.matches("set_ttl", key) {
            self.list_prompt = Some(InputWidget::new("TTL in seconds (blank=persist)"));
            self.prompt_mode = Some(PromptMode::SetTtlSeconds);
        }
        if keymap.matches("copy", key)
            && let Some(RedisValue::String(ref s)) = self.value
        {
            return Some(InspectorAction::CopyValue(s.clone()));
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
                _ => {}
            }
            if keymap.matches("inspector_list_push", key) {
                self.list_prompt = Some(InputWidget::new("Value to RPUSH"));
                self.prompt_mode = Some(PromptMode::Rpush);
            } else if keymap.matches("inspector_list_prepend", key) {
                self.list_prompt = Some(InputWidget::new("Value to LPUSH"));
                self.prompt_mode = Some(PromptMode::Lpush);
            } else if keymap.matches("delete", key) {
                if let Some(val) = items.get(self.list_cursor) {
                    return Some(InspectorAction::ListRemove {
                        key: self.key.clone().unwrap_or_default(),
                        value: val.clone(),
                    });
                }
            } else if keymap.matches("edit", key) {
                if let Some(val) = items.get(self.list_cursor) {
                    let mut prompt = InputWidget::new(format!("Edit [{}]", self.list_cursor));
                    prompt.value = val.clone();
                    prompt.cursor = val.len();
                    self.list_prompt = Some(prompt);
                    self.prompt_mode = Some(PromptMode::Lset(self.list_cursor));
                }
            } else if keymap.matches("copy", key)
                && let Some(val) = items.get(self.list_cursor)
            {
                return Some(InspectorAction::CopyValue(val.clone()));
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
                _ => {}
            }
            if keymap.matches("inspector_hash_add", key) {
                self.list_prompt = Some(InputWidget::new("Field name"));
                self.prompt_mode = Some(PromptMode::HashAddField);
            } else if keymap.matches("delete", key) {
                if let Some((field, _)) = fields.get(self.hash_cursor) {
                    return Some(InspectorAction::HashDel {
                        key: self.key.clone().unwrap_or_default(),
                        field: (*field).clone(),
                    });
                }
            } else if keymap.matches("edit", key) {
                if let Some((field, val)) = fields.get(self.hash_cursor) {
                    let mut prompt = InputWidget::new(format!("Value for '{}'", field));
                    prompt.value = (*val).clone();
                    prompt.cursor = val.len();
                    self.list_prompt = Some(prompt);
                    self.prompt_mode = Some(PromptMode::HashEdit((*field).clone(), self.hash_cursor));
                }
            } else if keymap.matches("copy", key)
                && let Some((_, val)) = fields.get(self.hash_cursor)
            {
                return Some(InspectorAction::CopyValue((*val).clone()));
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
                _ => {}
            }
            if keymap.matches("inspector_set_add", key) {
                self.list_prompt = Some(InputWidget::new("Member to SADD"));
                self.prompt_mode = Some(PromptMode::SetAddMember);
            } else if keymap.matches("delete", key)
                && let Some(member) = items.get(self.set_cursor)
            {
                return Some(InspectorAction::SetRem {
                    key: self.key.clone().unwrap_or_default(),
                    member: (*member).clone(),
                });
            } else if keymap.matches("copy", key)
                && let Some(member) = items.get(self.set_cursor)
            {
                return Some(InspectorAction::CopyValue((*member).clone()));
            }
        }

        // ZSet-specific key handling
        if let Some(RedisValue::ZSet(ref entries)) = self.value {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.zset_cursor > 0 { self.zset_cursor -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.zset_cursor + 1 < entries.len() { self.zset_cursor += 1; }
                }
                _ => {}
            }
            if keymap.matches("inspector_zset_add", key) {
                self.list_prompt = Some(InputWidget::new("Member to ZADD"));
                self.prompt_mode = Some(PromptMode::ZSetAddMember);
            } else if keymap.matches("edit", key) {
                if let Some(entry) = entries.get(self.zset_cursor) {
                    let mut prompt = InputWidget::new(format!("Score for '{}'", entry.member));
                    prompt.value = entry.score.to_string();
                    prompt.cursor = entry.score.to_string().len();
                    self.list_prompt = Some(prompt);
                    self.prompt_mode = Some(PromptMode::ZSetEditScore(entry.member.clone(), self.zset_cursor));
                }
            } else if keymap.matches("delete", key)
                && let Some(entry) = entries.get(self.zset_cursor)
            {
                return Some(InspectorAction::ZRem {
                    key: self.key.clone().unwrap_or_default(),
                    member: entry.member.clone(),
                });
            } else if keymap.matches("cycle_sort", key) {
                self.zset_sort_score_asc = !self.zset_sort_score_asc;
            } else if keymap.matches("copy", key)
                && let Some(entry) = entries.get(self.zset_cursor)
            {
                return Some(InspectorAction::CopyValue(entry.member.clone()));
            } else if keymap.matches("inspector_zset_range_query", key) {
                if self.zset_score_min.is_some() || self.zset_score_max.is_some() {
                    self.zset_score_min = None;
                    self.zset_score_max = None;
                } else {
                    self.list_prompt = Some(InputWidget::new("Min score (empty=-inf):"));
                    self.prompt_mode = Some(PromptMode::ZSetScoreMin);
                }
            }
        }

        // Stream-specific key handling
        if let Some(RedisValue::Stream(ref entries)) = self.value {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.stream_cursor > 0 { self.stream_cursor -= 1; }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.stream_cursor + 1 < entries.len() { self.stream_cursor += 1; }
                }
                _ => {}
            }
            if keymap.matches("inspector_stream_add", key) {
                self.list_prompt = Some(InputWidget::new("Entry ID (Enter=auto)"));
                self.prompt_mode = Some(PromptMode::StreamAddId);
            } else if keymap.matches("delete", key)
                && let Some(entry) = entries.get(self.stream_cursor)
            {
                return Some(InspectorAction::StreamRem {
                    key: self.key.clone().unwrap_or_default(),
                    entry_id: entry.id.clone(),
                });
            } else if keymap.matches("inspector_toggle_view", key) {
                self.stream_compact = !self.stream_compact;
            } else if keymap.matches("inspector_goto_end", key) {
                if !entries.is_empty() {
                    self.stream_cursor = entries.len() - 1;
                }
            } else if keymap.matches("inspector_stream_groups", key) {
                return Some(InspectorAction::StreamToggleGroups);
            } else if keymap.matches("copy", key)
                && let Some(entry) = entries.get(self.stream_cursor)
            {
                return Some(InspectorAction::CopyValue(entry.id.clone()));
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

        // ZSet-specific render
        if let Some(RedisValue::ZSet(ref entries)) = self.value {
            self.render_zset(frame, area, entries);
            return;
        }

        // Stream-specific render
        if let Some(RedisValue::Stream(ref entries)) = self.value {
            self.render_stream(frame, area, entries);
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

        let mut lines = Vec::new();
        if !meta_parts.is_empty() {
            lines.push(Line::from(meta_parts));
            lines.push(Line::from(""));
        }

        if self.loading {
            lines.push(Line::from("Loading value..."));
        } else if let Some(err) = &self.error {
            lines.push(Line::from(format!("Error: {err}")));
        } else if let Some(RedisValue::String(ref s)) = self.value {
            // Strings render through the view mode so JSON gets syntax
            // highlighting rather than a flat pretty-print.
            let label = Span::styled(
                format!("[{}]", self.string_view.label()),
                Style::default().fg(Color::DarkGray),
            );
            lines.push(Line::from(vec![label, Span::raw(" ")]));
            lines.extend(self.string_view.render_lines(s));
        } else if let Some(value) = &self.value {
            for line in render_value_preview(value, &self.string_view).lines() {
                lines.push(Line::from(line.to_string()));
            }
        } else {
            lines.push(Line::from(
                "Select a key and press Enter to inspect its value",
            ));
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

    fn render_zset(&self, frame: &mut Frame, area: Rect, entries: &[ZSetEntry]) {
        let mode_label = if self.zset_sort_score_asc { "asc" } else { "desc" };
        let filtered = self.zset_score_min.is_some() || self.zset_score_max.is_some();
        let visible: Vec<&ZSetEntry> = if filtered && !entries.is_empty() {
            entries.iter().filter(|e| {
                let above_min = self.zset_score_min.as_ref().is_none_or(|min| {
                    if min == "-inf" { true } else { e.score >= min.parse::<f64>().unwrap_or(f64::NEG_INFINITY) }
                });
                let below_max = self.zset_score_max.as_ref().is_none_or(|max| {
                    if max == "+inf" { true } else { e.score <= max.parse::<f64>().unwrap_or(f64::INFINITY) }
                });
                above_min && below_max
            }).collect()
        } else {
            entries.iter().collect()
        };
        let title = self
            .key
            .as_deref()
            .map(|k| {
                let base = format!("ZSet [{}] (len={}, score:{})", k, visible.len(), mode_label);
                if filtered {
                    format!("{} filter:{}~{}", base, self.zset_score_min.as_deref().unwrap_or("-inf"), self.zset_score_max.as_deref().unwrap_or("+inf"))
                } else {
                    base
                }
            })
            .unwrap_or_else(|| format!("ZSet (len={})", visible.len()));

        let mut items: Vec<&&ZSetEntry> = visible.iter().collect();
        if self.zset_sort_score_asc {
            items.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }

        let mut text = String::new();
        for (i, entry) in items.iter().enumerate() {
            let marker = if i == self.zset_cursor { ">" } else { " " };
            text.push_str(&format!("{}[{}] {} {}\n", marker, i, entry.score, entry.member));
        }

        if let Some(ref prompt) = self.list_prompt {
            let prompt_label = match self.prompt_mode {
                Some(PromptMode::ZSetAddMember) => "Member to ZADD: ",
                Some(PromptMode::ZSetAddScore(ref m)) => &format!("Score for '{}': ", m),
                Some(PromptMode::ZSetEditScore(ref m, _)) => &format!("Score for '{}': ", m),
                _ => "Input: ",
            };
            text.push_str(&format!(
                "\n{}{}{}",
                prompt_label,
                prompt.value,
                if prompt.cursor >= prompt.value.len() { "_" } else { " " }
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

    fn render_stream(&self, frame: &mut Frame, area: Rect, entries: &[StreamEntry]) {
        let mode = if self.stream_compact { "compact" } else { "full" };

        if self.stream_show_groups {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            let left = chunks[0];
            let right = chunks[1];

            // Left: stream messages
            let title_left = self
                .key
                .as_deref()
                .map(|k| format!("Stream [{}] (len={}, {})", k, entries.len(), mode))
                .unwrap_or_else(|| format!("Stream (len={})", entries.len()));
            let mut text = String::new();
            for (i, entry) in entries.iter().enumerate() {
                let marker = if i == self.stream_cursor { ">" } else { " " };
                if self.stream_compact {
                    text.push_str(&format!("{}[{}] {} ({} fields)\n", marker, i, entry.id, entry.fields.len()));
                } else {
                    text.push_str(&format!("{}[{}] {}\n", marker, i, entry.id));
                    for (field, val) in &entry.fields {
                        text.push_str(&format!("  {}: {}\n", field, val));
                    }
                }
            }
            if let Some(ref prompt) = self.list_prompt {
                let prompt_label = match self.prompt_mode {
                    Some(PromptMode::StreamAddId) => "Entry ID: ",
                    Some(PromptMode::StreamAddFields(_)) => "Fields: ",
                    _ => "Input: ",
                };
                text.push_str(&format!(
                    "\n{}{}{}",
                    prompt_label,
                    prompt.value,
                    if prompt.cursor >= prompt.value.len() { "_" } else { " " }
                ));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .title(title_left);
            let paragraph = Paragraph::new(text)
                .block(block)
                .wrap(Wrap { trim: false });
            frame.render_widget(paragraph, left);

            // Right: consumer groups
            let title_right = "Consumer Groups";
            if let Some(ref groups) = self.stream_groups {
                let header = TableRow::new(vec![
                    Cell::from("Group"),
                    Cell::from("Consumers"),
                    Cell::from("Pending"),
                    Cell::from("Last ID"),
                ]).style(Style::default().fg(Color::Yellow));
                let rows: Vec<TableRow> = groups.iter().map(|g| {
                    TableRow::new(vec![
                        Cell::from(g.name.clone()),
                        Cell::from(g.consumers.to_string()),
                        Cell::from(g.pending.to_string()),
                        Cell::from(g.last_delivered_id.clone()),
                    ])
                }).collect();
                let table = Table::new(rows, vec![
                    Constraint::Percentage(30),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(30),
                ])
                .header(header)
                .block(Block::default().borders(Borders::ALL).title(title_right));
                frame.render_widget(table, right);
            } else {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(title_right);
                let paragraph = Paragraph::new("Loading groups...").block(block);
                frame.render_widget(paragraph, right);
            }
        } else {
            let title = self
                .key
                .as_deref()
                .map(|k| format!("Stream [{}] (len={}, {})", k, entries.len(), mode))
                .unwrap_or_else(|| format!("Stream (len={})", entries.len()));

            let mut text = String::new();
            for (i, entry) in entries.iter().enumerate() {
                let marker = if i == self.stream_cursor { ">" } else { " " };
                if self.stream_compact {
                    text.push_str(&format!("{}[{}] {} ({} fields)\n", marker, i, entry.id, entry.fields.len()));
                } else {
                    text.push_str(&format!("{}[{}] {}\n", marker, i, entry.id));
                    for (field, val) in &entry.fields {
                        text.push_str(&format!("  {}: {}\n", field, val));
                    }
                }
            }

            if let Some(ref prompt) = self.list_prompt {
                let prompt_label = match self.prompt_mode {
                    Some(PromptMode::StreamAddId) => "Entry ID: ",
                    Some(PromptMode::StreamAddFields(_)) => "Fields: ",
                    _ => "Input: ",
                };
                text.push_str(&format!(
                    "\n{}{}{}",
                    prompt_label,
                    prompt.value,
                    if prompt.cursor >= prompt.value.len() { "_" } else { " " }
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
}

fn render_value_preview(value: &RedisValue, view_mode: &StringViewMode) -> String {
    match value {
        RedisValue::String(s) => view_mode.render(s),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- String view modes (2.3, 2.4, 2.5, 2.24, 2.25, 2.26) ----

    #[test]
    fn string_view_raw_is_verbatim() {
        assert_eq!(StringViewMode::Raw.render("hello"), "hello");
    }

    #[test]
    fn string_view_json_pretty_prints() {
        let out = StringViewMode::Json.render("{\"a\":1}");
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn string_view_json_falls_back_when_invalid() {
        // Not valid JSON: the value must remain readable, not become an error.
        assert_eq!(StringViewMode::Json.render("not json"), "not json");
    }

    #[test]
    fn string_view_base64_encodes() {
        use base64::Engine;
        let expected = base64::engine::general_purpose::STANDARD.encode(b"hello");
        assert_eq!(StringViewMode::Base64.render("hello"), expected);
    }

    #[test]
    fn string_view_hex_emits_spaced_bytes() {
        assert_eq!(StringViewMode::Hex.render("AB"), "41 42");
    }

    #[test]
    fn string_view_mode_cycles_and_labels() {
        let mut mode = StringViewMode::Raw;
        assert_eq!(mode.label(), "Raw");
        for expected in [
            StringViewMode::Json,
            StringViewMode::Base64,
            StringViewMode::Hex,
            StringViewMode::Raw,
        ] {
            mode = mode.next();
            assert_eq!(mode, expected);
        }
    }

    #[test]
    fn json_highlight_colors_keys_strings_and_numbers() {
        let parsed: serde_json::Value =
            serde_json::from_str("{\"name\":\"ada\",\"age\":36}").unwrap();
        let lines = highlight_json(&parsed);
        let all: Vec<&Span> = lines.iter().flat_map(|l| l.spans.iter()).collect();
        let joined: String = all.iter().map(|s| s.content.as_ref()).collect();

        // Content is preserved verbatim.
        assert!(joined.contains("\"name\""));
        assert!(joined.contains("\"ada\""));
        assert!(joined.contains("36"));

        // Keys yellow, string values green, numbers cyan.
        let find = |needle: &str| {
            all.iter()
                .find(|s| s.content.as_ref() == needle)
                .unwrap_or_else(|| panic!("{needle} span in {joined}"))
        };
        assert_eq!(find("\"name\"").style.fg, Some(JSON_KEY_COLOR));
        assert_eq!(find("\"ada\"").style.fg, Some(JSON_STRING_COLOR));
        assert_eq!(find("36").style.fg, Some(JSON_NUMBER_COLOR));
    }

    #[test]
    fn json_highlight_colors_literals() {
        let parsed: serde_json::Value =
            serde_json::from_str("{\"a\":true,\"b\":null}").unwrap();
        let lines = highlight_json(&parsed);
        let all: Vec<&Span> = lines.iter().flat_map(|l| l.spans.iter()).collect();
        for literal in ["true", "null"] {
            let span = all
                .iter()
                .find(|s| s.content.as_ref() == literal)
                .unwrap_or_else(|| panic!("{literal} span"));
            assert_eq!(span.style.fg, Some(JSON_LITERAL_COLOR));
        }
    }

    #[test]
    fn json_highlight_expands_containers_onto_multiple_lines() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":{\"b\":1}}").unwrap();
        let lines = highlight_json(&parsed);
        assert!(lines.len() > 1, "nested object should span multiple lines");
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(text[0], "{");
        assert_eq!(text.last().unwrap(), "}");
    }

    #[test]
    fn json_highlight_indents_nested_keys() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":{\"b\":1}}").unwrap();
        let lines = highlight_json(&parsed);
        let nested = lines.iter().find(|l| l.to_string().contains("\"b\""));
        let nested = nested.expect("nested key line");
        assert!(
            nested.to_string().starts_with("    \"b\""),
            "expected 4-space indent, got {:?}",
            nested.to_string()
        );
    }

    #[test]
    fn json_highlight_separates_entries_with_commas() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":1,\"b\":2,\"c\":3}").unwrap();
        let lines = highlight_json(&parsed);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        // Open brace, three entries (last has no trailing comma), close brace.
        assert_eq!(text.len(), 5);
        assert!(text[1].ends_with(','));
        assert!(text[2].ends_with(','));
        assert!(!text[3].ends_with(','));
    }

    #[test]
    fn json_highlight_arrays_have_no_keys() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":[1,2]}").unwrap();
        let lines = highlight_json(&parsed);
        let joined: String = lines.iter().flat_map(|l| l.spans.iter()).map(|s| s.content.as_ref()).collect();
        assert!(joined.contains('['));
        assert!(joined.contains(']'));
        assert!(joined.contains("1"));
        assert!(joined.contains("2"));
    }

    #[test]
    fn json_highlight_empty_containers_render_inline() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":{},\"b\":[]}").unwrap();
        let joined: String = highlight_json(&parsed)
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(joined.contains("{}"), "got: {joined}");
        assert!(joined.contains("[]"), "got: {joined}");
    }

    #[test]
    fn json_highlight_escapes_are_preserved() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":\"line\\nbreak\"}").unwrap();
        let joined: String = highlight_json(&parsed)
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(joined.contains(r"\n"), "got: {joined}");
    }

    #[test]
    fn json_highlight_scalars_render_on_one_line() {
        for raw in ["42", "\"hi\"", "true", "null"] {
            let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(highlight_json(&parsed).len(), 1, "input: {raw}");
        }
    }

    #[test]
    fn render_lines_json_mode_produces_spans() {
        let lines = StringViewMode::Json.render_lines("{\"a\":1}");
        assert!(lines.len() > 1);
        assert!(lines.iter().flat_map(|l| l.spans.iter()).any(|s| s.style.fg.is_some()));
    }

    #[test]
    fn render_lines_non_json_modes_are_unstyled() {
        for mode in [StringViewMode::Raw, StringViewMode::Base64, StringViewMode::Hex] {
            let lines = mode.render_lines("hello");
            assert!(lines.iter().flat_map(|l| l.spans.iter()).all(|s| s.style.fg.is_none()));
        }
    }

    #[test]
    fn render_lines_invalid_json_is_plain_but_readable() {
        let lines = StringViewMode::Json.render_lines("<html>not json</html>");
        let joined: String = lines.iter().flat_map(|l| l.spans.iter()).map(|s| s.content.as_ref()).collect();
        assert_eq!(joined, "<html>not json</html>");
    }

    #[test]
    fn inspector_renders_string_in_each_view_mode_without_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for mode in [
            StringViewMode::Raw,
            StringViewMode::Json,
            StringViewMode::Base64,
            StringViewMode::Hex,
        ] {
            let mut inspector = ValueInspector::new();
            inspector.set_value("k".into(), RedisValue::String(r#"{"a":[1,2],"b":null}"#.into()));
            inspector.string_view = mode;
            terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        }
    }

    #[test]
    fn inspector_renders_string_with_metadata_without_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value("k".into(), RedisValue::String("{\"a\":1}".into()));
        inspector.set_metadata(
            Some("embstr".into()),
            Some(2048),
            Some(Ttl::Expires(std::time::Duration::from_secs(90))),
        );
        inspector.string_view = StringViewMode::Json;
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn stream_groups_toggle_sets_state() {
        let mut inspector = ValueInspector::new();
        assert!(!inspector.stream_show_groups);
        assert!(inspector.stream_groups.is_none());

        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![StreamGroup {
            name: "g1".to_string(),
            consumers: 2,
            pending: 5,
            last_delivered_id: "123-0".to_string(),
        }]);
        assert!(inspector.stream_show_groups);
        assert_eq!(inspector.stream_groups.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn stream_groups_reset_on_set_value() {
        let mut inspector = ValueInspector::new();
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![]);
        inspector.set_value(
            "events".to_string(),
            RedisValue::Stream(vec![StreamEntry {
                id: "123-0".to_string(),
                fields: [("k".to_string(), "v".to_string())].into(),
            }]),
        );
        assert!(!inspector.stream_show_groups);
        assert!(inspector.stream_groups.is_none());
    }

    #[test]
    fn stream_groups_reset_on_set_loading() {
        let mut inspector = ValueInspector::new();
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![StreamGroup {
            name: "g1".to_string(),
            consumers: 1,
            pending: 0,
            last_delivered_id: "0-0".to_string(),
        }]);
        inspector.set_loading("events".to_string());
        assert!(!inspector.stream_show_groups);
        assert!(inspector.stream_groups.is_none());
    }

    #[test]
    fn render_stream_with_groups_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "events".to_string(),
            RedisValue::Stream(vec![StreamEntry {
                id: "123-0".to_string(),
                fields: [("k".to_string(), "v".to_string())].into(),
            }]),
        );
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![StreamGroup {
            name: "g1".to_string(),
            consumers: 2,
            pending: 5,
            last_delivered_id: "123-0".to_string(),
        }]);
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn render_stream_without_groups_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "events".to_string(),
            RedisValue::Stream(vec![StreamEntry {
                id: "123-0".to_string(),
                fields: [("k".to_string(), "v".to_string())].into(),
            }]),
        );
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn render_stream_groups_empty_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "events".to_string(),
            RedisValue::Stream(vec![]),
        );
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![]);
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }
}
