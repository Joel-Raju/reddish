use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Keymap {
    #[serde(default = "default_nav_up")]
    pub nav_up: KeyDef,
    #[serde(default = "default_nav_down")]
    pub nav_down: KeyDef,
    #[serde(default = "default_nav_left")]
    pub nav_left: KeyDef,
    #[serde(default = "default_nav_right")]
    pub nav_right: KeyDef,
    #[serde(default = "default_confirm")]
    pub confirm: KeyDef,
    #[serde(default = "default_cancel")]
    pub cancel: KeyDef,
    #[serde(default = "default_delete")]
    pub delete: KeyDef,
    #[serde(default = "default_refresh")]
    pub refresh: KeyDef,
    #[serde(default = "default_filter")]
    pub filter: KeyDef,
    #[serde(default = "default_edit")]
    pub edit: KeyDef,
    #[serde(default = "default_quit")]
    pub quit: KeyDef,
    #[serde(default = "default_copy")]
    pub copy: KeyDef,
    #[serde(default = "default_help")]
    pub help: KeyDef,
    #[serde(default = "default_palette")]
    pub palette: KeyDef,
    #[serde(default = "default_tab_keys")]
    pub tab_keys: KeyDef,
    #[serde(default = "default_tab_repl")]
    pub tab_repl: KeyDef,
    #[serde(default = "default_tab_info")]
    pub tab_info: KeyDef,
    #[serde(default = "default_tab_pubsub")]
    pub tab_pubsub: KeyDef,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct KeyDef {
    pub key: String,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::vim()
    }
}

impl Keymap {
    pub fn vim() -> Self {
        Self {
            nav_up: KeyDef {
                key: "k".to_string(),
            },
            nav_down: KeyDef {
                key: "j".to_string(),
            },
            nav_left: KeyDef {
                key: "h".to_string(),
            },
            nav_right: KeyDef {
                key: "l".to_string(),
            },
            confirm: KeyDef {
                key: "Enter".to_string(),
            },
            cancel: KeyDef {
                key: "Esc".to_string(),
            },
            delete: KeyDef {
                key: "d".to_string(),
            },
            refresh: KeyDef {
                key: "R".to_string(),
            },
            filter: KeyDef {
                key: "/".to_string(),
            },
            // ... vim preset
            edit: KeyDef {
                key: "e".to_string(),
            },
            quit: KeyDef {
                key: "q".to_string(),
            },
            copy: KeyDef {
                key: "y".to_string(),
            },
            help: KeyDef {
                key: "?".to_string(),
            },
            palette: KeyDef {
                key: "ctrl+p".to_string(),
            },
            tab_keys: KeyDef {
                key: "1".to_string(),
            },
            tab_repl: KeyDef {
                key: "2".to_string(),
            },
            tab_info: KeyDef {
                key: "3".to_string(),
            },
            tab_pubsub: KeyDef {
                key: "4".to_string(),
            },
        }
    }

    pub fn emacs() -> Self {
        Self {
            nav_up: KeyDef {
                key: "ctrl+p".to_string(),
            },
            nav_down: KeyDef {
                key: "ctrl+n".to_string(),
            },
            nav_left: KeyDef {
                key: "ctrl+b".to_string(),
            },
            nav_right: KeyDef {
                key: "ctrl+f".to_string(),
            },
            confirm: KeyDef {
                key: "Enter".to_string(),
            },
            cancel: KeyDef {
                key: "ctrl+g".to_string(),
            },
            delete: KeyDef {
                key: "ctrl+d".to_string(),
            },
            refresh: KeyDef {
                key: "ctrl+l".to_string(),
            },
            filter: KeyDef {
                key: "ctrl+s".to_string(),
            },
            edit: KeyDef {
                key: "ctrl+e".to_string(),
            },
            quit: KeyDef {
                key: "ctrl+c".to_string(),
            },
            copy: KeyDef {
                key: "ctrl+w".to_string(),
            },
            help: KeyDef {
                key: "?".to_string(),
            },
            palette: KeyDef {
                key: "ctrl+x".to_string(),
            },
            tab_keys: KeyDef {
                key: "alt+1".to_string(),
            },
            tab_repl: KeyDef {
                key: "alt+2".to_string(),
            },
            tab_info: KeyDef {
                key: "alt+3".to_string(),
            },
            tab_pubsub: KeyDef {
                key: "alt+4".to_string(),
            },
        }
    }

    pub fn matches(&self, action: &str, event: &KeyEvent) -> bool {
        let def = match action {
            "nav_up" => &self.nav_up,
            "nav_down" => &self.nav_down,
            "nav_left" => &self.nav_left,
            "nav_right" => &self.nav_right,
            "confirm" => &self.confirm,
            "cancel" => &self.cancel,
            "delete" => &self.delete,
            "refresh" => &self.refresh,
            "filter" => &self.filter,
            "edit" => &self.edit,
            "quit" => &self.quit,
            "copy" => &self.copy,
            "help" => &self.help,
            "palette" => &self.palette,
            "tab_keys" => &self.tab_keys,
            "tab_repl" => &self.tab_repl,
            "tab_info" => &self.tab_info,
            "tab_pubsub" => &self.tab_pubsub,
            _ => return false,
        };
        let (expected_code, expected_mods) = parse_keydef(&def.key);
        event.code == expected_code && event.modifiers == expected_mods
    }
}

fn parse_keydef(key: &str) -> (KeyCode, KeyModifiers) {
    let parts: Vec<&str> = key.split('+').collect();
    let mut modifiers = KeyModifiers::empty();
    let code_str = parts.last().unwrap();
    for part in &parts[..parts.len().saturating_sub(1)] {
        match *part {
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => {}
        }
    }
    let code = match *code_str {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Space" => KeyCode::Char(' '),
        s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
        _ => KeyCode::Null,
    };
    (code, modifiers)
}

fn default_nav_up() -> KeyDef {
    KeyDef {
        key: "k".to_string(),
    }
}
fn default_nav_down() -> KeyDef {
    KeyDef {
        key: "j".to_string(),
    }
}
fn default_nav_left() -> KeyDef {
    KeyDef {
        key: "h".to_string(),
    }
}
fn default_nav_right() -> KeyDef {
    KeyDef {
        key: "l".to_string(),
    }
}
fn default_confirm() -> KeyDef {
    KeyDef {
        key: "Enter".to_string(),
    }
}
fn default_cancel() -> KeyDef {
    KeyDef {
        key: "Esc".to_string(),
    }
}
fn default_delete() -> KeyDef {
    KeyDef {
        key: "d".to_string(),
    }
}
fn default_refresh() -> KeyDef {
    KeyDef {
        key: "R".to_string(),
    }
}
fn default_filter() -> KeyDef {
    KeyDef {
        key: "/".to_string(),
    }
}
fn default_edit() -> KeyDef {
    KeyDef {
        key: "e".to_string(),
    }
}
fn default_quit() -> KeyDef {
    KeyDef {
        key: "q".to_string(),
    }
}
fn default_copy() -> KeyDef {
    KeyDef {
        key: "y".to_string(),
    }
}
fn default_help() -> KeyDef {
    KeyDef {
        key: "?".to_string(),
    }
}
fn default_palette() -> KeyDef {
    KeyDef {
        key: "ctrl+p".to_string(),
    }
}
fn default_tab_keys() -> KeyDef {
    KeyDef {
        key: "1".to_string(),
    }
}
fn default_tab_repl() -> KeyDef {
    KeyDef {
        key: "2".to_string(),
    }
}
fn default_tab_info() -> KeyDef {
    KeyDef {
        key: "3".to_string(),
    }
}
fn default_tab_pubsub() -> KeyDef {
    KeyDef {
        key: "4".to_string(),
    }
}
