use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use color_eyre::Result;

pub struct CommandHistory {
    entries: VecDeque<String>,
    cursor: Option<usize>,
    path: PathBuf,
}

impl CommandHistory {
    pub fn load(path: &Path) -> Self {
        let entries = std::fs::read_to_string(path)
            .ok()
            .map(|content| {
                content
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect::<VecDeque<_>>()
            })
            .unwrap_or_default();

        Self {
            entries,
            cursor: None,
            path: path.to_path_buf(),
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = self.entries.iter().cloned().collect::<Vec<_>>().join("\n");
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    pub fn push(&mut self, cmd: String) {
        if cmd.trim().is_empty() {
            return;
        }

        if self.entries.back().is_some_and(|last| last == &cmd) {
            self.reset_cursor();
            return;
        }

        self.entries.push_back(cmd);
        if self.entries.len() > 1000 {
            self.entries.pop_front();
        }
        self.reset_cursor();
    }

    pub fn prev(&mut self, current_input: &str) -> String {
        if self.entries.is_empty() {
            return current_input.to_string();
        }

        self.cursor = match self.cursor {
            None => Some(self.entries.len().saturating_sub(1)),
            Some(idx) => Some(idx.saturating_sub(1)),
        };

        let idx = self.cursor.unwrap_or(0);
        self.entries
            .get(idx)
            .cloned()
            .unwrap_or_else(|| current_input.to_string())
    }

    pub fn next_entry(&mut self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        match self.cursor {
            None => String::new(),
            Some(idx) if idx + 1 >= self.entries.len() => {
                self.cursor = None;
                String::new()
            }
            Some(idx) => {
                self.cursor = Some(idx + 1);
                self.entries.get(idx + 1).cloned().unwrap_or_default()
            }
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A history backed by a path in `dir`, cleaned up on drop.
    struct TempHistory {
        path: PathBuf,
    }

    impl TempHistory {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "reddish-repl-history-{}-{}",
                std::process::id(),
                name
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let path = dir.join("history");
            Self { path }
        }

        fn load(&self) -> CommandHistory {
            CommandHistory::load(&self.path)
        }
    }

    impl Drop for TempHistory {
        fn drop(&mut self) {
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    #[test]
    fn new_history_is_empty() {
        let temp = TempHistory::new("empty");
        let history = temp.load();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn push_appends_in_order() {
        let temp = TempHistory::new("order");
        let mut history = temp.load();
        history.push("GET a".to_string());
        history.push("GET b".to_string());
        assert_eq!(history.len(), 2);
        assert_eq!(history.prev(""), "GET b");
        assert_eq!(history.prev(""), "GET a");
    }

    #[test]
    fn push_ignores_blank_commands() {
        let temp = TempHistory::new("blank");
        let mut history = temp.load();
        history.push(String::new());
        history.push("   ".to_string());
        history.push("\t\n".to_string());
        assert!(history.is_empty());
    }

    #[test]
    fn push_skips_consecutive_duplicates() {
        let temp = TempHistory::new("dupes");
        let mut history = temp.load();
        history.push("PING".to_string());
        history.push("PING".to_string());
        history.push("PING".to_string());
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn push_allows_non_consecutive_duplicates() {
        // Only *consecutive* duplicates collapse; re-running a command later
        // should still be recorded.
        let temp = TempHistory::new("nonconsecutive");
        let mut history = temp.load();
        history.push("A".to_string());
        history.push("B".to_string());
        history.push("A".to_string());
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn push_caps_at_one_thousand_entries() {
        let temp = TempHistory::new("cap");
        let mut history = temp.load();
        for i in 0..1_100 {
            history.push(format!("GET k{i}"));
        }
        assert_eq!(history.len(), 1_000);
        // The most recent command must survive.
        assert_eq!(history.prev(""), "GET k1099");
    }

    #[test]
    fn prev_walks_backwards_and_clamps_at_the_oldest() {
        let temp = TempHistory::new("prev");
        let mut history = temp.load();
        for cmd in ["first", "second", "third"] {
            history.push(cmd.to_string());
        }
        assert_eq!(history.prev(""), "third");
        assert_eq!(history.prev(""), "second");
        assert_eq!(history.prev(""), "first");
        // Clamped: cannot go before the oldest entry.
        assert_eq!(history.prev(""), "first");
    }

    #[test]
    fn next_walks_forwards_and_clears_past_the_newest() {
        let temp = TempHistory::new("next");
        let mut history = temp.load();
        for cmd in ["older", "newer"] {
            history.push(cmd.to_string());
        }
        history.prev("");
        history.prev("");
        assert_eq!(history.next_entry(), "newer");
        assert_eq!(
            history.next_entry(),
            "",
            "clears once past the newest entry"
        );
    }

    #[test]
    fn next_without_browsing_returns_empty() {
        let temp = TempHistory::new("next-none");
        let mut history = temp.load();
        history.push("cmd".to_string());
        assert_eq!(history.next_entry(), "");
    }

    #[test]
    fn prev_on_empty_history_preserves_current_input() {
        let temp = TempHistory::new("prev-empty");
        let mut history = temp.load();
        assert_eq!(
            history.prev("typed but unsubmitted"),
            "typed but unsubmitted"
        );
    }

    #[test]
    fn reset_cursor_returns_to_the_newest_entry() {
        let temp = TempHistory::new("reset");
        let mut history = temp.load();
        history.push("older".to_string());
        history.push("newer".to_string());
        history.prev("");
        history.prev("");
        history.reset_cursor();
        assert_eq!(history.prev(""), "newer", "cursor restarts from the newest");
    }

    #[test]
    fn push_resets_the_browse_cursor() {
        let temp = TempHistory::new("push-resets");
        let mut history = temp.load();
        history.push("a".to_string());
        history.prev("");
        history.push("b".to_string());
        assert_eq!(history.prev(""), "b");
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp = TempHistory::new("persist");
        let mut history = temp.load();
        for i in 0..5 {
            history.push(format!("GET key{i}"));
        }
        history.save().expect("save");

        // A fresh instance must observe the persisted commands.
        let mut reloaded = temp.load();
        assert_eq!(reloaded.len(), 5);
        assert_eq!(reloaded.prev(""), "GET key4");
        assert_eq!(reloaded.prev(""), "GET key3");
    }

    #[test]
    fn load_from_missing_file_is_empty_not_an_error() {
        let temp = TempHistory::new("missing");
        let history = temp.load();
        assert!(history.is_empty());
    }

    #[test]
    fn load_skips_blank_lines_and_trims() {
        let temp = TempHistory::new("whitespace");
        std::fs::create_dir_all(temp.path.parent().unwrap()).unwrap();
        std::fs::write(&temp.path, "  GET a  \n\n\nGET b\n   \n").unwrap();

        let mut history = temp.load();
        assert_eq!(history.len(), 2);
        assert_eq!(history.prev(""), "GET b");
        assert_eq!(history.prev(""), "GET a");
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let temp = TempHistory::new("mkdir");
        let mut history = temp.load();
        history.push("PING".to_string());
        history.save().expect("save should create the directory");
        assert!(temp.path.exists());
    }
}
