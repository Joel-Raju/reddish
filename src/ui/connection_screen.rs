use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::config::connections::{ConnectionProfile, ConnectionStore};
use crate::events::Event;

pub struct ConnectionScreen {
    pub store: ConnectionStore,
    pub cursor: usize,
    pub error: Option<String>,
}

impl ConnectionScreen {
    pub fn new(store: ConnectionStore) -> Self {
        Self {
            store,
            cursor: 0,
            error: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ConnectionScreenAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Down if self.cursor + 1 < self.store.profiles.len() => self.cursor += 1,
                KeyCode::Up if self.cursor > 0 => self.cursor -= 1,
                KeyCode::Enter => {
                    if let Some(profile) = self.store.profiles.get(self.cursor) {
                        return Some(ConnectionScreenAction::Connect(profile.clone()));
                    }
                }
                KeyCode::Esc => return Some(ConnectionScreenAction::Cancel),
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Connections (Esc to cancel)");
        let items: Vec<ListItem> = self
            .store
            .profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let style = if i == self.cursor {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(p.name.as_str()).style(style)
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(self.cursor));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, area, &mut state);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionScreenAction {
    Connect(ConnectionProfile),
    Save(ConnectionProfile),
    Delete(String),
    Cancel,
}
