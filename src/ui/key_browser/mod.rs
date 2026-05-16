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
use crate::ui::key_browser::tree::{KeyEntry, NamespaceTree};
use crate::ui::widgets::input::InputWidget;

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserPrompt {
    NewKeyName,
    NewKeyType(String),
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
                let rows = self.tree.visible_rows();
                if self.cursor + 1 < rows.len() {
                    self.cursor += 1;
                }
            } else if keymap.matches("nav_up", key) || matches!(key.code, KeyCode::Up) {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            } else if keymap.matches("confirm", key) || matches!(key.code, KeyCode::Enter) {
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
            } else if keymap.matches("nav_right", key)
                || matches!(key.code, KeyCode::Right | KeyCode::Char('l'))
            {
                let rows = self.tree.visible_rows();
                if let Some(row) = rows.get(self.cursor)
                    && row.is_namespace
                {
                    let path: Vec<&str> = row.path.iter().map(|s| s.as_str()).collect();
                    self.tree.expand(&path);
                }
            } else if keymap.matches("nav_left", key)
                || matches!(key.code, KeyCode::Left | KeyCode::Char('h'))
            {
                let rows = self.tree.visible_rows();
                if let Some(row) = rows.get(self.cursor)
                    && row.is_namespace
                {
                    let path: Vec<&str> = row.path.iter().map(|s| s.as_str()).collect();
                    self.tree.collapse(&path);
                }
            } else if key.code == KeyCode::Backspace {
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
                    let rows = self.tree.visible_rows();
                    if let Some(row) = rows.get(self.cursor)
                        && let Some(ref key) = row.key
                    {
                        return Some(BrowserAction::DeleteKey(key.full_name.clone()));
                    }
            } else if keymap.matches("refresh", key) {
                return Some(BrowserAction::RefreshRequested);
            } else if key.code == KeyCode::Char('n') {
                    self.prompt = Some(InputWidget::new("New key name:"));
                    self.prompt_mode = Some(BrowserPrompt::NewKeyName);
            }
        }
        None
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
        frame.render_stateful_widget(list, chunks[1], &mut state);

        // Render prompt overlay
        if let Some(ref prompt) = self.prompt {
            let prompt_label = match self.prompt_mode {
                Some(BrowserPrompt::NewKeyName) => "Key name: ",
                Some(BrowserPrompt::NewKeyType(_)) => "Type (s/l/h/z/t/x): ",
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
