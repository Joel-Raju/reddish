use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::events::Event;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemStats {
    pub redis_version: Option<String>,
    pub uptime_in_seconds: Option<u64>,
    pub connected_clients: Option<u64>,
    pub used_memory_human: Option<String>,
    pub total_commands_processed: Option<u64>,
    pub instantaneous_ops_per_sec: Option<u64>,
    pub keyspace_hits: Option<u64>,
    pub keyspace_misses: Option<u64>,
    pub evicted_keys: Option<u64>,
    pub expired_keys: Option<u64>,
}

impl SystemStats {
    pub fn from_info_sections(info: &str) -> Self {
        Self {
            redis_version: get_val(info, "Server", "redis_version"),
            uptime_in_seconds: get_val(info, "Server", "uptime_in_seconds")
                .and_then(|s| s.parse().ok()),
            connected_clients: get_val(info, "Clients", "connected_clients")
                .and_then(|s| s.parse().ok()),
            used_memory_human: get_val(info, "Memory", "used_memory_human"),
            total_commands_processed: get_val(info, "Stats", "total_commands_processed")
                .and_then(|s| s.parse().ok()),
            instantaneous_ops_per_sec: get_val(info, "Stats", "instantaneous_ops_per_sec")
                .and_then(|s| s.parse().ok()),
            keyspace_hits: get_val(info, "Stats", "keyspace_hits").and_then(|s| s.parse().ok()),
            keyspace_misses: get_val(info, "Stats", "keyspace_misses").and_then(|s| s.parse().ok()),
            evicted_keys: get_val(info, "Stats", "evicted_keys").and_then(|s| s.parse().ok()),
            expired_keys: get_val(info, "Stats", "expired_keys").and_then(|s| s.parse().ok()),
        }
    }
}

fn get_val(info: &str, section: &str, key: &str) -> Option<String> {
    let pattern = format!("# {}", section);
    let chunk = info.split("\r\n\r\n").find(|c| c.starts_with(&pattern))?;
    chunk.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        if k == key { Some(v.to_string()) } else { None }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoAction {
    ScrollUp,
    ScrollDown,
}

pub struct InfoDashboard {
    pub stats: SystemStats,
    pub slowlog: Vec<crate::redis::server::SlowLogEntry>,
    pub slowlog_scroll: u16,
    pub compact_mode: bool,
}

impl Default for InfoDashboard {
    fn default() -> Self {
        Self::new()
    }
}

impl InfoDashboard {
    pub fn new() -> Self {
        Self {
            stats: SystemStats::default(),
            slowlog: Vec::new(),
            slowlog_scroll: 0,
            compact_mode: false,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InfoAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.slowlog_scroll = self.slowlog_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.slowlog.len().saturating_sub(1) as u16;
                    if self.slowlog_scroll < max {
                        self.slowlog_scroll += 1;
                    }
                }
                KeyCode::Char(' ') => {
                    self.compact_mode = !self.compact_mode;
                }
                _ => {}
            }
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(11), Constraint::Min(0)])
            .split(area);
        self.render_stats_panel(frame, chunks[0]);
        self.render_slowlog_panel(frame, chunks[1]);
    }

    fn format_uptime(seconds: u64) -> String {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        let mins = (seconds % 3600) / 60;
        if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }

    fn render_stats_panel(&self, frame: &mut Frame, area: Rect) {
        let hit_ratio = match (self.stats.keyspace_hits, self.stats.keyspace_misses) {
            (Some(h), Some(m)) if h + m > 0 => {
                format!("{:.1}%", (h as f64 / (h + m) as f64) * 100.0)
            }
            _ => "N/A".to_string(),
        };

        let uptime = self
            .stats
            .uptime_in_seconds
            .map_or("N/A".to_string(), Self::format_uptime);

        let lines = vec![
            Line::from(Span::styled(
                format!(
                    "  Redis {}  |  Uptime: {}",
                    self.stats.redis_version.as_deref().unwrap_or("N/A"),
                    uptime,
                ),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "  Clients: {}  |  Memory: {}",
                    self.stats
                        .connected_clients
                        .map_or("N/A".to_string(), |v| v.to_string()),
                    self.stats.used_memory_human.as_deref().unwrap_or("N/A"),
                ),
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::styled(
                format!(
                    "  Commands: {}  |  Ops/sec: {}",
                    self.stats
                        .total_commands_processed
                        .map_or("N/A".to_string(), |v| v.to_string()),
                    self.stats
                        .instantaneous_ops_per_sec
                        .map_or("N/A".to_string(), |v| v.to_string()),
                ),
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::styled(
                format!(
                    "  Hits: {}  |  Misses: {}  |  Hit Ratio: {}",
                    self.stats
                        .keyspace_hits
                        .map_or("N/A".to_string(), |v| v.to_string()),
                    self.stats
                        .keyspace_misses
                        .map_or("N/A".to_string(), |v| v.to_string()),
                    hit_ratio,
                ),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                format!(
                    "  Evicted: {}  |  Expired: {}",
                    self.stats
                        .evicted_keys
                        .map_or("N/A".to_string(), |v| v.to_string()),
                    self.stats
                        .expired_keys
                        .map_or("N/A".to_string(), |v| v.to_string()),
                ),
                Style::default().fg(Color::Yellow),
            )),
        ];

        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Server Stats "))
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }

    fn render_slowlog_panel(&self, frame: &mut Frame, area: Rect) {
        let title = if self.compact_mode {
            " Slow Log (compact) "
        } else {
            " Slow Log "
        };

        if self.slowlog.is_empty() {
            let para = Paragraph::new("No slow log entries")
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(para, area);
            return;
        }

        let max_visible = area.height.saturating_sub(2) as usize;
        let start = self.slowlog_scroll as usize;

        let lines: Vec<Line> = self
            .slowlog
            .iter()
            .skip(start)
            .take(max_visible)
            .map(|entry| {
                let cmd_str = entry.command.join(" ");
                if self.compact_mode {
                    Line::from(Span::raw(format!(
                        "  #{} {}us {}",
                        entry.id, entry.duration_us, cmd_str,
                    )))
                } else {
                    let dt = format_unix_timestamp(entry.timestamp);
                    Line::from(Span::raw(format!(
                        "  #{} {} {}us: {}",
                        entry.id, dt, entry.duration_us, cmd_str,
                    )))
                }
            })
            .collect();

        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(para, area);
    }
}

fn format_unix_timestamp(ts: u64) -> String {
    let since_epoch = std::time::Duration::from_secs(ts);
    let total_secs = since_epoch.as_secs();
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, seconds)
}
