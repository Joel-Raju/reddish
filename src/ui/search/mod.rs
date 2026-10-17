use std::collections::HashSet;

use nucleo::{Config, Matcher, Utf32String};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::events::Event;

/// Upper bound on retained matches. Protects render time and memory when a
/// scan runs against a large keyspace.
pub const MAX_MATCHES: usize = 10_000;

/// Upper bound on retained scan candidates, independent of whether they match.
const MAX_CANDIDATES: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    Execute(String),
    Close,
    QueryChanged,
    /// Query interpretation flipped between fuzzy and regex.
    ModeToggled,
}

/// How the query text is interpreted when matching key names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Subsequence match per query atom, ranked by `nucleo` score (default).
    Fuzzy,
    /// Query compiled as a regular expression.
    Regex,
}

impl SearchMode {
    pub fn label(&self) -> &'static str {
        match self {
            SearchMode::Fuzzy => "fuzzy",
            SearchMode::Regex => "regex",
        }
    }

    pub fn toggled(&self) -> Self {
        match self {
            SearchMode::Fuzzy => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Fuzzy,
        }
    }
}

/// Split a query into alphanumeric atoms on non-alphanumeric boundaries.
///
/// Atoms are the unit of both fuzzy matching and server-side glob narrowing.
/// Because the split predicate keeps only `char::is_alphanumeric` runs, Redis
/// glob metacharacters (`*`, `?`, `[`, `]`, `^`, `\`) are naturally consumed as
/// separators and can never widen the generated pattern.
pub fn query_atoms(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Build a Redis `MATCH` glob used to narrow candidates server-side.
///
/// SCAN is always used (never `KEYS`), so this only needs to be a superset of
/// the client-side filter rather than an exact match.
///
/// - Fuzzy: atoms joined by `*`, so `user 123` matches `user:session:123`.
/// - Regex: the longest alphanumeric run in the pattern, since a regex cannot
///   be meaningfully translated into a glob. Falls back to `*` when the pattern
///   contains no alphanumeric run.
pub fn glob_pattern_for_query(query: &str, mode: SearchMode) -> String {
    if mode == SearchMode::Regex {
        // `max_by_key` keeps the last maximal element; use an explicit scan so
        // ties resolve to the first (leftmost) run and the pattern is stable.
        let longest = query_atoms(query)
            .into_iter()
            .reduce(|best, atom| if atom.len() > best.len() { atom } else { best })
            .unwrap_or_default();
        return if longest.is_empty() {
            "*".to_string()
        } else {
            format!("*{longest}*")
        };
    }

    let atoms = query_atoms(query);
    if atoms.is_empty() {
        return "*".to_string();
    }
    format!("*{}*", atoms.join("*"))
}

pub struct GlobalSearch {
    pub query: String,
    pub results: Vec<String>,
    pub filtered: Vec<String>,
    pub cursor: usize,
    pub scanning: bool,
    pub scan_results: Vec<String>,
    pub mode: SearchMode,
    /// Compiled pattern for `SearchMode::Regex`. `None` in fuzzy mode, or when
    /// the query is not a valid regex.
    regex: Option<regex::Regex>,
    /// Parse error to surface when the query is an invalid regex.
    pub regex_error: Option<String>,
    matcher: Matcher,
    /// Dedupe set for streamed scan batches.
    seen: HashSet<String>,
    /// Match score per entry in `filtered`, index-aligned.
    scores: Vec<u32>,
    /// Lowercased atoms for the current query, rebuilt on query/mode change.
    atoms: Vec<String>,
}

impl Default for GlobalSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            scanning: false,
            scan_results: Vec::new(),
            mode: SearchMode::Fuzzy,
            regex: None,
            regex_error: None,
            matcher: Matcher::new(Config::DEFAULT),
            seen: HashSet::new(),
            scores: Vec::new(),
            atoms: Vec::new(),
        }
    }

    pub fn set_results(&mut self, results: Vec<String>) {
        self.results = results;
        self.seen = self.results.iter().cloned().collect();
        self.scanning = false;
        self.apply_filter();
    }

    /// Rebuild the cached query state (atoms + compiled regex).
    fn refresh_query_state(&mut self) {
        self.atoms = query_atoms(&self.query);
        self.regex_error = None;
        self.regex = match self.mode {
            SearchMode::Regex => match regex::Regex::new(&self.query) {
                Ok(re) => Some(re),
                Err(err) => {
                    self.regex_error = Some(first_line(&err.to_string()));
                    None
                }
            },
            SearchMode::Fuzzy => None,
        };
    }

    /// Score one key name against the current query. `None` means no match.
    ///
    /// An empty query matches everything with score `0` so that a bare SCAN
    /// populates the list. Fuzzy mode requires every atom to match as a
    /// subsequence and ranks by the summed `nucleo` score.
    fn score(&mut self, key: &str) -> Option<u32> {
        if self.query.trim().is_empty() {
            return Some(0);
        }
        match self.mode {
            SearchMode::Fuzzy => {
                if self.atoms.is_empty() {
                    return Some(0);
                }
                let lower = key.to_lowercase();
                let hay = Utf32String::from(lower.as_str());
                let mut total = 0u32;
                for atom in self.atoms.clone() {
                    let needle = Utf32String::from(atom.as_str());
                    total += u32::from(
                        self.matcher
                            .fuzzy_match_greedy(hay.slice(..), needle.slice(..))?,
                    );
                }
                Some(total)
            }
            SearchMode::Regex => self.regex.as_ref()?.is_match(key).then_some(1),
        }
    }

    /// Install a scored result set, sorting by descending score and trimming to
    /// `MAX_MATCHES`. Ties keep their existing relative order.
    fn set_ranked(&mut self, mut pairs: Vec<(u32, String)>) {
        pairs.sort_by(|a, b| b.0.cmp(&a.0));
        pairs.truncate(MAX_MATCHES);
        self.scores = pairs.iter().map(|(s, _)| *s).collect();
        self.filtered = pairs.into_iter().map(|(_, key)| key).collect();
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    /// Re-sort `filtered` by descending score without re-scoring every key.
    /// Ties preserve insertion (scan) order.
    fn rank_filtered(&mut self) {
        let mut pairs: Vec<(u32, String)> = self
            .scores
            .iter()
            .copied()
            .zip(self.filtered.iter().cloned())
            .collect();
        pairs.sort_by(|a, b| b.0.cmp(&a.0));
        pairs.truncate(MAX_MATCHES);
        self.scores = pairs.iter().map(|(s, _)| *s).collect();
        self.filtered = pairs.into_iter().map(|(_, key)| key).collect();
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn apply_filter(&mut self) {
        self.refresh_query_state();
        let results = std::mem::take(&mut self.results);
        let pairs: Vec<(u32, String)> = results
            .iter()
            .filter_map(|key| self.score(key).map(|s| (s, key.clone())))
            .collect();
        self.results = results;
        self.set_ranked(pairs);
    }

    /// Drain a batch of scan results into the ranked list.
    ///
    /// Only new keys are scored, so cost is proportional to the batch rather
    /// than to the total result set.
    pub fn drain_scan_batch(&mut self, batch: Vec<String>) {
        if batch.is_empty() {
            self.scanning = false;
            return;
        }
        let mut scored_any = false;
        for item in batch {
            if !self.seen.insert(item.clone()) {
                continue;
            }
            if self.results.len() >= MAX_CANDIDATES {
                break;
            }
            self.results.push(item.clone());
            if let Some(score) = self.score(&item) {
                self.filtered.push(item);
                self.scores.push(score);
                scored_any = true;
            }
        }
        if scored_any {
            self.rank_filtered();
        }
    }

    /// Prepare for a new scan: clear old results and mark scanning.
    pub fn start_scan(&mut self) {
        self.scanning = true;
        self.results.clear();
        self.filtered.clear();
        self.seen.clear();
        self.scores.clear();
        self.cursor = 0;
        self.refresh_query_state();
    }

    /// Switch between fuzzy and regex interpretation of the query.
    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggled();
        self.apply_filter();
    }

    /// The key currently under the cursor, if any.
    pub fn selected(&self) -> Option<&String> {
        self.filtered.get(self.cursor)
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<SearchAction> {
        self.handle_event_with_keymap(event, &crate::config::keybindings::Keymap::default())
    }

    pub fn handle_event_with_keymap(
        &mut self,
        event: &Event,
        keymap: &crate::config::keybindings::Keymap,
    ) -> Option<SearchAction> {
        use crossterm::event::KeyCode;
        if let Event::Key(key) = event {
            if keymap.matches("search_close", key) {
                return Some(SearchAction::Close);
            }
            if keymap.matches("search_toggle_mode", key) {
                self.toggle_mode();
                return Some(SearchAction::ModeToggled);
            }
            if keymap.matches("search_execute", key) {
                let target = self
                    .selected()
                    .cloned()
                    .unwrap_or_else(|| self.query.clone());
                return Some(SearchAction::Execute(target));
            }
            match key.code {
                KeyCode::Char(c) => {
                    self.query.push(c);
                    self.apply_filter();
                    Some(SearchAction::QueryChanged)
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.apply_filter();
                    Some(SearchAction::QueryChanged)
                }
                KeyCode::Down => {
                    if self.cursor + 1 < self.filtered.len() {
                        self.cursor += 1;
                    }
                    None
                }
                KeyCode::Up => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    None
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn title(&self) -> String {
        let mode = format!("[{}]", self.mode.label());
        if let Some(err) = &self.regex_error {
            return format!("Search {mode} — invalid regex: {err}");
        }
        if self.scanning {
            format!(
                "Search {mode} (Esc to close) [scanning… {} candidates]",
                self.results.len()
            )
        } else {
            format!("Search {mode} (Esc to close)")
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 50, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default().borders(Borders::ALL).title(self.title());
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let query_widget = Paragraph::new(vec![ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("> ", Style::default().fg(Color::Green)),
            ratatui::text::Span::styled(
                self.query.clone(),
                if self.regex_error.is_some() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ])]);
        frame.render_widget(query_widget, inner);

        let result_area = Rect {
            x: inner.x,
            y: inner.y.saturating_add(2),
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        if !self.filtered.is_empty() {
            let visible = result_area.height as usize;
            // Keep the cursor in view when the result set is taller than the popup.
            let scroll = self
                .cursor
                .saturating_sub(visible.saturating_sub(1))
                .min(self.filtered.len().saturating_sub(visible));
            let preview = self
                .filtered
                .iter()
                .enumerate()
                .skip(scroll)
                .take(visible)
                .map(|(idx, item)| {
                    let style = if idx == self.cursor {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        format!("  {item}"),
                        style,
                    ))
                })
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(preview), result_area);
        } else if !self.scanning && self.regex_error.is_none() {
            let no_results = Paragraph::new("No matches found")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(no_results, result_area);
        }
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_string()
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn ctrl(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    #[test]
    fn empty_query_matches_everything() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["a".into(), "b".into()]);
        assert_eq!(s.filtered.len(), 2);
    }

    #[test]
    fn fuzzy_subsequence_match_reorders_by_score() {
        let mut s = GlobalSearch::new();
        s.set_results(vec![
            "zzz_user_zzz".into(),
            "user:1".into(),
            "usr".into(),
        ]);
        s.query = "user".into();
        s.apply_filter();
        // Every atom ("user") must match as a subsequence.
        assert_eq!(s.filtered.len(), 2, "{:?}", s.filtered);
        // Tighter match ranks first.
        assert_eq!(s.filtered[0], "user:1");
    }

    #[test]
    fn fuzzy_superset_of_substring_match() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["aXb".into(), "ab".into()]);
        s.query = "ab".into();
        s.apply_filter();
        // "aXb" is a fuzzy (non-substring) hit and must be retained.
        assert!(s.filtered.contains(&"aXb".to_string()));
        assert!(s.filtered.contains(&"ab".to_string()));
    }

    #[test]
    fn fuzzy_multi_atom_requires_all_atoms() {
        let mut s = GlobalSearch::new();
        s.set_results(vec![
            "user:123".into(),
            "user:999".into(),
            "session:123".into(),
        ]);
        s.query = "user 123".into();
        s.apply_filter();
        assert_eq!(s.filtered, vec!["user:123".to_string()]);
    }

    #[test]
    fn regex_mode_matches_pattern() {
        let mut s = GlobalSearch::new();
        s.mode = SearchMode::Regex;
        s.set_results(vec![
            "user:1".into(),
            "user:22".into(),
            "session:1".into(),
        ]);
        s.query = "^user:[0-9]+$".into();
        s.apply_filter();
        assert_eq!(s.filtered.len(), 2, "{:?}", s.filtered);
    }

    #[test]
    fn regex_mode_invalid_pattern_yields_error_and_no_matches() {
        let mut s = GlobalSearch::new();
        s.mode = SearchMode::Regex;
        s.set_results(vec!["user:1".into()]);
        s.query = "[unclosed".into();
        s.apply_filter();
        assert!(s.regex_error.is_some());
        assert!(s.filtered.is_empty());
    }

    #[test]
    fn toggle_mode_flips_and_rescores() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["user:1".into()]);
        assert_eq!(s.mode, SearchMode::Fuzzy);

        // `usr` matches `user:1` as a subsequence, but not as a literal regex.
        s.query = "usr".into();
        s.apply_filter();
        assert_eq!(s.filtered.len(), 1, "fuzzy matches a subsequence");

        s.toggle_mode();
        assert_eq!(s.mode, SearchMode::Regex);
        assert_eq!(s.filtered.len(), 0, "regex requires a literal match");

        s.toggle_mode();
        assert_eq!(s.mode, SearchMode::Fuzzy);
        assert_eq!(s.filtered.len(), 1);
    }

    #[test]
    fn fuzzy_mode_ignores_regex_anchors() {
        // `^` is a non-alphanumeric separator, so it becomes an atom boundary
        // rather than a literal the query would have to match.
        let mut s = GlobalSearch::new();
        s.set_results(vec!["user:1".into(), "xuser:1".into()]);
        s.query = "^user".into();
        s.apply_filter();
        assert_eq!(s.filtered.len(), 2);
    }

    #[test]
    fn ctrl_r_emits_mode_toggled() {
        let mut s = GlobalSearch::new();
        let action = s.handle_event_with_keymap(&ctrl('r'), &crate::config::keybindings::Keymap::default());
        assert_eq!(action, Some(SearchAction::ModeToggled));
        assert_eq!(s.mode, SearchMode::Regex);
    }

    #[test]
    fn drain_scan_batch_scores_only_new_keys_and_ranks() {
        let mut s = GlobalSearch::new();
        s.query = "user".into();
        s.start_scan();
        assert!(s.scanning);

        s.drain_scan_batch(vec!["zz_user_zz".into(), "user:1".into()]);
        assert_eq!(s.filtered.len(), 2);
        assert_eq!(s.filtered[0], "user:1");
        assert!(s.scanning, "scan still in progress");

        // Empty batch signals completion.
        s.drain_scan_batch(vec![]);
        assert!(!s.scanning);
    }

    #[test]
    fn drain_scan_batch_dedupes_repeated_keys() {
        let mut s = GlobalSearch::new();
        s.start_scan();
        s.drain_scan_batch(vec!["a".into(), "a".into(), "a".into()]);
        assert_eq!(s.results.len(), 1);
        assert_eq!(s.filtered.len(), 1);
    }

    #[test]
    fn drain_scan_batch_excludes_non_matching_candidates() {
        let mut s = GlobalSearch::new();
        s.query = "user".into();
        s.start_scan();
        s.drain_scan_batch(vec!["user:1".into(), "session:1".into()]);
        assert_eq!(s.results.len(), 2);
        assert_eq!(s.filtered, vec!["user:1".to_string()]);
    }

    #[test]
    fn start_scan_resets_previous_results() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["a".into()]);
        s.start_scan();
        assert!(s.results.is_empty());
        assert!(s.filtered.is_empty());
        // Dedupe set must be cleared too, otherwise the key is never re-added.
        s.drain_scan_batch(vec!["a".into()]);
        assert_eq!(s.results, vec!["a".to_string()]);
    }

    #[test]
    fn cursor_clamped_when_result_set_shrinks() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["a".into(), "ab".into(), "abc".into()]);
        s.cursor = 2;
        s.query = "ab".into();
        s.apply_filter();
        assert!(s.cursor < s.filtered.len());
    }

    #[test]
    fn selected_returns_cursor_entry() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["a".into(), "b".into()]);
        s.cursor = 1;
        assert_eq!(s.selected(), Some(&"b".to_string()));
    }

    #[test]
    fn execute_returns_selected_key_not_query() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["user:1".into(), "user:2".into()]);
        s.query = "user".into();
        s.apply_filter();
        s.cursor = 1;
        let action = s.handle_event(&key(KeyCode::Enter));
        assert_eq!(action, Some(SearchAction::Execute("user:2".into())));
    }

    #[test]
    fn execute_falls_back_to_query_when_no_matches() {
        let mut s = GlobalSearch::new();
        s.query = "nope".into();
        s.apply_filter();
        let action = s.handle_event(&key(KeyCode::Enter));
        assert_eq!(action, Some(SearchAction::Execute("nope".into())));
    }

    #[test]
    fn typing_emits_query_changed_and_filters() {
        let mut s = GlobalSearch::new();
        s.set_results(vec!["user:1".into(), "session:1".into()]);
        let action = s.handle_event(&key(KeyCode::Char('u')));
        assert_eq!(action, Some(SearchAction::QueryChanged));
        assert_eq!(s.filtered, vec!["user:1".to_string()]);
    }

    #[test]
    fn glob_joins_atoms_for_namespace_queries() {
        assert_eq!(glob_pattern_for_query("user:1", SearchMode::Fuzzy), "*user*1*");
        assert_eq!(glob_pattern_for_query("a b c", SearchMode::Fuzzy), "*a*b*c*");
        assert_eq!(glob_pattern_for_query("", SearchMode::Fuzzy), "*");
        assert_eq!(glob_pattern_for_query("***", SearchMode::Fuzzy), "*");
    }

    #[test]
    fn glob_metacharacters_cannot_widen_the_scan() {
        // `*` is consumed as a separator, so the pattern stays anchored to the
        // surrounding atoms rather than becoming a match-everything glob.
        assert_eq!(glob_pattern_for_query("user:*", SearchMode::Fuzzy), "*user*");
        assert_eq!(glob_pattern_for_query("?abc", SearchMode::Fuzzy), "*abc*");
    }

    #[test]
    fn glob_for_regex_uses_longest_alphanumeric_run() {
        assert_eq!(
            glob_pattern_for_query("^user:[0-9]+$", SearchMode::Regex),
            "*user*"
        );
        // Equal-length runs resolve to the leftmost for stability.
        assert_eq!(glob_pattern_for_query("[a-z]+", SearchMode::Regex), "*a*");
        assert_eq!(glob_pattern_for_query(".*", SearchMode::Regex), "*");
    }

    #[test]
    fn query_atoms_lowercases_and_drops_separators() {
        assert_eq!(query_atoms("User:123"), vec!["user", "123"]);
        assert_eq!(query_atoms("a.b-c d"), vec!["a", "b", "c", "d"]);
        assert!(query_atoms("***").is_empty());
    }

    #[test]
    fn matches_are_capped() {
        let mut s = GlobalSearch::new();
        let big: Vec<String> = (0..MAX_MATCHES + 500).map(|i| format!("k{i:06}")).collect();
        s.set_results(big);
        s.query = "k".into();
        s.apply_filter();
        assert_eq!(s.filtered.len(), MAX_MATCHES);
    }

    #[test]
    fn render_does_not_panic_in_any_mode() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut s = GlobalSearch::new();

        // Empty state.
        let _ = terminal.draw(|f| s.render(f, f.area()));

        // Populated + scanning.
        s.start_scan();
        s.query = "user".into();
        s.apply_filter();
        s.drain_scan_batch(vec!["user:1".into(), "user:2".into(), "other".into()]);
        let _ = terminal.draw(|f| s.render(f, f.area()));

        // Regex mode with a parse error.
        s.mode = SearchMode::Regex;
        s.query = "[bad".into();
        s.apply_filter();
        let _ = terminal.draw(|f| s.render(f, f.area()));
    }

    #[test]
    fn render_with_more_results_than_visible_scrolls() {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut s = GlobalSearch::new();
        let many: Vec<String> = (0..500).map(|i| format!("user:{i}")).collect();
        s.set_results(many);
        s.query = "user".into();
        s.apply_filter();
        s.cursor = 400;
        let _ = terminal.draw(|f| s.render(f, f.area()));
    }
}