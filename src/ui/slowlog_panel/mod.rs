use std::io::Write;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::events::Event;
use crate::redis::server::SlowLogEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlowLogAction {
    ScrollUp,
    ScrollDown,
    ToggleCompact,
    CycleThreshold,
    ExportCsv,
    /// Clear the server's slow log.
    Reset,
}

pub struct SlowLogPanel {
    pub entries: Vec<SlowLogEntry>,
    pub scroll: u16,
    pub compact_mode: bool,
    pub threshold_us: Option<u64>,
    pub last_export_path: Option<String>,
    /// Server-reported `SLOWLOG LEN`, shown in the header when known.
    pub server_len: Option<usize>,
}

impl Default for SlowLogPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SlowLogPanel {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: 0,
            compact_mode: false,
            threshold_us: None,
            last_export_path: None,
            server_len: None,
        }
    }

    pub fn set_entries(&mut self, entries: Vec<SlowLogEntry>) {
        self.entries = entries;
        self.clamp_scroll();
    }

    fn filtered_entries(&self) -> Vec<&SlowLogEntry> {
        self.entries
            .iter()
            .filter(|e| self.threshold_us.is_none_or(|t| e.duration_us >= t))
            .collect()
    }

    fn clamp_scroll(&mut self) {
        let max = self.filtered_entries().len().saturating_sub(1) as u16;
        if self.scroll > max {
            self.scroll = max;
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<SlowLogAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll = self.scroll.saturating_sub(1);
                    return Some(SlowLogAction::ScrollUp);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.filtered_entries().len().saturating_sub(1) as u16;
                    if self.scroll < max {
                        self.scroll += 1;
                        return Some(SlowLogAction::ScrollDown);
                    }
                }
                KeyCode::Char(' ') => {
                    self.compact_mode = !self.compact_mode;
                    return Some(SlowLogAction::ToggleCompact);
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.threshold_us = match self.threshold_us {
                        None => Some(1_000),
                        Some(1_000) => Some(10_000),
                        Some(10_000) => Some(100_000),
                        Some(100_000) => Some(1_000_000),
                        Some(_) => None,
                    };
                    self.scroll = 0;
                    return Some(SlowLogAction::CycleThreshold);
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    return Some(SlowLogAction::ExportCsv);
                }
                KeyCode::Char('R') => {
                    return Some(SlowLogAction::Reset);
                }
                _ => {}
            }
        }
        None
    }

    pub fn export_csv(&mut self) -> Result<String, std::io::Error> {
        let filename = format!(
            "slowlog_{}.csv",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
        );
        let mut file = std::fs::File::create(&filename)?;
        writeln!(file, "id,timestamp,duration_us,command,client,name")?;
        for entry in &self.entries {
            let cmd = entry.command.join(" ");
            let client = entry.client.as_deref().unwrap_or("");
            let name = entry.name.as_deref().unwrap_or("");
            writeln!(
                file,
                "{},{},{},\"{}\",\"{}\",\"{}\"",
                entry.id, entry.timestamp, entry.duration_us, cmd, client, name
            )?;
        }
        self.last_export_path = Some(filename.clone());
        Ok(filename)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let filtered = self.filtered_entries();
        let mut title = if let Some(th) = self.threshold_us {
            if self.compact_mode {
                format!(" Slow Log (compact, >= {}µs) ", th)
            } else {
                format!(" Slow Log (>= {}µs) ", th)
            }
        } else if self.compact_mode {
            " Slow Log (compact) ".to_string()
        } else {
            " Slow Log ".to_string()
        };
        // SLOWLOG LEN is the server's count, which can exceed what is loaded.
        if let Some(len) = self.server_len {
            title = title.trim_end().to_string();
            title.push_str(&format!(" [{len} on server] "));
        }

        if filtered.is_empty() {
            let para = ratatui::widgets::Paragraph::new("No slow log entries")
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(para, area);
            return;
        }

        let max_dur = filtered
            .iter()
            .map(|e| e.duration_us)
            .max()
            .unwrap_or(1)
            .max(1);
        let start = self.scroll as usize;
        let max_visible = area.height.saturating_sub(4) as usize; // header + borders

        if self.compact_mode {
            let lines: Vec<Line> = filtered
                .iter()
                .skip(start)
                .take(max_visible)
                .map(|entry| {
                    let cmd_str = entry.command.join(" ");
                    Line::from(Span::raw(format!(
                        "  #{} {}us {}",
                        entry.id, entry.duration_us, cmd_str,
                    )))
                })
                .collect();
            let para = ratatui::widgets::Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title));
            frame.render_widget(para, area);
        } else {
            let header = Row::new(vec!["ID", "Time", "Duration", "Command"])
                .style(Style::default().add_modifier(Modifier::BOLD))
                .height(1);

            let rows: Vec<Row> = filtered
                .iter()
                .skip(start)
                .take(max_visible)
                .map(|entry| {
                    let bar = Self::duration_bar(entry.duration_us, max_dur, 12);
                    let dur_text = format!("{} µs", entry.duration_us);
                    let dur_cell = Cell::from(Line::from(vec![
                        Span::styled(dur_text, Self::duration_style(entry.duration_us)),
                        Span::raw(" "),
                        bar,
                    ]));

                    Row::new(vec![
                        Cell::from(entry.id.to_string()),
                        Cell::from(format_unix_timestamp(entry.timestamp)),
                        dur_cell,
                        Cell::from(entry.command.join(" ")),
                    ])
                    .height(1)
                })
                .collect();

            let table = Table::new(
                rows,
                &[
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Length(24),
                    Constraint::Min(20),
                ],
            )
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title));

            frame.render_widget(table, area);
        }
    }

    fn duration_style(duration_us: u64) -> Style {
        let color = if duration_us < 1_000 {
            Color::Green
        } else if duration_us < 10_000 {
            Color::Yellow
        } else {
            Color::Red
        };
        Style::default().fg(color)
    }

    fn duration_bar(duration_us: u64, max_us: u64, width: usize) -> Span<'static> {
        let ratio = (duration_us as f64 / max_us as f64).min(1.0);
        let filled = (ratio * width as f64).round() as usize;
        let empty = width.saturating_sub(filled);
        let bar = "█".repeat(filled) + &"░".repeat(empty);
        let color = if duration_us < 1_000 {
            Color::Green
        } else if duration_us < 10_000 {
            Color::Yellow
        } else {
            Color::Red
        };
        Span::styled(bar, Style::default().fg(color))
    }
}

fn format_unix_timestamp(ts: u64) -> String {
    let hours = (ts % 86400) / 3600;
    let mins = (ts % 3600) / 60;
    let secs = ts % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};

    fn sample_entry(id: u64, duration_us: u64) -> SlowLogEntry {
        SlowLogEntry {
            id,
            timestamp: 1000 + id,
            duration_us,
            command: vec!["GET".to_string(), format!("key{id}")],
            client: None,
            name: None,
        }
    }

    #[test]
    fn test_slowlog_panel_new() {
        let panel = SlowLogPanel::new();
        assert!(panel.entries.is_empty());
        assert_eq!(panel.scroll, 0);
        assert!(!panel.compact_mode);
        assert_eq!(panel.threshold_us, None);
    }

    #[test]
    fn test_slowlog_panel_set_entries() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(1, 100), sample_entry(2, 200)]);
        assert_eq!(panel.entries.len(), 2);
    }

    #[test]
    fn test_slowlog_panel_scroll_down() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries((0..10).map(|i| sample_entry(i, 100)).collect());
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
        assert_eq!(panel.scroll, 1);
    }

    #[test]
    fn test_slowlog_panel_scroll_up() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries((0..10).map(|i| sample_entry(i, 100)).collect());
        panel.scroll = 3;
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Up)));
        assert_eq!(panel.scroll, 2);
    }

    #[test]
    fn test_slowlog_panel_scroll_does_not_overflow() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(0, 100)]);
        panel.scroll = 0;
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Down)));
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_slowlog_panel_compact_toggle() {
        let mut panel = SlowLogPanel::new();
        assert!(!panel.compact_mode);
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char(' '))));
        assert!(panel.compact_mode);
    }

    #[test]
    fn test_slowlog_panel_threshold_cycle() {
        let mut panel = SlowLogPanel::new();
        assert_eq!(panel.threshold_us, None);

        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.threshold_us, Some(1_000));

        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.threshold_us, Some(10_000));

        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.threshold_us, Some(100_000));

        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.threshold_us, Some(1_000_000));

        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.threshold_us, None);
    }

    #[test]
    fn test_slowlog_panel_threshold_filters_entries() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![
            sample_entry(1, 500),
            sample_entry(2, 1_500),
            sample_entry(3, 15_000),
        ]);
        panel.threshold_us = Some(1_000);
        let filtered = panel.filtered_entries();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 2);
        assert_eq!(filtered[1].id, 3);
    }

    #[test]
    fn test_slowlog_panel_export_csv() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(1, 100), sample_entry(2, 200)]);
        let path = panel.export_csv().unwrap();
        assert!(std::path::Path::new(&path).exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("id,timestamp,duration_us,command,client,name"));
        assert!(content.contains("1,"));
        assert!(content.contains("2,"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_slowlog_panel_render_empty_does_not_panic() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let panel = SlowLogPanel::new();
        let _ = terminal.draw(|f| panel.render(f, f.area()));
    }

    #[test]
    fn test_slowlog_panel_render_with_entries_does_not_panic() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut panel = SlowLogPanel::new();
        panel.set_entries((0..20).map(|i| sample_entry(i, 100 * (i + 1))).collect());
        let _ = terminal.draw(|f| panel.render(f, f.area()));
    }

    #[test]
    fn test_slowlog_reset_key_emits_action() {
        let mut panel = SlowLogPanel::new();
        let action = panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('R'))));
        assert_eq!(action, Some(SlowLogAction::Reset));
    }

    #[test]
    fn test_slowlog_reset_does_not_clobber_local_entries() {
        // The reset is a server operation; app.rs clears the local list only
        // after the server confirms.
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(1, 100)]);
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('R'))));
        assert_eq!(panel.entries.len(), 1);
    }

    #[test]
    fn test_slowlog_lower_threshold_letter_is_not_reset() {
        let mut panel = SlowLogPanel::new();
        // `r` is not bound; only capital `R` resets.
        assert_eq!(
            panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('r')))),
            None
        );
    }

    #[test]
    fn test_slowlog_header_shows_server_len() {
        let backend = ratatui::backend::TestBackend::new(60, 8);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(1, 100)]);
        panel.server_len = Some(42);
        let _ = terminal.draw(|f| panel.render(f, f.area()));

        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("42 on server"), "got: {text}");
    }

    #[test]
    fn test_slowlog_header_omits_len_when_unknown() {
        let backend = ratatui::backend::TestBackend::new(60, 8);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut panel = SlowLogPanel::new();
        panel.set_entries(vec![sample_entry(1, 100)]);
        let _ = terminal.draw(|f| panel.render(f, f.area()));

        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(!text.contains("on server"), "got: {text}");
    }

    #[test]
    fn test_slowlog_panel_render_compact_does_not_panic() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut panel = SlowLogPanel::new();
        panel.compact_mode = true;
        panel.set_entries((0..20).map(|i| sample_entry(i, 100 * (i + 1))).collect());
        let _ = terminal.draw(|f| panel.render(f, f.area()));
    }

    #[test]
    fn test_slowlog_panel_threshold_resets_scroll() {
        let mut panel = SlowLogPanel::new();
        panel.set_entries((0..20).map(|i| sample_entry(i, 100)).collect());
        panel.scroll = 10;
        panel.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('t'))));
        assert_eq!(panel.scroll, 0);
    }
}
