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
                || matches!(key.code, KeyCode::Right | KeyCode::Char('l'))
            {
                let rows = self.sorted_rows();
                if let Some(row) = rows.get(self.cursor)
                    && row.is_namespace
                {
                    let path: Vec<&str> = row.path.iter().map(|s| s.as_str()).collect();
                    self.tree.expand(&path);
                }
            } else if keymap.matches("nav_left", key)
                || matches!(key.code, KeyCode::Left | KeyCode::Char('h'))
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
            } else if key.code == KeyCode::Char('g') {
                if !self.current_path.is_empty() {
                    self.current_path.clear();
                    self.cursor = 0;
                }
            } else if keymap.matches("delete", key) || matches!(key.code, KeyCode::Char('D')) {
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
        let max_offset = rows.len().saturating_sub(inner_height);
        // Scroll the minimum amount that keeps the cursor on screen, matching
        // what ratatui would do with a zero offset.
        let cursor = self.cursor.min(rows.len().saturating_sub(1));
        let offset = cursor
            .saturating_sub(inner_height.saturating_sub(1))
            .min(max_offset);

        let items: Vec<ListItem> = rows
            .iter()
            .skip(offset)
            .take(inner_height)
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
                "Keys ({}) [sort: {}] ",
                self.tree.total_keys(),
                self.sort_mode.label()
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
            self.tree.insert(entry);
        }
        let total = self.tree.total_keys();
        self.state = BrowserState::Scanning { keys_loaded: total };
    }

    pub fn finish_scan(&mut self) {
        self.state = BrowserState::Ready;
    }

    pub fn index_of_key(&self, full_name: &str) -> Option<usize> {
        self.tree
            .visible_rows()
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

    /// Render cost must grow far more slowly than the row count.
    ///
    /// The bound is empirical, not arbitrary. Measured with 100 rows against
    /// 10,000 — a 100x increase in rows:
    ///
    /// ```text
    ///   windowed    ~4.9x    <- what this allows up to 9x of
    ///   un-windowed ~13.0x   <- the state a regression returns to
    /// ```
    ///
    /// Rendering is still O(rows) because `sorted_rows` walks the whole tree to
    /// learn the row count and to sort it; windowing only removed the per-row
    /// `ListItem` allocation, which was the dominant cost. So this cannot
    /// assert flat timing and does not try to — it pins the gap between the two
    /// states, which is what protects the optimization.
    ///
    /// A 5x bound was tried first and failed on a GitHub runner at 6.2x, which
    /// is how the true windowed figure was established in the first place.
    #[test]
    fn render_cost_does_not_scale_with_the_row_count() {
        // Best of three: the test binary shares a CPU with everything else
        // running in CI, and a slow neighbour should not fail the build.
        let time = |count: usize| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            let b = browser_with(count);
            let start = std::time::Instant::now();
            for _ in 0..20 {
                terminal
                    .draw(|f| {
                        b.render(f, f.area());
                    })
                    .unwrap();
            }
            start.elapsed()
        };

        let small = time(100).min(time(100)).min(time(100));
        let large = time(10_000).min(time(10_000)).min(time(10_000));

        assert!(
            large < small * 9,
            "20 frames over 100 rows took {small:?} but over 10,000 rows took \
             {large:?}: the window is not being applied"
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
