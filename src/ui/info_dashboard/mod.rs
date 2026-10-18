use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use crate::events::Event;
use crate::ui::slowlog_panel::{SlowLogAction, SlowLogPanel};
use crate::ui::stats::sparkline::SparklineBuffer;
use crate::ui::theme;

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

    // ---- Replication (4.6) ----
    /// `master` / `slave`, or `None` when the server has no replication role.
    pub role: Option<String>,
    pub connected_slaves: Option<u64>,
    pub master_repl_offset: Option<u64>,
    pub master_link_up: Option<bool>,

    // ---- CPU (4.7) ----
    pub used_cpu_sys: Option<f64>,
    pub used_cpu_user: Option<f64>,

    // ---- Persistence (4.8) ----
    pub rdb_changes_since_last_save: Option<u64>,
    pub rdb_last_save_time: Option<u64>,
    pub rdb_last_bgsave_ok: Option<bool>,
    pub aof_enabled: Option<bool>,
    pub aof_rewrite_in_progress: Option<bool>,
    pub loading: Option<bool>,

    // ---- Keyspace (4.5) ----
    /// Raw `db0:keys=10,expires=2,avg_ttl=0` lines from the Keyspace section.
    pub keyspace: Vec<KeyspaceEntry>,
}

/// One `dbN` line from the INFO Keyspace section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyspaceEntry {
    pub db: String,
    pub keys: u64,
    pub expires: u64,
    pub avg_ttl: u64,
}

impl SystemStats {
    /// Total keys across every database, or `None` when Keyspace is absent.
    pub fn total_keys(&self) -> Option<u64> {
        if self.keyspace.is_empty() {
            return None;
        }
        Some(self.keyspace.iter().map(|e| e.keys).sum())
    }
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

            role: get_val(info, "Replication", "role"),
            connected_slaves: get_val(info, "Replication", "connected_slaves")
                .and_then(|s| s.parse().ok()),
            master_repl_offset: get_val(info, "Replication", "master_repl_offset")
                .and_then(|s| s.parse().ok()),
            master_link_up: get_val(info, "Replication", "master_link_status")
                .map(|s| s.eq_ignore_ascii_case("up")),

            used_cpu_sys: get_val(info, "CPU", "used_cpu_sys").and_then(|s| s.parse().ok()),
            used_cpu_user: get_val(info, "CPU", "used_cpu_user").and_then(|s| s.parse().ok()),

            rdb_changes_since_last_save: get_val(
                info,
                "Persistence",
                "rdb_changes_since_last_save",
            )
            .and_then(|s| s.parse().ok()),
            rdb_last_save_time: get_val(info, "Persistence", "rdb_last_save_time")
                .and_then(|s| s.parse().ok()),
            rdb_last_bgsave_ok: get_val(info, "Persistence", "rdb_last_bgsave_status")
                .map(|s| s.eq_ignore_ascii_case("ok")),
            aof_enabled: get_val(info, "Persistence", "aof_enabled").map(|s| s == "1"),
            aof_rewrite_in_progress: get_val(info, "Persistence", "aof_rewrite_in_progress")
                .map(|s| s == "1"),
            loading: get_val(info, "Persistence", "loading").map(|s| s == "1"),

            keyspace: parse_keyspace(info),
        }
    }
}

/// Parse the `Keyspace` section into per-database entries.
///
/// The section looks like:
/// ```text
/// # Keyspace
/// db0:keys=42,expires=3,avg_ttl=0
/// ```
fn parse_keyspace(info: &str) -> Vec<KeyspaceEntry> {
    let mut out = Vec::new();
    for line in info.lines() {
        let line = line.trim();
        // db0:keys=42,...
        let Some((db, rest)) = line.split_once(':') else {
            continue;
        };
        if !db.starts_with("db") {
            continue;
        }
        let mut keys = 0u64;
        let mut expires = 0u64;
        let mut avg_ttl = 0u64;
        let mut saw_any = false;
        for field in rest.split(',') {
            let Some((k, v)) = field.split_once('=') else {
                continue;
            };
            let parsed: u64 = v.parse().unwrap_or(0);
            match k.trim() {
                "keys" => {
                    keys = parsed;
                    saw_any = true;
                }
                "expires" => {
                    expires = parsed;
                    saw_any = true;
                }
                "avg_ttl" => {
                    avg_ttl = parsed;
                    saw_any = true;
                }
                _ => {}
            }
        }
        if saw_any {
            out.push(KeyspaceEntry {
                db: db.to_string(),
                keys,
                expires,
                avg_ttl,
            });
        }
    }
    out
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
    /// Clear the server's slow log (SLOWLOG RESET).
    ResetSlowLog,
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
                SlowLogAction::Reset => Some(InfoAction::ResetSlowLog),
                SlowLogAction::ToggleCompact | SlowLogAction::CycleThreshold => None,
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
            theme::current().success,
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
            theme::current().title,
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
            theme::current().warning,
        );

        // Row 2: Hit Rate sparkline, Keyspace bars, Replication + CPU + Persistence
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
            theme::current().type_hash,
        );

        self.render_keyspace_cell(frame, bot[1]);
        self.render_replication_cell(frame, bot[2]);
    }

    /// Keyspace breakdown as proportional bars, one row per database.
    fn render_keyspace_cell(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(" Keyspace ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let Some(max) = self.stats.keyspace.iter().map(|e| e.keys).max() else {
            frame.render_widget(Paragraph::new("no databases"), inner);
            return;
        };

        // Reserve one column of padding inside the border.
        let bar_width = (inner.width as usize).saturating_sub(1).max(1);
        let mut lines: Vec<Line> = Vec::new();
        for entry in &self.stats.keyspace {
            let filled = if max == 0 {
                0
            } else {
                ((entry.keys as f64 / max as f64) * bar_width as f64).round() as usize
            };
            let filled = filled.min(bar_width);
            let bar = format!("{}{}", "█".repeat(filled), "·".repeat(bar_width - filled));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", entry.db),
                    Style::default().fg(theme::current().border),
                ),
                Span::styled(bar, Style::default().fg(theme::current().type_zset)),
                Span::styled(
                    format!(" {}", entry.keys),
                    Style::default().fg(theme::current().fg),
                ),
            ]));
        }
        let total = self.stats.total_keys().unwrap_or(0);
        lines.push(Line::from(Span::styled(
            format!("total {total} keys"),
            Style::default().fg(theme::current().border),
        )));

        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// Replication role/offset plus CPU gauges and persistence flags.
    fn render_replication_cell(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_style())
            .title_style(theme::title_style())
            .title(" Replication ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();

        // Identity + uptime, which the previous layout showed as its own cells.
        let version = self.stats.redis_version.as_deref().unwrap_or("?");
        let uptime = self
            .stats
            .uptime_in_seconds
            .map_or("?".to_string(), Self::format_uptime);
        lines.push(Line::from(vec![
            Span::styled(
                format!("redis {version}"),
                Style::default().fg(theme::current().fg),
            ),
            Span::styled(
                format!("  up {uptime}"),
                Style::default().fg(theme::current().border),
            ),
        ]));

        let role = self.stats.role.as_deref().unwrap_or("unknown");
        let role_color = match role {
            "master" => theme::current().warning,
            "slave" => theme::current().title,
            _ => theme::current().border,
        };
        let mut repl = vec![Span::styled(
            "role ",
            Style::default().fg(theme::current().border),
        )];
        repl.push(Span::styled(
            role.to_string(),
            Style::default().fg(role_color).add_modifier(Modifier::BOLD),
        ));
        if let Some(slaves) = self.stats.connected_slaves {
            repl.push(Span::styled(
                format!("  replicas {slaves}"),
                Style::default().fg(theme::current().fg),
            ));
        }
        if let Some(up) = self.stats.master_link_up {
            repl.push(Span::styled(
                format!("  link {}", if up { "up" } else { "down" }),
                Style::default().fg(if up {
                    theme::current().success
                } else {
                    theme::current().error
                }),
            ));
        }
        lines.push(Line::from(repl));

        // The cell is only four rows tall, so CPU and the remaining metadata
        // share lines rather than each claiming its own.
        let mut cpu_spans = vec![Span::styled(
            "cpu ",
            Style::default().fg(theme::current().border),
        )];
        let mut cpu_count = 0usize;
        for (label, value) in [
            ("sys", self.stats.used_cpu_sys),
            ("usr", self.stats.used_cpu_user),
        ] {
            let Some(pct) = value else { continue };
            if cpu_count > 0 {
                cpu_spans.push(Span::raw(" "));
            }
            cpu_spans.extend(Self::compact_gauge(label, pct).spans);
            cpu_count += 1;
        }
        lines.push(Line::from(cpu_spans));

        let mut flags = vec![
            Span::styled("rdb ", Style::default().fg(theme::current().border)),
            match self.stats.rdb_last_bgsave_ok {
                Some(true) => Span::styled("ok", Style::default().fg(theme::current().success)),
                Some(false) => Span::styled("err", Style::default().fg(theme::current().error)),
                None => Span::styled("-", Style::default().fg(theme::current().border)),
            },
            Span::styled("  aof ", Style::default().fg(theme::current().border)),
            match self.stats.aof_enabled {
                Some(true) => Span::styled("on", Style::default().fg(theme::current().success)),
                Some(false) => Span::styled("off", Style::default().fg(theme::current().border)),
                None => Span::styled("-", Style::default().fg(theme::current().border)),
            },
        ];
        if self.stats.aof_rewrite_in_progress == Some(true) {
            flags.push(Span::styled(
                " rewriting",
                Style::default().fg(theme::current().warning),
            ));
        }
        if let Some(changes) = self.stats.rdb_changes_since_last_save {
            flags.push(Span::styled(
                format!(" +{changes}"),
                Style::default().fg(theme::current().border),
            ));
        }
        if let Some(offset) = self.stats.master_repl_offset {
            flags.push(Span::styled(
                format!(" off {offset}"),
                Style::default().fg(theme::current().border),
            ));
        }
        lines.push(Line::from(flags));

        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// A short `label[####··]12%` gauge sized to sit two-per-line.
    fn compact_gauge(label: &str, percent: f64) -> Line<'static> {
        const WIDTH: usize = 6;
        let clamped = percent.clamp(0.0, 100.0);
        let filled = (((clamped / 100.0) * WIDTH as f64).round() as usize).min(WIDTH);
        let color = if clamped >= 90.0 {
            theme::current().ttl_crit
        } else if clamped >= 70.0 {
            theme::current().ttl_warn
        } else {
            theme::current().ttl_ok
        };
        Line::from(vec![
            Span::styled(
                format!("{label} "),
                Style::default().fg(theme::current().border),
            ),
            Span::styled(
                format!("{}{}", "█".repeat(filled), "·".repeat(WIDTH - filled)),
                Style::default().fg(color),
            ),
            Span::styled(
                format!("{clamped:.0}%"),
                Style::default().fg(theme::current().fg),
            ),
        ])
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
            .border_style(theme::border_style())
            .title_style(theme::title_style())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// INFO output covering every section the dashboard reads.
    const FULL_INFO: &str = concat!(
        "# Server\r\nredis_version:7.2.4\r\nuptime_in_seconds:93784\r\n\r\n",
        "# Clients\r\nconnected_clients:12\r\n\r\n",
        "# Memory\r\nused_memory:1048576\r\nused_memory_human:1.00M\r\n\r\n",
        "# Stats\r\ntotal_commands_processed:123456\r\ninstantaneous_ops_per_sec:42\r\n",
        "keyspace_hits:900\r\nkeyspace_misses:100\r\nevicted_keys:3\r\nexpired_keys:7\r\n\r\n",
        "# Replication\r\nrole:master\r\nconnected_slaves:2\r\nmaster_repl_offset:998877\r\n",
        "master_link_status:up\r\n\r\n",
        "# CPU\r\nused_cpu_sys:12.5\r\nused_cpu_user:30.25\r\n\r\n",
        "# Persistence\r\nloading:0\r\nrdb_changes_since_last_save:42\r\n",
        "rdb_last_save_time:1600000000\r\nrdb_last_bgsave_status:ok\r\naof_enabled:1\r\n",
        "aof_rewrite_in_progress:0\r\n\r\n",
        "# Keyspace\r\ndb0:keys=1000,expires=10,avg_ttl=0\r\n",
        "db1:keys=250,expires=0,avg_ttl=0\r\ndb2:keys=4,expires=0,avg_ttl=0\r\n",
    );

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    // ---- INFO parsing (4.16, 4.17) ----

    #[test]
    fn parse_info_valid_reads_every_section() {
        let s = SystemStats::from_info_sections(FULL_INFO);
        assert_eq!(s.redis_version.as_deref(), Some("7.2.4"));
        assert_eq!(s.uptime_in_seconds, Some(93784));
        assert_eq!(s.connected_clients, Some(12));
        assert_eq!(s.used_memory, Some(1_048_576));
        assert_eq!(s.instantaneous_ops_per_sec, Some(42));
        assert_eq!(s.keyspace_hits, Some(900));
        assert_eq!(s.keyspace_misses, Some(100));
    }

    #[test]
    fn parse_info_replication_section() {
        let s = SystemStats::from_info_sections(FULL_INFO);
        assert_eq!(s.role.as_deref(), Some("master"));
        assert_eq!(s.connected_slaves, Some(2));
        assert_eq!(s.master_repl_offset, Some(998_877));
        assert_eq!(s.master_link_up, Some(true));
    }

    #[test]
    fn parse_info_cpu_section() {
        let s = SystemStats::from_info_sections(FULL_INFO);
        assert_eq!(s.used_cpu_sys, Some(12.5));
        assert_eq!(s.used_cpu_user, Some(30.25));
    }

    #[test]
    fn parse_info_persistence_section() {
        let s = SystemStats::from_info_sections(FULL_INFO);
        assert_eq!(s.loading, Some(false));
        assert_eq!(s.rdb_changes_since_last_save, Some(42));
        assert_eq!(s.rdb_last_save_time, Some(1_600_000_000));
        assert_eq!(s.rdb_last_bgsave_ok, Some(true));
        assert_eq!(s.aof_enabled, Some(true));
        assert_eq!(s.aof_rewrite_in_progress, Some(false));
    }

    #[test]
    fn parse_info_keyspace_section() {
        let s = SystemStats::from_info_sections(FULL_INFO);
        assert_eq!(s.keyspace.len(), 3);
        assert_eq!(
            s.keyspace[0],
            KeyspaceEntry {
                db: "db0".to_string(),
                keys: 1000,
                expires: 10,
                avg_ttl: 0,
            }
        );
        assert_eq!(s.total_keys(), Some(1254));
    }

    #[test]
    fn parse_info_missing_field_yields_none() {
        // Section present but the specific field absent.
        let info = "# Memory\r\nused_memory_human:1M\r\n";
        let s = SystemStats::from_info_sections(info);
        assert_eq!(s.used_memory_human.as_deref(), Some("1M"));
        assert_eq!(s.used_memory, None);
        assert_eq!(s.redis_version, None);
        assert_eq!(s.role, None);
    }

    #[test]
    fn parse_info_missing_section_yields_defaults() {
        let s = SystemStats::from_info_sections("# Server\r\nredis_version:7.0.0\r\n");
        assert!(s.keyspace.is_empty());
        assert_eq!(s.total_keys(), None);
        assert_eq!(s.connected_clients, None);
    }

    #[test]
    fn parse_info_empty_input_is_all_none() {
        assert_eq!(SystemStats::from_info_sections(""), SystemStats::default());
    }

    #[test]
    fn parse_keyspace_ignores_non_db_lines() {
        let info = "# Keyspace\r\nnotakeys=1\r\ndb0:keys=5,expires=0,avg_ttl=0\r\n";
        let entries = parse_keyspace(info);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].db, "db0");
        assert_eq!(entries[0].keys, 5);
    }

    #[test]
    fn parse_keyspace_tolerates_partial_fields() {
        let entries = parse_keyspace("# Keyspace\r\ndb0:keys=7\r\n");
        assert_eq!(entries[0].keys, 7);
        assert_eq!(entries[0].expires, 0);
    }

    #[test]
    fn parse_keyspace_skips_entries_with_no_known_fields() {
        assert!(parse_keyspace("# Keyspace\r\ndb0:subexpiry=1\r\n").is_empty());
    }

    #[test]
    fn total_keys_sums_all_databases() {
        let s = SystemStats {
            keyspace: vec![
                KeyspaceEntry {
                    db: "db0".into(),
                    keys: 10,
                    expires: 0,
                    avg_ttl: 0,
                },
                KeyspaceEntry {
                    db: "db1".into(),
                    keys: 5,
                    expires: 0,
                    avg_ttl: 0,
                },
            ],
            ..Default::default()
        };
        assert_eq!(s.total_keys(), Some(15));
    }

    // ---- Rendering (4.5, 4.6, 4.7, 4.8, 4.21) ----

    fn bar_color(pct: f64) -> Option<Color> {
        InfoDashboard::compact_gauge("cpu", pct)
            .spans
            .iter()
            .find(|s| s.content.contains('█') || s.content.contains('·'))
            .and_then(|s| s.style.fg)
    }

    #[test]
    fn compact_gauge_clamps_over_100() {
        let text: String = InfoDashboard::compact_gauge("cpu", 150.0)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("100%"), "got: {text}");
    }

    #[test]
    fn gauge_line_colors_escalate_with_load() {
        let t = theme::current();
        assert_eq!(bar_color(10.0), Some(t.ttl_ok));
        assert_eq!(bar_color(75.0), Some(t.ttl_warn));
        assert_eq!(bar_color(95.0), Some(t.ttl_crit));
    }

    fn rendered(stats: &SystemStats, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut d = InfoDashboard::new();
        d.update_stats(stats.clone());
        terminal.draw(|f| d.render(f, f.area())).unwrap();
        buffer_text(&terminal)
    }

    #[test]
    fn render_dashboard_shows_keyspace_bars() {
        let text = rendered(&SystemStats::from_info_sections(FULL_INFO), 120, 40);
        assert!(text.contains("db0"), "keyspace rows missing:\n{text}");
        assert!(text.contains("1254"), "total missing:\n{text}");
    }

    #[test]
    fn render_dashboard_shows_replication_and_cpu() {
        let text = rendered(&SystemStats::from_info_sections(FULL_INFO), 120, 40);
        assert!(text.contains("master"), "role missing:\n{text}");
        assert!(text.contains("replicas 2"));
        assert!(text.contains("link up"));
        assert!(text.contains("usr"), "cpu user gauge missing:\n{text}");
        assert!(text.contains("30%"), "cpu user percentage missing:\n{text}");
        assert!(text.contains("12%"), "cpu sys percentage missing:\n{text}");
        assert!(text.contains("aof on"));
        assert!(text.contains("rdb ok"));
    }

    #[test]
    fn render_dashboard_shows_version_and_uptime() {
        let text = rendered(&SystemStats::from_info_sections(FULL_INFO), 120, 40);
        assert!(text.contains("7.2.4"), "version missing:\n{text}");
        assert!(text.contains("1d 2h"), "uptime missing:\n{text}");
    }

    #[test]
    fn render_dashboard_with_no_keyspace_says_so() {
        let text = rendered(&SystemStats::default(), 120, 40);
        assert!(text.contains("no databases"), "got:\n{text}");
    }

    #[test]
    fn render_dashboard_with_partial_stats_does_not_panic() {
        let stats = SystemStats::from_info_sections(
            "# Replication\r\nrole:slave\r\nconnected_slaves:0\r\n",
        );
        let text = rendered(&stats, 120, 40);
        assert!(text.contains("slave"), "got:\n{text}");
    }

    #[test]
    fn render_dashboard_at_extreme_sizes_does_not_panic() {
        let stats = SystemStats::from_info_sections(FULL_INFO);
        for (w, h) in [(40u16, 12u16), (20, 8), (200, 60)] {
            rendered(&stats, w, h);
        }
    }

    #[test]
    fn render_dashboard_keyspace_with_single_zero_key_db() {
        // A zero-key database must not divide by zero when scaling the bar.
        let stats =
            SystemStats::from_info_sections("# Keyspace\r\ndb0:keys=0,expires=0,avg_ttl=0\r\n");
        let text = rendered(&stats, 120, 40);
        assert!(text.contains("db0"), "got:\n{text}");
    }
}
