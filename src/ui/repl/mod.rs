use std::collections::VecDeque;

use crossterm::event::KeyModifiers;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::events::Event;

pub mod autocomplete;
pub mod history;
use autocomplete::ReplAutocomplete;
use history::CommandHistory;

pub fn tokenize_shell_like(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in input.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

pub fn parse_pipeline(input: &str) -> Vec<Vec<String>> {
    input
        .split(";;")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(tokenize_shell_like)
        .filter(|tokens| !tokens.is_empty())
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplLineStatus {
    Success,
    Error(String),
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplLine {
    pub input: String,
    pub output: String,
    pub status: ReplLineStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplAction {
    Submit(String),
}

pub struct ReplWidget {
    pub input: String,
    pub history: VecDeque<ReplLine>,
    pub command_history: CommandHistory,
    pub cursor: usize,
    pub scroll: u16,
    pub max_history: usize,
    pub raw_mode: bool,
    pub autocomplete: ReplAutocomplete,
    pub ac_visible: bool,
    pub ac_suggestions: Vec<String>,
    pub ac_selected: usize,
    pub monitor_visible: bool,
    pub monitor_lines: VecDeque<String>,
    pub monitor_scroll: u16,
}

impl Default for ReplWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplWidget {
    pub fn new() -> Self {
        Self::with_history_path(Self::default_history_path())
    }

    /// Where the command history is persisted.
    fn default_history_path() -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("redis-tui")
            .join("history")
    }

    /// Build a widget whose history is stored at `path`.
    ///
    /// Exposed so tests can use an isolated file. `new()` reads *and writes*
    /// the user's real history, so using it in tests would both pollute their
    /// data directory and make results depend on test execution order.
    pub fn with_history_path(path: std::path::PathBuf) -> Self {
        Self {
            input: String::new(),
            history: VecDeque::new(),
            command_history: CommandHistory::load(&path),
            cursor: 0,
            scroll: 0,
            max_history: 1000,
            raw_mode: false,
            autocomplete: ReplAutocomplete::new(),
            ac_visible: false,
            ac_suggestions: Vec::new(),
            ac_selected: 0,
            monitor_visible: false,
            monitor_lines: VecDeque::new(),
            monitor_scroll: 0,
        }
    }

    pub fn push_monitor_line(&mut self, line: String) {
        if self.monitor_lines.len() > 1000 {
            self.monitor_lines.pop_front();
        }
        self.monitor_lines.push_back(line);
    }

    pub fn update_key_cache(&mut self, keys: Vec<String>) {
        self.autocomplete.update_key_cache(keys);
    }

    fn open_autocomplete(&mut self) {
        let trimmed = self.input.trim_start();
        if trimmed.is_empty() {
            self.ac_visible = false;
            return;
        }
        if !trimmed.contains(' ') {
            self.ac_suggestions = self.autocomplete.suggest_commands(trimmed, 8);
        } else {
            let tokens = tokenize_shell_like(trimmed);
            if tokens.len() >= 2 {
                let last = &tokens[tokens.len() - 1];
                self.ac_suggestions = self.autocomplete.suggest_keys(last, 8);
            } else {
                self.ac_suggestions.clear();
            }
        }
        self.ac_selected = 0;
        self.ac_visible = !self.ac_suggestions.is_empty();
    }

    fn accept_autocomplete(&mut self) {
        if let Some(suggestion) = self.ac_suggestions.get(self.ac_selected) {
            let trimmed = self.input.trim_start();
            if !trimmed.contains(' ') {
                let leading_ws: String = self
                    .input
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                self.input = format!("{}{} ", leading_ws, suggestion);
                self.cursor = self.input.len();
            } else {
                let tokens = tokenize_shell_like(trimmed);
                let mut new_input = tokens[..tokens.len().saturating_sub(1)].join(" ");
                if !new_input.is_empty() {
                    new_input.push(' ');
                }
                new_input.push_str(suggestion);
                self.input = new_input;
                self.cursor = self.input.len();
            }
        }
        self.ac_visible = false;
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ReplAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' => {
                    self.input.clear();
                    self.cursor = 0;
                    self.ac_visible = false;
                    self.command_history.reset_cursor();
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'l' => {
                    self.history.clear();
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'r' => {
                    self.raw_mode = !self.raw_mode;
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'm' => {
                    self.monitor_visible = !self.monitor_visible;
                }
                KeyCode::Esc => {
                    self.ac_visible = false;
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
                    self.ac_visible = false;
                    self.command_history.reset_cursor();
                }
                KeyCode::Backspace => {
                    if self.cursor > 0 {
                        self.input.remove(self.cursor - 1);
                        self.cursor -= 1;
                        self.ac_visible = false;
                        self.command_history.reset_cursor();
                    }
                }
                KeyCode::Delete => {
                    if self.cursor < self.input.len() {
                        self.input.remove(self.cursor);
                        self.ac_visible = false;
                        self.command_history.reset_cursor();
                    }
                }
                KeyCode::Left if self.cursor > 0 => {
                    self.cursor -= 1;
                    self.ac_visible = false;
                }
                KeyCode::Right if self.cursor < self.input.len() => {
                    self.cursor += 1;
                    self.ac_visible = false;
                }
                KeyCode::Home => {
                    self.cursor = 0;
                    self.ac_visible = false;
                }
                KeyCode::End => {
                    self.cursor = self.input.len();
                    self.ac_visible = false;
                }
                KeyCode::Tab => {
                    if self.ac_visible {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            if self.ac_selected > 0 {
                                self.ac_selected -= 1;
                            }
                        } else if self.ac_selected + 1 < self.ac_suggestions.len() {
                            self.ac_selected += 1;
                        }
                    } else {
                        self.open_autocomplete();
                    }
                }
                KeyCode::Enter => {
                    if self.ac_visible {
                        self.accept_autocomplete();
                        return None;
                    }
                    let cmd = self.input.trim().to_string();
                    if !cmd.is_empty() {
                        self.command_history.push(cmd.clone());
                        let _ = self.command_history.save();
                        self.history.push_back(ReplLine {
                            input: cmd.clone(),
                            output: String::new(),
                            status: ReplLineStatus::Info,
                        });
                        if self.history.len() > self.max_history {
                            self.history.pop_front();
                        }
                        self.input.clear();
                        self.cursor = 0;
                        return Some(ReplAction::Submit(cmd));
                    }
                }
                KeyCode::Up => {
                    self.input = self.command_history.prev(&self.input);
                    self.cursor = self.input.len();
                    self.ac_visible = false;
                }
                KeyCode::Down => {
                    self.input = self.command_history.next_entry();
                    self.cursor = self.input.len();
                    self.ac_visible = false;
                }
                _ => {}
            }
        }
        None
    }

    pub fn add_result(&mut self, cmd: &str, output: String, status: ReplLineStatus) {
        if let Some(line) = self.history.iter_mut().rev().find(|l| l.input == cmd) {
            line.output = output;
            line.status = status;
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let ac_height = if self.ac_visible && !self.ac_suggestions.is_empty() {
            (self.ac_suggestions.len() as u16 + 2).min(10)
        } else {
            0
        };
        let mon_height = if self.monitor_visible {
            area.height / 3
        } else {
            0
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(ac_height),
                Constraint::Length(mon_height),
            ])
            .split(area);

        let block = Block::default().borders(Borders::ALL).title("REPL");
        let history_text: String = self
            .history
            .iter()
            .rev()
            .take(chunks[0].height.saturating_sub(3) as usize)
            .map(|line| {
                let prefix = match line.status {
                    ReplLineStatus::Success => "✓ ".to_string(),
                    ReplLineStatus::Error(ref e) => format!("✗ {} ", e),
                    ReplLineStatus::Info => "› ".to_string(),
                };
                format!("{}{}\n{}", prefix, line.input, line.output)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!("{}\n> {}", history_text, self.input);
        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(paragraph, chunks[0]);

        if self.ac_visible && !self.ac_suggestions.is_empty() {
            let lines: Vec<Line> = self
                .ac_suggestions
                .iter()
                .enumerate()
                .map(|(i, sug)| {
                    let style = if i == self.ac_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(Span::styled(format!("  {}  ", sug), style))
                })
                .collect();
            let ac_block = Block::default().borders(Borders::ALL).title(" Suggest ");
            let ac_para = Paragraph::new(lines).block(ac_block);
            frame.render_widget(Clear, chunks[1]);
            frame.render_widget(ac_para, chunks[1]);
        }

        if self.monitor_visible {
            let mon_text: String = self
                .monitor_lines
                .iter()
                .rev()
                .take(chunks[2].height.saturating_sub(2) as usize)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            let mon_block = Block::default().borders(Borders::ALL).title(" MONITOR ");
            let mon_para = Paragraph::new(mon_text)
                .block(mon_block)
                .wrap(Wrap { trim: false })
                .scroll((self.monitor_scroll, 0));
            frame.render_widget(mon_para, chunks[2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplLine, ReplLineStatus, ReplWidget, parse_pipeline, tokenize_shell_like};
    use crate::events::Event;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Unique, isolated history path per test, so tests never read or write
    /// the developer's real REPL history.
    fn temp_history_path() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("reddish-repl-test-{}-{n}", std::process::id()))
            .join("history")
    }

    #[test]
    fn test_tokenize_shell_like_quotes() {
        let tokens = tokenize_shell_like("SET foo 'hello world'");
        assert_eq!(tokens, vec!["SET", "foo", "hello world"]);

        let tokens = tokenize_shell_like("SET foo \"hello world\"");
        assert_eq!(tokens, vec!["SET", "foo", "hello world"]);
    }

    #[test]
    fn test_parse_pipeline_stages() {
        let stages = parse_pipeline("GET foo ;; STRLEN");
        assert_eq!(stages, vec![vec!["GET", "foo"], vec!["STRLEN"]]);
    }

    #[test]
    fn test_repl_tab_autocomplete() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
        assert_eq!(repl.input, "GET ");
    }

    #[test]
    fn test_repl_tab_cycles_suggestions() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);
        let first = repl.ac_suggestions[repl.ac_selected].clone();

        // Tab again cycles to next
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        let second = repl.ac_suggestions[repl.ac_selected].clone();
        assert_ne!(first, second);
    }

    #[test]
    fn test_repl_shift_tab_cycles_back() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        let second = repl.ac_selected;

        let shift_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
        repl.handle_event(&Event::Key(shift_tab));
        assert_eq!(repl.ac_selected, second - 1);
    }

    #[test]
    fn test_repl_enter_accepts_suggestion() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);

        let action = repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(action.is_none()); // accepting suggestion does not submit
        assert!(!repl.ac_visible);
        assert_eq!(repl.input, "GET ");
    }

    #[test]
    fn test_repl_ctrl_l_clears_output() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        // Mirror the real flow: Enter pushes an Info line, then the result
        // fills it in. add_result only updates an existing entry.
        for cmd in ["GET a", "GET b"] {
            repl.history.push_back(ReplLine {
                input: cmd.to_string(),
                output: String::new(),
                status: ReplLineStatus::Info,
            });
            repl.add_result(cmd, "value".to_string(), ReplLineStatus::Success);
        }
        assert_eq!(repl.history.len(), 2);

        let ctrl_l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        repl.handle_event(&Event::Key(ctrl_l));

        assert!(repl.history.is_empty(), "Ctrl+L must clear the output pane");
    }

    #[test]
    fn test_repl_ctrl_l_does_not_clear_command_history() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.command_history.push("GET a".to_string());
        repl.command_history.push("GET b".to_string());
        repl.history.push_back(ReplLine {
            input: "GET a".to_string(),
            output: String::new(),
            status: ReplLineStatus::Info,
        });
        repl.add_result("GET a", "hello".to_string(), ReplLineStatus::Success);

        let ctrl_l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        repl.handle_event(&Event::Key(ctrl_l));

        assert!(repl.history.is_empty(), "output pane cleared");
        assert_eq!(
            repl.command_history.len(),
            2,
            "Ctrl+L clears output, not the persisted command history"
        );
    }

    #[test]
    fn test_repl_ctrl_r_toggles_raw_mode() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        assert!(!repl.raw_mode);
        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        repl.handle_event(&Event::Key(ctrl_r));
        assert!(repl.raw_mode);
        repl.handle_event(&Event::Key(ctrl_r));
        assert!(!repl.raw_mode);
    }

    #[test]
    fn test_repl_output_is_capped_at_max_history() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        for i in 0..(repl.max_history + 50) {
            repl.history.push_back(ReplLine {
                input: format!("GET k{i}"),
                output: String::new(),
                status: ReplLineStatus::Info,
            });
            if repl.history.len() > repl.max_history {
                repl.history.pop_front();
            }
        }
        assert_eq!(repl.history.len(), repl.max_history);
        // Oldest entries must be the ones dropped.
        assert!(
            repl.history.front().unwrap().input.starts_with("GET k50"),
            "expected the oldest entries evicted"
        );
    }

    #[test]
    fn test_repl_esc_closes_autocomplete() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);

        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Esc)));
        assert!(!repl.ac_visible);
    }

    #[test]
    fn test_repl_typing_closes_autocomplete() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);

        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert!(!repl.ac_visible);
    }

    #[test]
    fn test_repl_key_cache_suggestions() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.update_key_cache(vec!["user:123".to_string(), "user:456".to_string()]);
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('G'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('E'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('T'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('u'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('s'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);
        assert!(repl.ac_suggestions.iter().any(|s| s.starts_with("user:")));
    }

    #[test]
    fn test_repl_render_with_autocomplete_does_not_panic() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert!(repl.ac_visible);
        let _ = terminal.draw(|f| repl.render(f, f.area()));
    }

    #[test]
    fn test_repl_monitor_toggle() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        assert!(!repl.monitor_visible);

        let ctrl_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL);
        repl.handle_event(&Event::Key(ctrl_m));
        assert!(repl.monitor_visible);

        let ctrl_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL);
        repl.handle_event(&Event::Key(ctrl_m));
        assert!(!repl.monitor_visible);
    }

    #[test]
    fn test_repl_monitor_push_line() {
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.push_monitor_line("GET foo".to_string());
        repl.push_monitor_line("SET bar 1".to_string());
        assert_eq!(repl.monitor_lines.len(), 2);
        assert_eq!(repl.monitor_lines[0], "GET foo");
        assert_eq!(repl.monitor_lines[1], "SET bar 1");
    }

    #[test]
    fn test_repl_render_with_monitor_does_not_panic() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut repl = ReplWidget::with_history_path(temp_history_path());
        repl.monitor_visible = true;
        repl.push_monitor_line("GET foo".to_string());
        repl.push_monitor_line("SET bar 1".to_string());
        let _ = terminal.draw(|f| repl.render(f, f.area()));
    }
}
