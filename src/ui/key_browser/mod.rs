pub mod scanner_task;
pub mod tree;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::config::keybindings::Keymap;
use crate::events::Event;
use crate::redis::client::RedisType;
use crate::ui::key_browser::tree::{KeyEntry, NamespaceTree, TreeRow};
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
    NewKey { name: String, key_type: RedisType },
    RenameKey { old_name: String, new_name: String },
    ExpireKey(String),
    SetTtl { key: String, seconds: i64 },
    CopyKeyName(String),
    DuplicateKey { old_name: String, new_name: String },
    RefreshRequested,
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
                            "Key type? (s=String, l=List, h=Hash, z=ZSet, t=Set, x=Stream)"
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
                        Some(BrowserAction::RenameKey { old_name, new_name: val })
                    }
                    Some(BrowserPrompt::SetTtl(key)) => {
                        let seconds = val.parse::<i64>().ok();
                        seconds.map(|s| BrowserAction::SetTtl { key, seconds: s })
                    }
                    Some(BrowserPrompt::DuplicateKey(old_name)) => {
                        if val.is_empty() || val == old_name {
                            return None;
                        }
                        Some(BrowserAction::DuplicateKey { old_name, new_name: val })
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
        let mut rows = self.tree.visible_rows().to_vec();
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
                        type_order(&a_type).cmp(&type_order(&b_type))
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
                        ttl_order(&a_ttl).cmp(&ttl_order(&b_ttl))
                            .then_with(|| {
                                let a_secs: u64 = a_ttl.and_then(|t| match t {
                                    crate::redis::client::Ttl::Expires(d) => Some(d.as_secs()),
                                    _ => None,
                                }).unwrap_or(0);
                                let b_secs: u64 = b_ttl.and_then(|t| match t {
                                    crate::redis::client::Ttl::Expires(d) => Some(d.as_secs()),
                                    _ => None,
                                }).unwrap_or(0);
                                a_secs.cmp(&b_secs)
                            })
                            .then(a.label.cmp(&b.label))
                    }
                }
            });
        }
        rows
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
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
        let breadcrumb_style = Style::default().fg(Color::Cyan).bg(Color::Black);
        let breadcrumb = ratatui::widgets::Paragraph::new(breadcrumb_text)
            .style(breadcrumb_style);
        frame.render_widget(breadcrumb, chunks[0]);

        // Key list
        let rows = self.sorted_rows();
        let items: Vec<ListItem> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
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
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if let Some(ref key) = row.key
                    && self.selected.contains(&key.full_name)
                {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(self.cursor));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Keys ({}) [sort: {}] ", self.tree.total_keys(), self.sort_mode.label()));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, chunks[1], &mut state);

        // Render prompt overlay
        if let Some(ref prompt) = self.prompt {
            let prompt_label = match self.prompt_mode {
                Some(BrowserPrompt::NewKeyName) => "Key name: ",
                Some(BrowserPrompt::NewKeyType(_)) => "Type (s/l/h/z/t/x): ",
                Some(BrowserPrompt::RenameKey(_)) => "New name: ",
                Some(BrowserPrompt::SetTtl(_)) => "TTL seconds: ",
                Some(BrowserPrompt::DuplicateKey(_)) => "Duplicate as: ",
                None => "Input: ",
            };
            let prompt_text = format!(
                "{}{}{}",
                prompt_label,
                prompt.value,
                if prompt.cursor >= prompt.value.len() { "_" } else { " " }
            );
            let overlay = ratatui::widgets::Paragraph::new(prompt_text)
                .style(Style::default().fg(Color::White).bg(Color::Black));
            frame.render_widget(overlay, chunks[1]);
        }
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
