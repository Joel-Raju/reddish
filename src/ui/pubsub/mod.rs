use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub pattern: Option<String>,
    pub payload: String,
    pub timestamp: std::time::Instant,
}

impl PubSubMessage {
    fn relative_time(&self) -> String {
        let secs = self.timestamp.elapsed().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else if secs < 86400 {
            format!("{}h", secs / 3600)
        } else {
            format!("{}d", secs / 86400)
        }
    }
}

pub struct PubSubWidget {
    pub channels: Vec<String>,
    pub messages: VecDeque<PubSubMessage>,
    pub scroll: u16,
    pub input: String,
    pub cursor: usize,
    pub active_channel: Option<String>,
    pub input_mode: PubSubInputMode,
    pub channel_cursor: usize,
    pub channel_scroll: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSubInputMode {
    Channel,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubAction {
    Subscribe(String),
    Unsubscribe(String),
    Publish { channel: String, message: String },
}

impl Default for PubSubWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSubWidget {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            messages: VecDeque::new(),
            scroll: 0,
            input: String::new(),
            cursor: 0,
            active_channel: None,
            input_mode: PubSubInputMode::Channel,
            channel_cursor: 0,
            channel_scroll: 0,
        }
    }

    pub fn handle_event(&mut self, event: &crate::events::Event) -> Option<PubSubAction> {
        use crossterm::event::KeyCode;

        if let crate::events::Event::Key(key) = event {
            match key.code {
                KeyCode::Tab => {
                    self.input_mode = match self.input_mode {
                        PubSubInputMode::Channel => PubSubInputMode::Message,
                        PubSubInputMode::Message => PubSubInputMode::Channel,
                    };
                    self.input.clear();
                    self.cursor = 0;
                }
                KeyCode::Backspace => {
                    if self.cursor > 0 {
                        self.input.remove(self.cursor - 1);
                        self.cursor -= 1;
                    }
                }
                KeyCode::Left if self.cursor > 0 => self.cursor -= 1,
                KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.input.len(),
                KeyCode::Up => {
                    if self.input_mode == PubSubInputMode::Channel {
                        if self.channel_cursor > 0 {
                            self.channel_cursor -= 1;
                        }
                    } else if self.scroll > 0 {
                        self.scroll -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.input_mode == PubSubInputMode::Channel {
                        if self.channel_cursor + 1 < self.channels.len() {
                            self.channel_cursor += 1;
                        }
                    } else {
                        let max_scroll = self.messages.len().saturating_sub(1) as u16;
                        if self.scroll < max_scroll {
                            self.scroll += 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    let value = self.input.trim().to_string();
                    if value.is_empty() {
                        return None;
                    }

                    match self.input_mode {
                        PubSubInputMode::Channel => {
                            self.active_channel = Some(value.clone());
                            if !self.channels.iter().any(|c| c == &value) {
                                self.channels.push(value.clone());
                            }
                            self.input.clear();
                            self.cursor = 0;
                            return Some(PubSubAction::Subscribe(value));
                        }
                        PubSubInputMode::Message => {
                            if let Some(channel) = self.active_channel.clone() {
                                self.input.clear();
                                self.cursor = 0;
                                return Some(PubSubAction::Publish {
                                    channel,
                                    message: value,
                                });
                            }
                        }
                    }
                }
                KeyCode::Char('D') => {
                    if self.input_mode == PubSubInputMode::Channel
                        && let Some(channel) = self.channels.get(self.channel_cursor).cloned()
                    {
                        self.channels.remove(self.channel_cursor);
                        if self.channel_cursor >= self.channels.len() && self.channel_cursor > 0 {
                            self.channel_cursor -= 1;
                        }
                        if self.active_channel.as_deref() == Some(&channel) {
                            self.active_channel = self.channels.first().cloned();
                        }
                        return Some(PubSubAction::Unsubscribe(channel));
                    }
                }
                KeyCode::Char('l')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.messages.clear();
                    self.scroll = 0;
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
                }
                _ => {}
            }
        }

        None
    }

    pub fn push_message(&mut self, msg: PubSubMessage) {
        if self.messages.len() > 1000 {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(area);

        self.render_channel_panel(frame, chunks[0]);
        self.render_message_panel(frame, chunks[1]);
    }

    fn render_channel_panel(&self, frame: &mut Frame, area: Rect) {
        let mode_label = match self.input_mode {
            PubSubInputMode::Channel => "[channel]",
            PubSubInputMode::Message => "[message]",
        };

        let mut lines: Vec<Line> = Vec::new();

        if self.channels.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No channels",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let max_visible = area.height.saturating_sub(6) as usize;
            let start = self.channel_scroll as usize;
            for (i, ch) in self.channels.iter().skip(start).take(max_visible).enumerate() {
                let abs_idx = start + i;
                let is_active = self.active_channel.as_deref() == Some(ch);
                let prefix = if abs_idx == self.channel_cursor {
                    if is_active { " ▶" } else { " ▷" }
                } else {
                    "  "
                };
                let style = if is_active {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if abs_idx == self.channel_cursor {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };
                lines.push(Line::from(Span::styled(
                    format!("{} {}", prefix, ch),
                    style,
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" Mode: {}", mode_label),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            format!(" > {}", self.input),
            Style::default().fg(Color::Green),
        )));

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Channels "),
            );
        frame.render_widget(para, area);
    }

    fn render_message_panel(&self, frame: &mut Frame, area: Rect) {
        let mode_label = match self.input_mode {
            PubSubInputMode::Channel => "channel",
            PubSubInputMode::Message => "message",
        };
        let active = self.active_channel.as_deref().unwrap_or("<none>");

        let mut lines: Vec<Line> = Vec::new();

        let info_line = Line::from(vec![
            Span::styled("Channel: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                active,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("Tab: {}", mode_label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" [{} msgs]", self.messages.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        lines.push(info_line);
        lines.push(Line::from(""));

        let max_visible = area.height.saturating_sub(4) as usize;

        for msg in self.messages.iter().rev().skip(self.scroll as usize).take(max_visible) {
            let channel_tag = if let Some(ref p) = msg.pattern {
                format!("[{}:{}]", p, msg.channel)
            } else {
                format!("[{}]", msg.channel)
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<4}", msg.relative_time()),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    channel_tag,
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().fg(Color::White)),
                Span::styled(&msg.payload, Style::default().fg(Color::White)),
            ]));
        }

        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Messages "));
        frame.render_widget(para, area);
    }
}
