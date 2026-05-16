use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::config::connections::{ConnectionProfile, ConnectionStore};
use crate::events::Event;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionScreenAction {
    Connect(ConnectionProfile),
    Save(ConnectionProfile),
    Delete(String),
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
enum ScreenMode {
    List,
    New,
    Edit(usize),
    ConfirmDelete(String),
}

#[derive(Debug, Clone)]
struct FormField {
    label: String,
    value: String,
    cursor: usize,
}

impl FormField {
    fn new(label: String, value: String) -> Self {
        Self {
            cursor: value.len(),
            label,
            value,
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.value.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.value.len();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.clear();
                self.cursor = 0;
            }
            _ => {}
        }
    }

    fn render(&self, active: bool) -> String {
        if active {
            let indicator = "_";
            if self.cursor >= self.value.len() {
                format!("{}: {}{}", self.label, self.value, indicator)
            } else {
                let before = &self.value[..self.cursor];
                let after = &self.value[self.cursor + 1..];
                format!("{}: {}{}{}", self.label, before, indicator, after)
            }
        } else {
            format!("{}: {}", self.label, self.value)
        }
    }
}

struct ConnectionForm {
    fields: Vec<FormField>,
    active: usize,
}

impl ConnectionForm {
    fn new(profile: Option<&ConnectionProfile>) -> Self {
        let (name, host, port, db, username, password) = match profile {
            Some(p) => (
                p.name.clone(),
                p.host.clone(),
                p.port.to_string(),
                p.db.to_string(),
                p.username.clone().unwrap_or_default(),
                p.password
                    .as_ref()
                    .map(|pw| match pw {
                        crate::config::connections::PasswordRef::Plaintext(s) => s.clone(),
                        crate::config::connections::PasswordRef::Env(s) => format!("env:{}", s),
                        crate::config::connections::PasswordRef::Keychain { service, account } => {
                            format!("keychain:{}:{}", service, account)
                        }
                    })
                    .unwrap_or_default(),
            ),
            None => Default::default(),
        };

        Self {
            fields: vec![
                FormField::new("Name".to_string(), name),
                FormField::new("Host".to_string(), host),
                FormField::new("Port".to_string(), port),
                FormField::new("DB".to_string(), db),
                FormField::new("Username".to_string(), username),
                FormField::new("Password".to_string(), password),
            ],
            active: 0,
        }
    }

    fn build_profile(&self) -> ConnectionProfile {
        let port = self.fields[2].value.parse().unwrap_or(6379);
        let db = self.fields[3].value.parse().unwrap_or(0);
        let username = if self.fields[4].value.is_empty() {
            None
        } else {
            Some(self.fields[4].value.clone())
        };
        let password = if self.fields[5].value.is_empty() {
            None
        } else {
            let raw = &self.fields[5].value;
            if let Some(env_var) = raw.strip_prefix("env:") {
                Some(crate::config::connections::PasswordRef::Env(env_var.to_string()))
            } else if let Some(rest) = raw.strip_prefix("keychain:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    Some(crate::config::connections::PasswordRef::Keychain {
                        service: parts[0].to_string(),
                        account: parts[1].to_string(),
                    })
                } else {
                    Some(crate::config::connections::PasswordRef::Plaintext(raw.clone()))
                }
            } else {
                Some(crate::config::connections::PasswordRef::Plaintext(raw.clone()))
            }
        };

        ConnectionProfile {
            name: self.fields[0].value.clone(),
            host: self.fields[1].value.clone(),
            port,
            db,
            username,
            password,
            last_connected: None,
            mode: crate::config::connections::ConnectionMode::Standalone,
            tls: None,
            ssh_tunnel: None,
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> Option<FormAction> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Tab => {
                self.active = (self.active + 1) % self.fields.len();
                None
            }
            KeyCode::BackTab => {
                self.active = if self.active == 0 {
                    self.fields.len() - 1
                } else {
                    self.active - 1
                };
                None
            }
            KeyCode::Enter => Some(FormAction::Submit),
            KeyCode::Esc => Some(FormAction::Cancel),
            _ => {
                self.fields[self.active].handle_key(key);
                None
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Connection Profile (Tab: next, Enter: save, Esc: cancel)");
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(self.fields.len() as u16 + 2)])
            .split(inner);

        let lines: Vec<String> = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let text = field.render(i == self.active);
                format!("  {}", text)
            })
            .collect();

        let text = lines.join("\n");
        let paragraph = Paragraph::new(text).block(Block::default());
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(paragraph, chunks[0]);
    }
}

#[derive(Debug, Clone, PartialEq)]
enum FormAction {
    Submit,
    Cancel,
}

pub struct ConnectionScreen {
    pub store: ConnectionStore,
    pub cursor: usize,
    pub error: Option<String>,
    mode: ScreenMode,
    form: Option<ConnectionForm>,
}

impl ConnectionScreen {
    pub fn new(store: ConnectionStore) -> Self {
        Self {
            store,
            cursor: 0,
            error: None,
            mode: ScreenMode::List,
            form: None,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ConnectionScreenAction> {
        use crossterm::event::KeyCode;
        let Event::Key(key) = event else {
            return None;
        };

        match &self.mode {
            ScreenMode::List => match key.code {
                KeyCode::Down if self.cursor + 1 < self.store.profiles.len() => {
                    self.cursor += 1;
                    None
                }
                KeyCode::Up if self.cursor > 0 => {
                    self.cursor -= 1;
                    None
                }
                KeyCode::Enter => self
                    .store
                    .profiles
                    .get(self.cursor)
                    .map(|profile| ConnectionScreenAction::Connect(profile.clone())),
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.mode = ScreenMode::New;
                    self.form = Some(ConnectionForm::new(None));
                    None
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if !self.store.profiles.is_empty() {
                        self.mode = ScreenMode::Edit(self.cursor);
                        self.form = Some(ConnectionForm::new(
                            self.store.profiles.get(self.cursor),
                        ));
                    }
                    None
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(profile) = self.store.profiles.get(self.cursor) {
                        self.mode = ScreenMode::ConfirmDelete(profile.name.clone());
                    }
                    None
                }
                KeyCode::Esc => Some(ConnectionScreenAction::Cancel),
                _ => None,
            },
            ScreenMode::New | ScreenMode::Edit(_) => {
                if let Some(ref mut form) = self.form {
                    match form.handle_key(key) {
                        Some(FormAction::Submit) => {
                            let profile = form.build_profile();
                            self.mode = ScreenMode::List;
                            self.form = None;
                            Some(ConnectionScreenAction::Save(profile))
                        }
                        Some(FormAction::Cancel) => {
                            self.mode = ScreenMode::List;
                            self.form = None;
                            None
                        }
                        None => None,
                    }
                } else {
                    None
                }
            }
            ScreenMode::ConfirmDelete(name) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let n = name.clone();
                    self.mode = ScreenMode::List;
                    Some(ConnectionScreenAction::Delete(n))
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = ScreenMode::List;
                    None
                }
                _ => None,
            },
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.mode {
            ScreenMode::List => {
                self.render_list(frame, area);
            }
            ScreenMode::New | ScreenMode::Edit(_) => {
                if let Some(ref form) = self.form {
                    let mut form_area = area;
                    form_area.width = form_area.width.min(60);
                    form_area.height = form_area.height.min(12);
                    form_area.x = area.x + (area.width - form_area.width) / 2;
                    form_area.y = area.y + (area.height - form_area.height) / 2;
                    form.render(frame, form_area);
                }
            }
            ScreenMode::ConfirmDelete(name) => {
                self.render_list(frame, area);
                let confirm_block = Block::default()
                    .borders(Borders::ALL)
                    .title("Confirm Delete")
                    .style(Style::default().bg(Color::Black).fg(Color::White));
                let text = format!("Delete connection '{}'?\n\n[y]es / [n]o", name);
                let paragraph = Paragraph::new(text).block(confirm_block);
                let popup_area = centered_rect(50, 15, area);
                frame.render_widget(Clear, popup_area);
                frame.render_widget(paragraph, popup_area);
            }
        }
    }

    fn render_list(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Connections (n: new, e: edit, d: delete, Enter: connect, Esc: cancel)");
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
                let label = format!(
                    "{}  {}:{}[{}]  {}",
                    if i == self.cursor { ">" } else { " " },
                    p.host,
                    p.port,
                    p.db,
                    p.name,
                );
                ListItem::new(label).style(style)
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(self.cursor));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, chunks[0], &mut state);

        // Quick-connect bar
        let qc_block = Block::default()
            .borders(Borders::NONE)
            .title("");
        let qc_text = Paragraph::new("Quick connect: press n to add a new profile")
            .style(Style::default().fg(Color::DarkGray))
            .block(qc_block);
        frame.render_widget(qc_text, chunks[1]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
