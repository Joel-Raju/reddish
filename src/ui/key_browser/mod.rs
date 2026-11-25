pub mod scanner_task;
pub mod tree;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::config::keybindings::Keymap;
use crate::events::Event;
use crate::redis::client::{RedisType, SetOpType};
use crate::ui::key_browser::tree::{KeyEntry, NamespaceTree, TreeRow};
use crate::ui::theme;
use crate::ui::widgets::input::InputWidget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Alpha,
    ByType,
    ByTtl,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Alpha => SortMode::ByType,
            SortMode::ByType => SortMode::ByTtl,
            SortMode::ByTtl => SortMode::Alpha,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Alpha => "alpha",
            SortMode::ByType => "type",
            SortMode::ByTtl => "ttl",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserPrompt {
    NewKeyName,
    NewKeyType(String),
    RenameKey(String),
    SetTtl(String),
    DuplicateKey(String),
    SetOpDest {
        op: SetOpType,
        source_keys: Vec<String>,
    },
    SetOpAdditionalSources {
        op: SetOpType,
        source_keys: Vec<String>,
        dest: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserState {
    Scanning { keys_loaded: usize },
    Ready,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserAction {
    SelectKey(String, RedisType),
    DeleteKey(String),
    NewKey {
        name: String,
        key_type: RedisType,
    },
    RenameKey {
        old_name: String,
        new_name: String,
    },
    ExpireKey(String),
    SetTtl {
        key: String,
        seconds: i64,
    },
    CopyKeyName(String),
    DuplicateKey {
        old_name: String,
        new_name: String,
    },
    SetOp {
        op: SetOpType,
        source_keys: Vec<String>,
        dest: String,
    },
    RefreshRequested,
}

/// The slice of rows a viewport can actually show, as `(offset, count)`.
///
/// Scrolls the minimum amount that keeps `cursor` on screen, matching what
/// ratatui does with a zero offset, and never scrolls past the last page.
///
/// Split out of `render` because it carries the load-bearing property of the
/// windowing optimization and is worth asserting on its own: `count` must
/// depend on the viewport height and never on `row_count`. Asserting that
/// deterministically beats a timing test, which measured the same code at 4.9x
/// growth in debug, 6.2x on a GitHub runner and 16.3x in release.
pub fn visible_window(row_count: usize, cursor: usize, inner_height: usize) -> (usize, usize) {
    let inner_height = inner_height.max(1);
    let max_offset = row_count.saturating_sub(inner_height);
    let cursor = cursor.min(row_count.saturating_sub(1));
    let offset = cursor
        .saturating_sub(inner_height.saturating_sub(1))
        .min(max_offset);
    let count = inner_height.min(row_count.saturating_sub(offset));
    (offset, count)
}

/// Where the key list was drawn and how far it was scrolled, so mouse clicks
/// can be mapped back to a logical row index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrowserRenderInfo {
    /// Content area of the list widget (borders excluded).
    pub list_area: Rect,
    /// Number of rows scrolled off the top of the list.
    pub scroll_offset: usize,
    /// Total number of logical rows, including those scrolled out of view.
    pub row_count: usize,
}

impl BrowserRenderInfo {
    /// Map a click to a logical row index.
    ///
    /// Returns `None` when the click lands outside the list, or below the last
    /// row (the area under a partially filled list).
    pub fn row_at(&self, column: u16, row: u16) -> Option<usize> {
        if column < self.list_area.x
            || column >= self.list_area.x + self.list_area.width
            || row < self.list_area.y
            || row >= self.list_area.y + self.list_area.height
        {
            return None;
        }
        let offset_in_view = (row - self.list_area.y) as usize;
        let index = self.scroll_offset + offset_in_view;
        (index < self.row_count).then_some(index)
    }
}

pub struct KeyBrowser {
    pub tree: NamespaceTree,
    pub cursor: usize,
    pub filter: Option<String>,
    pub state: BrowserState,
    pub current_path: Vec<String>,
    pub prompt: Option<InputWidget>,
    pub prompt_mode: Option<BrowserPrompt>,
    pub selected: std::collections::HashSet<String>,
    pub sort_mode: SortMode,
    /// The scan hit `max_keys_in_memory` and rows were dropped. Surfaced
    /// in the title so a capped scan cannot pass for a complete one.
    pub truncated: bool,
}

impl KeyBrowser {
    pub fn new(separator: char, max_keys: usize) -> Self {
        Self {
            tree: NamespaceTree::new(separator, max_keys),
            cursor: 0,
            filter: None,
            state: BrowserState::Scanning { keys_loaded: 0 },
            current_path: Vec::new(),
            prompt: None,
            prompt_mode: None,
            selected: std::collections::HashSet::new(),
            sort_mode: SortMode::Alpha,
            truncated: false,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<BrowserAction> {
        self.handle_event_with_keymap(event, &Keymap::default())
    }

    pub fn handle_event_with_keymap(
        &mut self,
        event: &Event,
        keymap: &Keymap,
    ) -> Option<BrowserAction> {
        use crossterm::event::KeyCode;

        // Handle inline prompts
        if let Some(ref mut prompt) = self.prompt {
            prompt.handle_event(event);
            if prompt.submitted.is_some() {
                let val = prompt.submitted.take().unwrap_or_default();
                let mode = self.prompt_mode.take();
                self.prompt = None;
                return match mode {
                    Some(BrowserPrompt::NewKeyName) => {
                        if val.is_empty() {
                            return None;
                        }
                        self.prompt = Some(InputWidget::new(
                            "Key type? (s=String, l=List, h=Hash, z=ZSet, t=Set, x=Stream)",
                        ));
                        self.prompt_mode = Some(BrowserPrompt::NewKeyType(val));
                        None
                    }
                    Some(BrowserPrompt::NewKeyType(name)) => {
                        let r#type = match val.to_lowercase().as_str() {
                            "s" => Some(RedisType::String),
                            "l" => Some(RedisType::List),
                            "h" => Some(RedisType::Hash),
                            "z" => Some(RedisType::ZSet),
                            "t" => Some(RedisType::Set),
                            "x" => Some(RedisType::Stream),
                            _ => None,
                        };
                        r#type.map(|key_type| BrowserAction::NewKey { name, key_type })
                    }
                    Some(BrowserPrompt::RenameKey(old_name)) => {
                        if val.is_empty() || val == old_name {
                            return None;
                        }
                        Some(BrowserAction::RenameKey {
                            old_name,
                            new_name: val,
                        })
                    }
                    Some(BrowserPrompt::SetTtl(key)) => {
                        let seconds = val.parse::<i64>().ok();
                        seconds.map(|s| BrowserAction::SetTtl { key, seconds: s })
                    }
                    Some(BrowserPrompt::DuplicateKey(old_name)) => {
                        if val.is_empty() || val == old_name {
                            return None;
                        }
                        Some(BrowserAction::DuplicateKey {
                            old_name,
                            new_name: val,
                        })
                    }
                    Some(BrowserPrompt::SetOpDest { op, source_keys }) => {
                        if val.is_empty() {
                            return None;
                        }
                        self.prompt = Some(InputWidget::new(
                            "Additional source keys (space-separated):",
                        ));
                        self.prompt_mode = Some(BrowserPrompt::SetOpAdditionalSources {
                            op,
                            source_keys,
                            dest: val,
                        });
                        None
                    }
                    Some(BrowserPrompt::SetOpAdditionalSources {
                        op,
                        mut source_keys,
                        dest,
                    }) => {
                        if !val.is_empty() {
                            for extra in val.split_whitespace() {
                                let trimmed = extra.trim();
                                if !trimmed.is_empty()
                                    && !source_keys.contains(&trimmed.to_string())
                                {
                                    source_keys.push(trimmed.to_string());
                                }
                            }
                        }
                        Some(BrowserAction::SetOp {
                            op,
                            source_keys,
                            dest,
                        })
                    }
                    None => None,
                };
            }
            if prompt.cancelled {
                self.prompt = None;
                self.prompt_mode = None;
            }
            return None;
        }

        if let Event::Key(key) = event {
            if keymap.matches("nav_down", key) || matches!(key.code, KeyCode::Down) {
                let rows = self.sorted_rows();
                if self.cursor + 1 < rows.len() {
                    self.cursor += 1;
                }
            } else if keymap.matches("nav_up", key) || matches!(key.code, KeyCode::Up) {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            } else if keymap.matches("confirm", key) || matches!(key.code, KeyCode::Enter) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && !row.is_namespace
                    && let Some(ref key) = row.key
                {
                    return Some(BrowserAction::SelectKey(
                        key.full_name.clone(),
                        key.redis_type.clone().unwrap_or(RedisType::Unknown),
                    ));
                }
            } else if keymap.matches("nav_right", key)
                // The letter fallbacks must be modifier-blind-proof: without
                // the empty-modifiers guard, Ctrl+L (emacs refresh) expanded
                // a node instead, and any CONTROL/ALT chord ending in that
                // letter was captured here.
                || (key.modifiers.is_empty()
                    && matches!(key.code, KeyCode::Right | KeyCode::Char('l')))
            {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && row.is_namespace
                {
                    let path: Vec<&str> = row.path.iter().map(|s| s.as_str()).collect();
                    self.tree.expand(&path);
                }
            } else if keymap.matches("nav_left", key)
                || (key.modifiers.is_empty()
                    && matches!(key.code, KeyCode::Left | KeyCode::Char('h')))
            {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && row.is_namespace
                {
                    let path: Vec<&str> = row.path.iter().map(|s| s.as_str()).collect();
                    self.tree.collapse(&path);
                }
            } else if keymap.matches("go_up", key) || matches!(key.code, KeyCode::Backspace) {
                if !self.current_path.is_empty() {
                    self.current_path.pop();
                    self.cursor = 0;
                }
            } else if key.modifiers.is_empty() && key.code == KeyCode::Char('g') {
                if !self.current_path.is_empty() {
                    self.current_path.clear();
                    self.cursor = 0;
                }
            // No `Char('D')` code-only fallback: it fired on Ctrl+D and any
            // other decorated variant of the letter (the inspector's
            // delete-selected binding is Ctrl+D, and in the browser that
            // chord must reach nothing). The keymap binding covers every
            // preset since uppercase spellings now parse with SHIFT.
            } else if keymap.matches("delete", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    return Some(BrowserAction::DeleteKey(key.full_name.clone()));
                }
            } else if keymap.matches("refresh", key) {
                return Some(BrowserAction::RefreshRequested);
            } else if keymap.matches("new_key", key) {
                self.prompt = Some(InputWidget::new("New key name:"));
                self.prompt_mode = Some(BrowserPrompt::NewKeyName);
            } else if keymap.matches("rename_key", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    let mut prompt = InputWidget::new("New name:");
                    prompt.value = key.full_name.clone();
                    prompt.cursor = key.full_name.len();
                    self.prompt = Some(prompt);
                    self.prompt_mode = Some(BrowserPrompt::RenameKey(key.full_name.clone()));
                }
            } else if keymap.matches("expire", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    return Some(BrowserAction::ExpireKey(key.full_name.clone()));
                }
            } else if keymap.matches("set_ttl", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    self.prompt = Some(InputWidget::new("TTL in seconds:"));
                    self.prompt_mode = Some(BrowserPrompt::SetTtl(key.full_name.clone()));
                }
            } else if keymap.matches("copy", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    return Some(BrowserAction::CopyKeyName(key.full_name.clone()));
                }
            } else if keymap.matches("duplicate_key", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    let mut prompt = InputWidget::new("New key name:");
                    prompt.value = format!("{}_copy", key.full_name);
                    prompt.cursor = prompt.value.len();
                    self.prompt = Some(prompt);
                    self.prompt_mode = Some(BrowserPrompt::DuplicateKey(key.full_name.clone()));
                }
            } else if keymap.matches("set_union", key)
                || keymap.matches("set_inter", key)
                || keymap.matches("set_diff", key)
            {
                let event_key = key;
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    let op = if keymap.matches("set_union", event_key) {
                        SetOpType::Union
                    } else if keymap.matches("set_inter", event_key) {
                        SetOpType::Inter
                    } else {
                        SetOpType::Diff
                    };
                    let mut source_keys = vec![key.full_name.clone()];
                    for sel in &self.selected {
                        if sel != &key.full_name {
                            source_keys.push(sel.clone());
                        }
                    }
                    let mut prompt = InputWidget::new("Destination key:");
                    prompt.value = format!("{}{}", key.full_name, op.dest_suffix());
                    prompt.cursor = prompt.value.len();
                    self.prompt = Some(prompt);
                    self.prompt_mode = Some(BrowserPrompt::SetOpDest { op, source_keys });
                }
            } else if keymap.matches("toggle_select", key) {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && let Some(ref key) = row.key
                {
                    let name = key.full_name.clone();
                    if !self.selected.remove(&name) {
                        self.selected.insert(name);
                    }
                }
            } else if keymap.matches("cycle_sort", key) {
                self.sort_mode = self.sort_mode.next();
            } else if keymap.matches("select_all", key) {
                for row in self.sorted_rows() {
                    if let Some(ref key) = row.key {
                        self.selected.insert(key.full_name.clone());
                    }
                }
            }
        }
        None
    }

    pub fn sorted_rows(&self) -> Vec<TreeRow> {
        // visible_rows() already returns an owned Vec. Calling .to_vec() on it
        // deep-cloned every TreeRow (including each KeyEntry's String) on every
        // frame, which dominated render time at 10K keys.
        let mut rows = self.tree.visible_rows();
        if self.sort_mode != SortMode::Alpha {
            rows.sort_by(|a, b| {
                match (a.is_namespace, b.is_namespace) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    (true, true) => return a.label.cmp(&b.label),
                    (false, false) => {}
                }
                match self.sort_mode {
                    SortMode::Alpha => a.label.cmp(&b.label),
                    SortMode::ByType => {
                        fn type_order(t: &Option<crate::redis::client::RedisType>) -> u8 {
                            match t {
                                None => 0,
                                Some(crate::redis::client::RedisType::String) => 1,
                                Some(crate::redis::client::RedisType::List) => 2,
                                Some(crate::redis::client::RedisType::Hash) => 3,
                                Some(crate::redis::client::RedisType::Set) => 4,
                                Some(crate::redis::client::RedisType::ZSet) => 5,
                                Some(crate::redis::client::RedisType::Stream) => 6,
                                Some(crate::redis::client::RedisType::Unknown) => 7,
                            }
                        }
                        let a_type = a.key.as_ref().and_then(|k| k.redis_type.clone());
                        let b_type = b.key.as_ref().and_then(|k| k.redis_type.clone());
                        type_order(&a_type)
                            .cmp(&type_order(&b_type))
                            .then(a.label.cmp(&b.label))
                    }
                    SortMode::ByTtl => {
                        fn ttl_order(ttl: &Option<crate::redis::client::Ttl>) -> u8 {
                            match ttl {
                                None => 3,
                                Some(crate::redis::client::Ttl::KeyNotFound) => 4,
                                Some(crate::redis::client::Ttl::NoExpiry) => 2,
                                Some(crate::redis::client::Ttl::Expires(_)) => 1,
                            }
                        }
                        let a_ttl = a.key.as_ref().and_then(|k| k.ttl.clone());
                        let b_ttl = b.key.as_ref().and_then(|k| k.ttl.clone());
                        ttl_order(&a_ttl)
                            .cmp(&ttl_order(&b_ttl))
                            .then_with(|| {
                                let a_secs: u64 = a_ttl
                                    .and_then(|t| match t {
                                        crate::redis::client::Ttl::Expires(d) => Some(d.as_secs()),
                                        _ => None,
                                    })
                                    .unwrap_or(0);
                                let b_secs: u64 = b_ttl
                                    .and_then(|t| match t {
                                        crate::redis::client::Ttl::Expires(d) => Some(d.as_secs()),
                                        _ => None,
                                    })
                                    .unwrap_or(0);
                                a_secs.cmp(&b_secs)
                            })
                            .then(a.label.cmp(&b.label))
                    }
                }
            });
        }
        rows
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) -> BrowserRenderInfo {
        use ratatui::layout::{Constraint, Direction, Layout};

        // Split area into breadcrumb (1 line) and key list (remaining)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        // Breadcrumb
        let breadcrumb_text = if self.current_path.is_empty() {
            " > root".to_string()
        } else {
            let path = self
                .current_path
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" > ");
            format!(" > {}", path)
        };
        let t = theme::current();
        let breadcrumb_style = Style::default().fg(t.title).bg(t.bg);
        let breadcrumb = ratatui::widgets::Paragraph::new(breadcrumb_text).style(breadcrumb_style);
        frame.render_widget(breadcrumb, chunks[0]);

        // Key list
        let rows = self.sorted_rows();

        // Only build the rows that can actually be drawn.
        //
        // With every namespace expanded this is 10,001 rows, of which an 80x24
        // terminal shows about 20. Allocating a ListItem — a String and a Style
        // — for each was the entire frame cost: ~15ms to draw 20 rows.
        let list_area = chunks[1];
        let inner_height = list_area.height.saturating_sub(2).max(1) as usize;
        let cursor = self.cursor;
        let (offset, visible) = visible_window(rows.len(), cursor, inner_height);

        let items: Vec<ListItem> = rows
            .iter()
            .skip(offset)
            .take(visible)
            .enumerate()
            .map(|(local, row)| {
                let i = offset + local;
                let indent = "  ".repeat(row.depth);
                let sel = if let Some(ref key) = row.key
                    && self.selected.contains(&key.full_name)
                {
                    "*"
                } else {
                    " "
                };
                let text = format!("{}{}{}", indent, sel, row.label);
                let style = if i == self.cursor {
                    Style::default().bg(t.highlight_bg).fg(t.highlight_fg)
                } else if let Some(ref key) = row.key
                    && self.selected.contains(&key.full_name)
                {
                    Style::default().bg(t.border).fg(t.fg)
                } else {
                    Style::default()
                };
                ListItem::new(text).style(style)
            })
            .collect();

        // The selection index is relative to the slice we passed, not to the
        // full row list.
        let state = ListState::default()
            .with_offset(offset)
            .with_selected(Some(cursor.saturating_sub(offset)));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(format!(
                "Keys ({}) [sort: {}]{}",
                self.tree.total_keys(),
                self.sort_mode.label(),
                // A capped scan must not pass for a complete one.
                if self.truncated {
                    " [limit reached]"
                } else {
                    ""
                }
            ));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, list_area, &mut state.clone());

        // The offset is computed above rather than read back from ratatui: we hand it
        // a pre-windowed slice, so the click mapping needs the full-list offset
        // we chose, which is `offset`.
        let info = BrowserRenderInfo {
            list_area: list_area.inner(ratatui::layout::Margin::new(1, 1)),
            scroll_offset: offset,
            row_count: rows.len(),
        };

        // Render prompt overlay
        if let Some(ref prompt) = self.prompt {
            let prompt_label = match self.prompt_mode {
                Some(BrowserPrompt::NewKeyName) => "Key name: ",
                Some(BrowserPrompt::NewKeyType(_)) => "Type (s/l/h/z/t/x): ",
                Some(BrowserPrompt::RenameKey(_)) => "New name: ",
                Some(BrowserPrompt::SetTtl(_)) => "TTL seconds: ",
                Some(BrowserPrompt::DuplicateKey(_)) => "Duplicate as: ",
                Some(BrowserPrompt::SetOpDest { .. }) => "Destination key: ",
                Some(BrowserPrompt::SetOpAdditionalSources { .. }) => "Additional sources: ",
                None => "Input: ",
            };
            let prompt_text = format!(
                "{}{}{}",
                prompt_label,
                prompt.value,
                if prompt.cursor >= prompt.value.len() {
                    "_"
                } else {
                    " "
                }
            );
            let overlay = ratatui::widgets::Paragraph::new(prompt_text)
                .style(Style::default().fg(t.fg).bg(t.bg));
            frame.render_widget(overlay, chunks[1]);
        }

        info
    }

    pub fn apply_scan_batch(&mut self, batch: Vec<KeyEntry>) {
        for entry in batch {
            // `insert` returns false at the max_keys cap; the return value
            // was discarded, so a truncated scan silently looked complete.
            if !self.tree.insert(entry) {
                self.truncated = true;
            }
        }
        let total = self.tree.total_keys();
        self.state = BrowserState::Scanning { keys_loaded: total };
    }

    pub fn finish_scan(&mut self) {
        self.state = BrowserState::Ready;
    }

    /// The scan task reported a failure. Surface it instead of leaving a
    /// partial tree marked `Ready` — a SCAN error used to be indistinguishable
    /// from a completed scan.
    pub fn fail_scan(&mut self, message: String) {
        self.state = BrowserState::Error(message);
    }

    pub fn index_of_key(&self, full_name: &str) -> Option<usize> {
        // Index into the same ordering the cursor, navigation and render
        // use. The previous lookup read `visible_rows()`, which only agrees
        // with `sorted_rows()` under the default Alpha sort — so in a
        // type/TTL-sorted view a search jump sent the cursor to a row that
        // wasn't the key it had just loaded.
        self.sorted_rows()
            .iter()
            .position(|row| row.key.as_ref().is_some_and(|k| k.full_name == full_name))
    }

    pub fn jump_to_key(&mut self, full_name: &str) -> bool {
        self.tree.expand_all();
        if let Some(idx) = self.index_of_key(full_name) {
            self.cursor = idx;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod render_info_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn info(list_area: Rect, scroll_offset: usize, row_count: usize) -> BrowserRenderInfo {
        BrowserRenderInfo {
            list_area,
            scroll_offset,
            row_count,
        }
    }

    fn list() -> Rect {
        Rect {
            x: 2,
            y: 3,
            width: 20,
            height: 5,
        }
    }

    fn key(name: &str) -> KeyEntry {
        KeyEntry {
            full_name: name.to_string(),
            redis_type: None,
            ttl: None,
        }
    }

    #[test]
    fn click_maps_to_row_index_within_list() {
        let i = info(list(), 0, 10);
        assert_eq!(i.row_at(5, 3), Some(0));
        assert_eq!(i.row_at(5, 4), Some(1));
        assert_eq!(i.row_at(5, 7), Some(4));
    }

    #[test]
    fn click_outside_the_list_horizontal_bounds_is_ignored() {
        let i = info(list(), 0, 10);
        assert_eq!(i.row_at(1, 4), None, "left of the list");
        assert_eq!(i.row_at(22, 4), None, "right of the list");
    }

    #[test]
    fn click_outside_the_list_vertical_bounds_is_ignored() {
        let i = info(list(), 0, 10);
        assert_eq!(i.row_at(5, 2), None, "above the list");
        assert_eq!(i.row_at(5, 8), None, "below the list");
    }

    #[test]
    fn scroll_offset_shifts_the_mapped_index() {
        let i = info(list(), 100, 200);
        assert_eq!(i.row_at(5, 3), Some(100));
        assert_eq!(i.row_at(5, 7), Some(104));
    }

    #[test]
    fn click_past_the_last_row_is_ignored() {
        // 10 rows exist, the view starts at 5, so only indices 8 and 9 are
        // reachable; the rest of the visible area is empty space.
        let i = info(list(), 8, 10);
        assert_eq!(i.row_at(5, 3), Some(8));
        assert_eq!(i.row_at(5, 4), Some(9));
        assert_eq!(i.row_at(5, 5), None, "past the end");
        assert_eq!(i.row_at(5, 7), None);
    }

    #[test]
    fn empty_list_has_no_hittable_rows() {
        assert_eq!(info(list(), 0, 0).row_at(5, 3), None);
    }

    #[test]
    fn default_geometry_is_empty() {
        assert_eq!(BrowserRenderInfo::default().row_at(0, 0), None);
    }

    #[test]
    fn render_reports_geometry_for_populated_browser() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut browser = KeyBrowser::new(':', 10_000);
        for i in 0..5 {
            browser.tree.insert(key(&format!("ns:key{i}")));
        }
        // Namespaces start collapsed, so only the namespace row is visible
        // until it is expanded.
        browser.tree.expand_all();

        let mut info = BrowserRenderInfo::default();
        terminal
            .draw(|f| info = browser.render(f, f.area()))
            .unwrap();
        assert_eq!(info.row_count, 6, "5 keys plus the namespace row");
        assert_eq!(info.scroll_offset, 0);
        // The list sits below the one-line breadcrumb, inside its border.
        assert!(info.list_area.y >= 2);
        assert_eq!(
            info.row_at(info.list_area.x, info.list_area.y),
            Some(0),
            "top visible row must map to the first logical row"
        );
    }

    #[test]
    fn collapsed_namespace_hides_its_keys_from_hit_testing() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut browser = KeyBrowser::new(':', 10_000);
        for i in 0..5 {
            browser.tree.insert(key(&format!("ns:key{i}")));
        }
        let mut info = BrowserRenderInfo::default();
        terminal
            .draw(|f| info = browser.render(f, f.area()))
            .unwrap();
        assert_eq!(info.row_count, 1, "only the collapsed namespace row");
    }

    #[test]
    fn render_reports_scroll_offset_when_cursor_is_past_the_viewport() {
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut browser = KeyBrowser::new(':', 10_000);
        for i in 0..200 {
            browser.tree.insert(key(&format!("ns:key{i:04}")));
        }
        browser.tree.expand_all();
        // Move the cursor to the end so ratatui scrolls the list.
        browser.cursor = browser.sorted_rows().len() - 1;

        let mut info = BrowserRenderInfo::default();
        terminal
            .draw(|f| info = browser.render(f, f.area()))
            .unwrap();
        assert!(info.scroll_offset > 0, "expected the list to scroll");
        // Clicking the top visible row must resolve to the scrolled index
        // rather than to row 0.
        assert_eq!(
            info.row_at(info.list_area.x, info.list_area.y),
            Some(info.scroll_offset)
        );
    }

    #[test]
    fn render_on_empty_browser_reports_no_rows() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let browser = KeyBrowser::new(':', 10_000);
        let mut info = BrowserRenderInfo::default();
        terminal
            .draw(|f| info = browser.render(f, f.area()))
            .unwrap();
        assert_eq!(info.row_count, 0);
        assert_eq!(info.row_at(5, 5), None);
    }
}

#[cfg(test)]
mod windowing_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn key(name: &str) -> KeyEntry {
        KeyEntry {
            full_name: name.to_string(),
            redis_type: None,
            ttl: None,
        }
    }

    fn browser_with(count: usize) -> KeyBrowser {
        let mut b = KeyBrowser::new(':', 10_000);
        for i in 0..count {
            b.tree.insert(key(&format!("ns:key{i:05}")));
        }
        b.tree.expand_all();
        b
    }

    fn info_at(browser: &mut KeyBrowser, w: u16, h: u16) -> BrowserRenderInfo {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        let mut info = BrowserRenderInfo::default();
        terminal
            .draw(|f| info = browser.render(f, f.area()))
            .unwrap();
        info
    }

    /// At the top of the list nothing is scrolled.
    #[test]
    fn no_scroll_when_the_cursor_is_at_the_top() {
        let mut b = browser_with(500);
        b.cursor = 0;
        let info = info_at(&mut b, 80, 24);
        assert_eq!(info.scroll_offset, 0);
        assert_eq!(info.row_count, 501);
    }

    /// The offset must place the cursor on screen, which is the whole point.
    #[test]
    fn the_cursor_is_always_inside_the_window() {
        let mut b = browser_with(500);
        // 24 rows minus 2 for the border and breadcrumb leaves ~21 visible.
        let visible = 24 - 2 - 1;
        for cursor in [0usize, 1, 20, 21, 100, 250, 499, 500] {
            b.cursor = cursor;
            let info = info_at(&mut b, 80, 24);
            let end = info.scroll_offset + visible;
            assert!(
                cursor >= info.scroll_offset && cursor < end,
                "cursor {cursor} outside window {}..{end} (offset {})",
                info.scroll_offset,
                info.scroll_offset
            );
        }
    }

    /// The window must not scroll further than there are rows to show.
    #[test]
    fn offset_never_exceeds_the_available_rows() {
        let mut b = browser_with(500);
        b.cursor = 500;
        let info = info_at(&mut b, 80, 24);
        let visible = 24 - 2 - 1;
        assert_eq!(
            info.scroll_offset,
            501 - visible,
            "the last page must be aligned to the end, not scrolled past it"
        );
    }

    /// A short list must never produce an offset: there is nothing to scroll.
    #[test]
    fn short_list_never_scrolls() {
        let mut b = browser_with(3);
        for cursor in 0..4 {
            b.cursor = cursor;
            let info = info_at(&mut b, 80, 24);
            assert_eq!(
                info.scroll_offset, 0,
                "a 4-row list scrolled at cursor {cursor}"
            );
        }
    }

    /// An empty browser must not underflow the offset computation.
    #[test]
    fn empty_browser_does_not_scroll_or_panic() {
        let mut b = KeyBrowser::new(':', 10_000);
        b.cursor = 0;
        let info = info_at(&mut b, 80, 24);
        assert_eq!(info.scroll_offset, 0);
        assert_eq!(info.row_count, 0);
    }

    /// Degenerate terminal heights must not produce a nonsense offset.
    #[test]
    fn tiny_terminals_do_not_break_the_offset() {
        let mut b = browser_with(200);
        for (w, h) in [(1u16, 1u16), (1, 10), (10, 1), (3, 3), (0, 0)] {
            b.cursor = 150;
            let info = info_at(&mut b, w, h);
            assert!(
                info.scroll_offset <= info.row_count,
                "{w}x{h}: offset {} exceeds {}",
                info.scroll_offset,
                info.row_count
            );
        }
    }

    /// The window is bounded by the viewport, never by the row count.
    ///
    /// This is the property the whole optimization rests on, asserted
    /// deterministically rather than by timing.
    ///
    /// A timing assertion was tried first and abandoned after it gave three
    /// different answers for identical code: 4.9x growth in debug, 6.2x on a
    /// GitHub runner, 16.3x in release. The ratio moves because in release the
    /// per-row `ListItem` allocation is optimised away, leaving only the
    /// unavoidable `sorted_rows` walk. No threshold is loose enough to survive
    /// all three and tight enough to catch a regression.
    #[test]
    fn the_window_is_bounded_by_the_viewport_not_the_row_count() {
        for row_count in [0usize, 1, 21, 100, 10_000, 1_000_000] {
            for cursor in [0usize, 1, 20, 21, row_count / 2, row_count] {
                for height in [1usize, 5, 21, 200] {
                    let (offset, count) = visible_window(row_count, cursor, height);
                    assert!(
                        count <= height,
                        "{row_count} rows, cursor {cursor}, height {height}: \
                         window wants {count} rows"
                    );
                    assert!(
                        offset + count <= row_count,
                        "{row_count} rows, cursor {cursor}, height {height}: \
                         window {offset}..{} runs past the end",
                        offset + count
                    );
                }
            }
        }
    }

    /// The concrete case that motivated the windowing.
    #[test]
    fn ten_thousand_rows_in_an_eighty_by_twenty_four_terminal_builds_one_page() {
        let (offset, count) = visible_window(10_001, 5_000, 21);
        assert_eq!(count, 21, "a 21-row viewport cannot show more than 21 rows");
        assert_eq!(
            offset,
            5_000 - 20,
            "scrolled the minimum needed to keep row 5000 visible"
        );
    }

    /// The click mapping must still agree with what is drawn.
    #[test]
    fn clicks_on_a_scrolled_view_map_to_the_row_shown() {
        let mut b = browser_with(500);
        b.cursor = 300;
        let info = info_at(&mut b, 80, 24);
        assert!(info.scroll_offset > 0);

        // The top visible row must be the first row of the window, not row 0.
        assert_eq!(
            info.row_at(info.list_area.x, info.list_area.y),
            Some(info.scroll_offset)
        );
    }

    /// Growing the window must never show a blank list.
    #[test]
    fn every_window_offset_has_rows_behind_it() {
        let mut b = browser_with(120);
        let mut seen = std::collections::BTreeSet::new();
        for cursor in (0..120).step_by(7) {
            b.cursor = cursor;
            let info = info_at(&mut b, 80, 24);
            assert!(info.scroll_offset < info.row_count);
            seen.insert(info.scroll_offset);
        }
        assert!(seen.len() > 3, "offsets should vary: {seen:?}");
    }
}

#[cfg(test)]
mod sort_jump_tests {
    use super::*;
    use crate::redis::client::RedisType;
    use crate::ui::key_browser::tree::KeyEntry;

    fn browser_with_keys() -> KeyBrowser {
        let mut b = KeyBrowser::new(':', 100);
        // Types chosen so type-sort order differs from alphabetical order.
        for (name, t) in [
            ("alpha", RedisType::ZSet),
            ("bravo", RedisType::String),
            ("charlie", RedisType::Set),
        ] {
            b.tree.insert(KeyEntry {
                full_name: name.to_string(),
                redis_type: Some(t),
                ttl: None,
            });
        }
        b
    }

    /// `jump_to_key` indexed `visible_rows()` while the cursor, navigation
    /// and render all use `sorted_rows()`. The two only agree under the
    /// default Alpha sort, so a search jump in a type/TTL-sorted view put
    /// the cursor on a row that was not the key it loaded.
    #[test]
    fn jump_lands_on_the_target_row_under_every_sort_mode() {
        for mode in [SortMode::Alpha, SortMode::ByType, SortMode::ByTtl] {
            let mut b = browser_with_keys();
            b.sort_mode = mode;
            assert!(b.jump_to_key("bravo"), "jump failed under {mode:?}");
            let rows = b.sorted_rows();
            let row = rows.get(b.cursor).expect("cursor row must exist");
            assert_eq!(
                row.key.as_ref().map(|k| k.full_name.as_str()),
                Some("bravo"),
                "the cursor must index the same ordering the render uses ({mode:?})"
            );
        }
    }

    /// Rows dropped at max_keys used to vanish silently; the scan then looked
    /// complete.
    #[test]
    fn a_capped_scan_is_marked_truncated() {
        let mut b = KeyBrowser::new(':', 2);
        b.apply_scan_batch(
            (0..5)
                .map(|i| KeyEntry {
                    full_name: format!("k{i}"),
                    redis_type: None,
                    ttl: None,
                })
                .collect(),
        );
        assert_eq!(b.tree.total_keys(), 2);
        assert!(b.truncated, "insert failures at the cap must be recorded");
    }

    #[test]
    fn an_uncapped_scan_is_not_marked_truncated() {
        let mut b = KeyBrowser::new(':', 10);
        b.apply_scan_batch(vec![KeyEntry {
            full_name: "only".to_string(),
            redis_type: None,
            ttl: None,
        }]);
        assert!(!b.truncated);
    }
}
