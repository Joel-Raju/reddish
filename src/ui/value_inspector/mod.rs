use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row as TableRow, Table, Wrap},
};

use crate::events::Event;
use crate::redis::client::Ttl;
use crate::redis::types::{RedisValue, StreamEntry, StreamGroup, ZSetEntry};
use crate::ui::theme;
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
///
/// These are functions rather than constants because the theme is resolved at
/// render time. Each maps to the theme field whose default value matches the
/// colour that used to be hardcoded, so the default appearance is unchanged
/// while every other theme now applies.
fn json_key_color() -> Color {
    theme::current().warning
}
fn json_string_color() -> Color {
    theme::current().type_string
}
fn json_number_color() -> Color {
    theme::current().title
}
fn json_literal_color() -> Color {
    theme::current().type_hash
}
fn json_punct_color() -> Color {
    theme::current().border
}

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
            json_string_color(),
        )],
        serde_json::Value::Number(n) => vec![styled(n.to_string(), json_number_color())],
        serde_json::Value::Bool(_) | serde_json::Value::Null => {
            vec![styled(value.to_string(), json_literal_color())]
        }
        _ => vec![
            styled(json_open_token(value), json_punct_color()),
            styled(json_close_token(value), json_punct_color()),
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
        out.push(Line::from(vec![styled(
            json_open_token(value),
            json_punct_color(),
        )]));
        write_json_entries(&mut out, value, 1);
        out.push(Line::from(vec![styled(
            json_close_token(value),
            json_punct_color(),
        )]));
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
        spans.push(styled(format!("\"{k}\""), json_key_color()));
        spans.push(Span::raw(": ".to_string()));
    }

    if json_is_expanded(value) {
        spans.push(styled(json_open_token(value), json_punct_color()));
        out.push(Line::from(spans));
        write_json_entries(out, value, indent + 1);
        out.push(Line::from(vec![
            Span::raw("  ".repeat(indent.saturating_sub(1))),
            styled(json_close_token(value), json_punct_color()),
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
    WriteString {
        key: String,
        value: String,
    },
    SetTtl {
        key: String,
        seconds: i64,
    },
    ListPush {
        key: String,
        value: String,
        head: bool,
    },
    ListSet {
        key: String,
        index: i64,
        value: String,
    },
    ListRemove {
        key: String,
        value: String,
    },
    ListInsert {
        key: String,
        pivot: String,
        value: String,
        before: bool,
    },
    HashSet {
        key: String,
        field: String,
        value: String,
    },
    HashDel {
        key: String,
        field: String,
    },
    /// Bulk delete of every currently selected hash field in one HDEL.
    HashDelMany {
        key: String,
        fields: Vec<String>,
    },
    SetAdd {
        key: String,
        member: String,
    },
    SetRem {
        key: String,
        member: String,
    },
    ZAdd {
        key: String,
        score: f64,
        member: String,
    },
    ZRem {
        key: String,
        member: String,
    },
    StreamAdd {
        key: String,
        entry_id: String,
        fields: Vec<(String, String)>,
    },
    StreamRem {
        key: String,
        entry_id: String,
    },
    StreamTrim {
        key: String,
        strategy: String,
        threshold: String,
        approximate: bool,
    },
    /// Reload the current stream value (used by follow mode).
    StreamRefresh,
    /// Hand a string value to the user's `$EDITOR`. The app layer performs the
    /// terminal suspend/resume and the file round-trip.
    EditStringExternal {
        key: String,
        value: String,
    },
    /// Client-side validation failure; surfaces in the status bar, no I/O.
    InvalidInput(String),
    StreamToggleGroups,
    CopyValue(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromptMode {
    Rpush,
    Lpush,
    Lset(usize),
    /// Step 1 of list insert: awaiting BEFORE/AFTER choice. Holds the pivot.
    ListInsertPosition(String),
    /// Step 2 of list insert: awaiting the value to insert.
    ListInsertValue {
        pivot: String,
        before: bool,
    },
    HashAddField,
    HashAddValue(String),
    HashEdit(String, usize),
    /// Field-name substring filter; an empty submission clears the filter.
    HashFilter,
    /// Member search across the current value; empty submission clears it.
    ValueFilter,
    SetAddMember,
    ZSetAddMember,
    ZSetAddScore(String),
    ZSetEditScore(String, usize),
    ZSetScoreMin,
    ZSetScoreMax(String),
    StreamAddId,
    StreamAddFields(String),
    /// Step 1 of XTRIM: awaiting MAXLEN or MINID.
    StreamTrimStrategy,
    /// Step 2 of XTRIM: awaiting the threshold value.
    StreamTrimThreshold(String),
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
    /// Field-name substring filter; empty means show all fields.
    pub hash_filter: String,
    /// Field names currently selected for bulk delete.
    pub hash_selected: Vec<String>,
    pub set_cursor: usize,
    pub zset_cursor: usize,
    pub zset_sort_score_asc: bool,
    pub zset_score_min: Option<String>,
    pub zset_score_max: Option<String>,
    pub stream_cursor: usize,
    pub stream_compact: bool,
    /// Byte budget for rendering a single string value; `usize::MAX` disables
    /// truncation. Set from `config.toml` via `max_value_display_bytes`.
    pub max_value_display_bytes: usize,
    /// When set, `e` on a string hands the value to `$EDITOR` instead of
    /// opening the built-in editor. Set from `app.rs` at startup.
    pub prefer_external_editor: bool,
    /// When true, the tick handler reloads the stream so new entries appear.
    pub stream_follow: bool,
    pub stream_show_groups: bool,
    pub stream_groups: Option<Vec<StreamGroup>>,
    /// Substring filter applied to the members of the current value
    /// (list items, hash field/value pairs, set members, zset members).
    pub value_filter: String,
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
            hash_filter: String::new(),
            hash_selected: Vec::new(),
            set_cursor: 0,
            zset_cursor: 0,
            zset_sort_score_asc: true,
            zset_score_min: None,
            zset_score_max: None,
            stream_cursor: 0,
            stream_compact: true,
            max_value_display_bytes: 1024 * 1024,
            prefer_external_editor: false,
            stream_follow: false,
            value_filter: String::new(),
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
        self.hash_filter = String::new();
        self.hash_selected.clear();
        self.set_cursor = 0;
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_follow = false;
        self.value_filter = String::new();
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
        self.hash_filter = String::new();
        self.hash_selected.clear();
        self.set_cursor = 0;
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_follow = false;
        self.value_filter = String::new();
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
        self.hash_filter = String::new();
        self.hash_selected.clear();
        self.set_cursor = 0;
        self.zset_cursor = 0;
        self.zset_sort_score_asc = true;
        self.zset_score_min = None;
        self.zset_score_max = None;
        self.stream_cursor = 0;
        self.stream_compact = true;
        self.stream_follow = false;
        self.value_filter = String::new();
        self.stream_show_groups = false;
        self.stream_groups = None;
        self.list_prompt = None;
        self.prompt_mode = None;
    }

    pub fn set_metadata(
        &mut self,
        encoding: Option<String>,
        memory_bytes: Option<u64>,
        ttl: Option<Ttl>,
    ) {
        self.encoding = encoding;
        self.memory_bytes = memory_bytes;
        self.ttl = ttl;
    }

    /// Field names currently visible under `hash_filter`, in value order.
    pub fn visible_hash_fields(&self) -> Vec<&String> {
        let Some(RedisValue::Hash(entries)) = self.value.as_ref() else {
            return Vec::new();
        };
        if self.hash_filter.is_empty() {
            entries.keys().collect()
        } else {
            let needle = self.hash_filter.to_lowercase();
            entries
                .keys()
                .filter(|k| k.to_lowercase().contains(&needle))
                .collect()
        }
    }

    /// Toggle `field` in the bulk-selection set. Returns true if now selected.
    pub fn toggle_hash_selection(&mut self, field: &str) -> bool {
        match self.hash_selected.iter().position(|f| f == field) {
            Some(idx) => {
                self.hash_selected.remove(idx);
                false
            }
            None => {
                self.hash_selected.push(field.to_string());
                true
            }
        }
    }

    pub fn clear_hash_selection(&mut self) {
        self.hash_selected.clear();
    }

    pub fn select_all_visible_hash_fields(&mut self) {
        self.hash_selected = self.visible_hash_fields().into_iter().cloned().collect();
    }

    /// Called on each tick while follow mode is active.
    ///
    /// Returns the last known entry id so the caller can fetch only entries newer
    /// than it, or `None` when no reload is needed (follow off, or the inspector
    /// is not showing a stream).
    pub fn follow_tick(&self) -> Option<String> {
        if !self.stream_follow {
            return None;
        }
        let entries = match self.value.as_ref() {
            Some(RedisValue::Stream(entries)) => entries,
            _ => return None,
        };
        entries.last().map(|e| e.id.clone())
    }

    /// Pin the cursor to the newest entry, used after a follow-mode reload.
    pub fn follow_cursor_to_tail(&mut self) {
        if let Some(RedisValue::Stream(entries)) = self.value.as_ref() {
            self.stream_cursor = entries.len().saturating_sub(1);
        }
    }

    /// Append newly fetched entries to the currently displayed stream.
    ///
    /// Returns how many were actually appended. Entries are deduplicated by id
    /// so a repeated fetch cannot grow the list, and the buffer is capped so a
    /// long-lived follow session cannot grow without bound.
    pub fn append_stream_entries(&mut self, incoming: Vec<StreamEntry>) -> usize {
        /// Cap on retained stream entries while following.
        const MAX_FOLLOW_ENTRIES: usize = 5_000;

        let Some(RedisValue::Stream(entries)) = self.value.as_mut() else {
            return 0;
        };
        let existing: std::collections::HashSet<String> =
            entries.iter().map(|e| e.id.clone()).collect();

        let mut added = 0usize;
        for entry in incoming {
            if existing.contains(&entry.id) {
                continue;
            }
            entries.push(entry);
            added += 1;
        }
        if added > 0 {
            let overflow = entries.len().saturating_sub(MAX_FOLLOW_ENTRIES);
            if overflow > 0 {
                entries.drain(0..overflow);
                self.stream_cursor = self.stream_cursor.saturating_sub(overflow);
            }
        }
        added
    }

    /// True when `haystack` contains `self.value_filter`, case-insensitively.
    /// An empty filter matches everything.
    pub fn value_matches_filter(&self, haystack: &str) -> bool {
        if self.value_filter.is_empty() {
            return true;
        }
        haystack
            .to_lowercase()
            .contains(&self.value_filter.to_lowercase())
    }

    pub fn clear_value_filter(&mut self) {
        self.value_filter.clear();
    }

    /// Cheap fingerprint of the interactive state a key press may have claimed.
    ///
    /// `app.rs` compares this before and after dispatching a key to decide
    /// whether the key browser should also see it. Comparing state rather than
    /// the returned action is necessary because prompts return `None` while
    /// still consuming the keystroke.
    pub fn consumed_state(&self) -> (bool, bool, bool, bool) {
        (
            self.edit_mode,
            self.text_editor.is_some(),
            self.list_prompt.is_some(),
            self.prompt_mode.is_some(),
        )
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InspectorAction> {
        self.handle_event_with_keymap(event, &crate::config::keybindings::Keymap::default())
    }

    pub fn handle_event_with_keymap(
        &mut self,
        event: &Event,
        keymap: &crate::config::keybindings::Keymap,
    ) -> Option<InspectorAction> {
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
                    Some(PromptMode::ListInsertPosition(pivot)) => {
                        // Accept b/B/a/A; anything else falls back to AFTER so
                        // the two-step prompt can never dead-end.
                        let before = matches!(val.trim(), "b" | "B" | "before");
                        self.list_prompt = Some(InputWidget::new("Value to insert"));
                        self.prompt_mode = Some(PromptMode::ListInsertValue { pivot, before });
                        None
                    }
                    Some(PromptMode::ListInsertValue { pivot, before }) => {
                        Some(InspectorAction::ListInsert {
                            key,
                            pivot,
                            value: val,
                            before,
                        })
                    }
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
                    Some(PromptMode::HashFilter) => {
                        // An empty submission clears the filter.
                        self.hash_filter = if val.trim().is_empty() {
                            String::new()
                        } else {
                            val
                        };
                        self.hash_cursor = 0;
                        None
                    }
                    Some(PromptMode::ValueFilter) => {
                        self.value_filter = val.trim().to_string();
                        self.list_cursor = 0;
                        self.hash_cursor = 0;
                        self.set_cursor = 0;
                        self.zset_cursor = 0;
                        self.stream_cursor = 0;
                        None
                    }
                    Some(PromptMode::SetAddMember) => {
                        Some(InspectorAction::SetAdd { key, member: val })
                    }
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
                        Some(InspectorAction::StreamAdd {
                            key,
                            entry_id,
                            fields,
                        })
                    }
                    Some(PromptMode::StreamTrimStrategy) => {
                        let strategy = val.trim().to_ascii_uppercase();
                        if strategy == "MAXLEN" || strategy == "MINID" {
                            let label = if strategy == "MAXLEN" {
                                "Approx max length (number)"
                            } else {
                                "Minimum id (e.g. 1526984818136-0)"
                            };
                            self.list_prompt = Some(InputWidget::new(label));
                            self.prompt_mode = Some(PromptMode::StreamTrimThreshold(strategy));
                            None
                        } else {
                            // Invalid strategy: report through the status bar
                            // rather than silently sending a bad command.
                            Some(InspectorAction::InvalidInput(
                                "XTRIM strategy must be MAXLEN or MINID".to_string(),
                            ))
                        }
                    }
                    Some(PromptMode::StreamTrimThreshold(strategy)) => {
                        let threshold = val.trim().to_string();
                        if threshold.is_empty() {
                            Some(InspectorAction::InvalidInput(
                                "XTRIM requires a threshold".to_string(),
                            ))
                        } else {
                            Some(InspectorAction::StreamTrim {
                                key,
                                strategy,
                                threshold,
                                approximate: false,
                            })
                        }
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
                        let min = if val.trim().is_empty() {
                            "-inf".to_string()
                        } else {
                            val.trim().to_string()
                        };
                        self.list_prompt = Some(InputWidget::new("Max score (empty=+inf):"));
                        self.prompt_mode = Some(PromptMode::ZSetScoreMax(min));
                        None
                    }
                    Some(PromptMode::ZSetScoreMax(min)) => {
                        let max = if val.trim().is_empty() {
                            "+inf".to_string()
                        } else {
                            val.trim().to_string()
                        };
                        self.zset_score_min = Some(min);
                        self.zset_score_max = Some(max);
                        None
                    }
                    None => None,
                };
            }
            if prompt.cancelled {
                // Esc on the filter prompt clears the filter rather than
                // leaving it applied.
                if self.prompt_mode == Some(PromptMode::HashFilter) {
                    self.hash_filter = String::new();
                    self.hash_cursor = 0;
                }
                if self.prompt_mode == Some(PromptMode::ValueFilter) {
                    self.clear_value_filter();
                }
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
            if self.prefer_external_editor {
                return Some(InspectorAction::EditStringExternal {
                    key: self.key.clone().unwrap_or_default(),
                    value: s.clone(),
                });
            }
            self.edit_mode = true;
            self.text_editor = Some(TextAreaEditor::new(s.clone()));
        }
        if keymap.matches("set_ttl", key) {
            self.list_prompt = Some(InputWidget::new("TTL in seconds (blank=persist)"));
            self.prompt_mode = Some(PromptMode::SetTtlSeconds);
        }
        if keymap.matches("inspector_value_search", key) && !self.value_filter.is_empty() {
            // Pressing the binding again with an active filter clears it,
            // giving a single toggle without a separate cancel step.
            self.clear_value_filter();
        } else if keymap.matches("inspector_value_search", key) {
            self.list_prompt = Some(InputWidget::new(
                "Search value members (empty = all, Esc clears)",
            ));
            self.prompt_mode = Some(PromptMode::ValueFilter);
        }
        if keymap.matches("copy", key)
            && let Some(RedisValue::String(ref s)) = self.value
        {
            return Some(InspectorAction::CopyValue(s.clone()));
        }

        // List-specific key handling
        if let Some(RedisValue::List(ref items)) = self.value {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if self.list_cursor > 0 => {
                    self.list_cursor -= 1;
                }
                KeyCode::Down | KeyCode::Char('j') if self.list_cursor + 1 < items.len() => {
                    self.list_cursor += 1;
                }
                _ => {}
            }
            if keymap.matches("inspector_list_push", key) {
                self.list_prompt = Some(InputWidget::new("Value to RPUSH"));
                self.prompt_mode = Some(PromptMode::Rpush);
            } else if keymap.matches("inspector_list_insert", key) {
                if let Some(pivot) = items.get(self.list_cursor) {
                    self.list_prompt =
                        Some(InputWidget::new("Insert [b]efore or [a]fter this element?"));
                    self.prompt_mode = Some(PromptMode::ListInsertPosition(pivot.clone()));
                }
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
            // Navigation is scoped to the fields visible under the active
            // filter, so the cursor cannot point at a hidden row.
            let visible: Vec<String> = self.visible_hash_fields().into_iter().cloned().collect();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.hash_cursor > 0 {
                        self.hash_cursor -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') if self.hash_cursor + 1 < visible.len() => {
                    self.hash_cursor += 1;
                }
                _ => {}
            }

            let current = visible.get(self.hash_cursor).cloned();
            let current_val = current.as_ref().and_then(|f| entries.get(f)).cloned();

            if keymap.matches("inspector_hash_filter", key) {
                self.list_prompt =
                    Some(InputWidget::new("Filter fields (empty = all, Esc clears)"));
                self.prompt_mode = Some(PromptMode::HashFilter);
            } else if keymap.matches("select_all", key) {
                self.select_all_visible_hash_fields();
            } else if keymap.matches("toggle_select", key) {
                if let Some(field) = current {
                    self.toggle_hash_selection(&field);
                }
            } else if keymap.matches("delete_selected", key) {
                if !self.hash_selected.is_empty() {
                    let action = InspectorAction::HashDelMany {
                        key: self.key.clone().unwrap_or_default(),
                        fields: std::mem::take(&mut self.hash_selected),
                    };
                    self.hash_cursor = 0;
                    return Some(action);
                }
            } else if keymap.matches("inspector_hash_add", key) {
                self.list_prompt = Some(InputWidget::new("Field name"));
                self.prompt_mode = Some(PromptMode::HashAddField);
            } else if keymap.matches("delete", key) {
                if let Some(field) = current {
                    // With a selection active, delete removes the whole
                    // selection; otherwise just the field under the cursor.
                    if !self.hash_selected.is_empty() {
                        let action = InspectorAction::HashDelMany {
                            key: self.key.clone().unwrap_or_default(),
                            fields: std::mem::take(&mut self.hash_selected),
                        };
                        self.hash_cursor = 0;
                        return Some(action);
                    }
                    return Some(InspectorAction::HashDel {
                        key: self.key.clone().unwrap_or_default(),
                        field,
                    });
                }
            } else if keymap.matches("edit", key) {
                if let (Some(field), Some(val)) = (current, current_val) {
                    let mut prompt = InputWidget::new(format!("Value for '{}'", field));
                    prompt.value = val.clone();
                    prompt.cursor = val.len();
                    self.prompt_mode = Some(PromptMode::HashEdit(field, self.hash_cursor));
                    self.list_prompt = Some(prompt);
                }
            } else if keymap.matches("copy", key)
                && let Some(val) = current_val
            {
                return Some(InspectorAction::CopyValue(val));
            }
        }

        // Set-specific key handling
        if let Some(RedisValue::Set(ref members)) = self.value {
            let items: Vec<&String> = members.iter().collect();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if self.set_cursor > 0 => {
                    self.set_cursor -= 1;
                }
                KeyCode::Down | KeyCode::Char('j') if self.set_cursor + 1 < items.len() => {
                    self.set_cursor += 1;
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
                KeyCode::Up | KeyCode::Char('k') if self.zset_cursor > 0 => {
                    self.zset_cursor -= 1;
                }
                KeyCode::Down | KeyCode::Char('j') if self.zset_cursor + 1 < entries.len() => {
                    self.zset_cursor += 1;
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
                    self.prompt_mode = Some(PromptMode::ZSetEditScore(
                        entry.member.clone(),
                        self.zset_cursor,
                    ));
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
                KeyCode::Up | KeyCode::Char('k') if self.stream_cursor > 0 => {
                    self.stream_cursor -= 1;
                }
                KeyCode::Down | KeyCode::Char('j') if self.stream_cursor + 1 < entries.len() => {
                    self.stream_cursor += 1;
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
            } else if keymap.matches("inspector_stream_follow", key) {
                self.stream_follow = !self.stream_follow;
                if self.stream_follow {
                    // Jump to the tail so follow mode tracks new entries.
                    if !entries.is_empty() {
                        self.stream_cursor = entries.len() - 1;
                    }
                    return Some(InspectorAction::StreamRefresh);
                }
            } else if keymap.matches("inspector_toggle_view", key) {
                self.stream_compact = !self.stream_compact;
            } else if keymap.matches("inspector_goto_end", key) {
                if !entries.is_empty() {
                    self.stream_cursor = entries.len() - 1;
                }
            } else if keymap.matches("inspector_stream_groups", key) {
                return Some(InspectorAction::StreamToggleGroups);
            } else if keymap.matches("inspector_stream_trim", key) {
                self.list_prompt = Some(InputWidget::new("Trim strategy: MAXLEN or MINID"));
                self.prompt_mode = Some(PromptMode::StreamTrimStrategy);
            } else if keymap.matches("copy", key)
                && let Some(entry) = entries.get(self.stream_cursor)
            {
                return Some(InspectorAction::CopyValue(entry.id.clone()));
            }
        }

        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_value(frame, area);

        // The editor draws as a centred popup over the value pane rather than
        // replacing it, so the value being edited stays visible around the
        // edges while typing.
        if self.edit_mode
            && let Some(ref editor) = self.text_editor
        {
            editor.render_popup(frame, area, self.key.as_deref());
        }
    }

    fn render_value(&self, frame: &mut Frame, area: Rect) {
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
                Style::default().fg(theme::current().title),
            ));
        }
        if let Some(ref ttl) = self.ttl {
            let ttl_str = ttl.display();
            if !ttl_str.is_empty() {
                let ttl_color = match ttl {
                    Ttl::NoExpiry => theme::current().ttl_ok,
                    Ttl::Expires(d) if d.as_secs() < 60 => theme::current().ttl_crit,
                    Ttl::Expires(d) if d.as_secs() < 600 => theme::current().ttl_warn,
                    _ => theme::current().ttl_ok,
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
                Style::default().fg(theme::current().memory),
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
                Style::default().fg(theme::current().border),
            );
            lines.push(Line::from(vec![label, Span::raw(" ")]));
            // Truncate before rendering. Building a styled line per source line
            // for a multi-megabyte value is what blows the frame budget, so the
            // cut happens on the raw string rather than on the output.
            let budget = self.max_value_display_bytes;
            let truncated = s.len() > budget;
            let shown = if truncated {
                // Cut on a char boundary: a byte index inside a multi-byte
                // codepoint would panic.
                let mut end = budget;
                while end > 0 && !s.is_char_boundary(end) {
                    end -= 1;
                }
                &s[..end]
            } else {
                s.as_str()
            };
            lines.extend(self.string_view.render_lines(shown));
            if truncated {
                lines.push(Line::from(Span::styled(
                    format!(
                        "... truncated at {} of {} bytes (max_value_display_bytes)",
                        budget,
                        s.len()
                    ),
                    Style::default().fg(theme::current().border),
                )));
            }
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border_style())
                    .title_style(theme::title_style())
                    .title(title),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
    }

    fn render_list(&self, frame: &mut Frame, area: Rect, items: &[String]) {
        let visible: Vec<&String> = items
            .iter()
            .filter(|item| self.value_matches_filter(item))
            .collect();
        let title = self.value_title("List", "len", items.len(), visible.len());

        let mut lines: Vec<Line> = Vec::new();
        if visible.is_empty() {
            lines.push(Line::from(Span::styled(
                if self.value_filter.is_empty() {
                    "No items"
                } else {
                    "No items match the filter"
                },
                Style::default().fg(theme::current().border),
            )));
        }
        for (i, item) in items.iter().enumerate() {
            if !self.value_matches_filter(item) {
                continue;
            }
            let marker = if i == self.list_cursor { ">" } else { " " };
            let style = if i == self.list_cursor {
                Style::default().add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}[{i}] {item}"),
                style,
            )));
        }

        if let Some(ref prompt) = self.list_prompt {
            let label = match self.prompt_mode {
                Some(PromptMode::Rpush) => "RPUSH value:".to_string(),
                Some(PromptMode::Lpush) => "LPUSH value:".to_string(),
                Some(PromptMode::Lset(idx)) => format!("LSET [{idx}] ="),
                Some(PromptMode::ListInsertPosition(ref pivot)) => {
                    format!("Insert before/after '{pivot}' (b/a): ")
                }
                Some(PromptMode::ListInsertValue { ref pivot, before }) => format!(
                    "Insert {} '{pivot}': ",
                    if before { "BEFORE" } else { "AFTER" }
                ),
                Some(PromptMode::ValueFilter) => "Search: ".to_string(),
                _ => "Input: ".to_string(),
            };
            lines.push(Line::from(Span::styled(
                format!("{label}{}", prompt_display(prompt)),
                Style::default().fg(theme::current().prompt),
            )));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(title);
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    /// Title for a collection view, reporting the active member filter.
    ///
    /// `noun` is the count label ("len" for sequences, "fields" for hashes).
    fn value_title(&self, kind: &str, noun: &str, total: usize, visible: usize) -> String {
        let base = match self.key.as_deref() {
            Some(k) => format!("{kind} [{k}] ({noun}={visible}"),
            None => format!("{kind} ({noun}={visible}"),
        };
        if visible != total {
            format!("{base}/{total}")
        } else {
            base
        }
    }

    fn render_hash(
        &self,
        frame: &mut Frame,
        area: Rect,
        entries: &indexmap::IndexMap<String, String>,
    ) {
        let visible: Vec<&String> = self
            .visible_hash_fields()
            .into_iter()
            .filter(|f| {
                // A hash match on either the field name or its value.
                self.value_matches_filter(f)
                    || entries
                        .get(*f)
                        .is_some_and(|v| self.value_matches_filter(v))
            })
            .collect();
        let total = entries.len();
        let filtered = !self.hash_filter.is_empty();

        let mut title = self.value_title("Hash", "fields", total, visible.len());
        if filtered {
            title.push_str(&format!(" field-filter:{}", self.hash_filter));
        }
        if !self.value_filter.is_empty() {
            title.push_str(&format!(" filter:{}", self.value_filter));
        }
        if !self.hash_selected.is_empty() {
            title.push_str(&format!(" [{} selected]", self.hash_selected.len()));
        }

        let mut lines: Vec<Line> = Vec::new();
        if visible.is_empty() {
            lines.push(Line::from(Span::styled(
                if filtered || !self.value_filter.is_empty() {
                    "No fields match the filter"
                } else {
                    "No fields"
                },
                Style::default().fg(theme::current().border),
            )));
        }
        for (i, field) in visible.iter().enumerate() {
            let cursor_marker = if i == self.hash_cursor { ">" } else { " " };
            let selected_marker = if self.hash_selected.iter().any(|f| f == *field) {
                "*"
            } else {
                " "
            };
            let val = entries.get(*field).map(String::as_str).unwrap_or("");
            let style = if self.hash_cursor == i {
                Style::default().add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{cursor_marker}{selected_marker}{field}: {val}"),
                style,
            )));
        }

        if let Some(ref prompt) = self.list_prompt {
            let label = match self.prompt_mode {
                Some(PromptMode::HashAddField) => "Field name: ".to_string(),
                Some(PromptMode::HashAddValue(ref f)) => format!("Value for '{f}': "),
                Some(PromptMode::HashEdit(ref f, _)) => format!("Value for '{f}': "),
                Some(PromptMode::HashFilter) => "Filter: ".to_string(),
                _ => "Input: ".to_string(),
            };
            lines.push(Line::from(Span::styled(
                format!("{label}{}", prompt_display(prompt)),
                Style::default().fg(theme::current().prompt),
            )));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(title);
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_set(
        &self,
        frame: &mut Frame,
        area: Rect,
        members: &std::collections::BTreeSet<String>,
    ) {
        let items: Vec<&String> = members
            .iter()
            .filter(|m| self.value_matches_filter(m))
            .collect();
        let title = self.value_title("Set", "len", members.len(), items.len());

        let mut lines: Vec<Line> = Vec::new();
        if items.is_empty() {
            lines.push(Line::from(Span::styled(
                if self.value_filter.is_empty() {
                    "No members"
                } else {
                    "No members match the filter"
                },
                Style::default().fg(theme::current().border),
            )));
        }
        for (i, member) in members.iter().enumerate() {
            if !self.value_matches_filter(member) {
                continue;
            }
            let marker = if i == self.set_cursor { ">" } else { " " };
            let style = if i == self.set_cursor {
                Style::default().add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(format!("{marker}{member}"), style)));
        }

        if let Some(ref prompt) = self.list_prompt {
            let label = match self.prompt_mode {
                Some(PromptMode::SetAddMember) => "Member to SADD: ",
                Some(PromptMode::ValueFilter) => "Search: ",
                _ => "Input: ",
            };
            lines.push(Line::from(Span::styled(
                format!("{label}{}", prompt_display(prompt)),
                Style::default().fg(theme::current().prompt),
            )));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(title);
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_zset(&self, frame: &mut Frame, area: Rect, entries: &[ZSetEntry]) {
        let mode_label = if self.zset_sort_score_asc {
            "asc"
        } else {
            "desc"
        };
        let filtered = self.zset_score_min.is_some() || self.zset_score_max.is_some();
        let visible: Vec<&ZSetEntry> = entries
            .iter()
            .filter(|e| {
                let above_min = self.zset_score_min.as_ref().is_none_or(|min| {
                    if min == "-inf" {
                        true
                    } else {
                        e.score >= min.parse::<f64>().unwrap_or(f64::NEG_INFINITY)
                    }
                });
                let below_max = self.zset_score_max.as_ref().is_none_or(|max| {
                    if max == "+inf" {
                        true
                    } else {
                        e.score <= max.parse::<f64>().unwrap_or(f64::INFINITY)
                    }
                });
                above_min && below_max && self.value_matches_filter(&e.member)
            })
            .collect();
        let title = self
            .key
            .as_deref()
            .map(|k| {
                let base = format!("ZSet [{}] (len={}, score:{})", k, visible.len(), mode_label);
                if filtered {
                    format!(
                        "{} filter:{}~{}",
                        base,
                        self.zset_score_min.as_deref().unwrap_or("-inf"),
                        self.zset_score_max.as_deref().unwrap_or("+inf")
                    )
                } else {
                    base
                }
            })
            .unwrap_or_else(|| format!("ZSet (len={})", visible.len()));
        let title = if self.value_filter.is_empty() {
            title
        } else {
            format!("{title} filter:{}", self.value_filter)
        };

        let mut items: Vec<&&ZSetEntry> = visible.iter().collect();
        if self.zset_sort_score_asc {
            items.sort_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            items.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let mut text = String::new();
        for (i, entry) in items.iter().enumerate() {
            let marker = if i == self.zset_cursor { ">" } else { " " };
            text.push_str(&format!(
                "{}[{}] {} {}\n",
                marker, i, entry.score, entry.member
            ));
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
                if prompt.cursor >= prompt.value.len() {
                    "_"
                } else {
                    " "
                }
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(title);
        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_stream(&self, frame: &mut Frame, area: Rect, entries: &[StreamEntry]) {
        let mode = if self.stream_compact {
            "compact"
        } else {
            "full"
        };
        let follow = if self.stream_follow { ", follow" } else { "" };
        let title = |k: Option<&str>| match k {
            Some(k) => format!("Stream [{k}] (len={}, {mode}{follow})", entries.len()),
            None => format!("Stream (len={}, {mode}{follow})", entries.len()),
        };

        if self.stream_show_groups {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            let left = chunks[0];
            let right = chunks[1];

            // Left: stream messages
            let title_left = title(self.key.as_deref());
            let mut text = String::new();
            for (i, entry) in entries.iter().enumerate() {
                let marker = if i == self.stream_cursor { ">" } else { " " };
                if self.stream_compact {
                    text.push_str(&format!(
                        "{}[{}] {} ({} fields)\n",
                        marker,
                        i,
                        entry.id,
                        entry.fields.len()
                    ));
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
                    Some(PromptMode::StreamTrimStrategy) => "Trim strategy (MAXLEN/MINID): ",
                    Some(PromptMode::StreamTrimThreshold(ref s)) => {
                        &format!("XTRIM {s} threshold: ")
                    }
                    Some(PromptMode::StreamAddFields(_)) => "Fields: ",
                    _ => "Input: ",
                };
                text.push_str(&format!("\n{} {}", prompt_label, prompt_display(prompt)));
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_style())
                .title_style(theme::title_style())
                .title(title_left);
            let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, left);

            // Right: consumer groups
            let title_right = "Consumer Groups";
            if let Some(ref groups) = self.stream_groups {
                let header = TableRow::new(vec![
                    Cell::from("Group"),
                    Cell::from("Consumers"),
                    Cell::from("Pending"),
                    Cell::from("Last ID"),
                ])
                .style(Style::default().fg(theme::current().warning));
                let rows: Vec<TableRow> = groups
                    .iter()
                    .map(|g| {
                        TableRow::new(vec![
                            Cell::from(g.name.clone()),
                            Cell::from(g.consumers.to_string()),
                            Cell::from(g.pending.to_string()),
                            Cell::from(g.last_delivered_id.clone()),
                        ])
                    })
                    .collect();
                let table = Table::new(
                    rows,
                    vec![
                        Constraint::Percentage(30),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                        Constraint::Percentage(30),
                    ],
                )
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border_style())
                        .title_style(theme::title_style())
                        .title(title_right),
                );
                frame.render_widget(table, right);
            } else {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border_style())
                    .title_style(theme::title_style())
                    .title(title_right);
                let paragraph = Paragraph::new("Loading groups...").block(block);
                frame.render_widget(paragraph, right);
            }
        } else {
            let title = title(self.key.as_deref());

            let mut text = String::new();
            for (i, entry) in entries.iter().enumerate() {
                let marker = if i == self.stream_cursor { ">" } else { " " };
                if self.stream_compact {
                    text.push_str(&format!(
                        "{}[{}] {} ({} fields)\n",
                        marker,
                        i,
                        entry.id,
                        entry.fields.len()
                    ));
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
                    Some(PromptMode::StreamTrimStrategy) => "Trim strategy (MAXLEN/MINID): ",
                    Some(PromptMode::StreamTrimThreshold(ref s)) => {
                        &format!("XTRIM {s} threshold: ")
                    }
                    Some(PromptMode::StreamAddFields(_)) => "Fields: ",
                    _ => "Input: ",
                };
                text.push_str(&format!("\n{} {}", prompt_label, prompt_display(prompt)));
            }

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_style())
                .title_style(theme::title_style())
                .title(title);
            let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
    }
}

/// Render a prompt's value with a visible caret at the cursor position.
fn prompt_display(prompt: &InputWidget) -> String {
    let value = &prompt.value;
    if prompt.cursor >= value.len() {
        return format!("{value}_");
    }
    let mut out = String::with_capacity(value.len() + 1);
    for (i, ch) in value.chars().enumerate() {
        if i == prompt.cursor {
            out.push('_');
        }
        out.push(ch);
    }
    out.push('_');
    out
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    // ---- Value member search (5.14) ----

    fn ctrl_shift_f() -> Event {
        Event::Key(KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ))
    }

    #[test]
    fn value_filter_defaults_empty_and_matches_all() {
        let inspector = ValueInspector::new();
        assert!(inspector.value_filter.is_empty());
        assert!(inspector.value_matches_filter("anything"));
    }

    #[test]
    fn value_filter_matches_substring_case_insensitively() {
        let mut inspector = ValueInspector::new();
        inspector.value_filter = "ERR".to_string();
        assert!(inspector.value_matches_filter("server_error"));
        assert!(!inspector.value_matches_filter("client"));
    }

    #[test]
    fn value_search_opens_prompt() {
        let mut inspector = list_inspector();
        inspector.handle_event(&ctrl_shift_f());
        assert_eq!(inspector.prompt_mode, Some(PromptMode::ValueFilter));
    }

    #[test]
    fn value_search_applies_on_submit() {
        let mut inspector = list_inspector();
        inspector.handle_event(&ctrl_shift_f());
        inspector.list_prompt.as_mut().unwrap().value = "b".to_string();
        inspector.handle_event(&press(KeyCode::Enter));
        assert_eq!(inspector.value_filter, "b");
    }

    #[test]
    fn value_search_empty_submission_clears() {
        let mut inspector = list_inspector();
        inspector.value_filter = "b".to_string();
        // Pressing the binding with an active filter clears it in one step.
        inspector.handle_event(&ctrl_shift_f());
        assert_eq!(inspector.prompt_mode, None);
        assert!(inspector.value_filter.is_empty());

        // With no filter active it opens the prompt, and an empty submission
        // leaves the filter cleared.
        inspector.handle_event(&ctrl_shift_f());
        assert_eq!(inspector.prompt_mode, Some(PromptMode::ValueFilter));
        inspector.handle_event(&press(KeyCode::Enter));
        assert!(inspector.value_filter.is_empty());
    }

    #[test]
    fn value_search_twice_toggles_filter_off() {
        let mut inspector = list_inspector();
        inspector.value_filter = "b".to_string();
        inspector.handle_event(&ctrl_shift_f());
        assert!(inspector.value_filter.is_empty());
    }

    #[test]
    fn value_search_esc_clears_filter() {
        let mut inspector = list_inspector();
        inspector.value_filter = "b".to_string();
        inspector.handle_event(&ctrl_shift_f());
        inspector.handle_event(&press(KeyCode::Esc));
        assert!(inspector.value_filter.is_empty());
    }

    #[test]
    fn value_filter_reset_on_new_value() {
        let mut inspector = list_inspector();
        inspector.value_filter = "b".to_string();
        inspector.set_value("l2".to_string(), RedisValue::List(vec![]));
        assert!(inspector.value_filter.is_empty());
    }

    #[test]
    fn render_list_respects_value_filter() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = list_inspector();
        inspector.value_filter = "nomatch".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        let rendered = buffer_text(&terminal);
        assert!(
            rendered.contains("No items match"),
            "expected empty-filter message, got:\n{rendered}"
        );
    }

    #[test]
    fn render_list_shows_only_matching_items() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = list_inspector();
        inspector.value_filter = "a".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        let rendered = buffer_text(&terminal);
        assert!(rendered.contains("[0] a"));
        assert!(!rendered.contains("[1] b"), "filtered-out item still shown");
    }

    #[test]
    fn render_hash_value_filter_matches_values_too() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = hash_inspector();
        // No field name contains "xyz" but one value does.
        inspector.value_filter = "xyz".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        let rendered = buffer_text(&terminal);
        assert!(rendered.contains("session:token"));
        assert!(!rendered.contains("user:name"));
    }

    #[test]
    fn render_set_respects_value_filter() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "s".to_string(),
            RedisValue::Set(["apple".to_string(), "banana".to_string()].into()),
        );
        inspector.value_filter = "app".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        let rendered = buffer_text(&terminal);
        assert!(rendered.contains("apple"));
        assert!(!rendered.contains("banana"));
    }

    #[test]
    fn render_zset_respects_value_filter() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "z".to_string(),
            RedisValue::ZSet(vec![
                ZSetEntry {
                    score: 1.0,
                    member: "alpha".to_string(),
                },
                ZSetEntry {
                    score: 2.0,
                    member: "beta".to_string(),
                },
            ]),
        );
        inspector.value_filter = "bet".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        let rendered = buffer_text(&terminal);
        assert!(rendered.contains("beta"));
        assert!(!rendered.contains("alpha"));
    }

    #[test]
    fn render_with_value_filter_prompt_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        for value in [
            RedisValue::List(vec!["a".into()]),
            RedisValue::Hash(indexmap::IndexMap::from([(
                "f".to_string(),
                "v".to_string(),
            )])),
            RedisValue::Set(["a".to_string()].into()),
            RedisValue::ZSet(vec![ZSetEntry {
                score: 1.0,
                member: "m".to_string(),
            }]),
        ] {
            let backend = TestBackend::new(80, 24);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut inspector = ValueInspector::new();
            inspector.set_value("k".to_string(), value);
            inspector.handle_event(&ctrl_shift_f());
            terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        }
    }

    /// Flatten a TestBackend buffer into one searchable string.
    fn buffer_text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    // ---- Stream follow mode (2.18) ----

    fn entry(id: &str) -> StreamEntry {
        StreamEntry {
            id: id.to_string(),
            fields: [("f".to_string(), id.to_string())].into(),
        }
    }

    #[test]
    fn follow_defaults_off() {
        let inspector = ValueInspector::new();
        assert!(!inspector.stream_follow);
    }

    #[test]
    fn follow_key_toggles_and_requests_refresh() {
        let mut inspector = stream_inspector();
        let action = inspector.handle_event(&press(KeyCode::Char('F')));
        assert!(inspector.stream_follow);
        assert_eq!(action, Some(InspectorAction::StreamRefresh));
    }

    #[test]
    fn follow_key_off_does_not_request_refresh() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&press(KeyCode::Char('F')));
        assert_eq!(inspector.handle_event(&press(KeyCode::Char('F'))), None);
        assert!(!inspector.stream_follow);
    }

    #[test]
    fn follow_jumps_cursor_to_tail_when_enabled() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&press(KeyCode::Char('F')));
        assert_eq!(inspector.stream_cursor, 1);
    }

    #[test]
    fn follow_leaves_compact_toggle_on_f_untouched() {
        let mut inspector = stream_inspector();
        let before = inspector.stream_compact;
        inspector.handle_event(&press(KeyCode::Char('f')));
        assert!(inspector.stream_compact != before);
        assert!(!inspector.stream_follow);
    }

    #[test]
    fn follow_tick_returns_last_id_when_following() {
        let mut inspector = stream_inspector();
        inspector.stream_follow = true;
        assert_eq!(inspector.follow_tick(), Some("2-0".to_string()));
    }

    #[test]
    fn follow_tick_returns_none_when_not_following() {
        let inspector = stream_inspector();
        assert_eq!(inspector.follow_tick(), None);
    }

    #[test]
    fn follow_tick_returns_none_for_non_stream_value() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("l".into(), RedisValue::List(vec!["a".into()]));
        inspector.stream_follow = true;
        assert_eq!(inspector.follow_tick(), None);
    }

    #[test]
    fn follow_tick_returns_none_for_empty_stream() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("s".into(), RedisValue::Stream(vec![]));
        inspector.stream_follow = true;
        assert_eq!(inspector.follow_tick(), None);
    }

    #[test]
    fn append_stream_entries_adds_new_entries() {
        let mut inspector = stream_inspector();
        let added = inspector.append_stream_entries(vec![entry("3-0"), entry("4-0")]);
        assert_eq!(added, 2);
        match inspector.value.as_ref().unwrap() {
            RedisValue::Stream(entries) => assert_eq!(entries.len(), 4),
            other => panic!("expected stream, got {other:?}"),
        }
    }

    #[test]
    fn append_stream_entries_dedupes_by_id() {
        let mut inspector = stream_inspector();
        // Re-fetching the same tail must not duplicate rows.
        let added = inspector.append_stream_entries(vec![entry("2-0"), entry("3-0")]);
        assert_eq!(added, 1);
        match inspector.value.as_ref().unwrap() {
            RedisValue::Stream(entries) => assert_eq!(entries.len(), 3),
            other => panic!("expected stream, got {other:?}"),
        }
    }

    #[test]
    fn append_stream_entries_caps_buffer() {
        let mut inspector = ValueInspector::new();
        let initial: Vec<StreamEntry> = (0..5_000).map(|i| entry(&format!("{i}-0"))).collect();
        inspector.set_value("s".to_string(), RedisValue::Stream(initial));
        let added = inspector.append_stream_entries(vec![entry("9999-0")]);
        assert_eq!(added, 1);
        match inspector.value.as_ref().unwrap() {
            RedisValue::Stream(entries) => {
                assert_eq!(entries.len(), 5_000, "buffer must stay capped");
                // Oldest entries dropped, newest retained.
                assert_eq!(entries.last().unwrap().id, "9999-0");
            }
            other => panic!("expected stream, got {other:?}"),
        }
    }

    #[test]
    fn append_stream_entries_on_non_stream_is_a_noop() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("l".into(), RedisValue::List(vec!["a".into()]));
        assert_eq!(inspector.append_stream_entries(vec![entry("1-0")]), 0);
    }

    #[test]
    fn follow_cursor_to_tail_pins_newest() {
        let mut inspector = stream_inspector();
        inspector.append_stream_entries(vec![entry("3-0")]);
        inspector.stream_cursor = 0;
        inspector.follow_cursor_to_tail();
        assert_eq!(inspector.stream_cursor, 2);
    }

    #[test]
    fn follow_reset_on_new_value() {
        let mut inspector = stream_inspector();
        inspector.stream_follow = true;
        inspector.set_value("s2".to_string(), RedisValue::Stream(vec![]));
        assert!(!inspector.stream_follow);
    }

    #[test]
    fn render_stream_shows_follow_indicator() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = stream_inspector();
        inspector.stream_follow = true;
        inspector.append_stream_entries(vec![entry("3-0")]);
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();

        let buffer = terminal.backend().buffer().clone();
        let rendered: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            rendered.contains("follow"),
            "title should show follow state"
        );
    }

    #[test]
    fn render_stream_follow_with_groups_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = stream_inspector();
        inspector.stream_follow = true;
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(Vec::new());
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    // ---- List insert (2.9) ----

    fn list_inspector() -> ValueInspector {
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "l".to_string(),
            RedisValue::List(vec!["a".to_string(), "b".to_string()]),
        );
        inspector
    }

    fn stream_inspector() -> ValueInspector {
        let mut inspector = ValueInspector::new();
        inspector.set_value(
            "s".to_string(),
            RedisValue::Stream(vec![
                StreamEntry {
                    id: "1-0".to_string(),
                    fields: [("f".to_string(), "v".to_string())].into(),
                },
                StreamEntry {
                    id: "2-0".to_string(),
                    fields: [("f".to_string(), "v".to_string())].into(),
                },
            ]),
        );
        inspector
    }

    #[test]
    fn list_insert_opens_position_prompt_on_cursor_item() {
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('I')));
        assert_eq!(
            inspector.prompt_mode,
            Some(PromptMode::ListInsertPosition("a".to_string()))
        );
    }

    #[test]
    fn list_insert_position_b_then_value_emits_before_action() {
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('I')));
        inspector.handle_event(&press(KeyCode::Char('b')));
        // Submit the position, then supply the value.
        inspector.handle_event(&press(KeyCode::Enter));
        assert_eq!(
            inspector.prompt_mode,
            Some(PromptMode::ListInsertValue {
                pivot: "a".to_string(),
                before: true
            })
        );
        inspector.list_prompt.as_mut().unwrap().value = "x".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::ListInsert {
                key,
                pivot,
                value,
                before,
            }) => {
                assert_eq!(key, "l");
                assert_eq!(pivot, "a");
                assert_eq!(value, "x");
                assert!(before);
            }
            other => panic!("expected ListInsert, got {other:?}"),
        }
    }

    #[test]
    fn list_insert_position_a_then_value_emits_after_action() {
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('I')));
        inspector.handle_event(&press(KeyCode::Char('a')));
        inspector.handle_event(&press(KeyCode::Enter));
        inspector.list_prompt.as_mut().unwrap().value = "y".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::ListInsert { before, pivot, .. }) => {
                assert!(!before);
                assert_eq!(pivot, "a");
            }
            other => panic!("expected ListInsert, got {other:?}"),
        }
    }

    #[test]
    fn list_insert_unrecognized_position_defaults_to_after() {
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('I')));
        inspector.handle_event(&press(KeyCode::Char('z')));
        inspector.handle_event(&press(KeyCode::Enter));
        inspector.list_prompt.as_mut().unwrap().value = "y".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::ListInsert { before, .. }) => assert!(!before),
            other => panic!("expected ListInsert, got {other:?}"),
        }
    }

    #[test]
    fn list_insert_targets_second_element_after_navigation() {
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('j')));
        inspector.handle_event(&press(KeyCode::Char('I')));
        assert_eq!(
            inspector.prompt_mode,
            Some(PromptMode::ListInsertPosition("b".to_string()))
        );
    }

    #[test]
    fn list_insert_on_empty_list_does_nothing() {
        let mut inspector = ValueInspector::new();
        inspector.set_value("l".to_string(), RedisValue::List(vec![]));
        inspector.handle_event(&press(KeyCode::Char('I')));
        assert_eq!(inspector.prompt_mode, None);
    }

    // ---- Stream XTRIM (2.20) ----

    #[test]
    fn stream_trim_opens_strategy_prompt() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        assert_eq!(inspector.prompt_mode, Some(PromptMode::StreamTrimStrategy));
    }

    #[test]
    fn stream_trim_maxlen_then_threshold_emits_action() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        inspector.list_prompt.as_mut().unwrap().value = "MAXLEN".to_string();
        inspector.handle_event(&press(KeyCode::Enter));
        inspector.list_prompt.as_mut().unwrap().value = "100".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::StreamTrim {
                key,
                strategy,
                threshold,
                ..
            }) => {
                assert_eq!(key, "s");
                assert_eq!(strategy, "MAXLEN");
                assert_eq!(threshold, "100");
            }
            other => panic!("expected StreamTrim, got {other:?}"),
        }
    }

    #[test]
    fn stream_trim_accepts_lowercase_strategy() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        inspector.list_prompt.as_mut().unwrap().value = "minid".to_string();
        inspector.handle_event(&press(KeyCode::Enter));
        inspector.list_prompt.as_mut().unwrap().value = "1000-0".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::StreamTrim {
                strategy,
                threshold,
                ..
            }) => {
                assert_eq!(strategy, "MINID");
                assert_eq!(threshold, "1000-0");
            }
            other => panic!("expected StreamTrim, got {other:?}"),
        }
    }

    #[test]
    fn stream_trim_rejects_unknown_strategy() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        inspector.list_prompt.as_mut().unwrap().value = "BOGUS".to_string();
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::InvalidInput(msg)) => {
                assert!(msg.contains("MAXLEN"), "got: {msg}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn stream_trim_rejects_empty_threshold() {
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        inspector.list_prompt.as_mut().unwrap().value = "MAXLEN".to_string();
        inspector.handle_event(&press(KeyCode::Enter));
        match inspector.handle_event(&press(KeyCode::Enter)) {
            Some(InspectorAction::InvalidInput(msg)) => {
                assert!(msg.contains("threshold"), "got: {msg}");
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn render_stream_with_trim_prompt_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = stream_inspector();
        inspector.handle_event(&ctrl('t'));
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn render_list_with_insert_prompt_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = list_inspector();
        inspector.handle_event(&press(KeyCode::Char('I')));
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    // ---- Hash filter + bulk select (2.11, 2.12) ----

    fn hash_inspector() -> ValueInspector {
        let mut inspector = ValueInspector::new();
        let mut map = indexmap::IndexMap::new();
        map.insert("user:name".to_string(), "ada".to_string());
        map.insert("user:email".to_string(), "a@b.c".to_string());
        map.insert("session:token".to_string(), "xyz".to_string());
        inspector.set_value("h".to_string(), RedisValue::Hash(map));
        inspector
    }

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn ctrl(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    #[test]
    fn visible_hash_fields_defaults_to_all() {
        let inspector = hash_inspector();
        assert_eq!(inspector.visible_hash_fields().len(), 3);
    }

    #[test]
    fn hash_filter_narrows_visible_fields() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "user:".to_string();
        let visible = inspector.visible_hash_fields();
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|f| f.starts_with("user:")));
    }

    #[test]
    fn hash_filter_is_case_insensitive() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "USER".to_string();
        assert_eq!(inspector.visible_hash_fields().len(), 2);
    }

    #[test]
    fn hash_filter_with_no_matches_yields_empty() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "zzz".to_string();
        assert!(inspector.visible_hash_fields().is_empty());
    }

    #[test]
    fn hash_filter_prompt_applies_and_clears() {
        let mut inspector = hash_inspector();
        // Open the filter prompt.
        assert!(inspector.handle_event(&ctrl('f')).is_none());
        assert_eq!(inspector.prompt_mode, Some(PromptMode::HashFilter));

        inspector.list_prompt.as_mut().unwrap().value = "session".to_string();
        inspector.handle_event(&press(KeyCode::Enter));

        assert_eq!(inspector.hash_filter, "session");
        assert_eq!(inspector.visible_hash_fields().len(), 1);
    }

    #[test]
    fn hash_filter_empty_submission_clears_filter() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "user:".to_string();
        inspector.handle_event(&ctrl('f'));
        inspector.handle_event(&press(KeyCode::Enter));
        assert_eq!(inspector.hash_filter, "");
        assert_eq!(inspector.visible_hash_fields().len(), 3);
    }

    #[test]
    fn hash_filter_reset_on_new_value() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "user".to_string();
        inspector.hash_selected.push("user:name".to_string());
        inspector.set_value(
            "h2".to_string(),
            RedisValue::Hash(indexmap::IndexMap::new()),
        );
        assert_eq!(inspector.hash_filter, "");
        assert!(inspector.hash_selected.is_empty());
    }

    #[test]
    fn toggle_hash_selection_adds_and_removes() {
        let mut inspector = hash_inspector();
        assert!(inspector.toggle_hash_selection("user:name"));
        assert_eq!(inspector.hash_selected, vec!["user:name".to_string()]);
        assert!(!inspector.toggle_hash_selection("user:name"));
        assert!(inspector.hash_selected.is_empty());
    }

    #[test]
    fn space_toggles_field_under_cursor() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&press(KeyCode::Char(' ')));
        assert_eq!(inspector.hash_selected.len(), 1);
        assert_eq!(inspector.hash_selected[0], "user:name");
    }

    #[test]
    fn space_navigation_then_toggle_targets_second_field() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&press(KeyCode::Char('j')));
        inspector.handle_event(&press(KeyCode::Char(' ')));
        assert_eq!(inspector.hash_selected, vec!["user:email".to_string()]);
    }

    #[test]
    fn ctrl_a_selects_all_visible_fields() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&ctrl('a'));
        assert_eq!(inspector.hash_selected.len(), 3);
    }

    #[test]
    fn ctrl_a_respects_active_filter() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "user:".to_string();
        inspector.handle_event(&ctrl('a'));
        assert_eq!(inspector.hash_selected.len(), 2);
    }

    #[test]
    fn ctrl_d_emits_bulk_delete() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&press(KeyCode::Char(' ')));
        let action = inspector.handle_event(&ctrl('d'));
        match action {
            Some(InspectorAction::HashDelMany { key, fields }) => {
                assert_eq!(key, "h");
                assert_eq!(fields, vec!["user:name".to_string()]);
            }
            other => panic!("expected HashDelMany, got {other:?}"),
        }
    }

    #[test]
    fn bulk_delete_clears_selection() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&press(KeyCode::Char(' ')));
        inspector.handle_event(&ctrl('d'));
        assert!(inspector.hash_selected.is_empty());
    }

    #[test]
    fn ctrl_d_with_empty_selection_is_a_no_op() {
        let mut inspector = hash_inspector();
        assert_eq!(inspector.handle_event(&ctrl('d')), None);
    }

    #[test]
    fn delete_with_selection_removes_all_selected() {
        let mut inspector = hash_inspector();
        inspector.handle_event(&press(KeyCode::Char(' ')));
        inspector.handle_event(&press(KeyCode::Char('j')));
        inspector.handle_event(&press(KeyCode::Char(' ')));
        match inspector.handle_event(&press(KeyCode::Char('D'))) {
            Some(InspectorAction::HashDelMany { fields, .. }) => assert_eq!(fields.len(), 2),
            other => panic!("expected HashDelMany, got {other:?}"),
        }
    }

    #[test]
    fn delete_without_selection_removes_single_field() {
        let mut inspector = hash_inspector();
        match inspector.handle_event(&press(KeyCode::Char('D'))) {
            Some(InspectorAction::HashDel { key, field }) => {
                assert_eq!(key, "h");
                assert_eq!(field, "user:name");
            }
            other => panic!("expected HashDel, got {other:?}"),
        }
    }

    #[test]
    fn hash_navigation_respects_filter_bounds() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "session".to_string();
        // Only one visible field, so Down cannot advance past it.
        inspector.handle_event(&press(KeyCode::Char('j')));
        assert_eq!(inspector.hash_cursor, 0);
        inspector.handle_event(&press(KeyCode::Char('k')));
        assert_eq!(inspector.hash_cursor, 0);
    }

    #[test]
    fn hash_edit_targets_filtered_field() {
        let mut inspector = hash_inspector();
        inspector.hash_filter = "session".to_string();
        inspector.handle_event(&press(KeyCode::Char('e')));
        match inspector.prompt_mode {
            Some(PromptMode::HashEdit(field, _)) => assert_eq!(field, "session:token"),
            other => panic!("expected HashEdit, got {other:?}"),
        }
    }

    #[test]
    fn render_hash_with_filter_and_selection_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = hash_inspector();
        inspector.hash_filter = "user".to_string();
        inspector.hash_selected.push("user:name".to_string());
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn render_hash_with_empty_filter_result_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = hash_inspector();
        inspector.hash_filter = "zzz".to_string();
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

    #[test]
    fn render_empty_hash_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.set_value("h".to_string(), RedisValue::Hash(indexmap::IndexMap::new()));
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }

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
        assert_eq!(find("\"name\"").style.fg, Some(json_key_color()));
        assert_eq!(find("\"ada\"").style.fg, Some(json_string_color()));
        assert_eq!(find("36").style.fg, Some(json_number_color()));
    }

    #[test]
    fn json_highlight_colors_literals() {
        let parsed: serde_json::Value = serde_json::from_str("{\"a\":true,\"b\":null}").unwrap();
        let lines = highlight_json(&parsed);
        let all: Vec<&Span> = lines.iter().flat_map(|l| l.spans.iter()).collect();
        for literal in ["true", "null"] {
            let span = all
                .iter()
                .find(|s| s.content.as_ref() == literal)
                .unwrap_or_else(|| panic!("{literal} span"));
            assert_eq!(span.style.fg, Some(json_literal_color()));
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
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
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
        assert!(
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .any(|s| s.style.fg.is_some())
        );
    }

    #[test]
    fn render_lines_non_json_modes_are_unstyled() {
        for mode in [
            StringViewMode::Raw,
            StringViewMode::Base64,
            StringViewMode::Hex,
        ] {
            let lines = mode.render_lines("hello");
            assert!(
                lines
                    .iter()
                    .flat_map(|l| l.spans.iter())
                    .all(|s| s.style.fg.is_none())
            );
        }
    }

    #[test]
    fn render_lines_invalid_json_is_plain_but_readable() {
        let lines = StringViewMode::Json.render_lines("<html>not json</html>");
        let joined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
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
            inspector.set_value(
                "k".into(),
                RedisValue::String(r#"{"a":[1,2],"b":null}"#.into()),
            );
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
        inspector.set_value("events".to_string(), RedisValue::Stream(vec![]));
        inspector.stream_show_groups = true;
        inspector.stream_groups = Some(vec![]);
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
    }
}

#[cfg(test)]
mod truncation_tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render an inspector holding `value` and return the visible text.
    fn render_text(value: &str, budget: usize, mode: StringViewMode) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut inspector = ValueInspector::new();
        inspector.max_value_display_bytes = budget;
        inspector.string_view = mode;
        inspector.set_value("k".to_string(), RedisValue::String(value.to_string()));
        terminal.draw(|f| inspector.render(f, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn small_value_is_not_truncated() {
        let text = render_text("hello", 1024, StringViewMode::Raw);
        assert!(text.contains("hello"), "got: {text}");
        assert!(!text.contains("truncated"), "got: {text}");
    }

    #[test]
    fn large_value_is_truncated_with_a_visible_marker() {
        let big = "x".repeat(5000);
        let text = render_text(&big, 100, StringViewMode::Raw);
        assert!(
            text.contains("truncated at 100 of 5000 bytes"),
            "expected a truncation marker, got: {text}"
        );
        assert!(text.contains("max_value_display_bytes"), "got: {text}");
    }

    /// A byte budget that lands mid-codepoint must cut on a char boundary
    /// rather than panicking on a slice that splits a character.
    #[test]
    fn truncation_respects_multi_byte_char_boundaries() {
        // Each char is 3 bytes, so a budget of 100 lands inside the 34th.
        let big = "あ".repeat(500);
        let text = render_text(&big, 100, StringViewMode::Raw);
        assert!(text.contains("truncated at 100"), "got: {text}");
        // The point is that rendering did not panic; the content is partial.
        assert!(!text.contains('\u{fffd}'), "no replacement chars: {text}");
    }

    /// The other view modes encode, so a raw byte budget is not a byte budget
    /// for their output. Truncating the input first keeps all of them bounded.
    #[test]
    fn truncation_applies_to_every_view_mode() {
        let big = "y".repeat(5000);
        for mode in [
            StringViewMode::Raw,
            StringViewMode::Json,
            StringViewMode::Base64,
            StringViewMode::Hex,
        ] {
            let text = render_text(&big, 100, mode.clone());
            assert!(
                text.contains("truncated at 100"),
                "{:?} should truncate, got: {text}",
                mode.label()
            );
        }
    }

    /// `0` in the config means no limit, and the inspector receives
    /// `usize::MAX` in that case.
    #[test]
    fn unlimited_budget_shows_the_whole_value() {
        let big = "z".repeat(20_000);
        let text = render_text(&big, usize::MAX, StringViewMode::Raw);
        assert!(!text.contains("truncated"), "got: {text}");
    }

    /// The budget exists because a huge value blew the frame budget. Assert the
    /// rendering actually stays cheap rather than just being short.
    #[test]
    fn truncation_keeps_render_cost_flat_as_the_value_grows() {
        let small = "a".repeat(100_000);
        let large = "a".repeat(2_000_000);

        let time = |value: &str| {
            let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
            let mut inspector = ValueInspector::new();
            inspector.max_value_display_bytes = 4096;
            inspector.set_value("k".to_string(), RedisValue::String(value.to_string()));
            let start = std::time::Instant::now();
            for _ in 0..10 {
                terminal.draw(|f| inspector.render(f, f.area())).unwrap();
            }
            start.elapsed()
        };

        let small_time = time(&small);
        let large_time = time(&large);
        assert!(
            large_time < small_time * 10,
            "render cost should not scale with the untruncated value: \
             100KB took {small_time:?}, 2MB took {large_time:?}"
        );
    }
}
