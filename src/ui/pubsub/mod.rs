use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub use crate::redis::pubsub::PubSubKind;
use crate::redis::pubsub::PubSubMessage;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct Subscription {
    pub kind: PubSubKind,
    pub color: Color,
}

pub struct PubSubWidget {
    pub subscriptions: Vec<Subscription>,
    pub messages: VecDeque<PubSubMessage>,
    pub scroll: u16,
    /// Whether the message pane follows new arrivals.
    ///
    /// The pane renders newest-first, so `scroll == 0` is already the newest
    /// message. "Following" therefore means *staying pinned at the top* rather
    /// than actively scrolling: turning it off freezes the current position so
    /// a busy channel cannot push the user's reading position away mid-sentence.
    pub follow: bool,
    pub input: String,
    pub cursor: usize,
    pub active_subscription: Option<PubSubKind>,
    pub input_mode: PubSubInputMode,
    pub channel_cursor: usize,
    pub channel_scroll: u16,
}

/// Colours assigned to subscriptions in rotation.
///
/// Deliberately *not* themed: this is a categorical palette whose job is to
/// make adjacent channels visually distinct at a glance. Mapping it onto theme
/// fields would make some channels indistinguishable under a theme whose
/// related colours are close together. These are also ANSI colors rather than
/// truecolor, which keeps them legible on terminals that ignore the theme's
/// background.
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
            follow: true,
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
                    if self.input_mode == PubSubInputMode::Channel
                        || self.input_mode == PubSubInputMode::Pattern
                    {
                        if self.channel_cursor > 0 {
                            self.channel_cursor -= 1;
                        }
                    } else if self.scroll > 0 {
                        // The pane is rendered newest-first, so Up moves toward
                        // the newest message. Landing back on it is the live
                        // tail, so resume following.
                        self.scroll -= 1;
                        if self.scroll == 0 {
                            self.follow = true;
                        }
                    }
                }
                KeyCode::Down => {
                    if self.input_mode == PubSubInputMode::Channel
                        || self.input_mode == PubSubInputMode::Pattern
                    {
                        if self.channel_cursor + 1 < self.subscriptions.len() {
                            self.channel_cursor += 1;
                        }
                    } else {
                        let max_scroll = self.messages.len().saturating_sub(1) as u16;
                        if self.scroll < max_scroll {
                            // Down moves into older history. That is a request
                            // to stay there, so detach: otherwise the next
                            // message would snap the view back to the newest.
                            self.follow = false;
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
                                let color =
                                    COLOR_CYCLE[self.subscriptions.len() % COLOR_CYCLE.len()];
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
                                let color =
                                    COLOR_CYCLE[self.subscriptions.len() % COLOR_CYCLE.len()];
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
                    if (self.input_mode == PubSubInputMode::Channel
                        || self.input_mode == PubSubInputMode::Pattern)
                        && let Some(sub) = self.subscriptions.get(self.channel_cursor).cloned()
                    {
                        self.subscriptions.remove(self.channel_cursor);
                        if self.channel_cursor >= self.subscriptions.len()
                            && self.channel_cursor > 0
                        {
                            self.channel_cursor -= 1;
                        }
                        if self.active_subscription.as_ref() == Some(&sub.kind) {
                            self.active_subscription =
                                self.subscriptions.first().map(|s| s.kind.clone());
                        }
                        return match sub.kind {
                            PubSubKind::Channel(ch) => Some(PubSubAction::Unsubscribe(ch)),
                            PubSubKind::Pattern(pat) => Some(PubSubAction::PUnsubscribe(pat)),
                        };
                    }
                }
                KeyCode::Char('f') => {
                    self.toggle_follow();
                }
                KeyCode::Char('l')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.messages.clear();
                    self.scroll = 0;
                    self.follow = true;
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

    /// Toggle follow mode.
    ///
    /// Turning it on snaps back to the newest message; turning it off leaves the
    /// viewport where it is so new arrivals do not move it.
    pub fn toggle_follow(&mut self) -> bool {
        self.follow = !self.follow;
        if self.follow {
            self.scroll = 0;
        }
        self.follow
    }

    /// Title for the message pane, including the follow indicator.
    ///
    /// The indicator is always shown, not only when detached: a pane that looks
    /// identical whether it is tailing or frozen leaves the user unable to tell
    /// which one they are looking at.
    pub fn messages_title(&self) -> String {
        let follow = if self.follow {
            "[f] follow "
        } else {
            "[f] PAUSED "
        };
        format!(" Messages {follow}({}) ", self.messages.len())
    }

    /// Whether a new message would move the viewport.
    ///
    /// Used by the render loop to decide whether a redraw is needed at all: a
    /// detached pane whose content did not change needs no frame.
    pub fn needs_redraw_on_push(&self) -> bool {
        self.follow || self.messages.len() < 1000
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
                Style::default().fg(theme::current().border),
            )));
        } else {
            let max_visible = area.height.saturating_sub(6) as usize;
            let start = self.channel_scroll as usize;
            for (i, sub) in self
                .subscriptions
                .iter()
                .skip(start)
                .take(max_visible)
                .enumerate()
            {
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
                    Style::default().fg(theme::current().prompt)
                } else {
                    Style::default().fg(theme::current().fg)
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
            Style::default().fg(theme::current().border),
        )));
        lines.push(Line::from(Span::styled(
            format!(" > {}", self.input),
            Style::default().fg(theme::current().success),
        )));

        let para = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_style())
                .title_style(theme::title_style())
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
        let active = self
            .active_subscription
            .as_ref()
            .map(|k| k.name())
            .unwrap_or("<none>");

        let mut lines: Vec<Line> = Vec::new();

        let info_line = Line::from(vec![
            Span::styled("Channel: ", Style::default().fg(theme::current().border)),
            Span::styled(
                active,
                Style::default()
                    .fg(theme::current().title)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | ", Style::default().fg(theme::current().border)),
            Span::styled(
                format!("Tab: {}", mode_label),
                Style::default().fg(theme::current().border),
            ),
            Span::styled(
                format!(" [{} msgs]", self.messages.len()),
                Style::default().fg(theme::current().border),
            ),
        ]);
        lines.push(info_line);
        lines.push(Line::from(""));

        let max_visible = area.height.saturating_sub(4) as usize;

        for msg in self
            .messages
            .iter()
            .rev()
            .skip(self.scroll as usize)
            .take(max_visible)
        {
            let channel_tag = if let Some(ref p) = msg.pattern {
                format!("[{}:{}]", p, msg.channel)
            } else {
                format!("[{}]", msg.channel)
            };
            let tag_color = self
                .subscriptions
                .iter()
                .find(|s| {
                    s.kind.name() == msg.channel || msg.pattern.as_deref() == Some(s.kind.name())
                })
                .map(|s| s.color)
                .unwrap_or_else(|| theme::current().type_zset);
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<4}", msg.relative_time()),
                    Style::default().fg(theme::current().border),
                ),
                Span::styled(
                    channel_tag,
                    Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().fg(theme::current().fg)),
                Span::styled(&msg.payload, Style::default().fg(theme::current().fg)),
            ]));
        }

        let para = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_style())
                .title_style(theme::title_style())
                .title(self.messages_title()),
        );
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

    #[allow(dead_code)]
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
        assert_eq!(
            action,
            Some(PubSubAction::Subscribe("test-chan".to_string()))
        );
        assert_eq!(widget.subscriptions.len(), 1);
        assert_eq!(
            widget.subscriptions[0].kind,
            PubSubKind::Channel("test-chan".to_string())
        );
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
        assert_eq!(
            widget.subscriptions[0].kind,
            PubSubKind::Pattern("test-*".to_string())
        );
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
        assert_eq!(
            widget.subscriptions[0].kind,
            PubSubKind::Pattern("pat*".to_string())
        );
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
        assert_eq!(
            widget.subscriptions[0].kind,
            PubSubKind::Channel("ch1".to_string())
        );
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
        terminal.draw(|f| widget.render(f, f.area())).unwrap();
    }
}

#[cfg(test)]
mod follow_tests {
    use super::*;
    use crate::events::Event;
    use crossterm::event::{KeyCode, KeyEvent};

    fn msg(n: usize) -> PubSubMessage {
        PubSubMessage {
            channel: format!("ch{n}"),
            payload: format!("payload {n}"),
            pattern: None,
            timestamp: std::time::Instant::now(),
        }
    }

    fn widget_with(count: usize) -> PubSubWidget {
        let mut w = PubSubWidget::new();
        // Message mode, not the default Channel mode: while the channel input
        // has focus, Up/Down navigate the subscription list rather than the
        // message pane, which is where follow mode applies.
        w.input_mode = PubSubInputMode::Message;
        for i in 0..count {
            w.push_message(msg(i));
        }
        w
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    /// The pane renders newest-first, so scroll 0 is the live tail. Follow is
    /// on by default because that is what a message pane is for.
    #[test]
    fn follow_is_on_by_default() {
        assert!(PubSubWidget::new().follow);
        assert_eq!(PubSubWidget::new().scroll, 0);
    }

    #[test]
    fn f_toggles_follow() {
        let mut w = widget_with(5);
        assert!(w.follow);

        w.handle_event(&key(KeyCode::Char('f')));
        assert!(!w.follow, "f should pause");

        w.handle_event(&key(KeyCode::Char('f')));
        assert!(w.follow, "f again should resume");
    }

    #[test]
    fn resuming_follow_snaps_back_to_the_newest_message() {
        let mut w = widget_with(10);
        // Detach and scroll into history.
        w.handle_event(&key(KeyCode::Char('f')));
        for _ in 0..4 {
            w.handle_event(&key(KeyCode::Down));
        }
        assert!(w.scroll > 0, "expected to be in history");

        w.handle_event(&key(KeyCode::Char('f')));
        assert!(w.follow);
        assert_eq!(w.scroll, 0, "resuming must snap to the newest message");
    }

    /// The whole point of the feature: a detached pane must not move when
    /// messages arrive.
    #[test]
    fn detached_pane_does_not_move_when_messages_arrive() {
        let mut w = widget_with(10);
        w.handle_event(&key(KeyCode::Char('f')));
        for _ in 0..3 {
            w.handle_event(&key(KeyCode::Down));
        }
        let position = w.scroll;
        assert!(position > 0);

        w.push_message(msg(99));
        assert_eq!(
            w.scroll, position,
            "the viewport must stay where the user put it"
        );
        assert_eq!(w.messages.back().unwrap().payload, "payload 99");
    }

    #[test]
    fn scrolling_into_history_detaches_automatically() {
        let mut w = widget_with(10);
        assert!(w.follow);
        w.handle_event(&key(KeyCode::Down));
        assert!(!w.follow, "scrolling into history should pause follow");
        assert_eq!(w.scroll, 1);
    }

    /// Coming back to the newest message is the live tail again.
    #[test]
    fn scrolling_back_to_the_newest_resumes_following() {
        let mut w = widget_with(10);
        w.handle_event(&key(KeyCode::Char('f')));
        w.handle_event(&key(KeyCode::Down));
        w.handle_event(&key(KeyCode::Down));
        assert!(!w.follow);

        w.handle_event(&key(KeyCode::Up));
        w.handle_event(&key(KeyCode::Up));
        assert_eq!(w.scroll, 0);
        assert!(w.follow, "returning to the tail should resume follow");
    }

    /// The pane must not silently resume while the user is still away from it.
    #[test]
    fn follow_does_not_resume_while_still_in_history() {
        let mut w = widget_with(10);
        w.handle_event(&key(KeyCode::Down));
        w.handle_event(&key(KeyCode::Down));
        w.handle_event(&key(KeyCode::Down));
        assert_eq!(w.scroll, 3);
        assert!(!w.follow);

        w.push_message(msg(99));
        assert!(!w.follow, "an arriving message must not re-attach");
        assert_eq!(w.scroll, 3);
    }

    #[test]
    fn follow_is_not_detached_by_scrolling_at_the_top() {
        let mut w = widget_with(1);
        w.handle_event(&key(KeyCode::Down));
        assert_eq!(w.scroll, 0, "nothing older to show");
        assert!(
            w.follow,
            "must stay attached when there is nothing to scroll"
        );
    }

    #[test]
    fn follow_is_not_detached_by_scrolling_with_no_messages() {
        let mut w = PubSubWidget::new();
        w.input_mode = PubSubInputMode::Message;
        w.handle_event(&key(KeyCode::Down));
        assert!(w.follow);
        w.handle_event(&key(KeyCode::Up));
        assert!(w.follow);
    }

    #[test]
    fn clearing_messages_restores_follow() {
        let mut w = widget_with(10);
        w.handle_event(&key(KeyCode::Char('f')));
        assert!(!w.follow);

        w.handle_event(&Event::Key(KeyEvent::new(
            KeyCode::Char('l'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        assert!(w.follow, "a cleared pane is trivially at the tail");
        assert_eq!(w.scroll, 0);
        assert!(w.messages.is_empty());
    }

    // ---- title ----

    #[test]
    fn title_shows_the_count() {
        let w = widget_with(7);
        assert!(
            w.messages_title().contains("(7)"),
            "got {:?}",
            w.messages_title()
        );
    }

    /// The indicator must appear in both states, otherwise the pane looks
    /// identical whether it is tailing or frozen.
    #[test]
    fn title_distinguishes_following_from_paused() {
        let mut w = widget_with(3);
        let following = w.messages_title();
        assert!(following.contains("follow"), "got {following:?}");
        assert!(!following.contains("PAUSED"), "got {following:?}");

        w.toggle_follow();
        let paused = w.messages_title();
        assert!(paused.contains("PAUSED"), "got {paused:?}");
    }

    #[test]
    fn title_mentions_the_toggle_key() {
        let w = PubSubWidget::new();
        assert!(w.messages_title().contains("[f]"));
    }

    #[test]
    fn needs_redraw_depends_on_follow_and_capacity() {
        let mut w = widget_with(5);
        assert!(w.needs_redraw_on_push());
        w.toggle_follow();
        // Below the 1000-message cap the list itself changed, so a redraw is
        // still needed even though the viewport did not move.
        assert!(
            w.needs_redraw_on_push(),
            "new content below the cap still needs painting"
        );

        for i in 0..1100 {
            w.push_message(msg(i));
        }
        assert!(
            !w.needs_redraw_on_push(),
            "a full buffer drops the oldest message, which is not visible"
        );
    }

    #[test]
    fn buffer_is_capped_at_one_thousand_messages() {
        let mut w = PubSubWidget::new();
        for i in 0..1200 {
            w.push_message(msg(i));
        }
        assert!(
            w.messages.len() <= 1001,
            "expected the buffer to be capped, got {}",
            w.messages.len()
        );
        // The newest must always be present.
        assert_eq!(w.messages.back().unwrap().payload, "payload 1199");
    }

    #[test]
    fn render_with_paused_follow_does_not_panic() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut w = widget_with(50);
        w.toggle_follow();
        for _ in 0..5 {
            w.handle_event(&key(KeyCode::Down));
        }
        terminal.draw(|f| w.render(f, f.area())).expect("renders");
    }
}
