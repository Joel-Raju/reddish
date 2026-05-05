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

    pub fn next(&mut self) -> String {
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
                self.entries
                    .get(idx + 1)
                    .cloned()
                    .unwrap_or_default()
            }
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
