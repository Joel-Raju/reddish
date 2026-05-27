use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubKind {
    Channel(String),
    Pattern(String),
}

impl PubSubKind {
    pub fn name(&self) -> &str {
        match self {
            PubSubKind::Channel(n) => n,
            PubSubKind::Pattern(n) => n,
        }
    }

    pub fn is_pattern(&self) -> bool {
        matches!(self, PubSubKind::Pattern(_))
    }
}

#[derive(Debug, Clone)]
pub struct Subscription {
    pub kind: PubSubKind,
    pub color: Color,
}

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
    pub subscriptions: Vec<Subscription>,
    pub messages: VecDeque<PubSubMessage>,
    pub scroll: u16,
    pub input: String,
    pub cursor: usize,
    pub active_subscription: Option<PubSubKind>,
    pub input_mode: PubSubInputMode,
    pub channel_cursor: usize,
    pub channel_scroll: u16,
}

const COLOR_CYCLE: &[Color] = &[
    Color::Cyan,
    Color::Magenta,
    Color::Green,
    Color::Yellow,
    Color::Red,
    Color::Blue,
    Color::White,
    Color::Gray,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSubInputMode {
    Channel,
    Message,
    Pattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubAction {
    Subscribe(String),
    Unsubscribe(String),
    PSubscribe(String),
    PUnsubscribe(String),
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
            subscriptions: Vec::new(),
            messages: VecDeque::new(),
            scroll: 0,
            input: String::new(),
            cursor: 0,
            active_subscription: None,
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
                        PubSubInputMode::Message => PubSubInputMode::Pattern,
                        PubSubInputMode::Pattern => PubSubInputMode::Channel,
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
                    if self.input_mode == PubSubInputMode::Channel || self.input_mode == PubSubInputMode::Pattern {
                        if self.channel_cursor > 0 {
                            self.channel_cursor -= 1;
                        }
                    } else if self.scroll > 0 {
                        self.scroll -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.input_mode == PubSubInputMode::Channel || self.input_mode == PubSubInputMode::Pattern {
                        if self.channel_cursor + 1 < self.subscriptions.len() {
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
                            let kind = PubSubKind::Channel(value.clone());
                            self.active_subscription = Some(kind.clone());
                            if !self.subscriptions.iter().any(|s| s.kind == kind) {
                                let color = COLOR_CYCLE[self.subscriptions.len() % COLOR_CYCLE.len()];
                                self.subscriptions.push(Subscription { kind, color });
                            }
                            self.input.clear();
                            self.cursor = 0;
                            return Some(PubSubAction::Subscribe(value));
                        }
                        PubSubInputMode::Pattern => {
                            let kind = PubSubKind::Pattern(value.clone());
                            self.active_subscription = Some(kind.clone());
                            if !self.subscriptions.iter().any(|s| s.kind == kind) {
                                let color = COLOR_CYCLE[self.subscriptions.len() % COLOR_CYCLE.len()];
                                self.subscriptions.push(Subscription { kind, color });
                            }
                            self.input.clear();
                            self.cursor = 0;
                            return Some(PubSubAction::PSubscribe(value));
                        }
                        PubSubInputMode::Message => {
                            if let Some(ref kind) = self.active_subscription.clone() {
                                self.input.clear();
                                self.cursor = 0;
                                return Some(PubSubAction::Publish {
                                    channel: kind.name().to_string(),
                                    message: value,
                                });
                            }
                        }
                    }
                }
                KeyCode::Char('D') => {
                    if (self.input_mode == PubSubInputMode::Channel || self.input_mode == PubSubInputMode::Pattern)
                        && let Some(sub) = self.subscriptions.get(self.channel_cursor).cloned()
                    {
                        self.subscriptions.remove(self.channel_cursor);
                        if self.channel_cursor >= self.subscriptions.len() && self.channel_cursor > 0 {
                            self.channel_cursor -= 1;
                        }
                        if self.active_subscription.as_ref() == Some(&sub.kind) {
                            self.active_subscription = self.subscriptions.first().map(|s| s.kind.clone());
                        }
                        return match sub.kind {
                            PubSubKind::Channel(ch) => Some(PubSubAction::Unsubscribe(ch)),
                            PubSubKind::Pattern(pat) => Some(PubSubAction::PUnsubscribe(pat)),
                        };
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
            PubSubInputMode::Pattern => "[pattern]",
        };

        let mut lines: Vec<Line> = Vec::new();

        if self.subscriptions.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No subscriptions",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let max_visible = area.height.saturating_sub(6) as usize;
            let start = self.channel_scroll as usize;
            for (i, sub) in self.subscriptions.iter().skip(start).take(max_visible).enumerate() {
                let abs_idx = start + i;
                let is_active = self.active_subscription.as_ref() == Some(&sub.kind);
                let prefix = if abs_idx == self.channel_cursor {
                    if is_active { " ▶" } else { " ▷" }
                } else {
                    "  "
                };
                let label = match &sub.kind {
                    PubSubKind::Channel(ch) => ch.to_string(),
                    PubSubKind::Pattern(pat) => format!("{}*", pat),
                };
                let style = if is_active {
                    Style::default().fg(sub.color).add_modifier(Modifier::BOLD)
                } else if abs_idx == self.channel_cursor {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };
                lines.push(Line::from(Span::styled(
                    format!("{} {}", prefix, label),
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
            PubSubInputMode::Pattern => "pattern",
        };
        let active = self.active_subscription.as_ref().map(|k| k.name()).unwrap_or("<none>");

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
            let tag_color = self.subscriptions.iter()
                .find(|s| s.kind.name() == msg.channel || msg.pattern.as_deref() == Some(s.kind.name()))
                .map(|s| s.color)
                .unwrap_or(Color::Blue);
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<4}", msg.relative_time()),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    channel_tag,
                    Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn key_event_with_mods(code: KeyCode, mods: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, mods))
    }

    #[test]
    fn tab_cycles_input_modes() {
        let mut widget = PubSubWidget::new();
        assert_eq!(widget.input_mode, PubSubInputMode::Channel);

        widget.handle_event(&key_event(KeyCode::Tab));
        assert_eq!(widget.input_mode, PubSubInputMode::Message);

        widget.handle_event(&key_event(KeyCode::Tab));
        assert_eq!(widget.input_mode, PubSubInputMode::Pattern);

        widget.handle_event(&key_event(KeyCode::Tab));
        assert_eq!(widget.input_mode, PubSubInputMode::Channel);
    }

    #[test]
    fn subscribe_adds_channel_subscription_with_color() {
        let mut widget = PubSubWidget::new();
        widget.input = "test-chan".to_string();
        let action = widget.handle_event(&key_event(KeyCode::Enter));
        assert_eq!(action, Some(PubSubAction::Subscribe("test-chan".to_string())));
        assert_eq!(widget.subscriptions.len(), 1);
        assert_eq!(widget.subscriptions[0].kind, PubSubKind::Channel("test-chan".to_string()));
        assert_eq!(widget.subscriptions[0].color, Color::Cyan);
    }

    #[test]
    fn psubscribe_adds_pattern_subscription_with_color() {
        let mut widget = PubSubWidget::new();
        widget.input_mode = PubSubInputMode::Pattern;
        widget.input = "test-*".to_string();
        let action = widget.handle_event(&key_event(KeyCode::Enter));
        assert_eq!(action, Some(PubSubAction::PSubscribe("test-*".to_string())));
        assert_eq!(widget.subscriptions.len(), 1);
        assert_eq!(widget.subscriptions[0].kind, PubSubKind::Pattern("test-*".to_string()));
        assert_eq!(widget.subscriptions[0].color, Color::Cyan);
    }

    #[test]
    fn unsubscribe_removes_channel_subscription() {
        let mut widget = PubSubWidget::new();
        widget.subscriptions.push(Subscription {
            kind: PubSubKind::Channel("ch1".to_string()),
            color: Color::Cyan,
        });
        widget.subscriptions.push(Subscription {
            kind: PubSubKind::Pattern("pat*".to_string()),
            color: Color::Magenta,
        });
        let action = widget.handle_event(&key_event(KeyCode::Char('D')));
        assert_eq!(action, Some(PubSubAction::Unsubscribe("ch1".to_string())));
        assert_eq!(widget.subscriptions.len(), 1);
        assert_eq!(widget.subscriptions[0].kind, PubSubKind::Pattern("pat*".to_string()));
    }

    #[test]
    fn punsubscribe_removes_pattern_subscription() {
        let mut widget = PubSubWidget::new();
        widget.input_mode = PubSubInputMode::Pattern;
        widget.subscriptions.push(Subscription {
            kind: PubSubKind::Channel("ch1".to_string()),
            color: Color::Cyan,
        });
        widget.subscriptions.push(Subscription {
            kind: PubSubKind::Pattern("pat*".to_string()),
            color: Color::Magenta,
        });
        widget.channel_cursor = 1;
        let action = widget.handle_event(&key_event(KeyCode::Char('D')));
        assert_eq!(action, Some(PubSubAction::PUnsubscribe("pat*".to_string())));
        assert_eq!(widget.subscriptions.len(), 1);
        assert_eq!(widget.subscriptions[0].kind, PubSubKind::Channel("ch1".to_string()));
    }

    #[test]
    fn color_cycle_wraps_after_8_subscriptions() {
        let mut widget = PubSubWidget::new();
        for i in 0..10 {
            widget.input = format!("ch{}", i);
            widget.handle_event(&key_event(KeyCode::Enter));
        }
        assert_eq!(widget.subscriptions.len(), 10);
        assert_eq!(widget.subscriptions[0].color, Color::Cyan);
        assert_eq!(widget.subscriptions[7].color, Color::Gray);
        assert_eq!(widget.subscriptions[8].color, Color::Cyan);
        assert_eq!(widget.subscriptions[9].color, Color::Magenta);
    }

    #[test]
    fn render_with_subscriptions_does_not_panic() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut widget = PubSubWidget::new();
        widget.subscriptions.push(Subscription {
            kind: PubSubKind::Channel("chan".to_string()),
            color: Color::Cyan,
        });
        widget.active_subscription = Some(PubSubKind::Channel("chan".to_string()));
        widget.messages.push_back(PubSubMessage {
            channel: "chan".to_string(),
            pattern: None,
            payload: "hello".to_string(),
            timestamp: std::time::Instant::now(),
        });
        terminal.draw(|f| widget.render(f, f.size())).unwrap();
    }
}
