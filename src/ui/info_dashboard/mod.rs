use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
};

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

pub struct InfoDashboard {
    pub stats: SystemStats,
    pub slowlog: Vec<crate::redis::server::SlowLogEntry>,
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
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let stats_text = format!(
            "Redis: {}\nUptime: {}s\nClients: {}\nMemory: {}\nCmds: {}\nOps/sec: {}\nHits: {}\nMisses: {}\nEvicted: {}\nExpired: {}",
            self.stats.redis_version.as_deref().unwrap_or("N/A"),
            self.stats
                .uptime_in_seconds
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .connected_clients
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats.used_memory_human.as_deref().unwrap_or("N/A"),
            self.stats
                .total_commands_processed
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .instantaneous_ops_per_sec
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .keyspace_hits
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .keyspace_misses
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .evicted_keys
                .map_or("N/A".to_string(), |v| v.to_string()),
            self.stats
                .expired_keys
                .map_or("N/A".to_string(), |v| v.to_string()),
        );
        let stats_para = Paragraph::new(stats_text)
            .block(Block::default().borders(Borders::ALL).title("Server Stats"))
            .wrap(Wrap { trim: false });
        frame.render_widget(stats_para, chunks[0]);

        let mut slow_text = "Slow Log:\n".to_string();
        for entry in self.slowlog.iter().take(10) {
            slow_text.push_str(&format!(
                "#{} {}us {:?}\n",
                entry.id, entry.duration_us, entry.command
            ));
        }
        let slow_para = Paragraph::new(slow_text)
            .block(Block::default().borders(Borders::ALL).title("Slow Log"))
            .wrap(Wrap { trim: false });
        frame.render_widget(slow_para, chunks[1]);
    }
}
