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
    #[serde(default = "default_new_key")]
    pub new_key: KeyDef,
    #[serde(default = "default_rename_key")]
    pub rename_key: KeyDef,
    #[serde(default = "default_set_ttl")]
    pub set_ttl: KeyDef,
    #[serde(default = "default_expire")]
    pub expire: KeyDef,
    #[serde(default = "default_select_all")]
    pub select_all: KeyDef,
    #[serde(default = "default_toggle_select")]
    pub toggle_select: KeyDef,
    #[serde(default = "default_cycle_sort")]
    pub cycle_sort: KeyDef,
    #[serde(default = "default_go_up")]
    pub go_up: KeyDef,
    #[serde(default = "default_repl_overlay")]
    pub repl_overlay: KeyDef,
    #[serde(default = "default_duplicate_key")]
    pub duplicate_key: KeyDef,
    #[serde(default = "default_confirm_yes")]
    pub confirm_yes: KeyDef,
    #[serde(default = "default_confirm_no")]
    pub confirm_no: KeyDef,
    #[serde(default = "default_inspector_list_push")]
    pub inspector_list_push: KeyDef,
    #[serde(default = "default_inspector_list_prepend")]
    pub inspector_list_prepend: KeyDef,
    #[serde(default = "default_inspector_hash_add")]
    pub inspector_hash_add: KeyDef,
    #[serde(default = "default_inspector_set_add")]
    pub inspector_set_add: KeyDef,
    #[serde(default = "default_inspector_zset_add")]
    pub inspector_zset_add: KeyDef,
    #[serde(default = "default_inspector_stream_add")]
    pub inspector_stream_add: KeyDef,
    #[serde(default = "default_inspector_toggle_view")]
    pub inspector_toggle_view: KeyDef,
    #[serde(default = "default_inspector_goto_end")]
    pub inspector_goto_end: KeyDef,
    #[serde(default = "default_set_union")]
    pub set_union: KeyDef,
    #[serde(default = "default_set_inter")]
    pub set_inter: KeyDef,
    #[serde(default = "default_set_diff")]
    pub set_diff: KeyDef,
    #[serde(default = "default_search_execute")]
    pub search_execute: KeyDef,
    #[serde(default = "default_search_close")]
    pub search_close: KeyDef,
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
            nav_up: KeyDef { key: "k".to_string() },
            nav_down: KeyDef { key: "j".to_string() },
            nav_left: KeyDef { key: "h".to_string() },
            nav_right: KeyDef { key: "l".to_string() },
            confirm: KeyDef { key: "Enter".to_string() },
            cancel: KeyDef { key: "Esc".to_string() },
            delete: KeyDef { key: "D".to_string() },
            refresh: KeyDef { key: "R".to_string() },
            filter: KeyDef { key: "/".to_string() },
            edit: KeyDef { key: "e".to_string() },
            quit: KeyDef { key: "q".to_string() },
            copy: KeyDef { key: "y".to_string() },
            help: KeyDef { key: "?".to_string() },
            palette: KeyDef { key: "ctrl+p".to_string() },
            tab_keys: KeyDef { key: "1".to_string() },
            tab_repl: KeyDef { key: "2".to_string() },
            tab_info: KeyDef { key: "3".to_string() },
            tab_pubsub: KeyDef { key: "4".to_string() },
            new_key: KeyDef { key: "n".to_string() },
            rename_key: KeyDef { key: "r".to_string() },
            set_ttl: KeyDef { key: "t".to_string() },
            expire: KeyDef { key: "e".to_string() },
            select_all: KeyDef { key: "ctrl+a".to_string() },
            toggle_select: KeyDef { key: "Space".to_string() },
            cycle_sort: KeyDef { key: "s".to_string() },
            go_up: KeyDef { key: "Backspace".to_string() },
            repl_overlay: KeyDef { key: ":".to_string() },
            duplicate_key: KeyDef { key: "d".to_string() },
            confirm_yes: KeyDef { key: "y".to_string() },
            confirm_no: KeyDef { key: "n".to_string() },
            inspector_list_push: KeyDef { key: "a".to_string() },
            inspector_list_prepend: KeyDef { key: "p".to_string() },
            inspector_hash_add: KeyDef { key: "a".to_string() },
            inspector_set_add: KeyDef { key: "a".to_string() },
            inspector_zset_add: KeyDef { key: "a".to_string() },
            inspector_stream_add: KeyDef { key: "a".to_string() },
            inspector_toggle_view: KeyDef { key: "f".to_string() },
            inspector_goto_end: KeyDef { key: "g".to_string() },
            set_union: KeyDef { key: "u".to_string() },
            set_inter: KeyDef { key: "i".to_string() },
            set_diff: KeyDef { key: "x".to_string() },
            search_execute: KeyDef { key: "Enter".to_string() },
            search_close: KeyDef { key: "Esc".to_string() },
        }
    }

    pub fn emacs() -> Self {
        Self {
            nav_up: KeyDef { key: "ctrl+p".to_string() },
            nav_down: KeyDef { key: "ctrl+n".to_string() },
            nav_left: KeyDef { key: "ctrl+b".to_string() },
            nav_right: KeyDef { key: "ctrl+f".to_string() },
            confirm: KeyDef { key: "Enter".to_string() },
            cancel: KeyDef { key: "ctrl+g".to_string() },
            delete: KeyDef { key: "ctrl+d".to_string() },
            refresh: KeyDef { key: "ctrl+l".to_string() },
            filter: KeyDef { key: "ctrl+s".to_string() },
            edit: KeyDef { key: "ctrl+e".to_string() },
            quit: KeyDef { key: "ctrl+c".to_string() },
            copy: KeyDef { key: "ctrl+w".to_string() },
            help: KeyDef { key: "?".to_string() },
            palette: KeyDef { key: "ctrl+x".to_string() },
            tab_keys: KeyDef { key: "alt+1".to_string() },
            tab_repl: KeyDef { key: "alt+2".to_string() },
            tab_info: KeyDef { key: "alt+3".to_string() },
            tab_pubsub: KeyDef { key: "alt+4".to_string() },
            new_key: KeyDef { key: "n".to_string() },
            rename_key: KeyDef { key: "ctrl+r".to_string() },
            set_ttl: KeyDef { key: "ctrl+t".to_string() },
            expire: KeyDef { key: "ctrl+e".to_string() },
            select_all: KeyDef { key: "ctrl+a".to_string() },
            toggle_select: KeyDef { key: "Space".to_string() },
            cycle_sort: KeyDef { key: "ctrl+o".to_string() },
            go_up: KeyDef { key: "ctrl+b".to_string() },
            repl_overlay: KeyDef { key: "alt+;".to_string() },
            duplicate_key: KeyDef { key: "ctrl+d".to_string() },
            confirm_yes: KeyDef { key: "y".to_string() },
            confirm_no: KeyDef { key: "n".to_string() },
            inspector_list_push: KeyDef { key: "ctrl+shift+a".to_string() },
            inspector_list_prepend: KeyDef { key: "ctrl+shift+p".to_string() },
            inspector_hash_add: KeyDef { key: "ctrl+shift+h".to_string() },
            inspector_set_add: KeyDef { key: "ctrl+shift+s".to_string() },
            inspector_zset_add: KeyDef { key: "ctrl+shift+z".to_string() },
            inspector_stream_add: KeyDef { key: "ctrl+shift+x".to_string() },
            inspector_toggle_view: KeyDef { key: "ctrl+shift+f".to_string() },
            inspector_goto_end: KeyDef { key: "ctrl+shift+g".to_string() },
            set_union: KeyDef { key: "ctrl+u".to_string() },
            set_inter: KeyDef { key: "ctrl+i".to_string() },
            set_diff: KeyDef { key: "ctrl+d".to_string() },
            search_execute: KeyDef { key: "Enter".to_string() },
            search_close: KeyDef { key: "ctrl+g".to_string() },
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
            "new_key" => &self.new_key,
            "rename_key" => &self.rename_key,
            "set_ttl" => &self.set_ttl,
            "expire" => &self.expire,
            "select_all" => &self.select_all,
            "toggle_select" => &self.toggle_select,
            "cycle_sort" => &self.cycle_sort,
            "go_up" => &self.go_up,
            "repl_overlay" => &self.repl_overlay,
            "duplicate_key" => &self.duplicate_key,
            "confirm_yes" => &self.confirm_yes,
            "confirm_no" => &self.confirm_no,
            "inspector_list_push" => &self.inspector_list_push,
            "inspector_list_prepend" => &self.inspector_list_prepend,
            "inspector_hash_add" => &self.inspector_hash_add,
            "inspector_set_add" => &self.inspector_set_add,
            "inspector_zset_add" => &self.inspector_zset_add,
            "inspector_stream_add" => &self.inspector_stream_add,
            "inspector_toggle_view" => &self.inspector_toggle_view,
            "inspector_goto_end" => &self.inspector_goto_end,
            "set_union" => &self.set_union,
            "set_inter" => &self.set_inter,
            "set_diff" => &self.set_diff,
            "search_execute" => &self.search_execute,
            "search_close" => &self.search_close,
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
fn default_new_key() -> KeyDef {
    KeyDef { key: "n".to_string() }
}
fn default_rename_key() -> KeyDef {
    KeyDef { key: "r".to_string() }
}
fn default_set_ttl() -> KeyDef {
    KeyDef { key: "t".to_string() }
}
fn default_expire() -> KeyDef {
    KeyDef { key: "e".to_string() }
}
fn default_select_all() -> KeyDef {
    KeyDef { key: "ctrl+a".to_string() }
}
fn default_toggle_select() -> KeyDef {
    KeyDef { key: "Space".to_string() }
}
fn default_cycle_sort() -> KeyDef {
    KeyDef { key: "s".to_string() }
}
fn default_go_up() -> KeyDef {
    KeyDef { key: "Backspace".to_string() }
}
fn default_repl_overlay() -> KeyDef {
    KeyDef { key: ":".to_string() }
}
fn default_duplicate_key() -> KeyDef {
    KeyDef { key: "d".to_string() }
}
fn default_confirm_yes() -> KeyDef {
    KeyDef { key: "y".to_string() }
}
fn default_confirm_no() -> KeyDef {
    KeyDef { key: "n".to_string() }
}
fn default_inspector_list_push() -> KeyDef {
    KeyDef { key: "a".to_string() }
}
fn default_inspector_list_prepend() -> KeyDef {
    KeyDef { key: "p".to_string() }
}
fn default_inspector_hash_add() -> KeyDef {
    KeyDef { key: "a".to_string() }
}
fn default_inspector_set_add() -> KeyDef {
    KeyDef { key: "a".to_string() }
}
fn default_inspector_zset_add() -> KeyDef {
    KeyDef { key: "a".to_string() }
}
fn default_inspector_stream_add() -> KeyDef {
    KeyDef { key: "a".to_string() }
}
fn default_inspector_toggle_view() -> KeyDef {
    KeyDef { key: "f".to_string() }
}
fn default_inspector_goto_end() -> KeyDef {
    KeyDef { key: "g".to_string() }
}
fn default_set_union() -> KeyDef {
    KeyDef { key: "u".to_string() }
}
fn default_set_inter() -> KeyDef {
    KeyDef { key: "i".to_string() }
}
fn default_set_diff() -> KeyDef {
    KeyDef { key: "x".to_string() }
}
fn default_search_execute() -> KeyDef {
    KeyDef { key: "Enter".to_string() }
}
fn default_search_close() -> KeyDef {
    KeyDef { key: "Esc".to_string() }
}
