use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use crate::events::Event;
use crate::ui::slowlog_panel::{SlowLogAction, SlowLogPanel};
use crate::ui::stats::sparkline::SparklineBuffer;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemStats {
    pub redis_version: Option<String>,
    pub uptime_in_seconds: Option<u64>,
    pub connected_clients: Option<u64>,
    pub used_memory: Option<u64>,
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
            used_memory: get_val(info, "Memory", "used_memory").and_then(|s| s.parse().ok()),
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
    ExportCsv,
}

pub struct InfoDashboard {
    pub stats: SystemStats,
    pub prev_stats: Option<SystemStats>,
    pub slowlog_panel: SlowLogPanel,
    pub buf_memory: SparklineBuffer,
    pub buf_clients: SparklineBuffer,
    pub buf_ops: SparklineBuffer,
    pub buf_hits: SparklineBuffer,
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
            prev_stats: None,
            slowlog_panel: SlowLogPanel::new(),
            buf_memory: SparklineBuffer::default(),
            buf_clients: SparklineBuffer::default(),
            buf_ops: SparklineBuffer::default(),
            buf_hits: SparklineBuffer::default(),
        }
    }

    /// Update stats, compute deltas against previous snapshot, and feed sparkline buffers.
    pub fn update_stats(&mut self, new_stats: SystemStats) {
        let prev = self.prev_stats.take();
        if let Some(ref p) = prev {
            if let (Some(new), Some(_old)) = (new_stats.used_memory, p.used_memory) {
                self.buf_memory.push(new as f64);
            }
            if let (Some(new), Some(_old)) = (new_stats.connected_clients, p.connected_clients) {
                self.buf_clients.push(new as f64);
            }
            if let (Some(new), Some(_old)) = (
                new_stats.instantaneous_ops_per_sec,
                p.instantaneous_ops_per_sec,
            ) {
                self.buf_ops.push(new as f64);
            }
            if let (Some(new_hits), Some(old_hits)) = (new_stats.keyspace_hits, p.keyspace_hits) {
                let new_misses = new_stats.keyspace_misses.unwrap_or(0);
                let old_misses = p.keyspace_misses.unwrap_or(0);
                let total_old = old_hits.saturating_add(old_misses);
                let total_new = new_hits.saturating_add(new_misses);
                let delta = total_new.saturating_sub(total_old) as f64;
                self.buf_hits.push(delta);
            }
        } else {
            // First update: just seed the buffers with current values
            if let Some(v) = new_stats.used_memory {
                self.buf_memory.push(v as f64);
            }
            if let Some(v) = new_stats.connected_clients {
                self.buf_clients.push(v as f64);
            }
            if let Some(v) = new_stats.instantaneous_ops_per_sec {
                self.buf_ops.push(v as f64);
            }
            if let (Some(hits), Some(misses)) = (new_stats.keyspace_hits, new_stats.keyspace_misses)
            {
                self.buf_hits.push((hits + misses) as f64);
            }
        }
        self.prev_stats = Some(new_stats.clone());
        self.stats = new_stats;
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<InfoAction> {
        if let Some(action) = self.slowlog_panel.handle_event(event) {
            return match action {
                SlowLogAction::ScrollUp => Some(InfoAction::ScrollUp),
                SlowLogAction::ScrollDown => Some(InfoAction::ScrollDown),
                SlowLogAction::ExportCsv => Some(InfoAction::ExportCsv),
                _ => None,
            };
        }
        None
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(13), Constraint::Min(0)])
            .split(area);
        self.render_stats_grid(frame, chunks[0]);
        self.slowlog_panel.render(frame, chunks[1]);
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

    fn delta_label(&self, buf: &SparklineBuffer) -> String {
        match buf.delta() {
            Some(d) if d >= 0.0 => format!(" (+{:.0})", d),
            Some(d) => format!(" ({:.0})", d),
            None => String::new(),
        }
    }

    fn render_stats_grid(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Length(6)])
            .split(area);
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3); 3])
            .split(rows[0]);
        let bot = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3); 3])
            .split(rows[1]);

        // Row 1: Memory, Clients, Ops/sec sparklines
        let mem_val = self.stats.used_memory_human.as_deref().unwrap_or("N/A");
        let mem_delta = self.delta_label(&self.buf_memory);
        self.render_sparkline_cell(
            frame,
            top[0],
            "Memory",
            &format!("{}{}", mem_val, mem_delta),
            &self.buf_memory,
            Color::Green,
        );

        let clients_val = self
            .stats
            .connected_clients
            .map_or("N/A".to_string(), |v| v.to_string());
        let clients_delta = self.delta_label(&self.buf_clients);
        self.render_sparkline_cell(
            frame,
            top[1],
            "Clients",
            &format!("{}{}", clients_val, clients_delta),
            &self.buf_clients,
            Color::Cyan,
        );

        let ops_val = self
            .stats
            .instantaneous_ops_per_sec
            .map_or("N/A".to_string(), |v| v.to_string());
        let ops_delta = self.delta_label(&self.buf_ops);
        self.render_sparkline_cell(
            frame,
            top[2],
            "Ops/sec",
            &format!("{}{}", ops_val, ops_delta),
            &self.buf_ops,
            Color::Yellow,
        );

        // Row 2: Hit Rate, Uptime, Version text cells
        let hit_ratio = match (self.stats.keyspace_hits, self.stats.keyspace_misses) {
            (Some(h), Some(m)) if h + m > 0 => {
                format!("{:.1}%", (h as f64 / (h + m) as f64) * 100.0)
            }
            _ => "N/A".to_string(),
        };
        let hits_delta = self.delta_label(&self.buf_hits);
        self.render_sparkline_cell(
            frame,
            bot[0],
            "Hit Rate",
            &format!("{}{}", hit_ratio, hits_delta),
            &self.buf_hits,
            Color::Magenta,
        );

        let uptime = self
            .stats
            .uptime_in_seconds
            .map_or("N/A".to_string(), Self::format_uptime);
        self.render_text_cell(frame, bot[1], "Uptime", &uptime, Color::Blue);

        let version = self.stats.redis_version.as_deref().unwrap_or("N/A");
        self.render_text_cell(frame, bot[2], "Version", version, Color::White);
    }

    fn render_sparkline_cell(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        value: &str,
        buf: &SparklineBuffer,
        color: Color,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", title));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let data: Vec<u64> = buf.to_vec().iter().map(|&v| v as u64).collect();
        let sparkline = Sparkline::default()
            .data(&data)
            .style(Style::default().fg(color))
            .max(if data.is_empty() {
                1
            } else {
                *data.iter().max().unwrap_or(&1)
            });
        frame.render_widget(sparkline, inner);

        let val_line = Line::from(Span::styled(
            value,
            Style::default()
                .fg(color)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        let para = Paragraph::new(vec![val_line]);
        frame.render_widget(para, inner);
    }

    fn render_text_cell(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        value: &str,
        color: Color,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", title));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let val_line = Line::from(Span::styled(
            value,
            Style::default()
                .fg(color)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        let para = Paragraph::new(vec![val_line]);
        frame.render_widget(para, inner);
    }
}
