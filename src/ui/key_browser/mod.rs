pub mod scanner_task;
pub mod tree;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::events::Event;
use crate::redis::client::RedisType;
use crate::ui::key_browser::tree::{KeyEntry, NamespaceTree};

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
    RefreshRequested,
}

pub struct KeyBrowser {
    pub tree: NamespaceTree,
    pub cursor: usize,
    pub filter: Option<String>,
    pub state: BrowserState,
}

impl KeyBrowser {
    pub fn new(separator: char) -> Self {
        Self {
            tree: NamespaceTree::new(separator),
            cursor: 0,
            filter: None,
            state: BrowserState::Scanning { keys_loaded: 0 },
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<BrowserAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    let rows = self.tree.visible_rows();
                    if self.cursor + 1 < rows.len() {
                        self.cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                KeyCode::Enter => {
                    let rows = self.tree.visible_rows();
                    if let Some(row) = rows.get(self.cursor)
                        && !row.is_namespace
                        && let Some(ref key) = row.key
                    {
                        return Some(BrowserAction::SelectKey(
                            key.full_name.clone(),
                            key.redis_type.clone().unwrap_or(RedisType::Unknown),
                        ));
                    }
                }
                KeyCode::Char('D') => {
                    let rows = self.tree.visible_rows();
                    if let Some(row) = rows.get(self.cursor)
                        && let Some(ref key) = row.key
                    {
                        return Some(BrowserAction::DeleteKey(key.full_name.clone()));
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let rows = self.tree.visible_rows();
        let items: Vec<ListItem> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let indent = "  ".repeat(row.depth);
                let text = format!("{}{}", indent, row.label);
                let style = if i == self.cursor {
                    Style::default().bg(Color::Blue).fg(Color::White)
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
            .title(format!("Keys ({}) ", self.tree.total_keys()));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, area, &mut state);
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
