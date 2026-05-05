use std::collections::VecDeque;

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

#[derive(Debug, Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub pattern: Option<String>,
    pub payload: String,
    pub timestamp: std::time::Instant,
}

pub struct PubSubWidget {
    pub channels: Vec<String>,
    pub messages: VecDeque<PubSubMessage>,
    pub scroll: u16,
    pub input: String,
    pub cursor: usize,
    pub active_channel: Option<String>,
    pub input_mode: PubSubInputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSubInputMode {
    Channel,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubAction {
    Subscribe(String),
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
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
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
        let block = Block::default().borders(Borders::ALL).title("Pub/Sub");
        let mode = match self.input_mode {
            PubSubInputMode::Channel => "channel",
            PubSubInputMode::Message => "message",
        };

        let text: String = self
            .messages
            .iter()
            .rev()
            .take(area.height.saturating_sub(2) as usize)
            .map(|m| {
                format!(
                    "[{}] {}: {}",
                    if let Some(ref p) = m.pattern {
                        format!("pattern {}", p)
                    } else {
                        m.channel.clone()
                    },
                    m.timestamp.elapsed().as_secs(),
                    m.payload
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let active = self
            .active_channel
            .as_deref()
            .unwrap_or("<none>");

        let composed = format!(
            "Active: {active}\nMode: {mode} (Tab to toggle)\nInput: {}\n\n{}",
            self.input, text
        );

        let paragraph = Paragraph::new(composed)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, area);
    }
}
