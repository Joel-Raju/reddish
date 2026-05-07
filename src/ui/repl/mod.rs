use std::collections::VecDeque;

use crossterm::event::KeyModifiers;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::events::Event;

pub mod history;
use history::CommandHistory;

const COMMON_REDIS_COMMANDS: &[&str] = &[
    "GET", "SET", "DEL", "EXISTS", "TTL", "EXPIRE", "TYPE", "RENAME", "PING", "INFO",
    "HGET", "HSET", "HGETALL", "HDEL", "LPUSH", "RPUSH", "LPOP", "RPOP", "LRANGE", "LSET",
    "LREM", "SADD", "SREM", "SMEMBERS", "ZADD", "ZREM", "ZRANGE", "PUBLISH", "SUBSCRIBE",
    "SCAN", "KEYS", "DBSIZE", "XADD", "XRANGE", "XDEL",
];

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

fn autocomplete_first_token(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() || trimmed.contains(' ') {
        return None;
    }

    let upper = trimmed.to_uppercase();
    COMMON_REDIS_COMMANDS
        .iter()
        .find(|cmd| cmd.starts_with(&upper))
        .map(|cmd| {
            if input.starts_with(' ') {
                format!(" {}", cmd)
            } else {
                (*cmd).to_string()
            }
        })
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplLineStatus {
    Success,
    Error(String),
    Info,
}

#[cfg(test)]
mod tests {
    use super::{autocomplete_first_token, parse_pipeline, tokenize_shell_like, ReplWidget};
    use crate::events::Event;
    use crossterm::event::{KeyCode, KeyEvent};

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
    fn test_autocomplete_first_token() {
        assert_eq!(autocomplete_first_token("ge"), Some("GET".to_string()));
        assert_eq!(autocomplete_first_token(" set"), Some(" SET".to_string()));
        assert_eq!(autocomplete_first_token("SET foo"), None);
    }

    #[test]
    fn test_repl_tab_autocomplete() {
        let mut repl = ReplWidget::new();
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('g'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('e'))));
        repl.handle_event(&Event::Key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(repl.input, "GET");
    }
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
}

impl Default for ReplWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplWidget {
    pub fn new() -> Self {
        let history_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("redis-tui")
            .join("history");

        Self {
            input: String::new(),
            history: VecDeque::new(),
            command_history: CommandHistory::load(&history_path),
            cursor: 0,
            scroll: 0,
            max_history: 1000,
            raw_mode: false,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ReplAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' => {
                    self.input.clear();
                    self.cursor = 0;
                    self.command_history.reset_cursor();
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'l' => {
                    self.history.clear();
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'r' => {
                    self.raw_mode = !self.raw_mode;
                }
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
                    self.command_history.reset_cursor();
                }
                KeyCode::Backspace => {
                    if self.cursor > 0 {
                        self.input.remove(self.cursor - 1);
                        self.cursor -= 1;
                        self.command_history.reset_cursor();
                    }
                }
                KeyCode::Delete => {
                    if self.cursor < self.input.len() {
                        self.input.remove(self.cursor);
                        self.command_history.reset_cursor();
                    }
                }
                KeyCode::Left if self.cursor > 0 => self.cursor -= 1,
                KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.input.len(),
                KeyCode::Tab => {
                    if let Some(completed) = autocomplete_first_token(&self.input) {
                        self.input = completed;
                        self.cursor = self.input.len();
                    }
                }
                KeyCode::Enter => {
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
                }
                KeyCode::Down => {
                    self.input = self.command_history.next();
                    self.cursor = 0;
                    self.cursor = self.input.len();
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
        let block = Block::default().borders(Borders::ALL).title("REPL");
        let history_text: String = self
            .history
            .iter()
            .rev()
            .take(area.height.saturating_sub(3) as usize)
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
        frame.render_widget(paragraph, area);
    }
}
