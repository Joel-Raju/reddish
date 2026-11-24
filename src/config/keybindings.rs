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
    #[serde(default = "default_inspector_list_insert")]
    pub inspector_list_insert: KeyDef,
    #[serde(default = "default_inspector_hash_add")]
    pub inspector_hash_add: KeyDef,
    #[serde(default = "default_inspector_hash_filter")]
    pub inspector_hash_filter: KeyDef,
    #[serde(default = "default_inspector_value_search")]
    pub inspector_value_search: KeyDef,
    #[serde(default = "default_delete_selected")]
    pub delete_selected: KeyDef,
    #[serde(default = "default_inspector_set_add")]
    pub inspector_set_add: KeyDef,
    #[serde(default = "default_inspector_zset_add")]
    pub inspector_zset_add: KeyDef,
    #[serde(default = "default_inspector_zset_range_query")]
    pub inspector_zset_range_query: KeyDef,
    #[serde(default = "default_inspector_stream_add")]
    pub inspector_stream_add: KeyDef,
    #[serde(default = "default_inspector_stream_groups")]
    pub inspector_stream_groups: KeyDef,
    #[serde(default = "default_inspector_stream_trim")]
    pub inspector_stream_trim: KeyDef,
    #[serde(default = "default_inspector_stream_follow")]
    pub inspector_stream_follow: KeyDef,
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
    #[serde(default = "default_search_toggle_mode")]
    pub search_toggle_mode: KeyDef,
    #[serde(default = "default_search_history_prev")]
    pub search_history_prev: KeyDef,
    #[serde(default = "default_search_history_next")]
    pub search_history_next: KeyDef,
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
                key: "D".to_string(),
            },
            refresh: KeyDef {
                key: "R".to_string(),
            },
            filter: KeyDef {
                key: "/".to_string(),
            },
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
            new_key: KeyDef {
                key: "n".to_string(),
            },
            rename_key: KeyDef {
                key: "r".to_string(),
            },
            set_ttl: KeyDef {
                key: "t".to_string(),
            },
            expire: KeyDef {
                key: "e".to_string(),
            },
            select_all: KeyDef {
                key: "ctrl+a".to_string(),
            },
            toggle_select: KeyDef {
                key: "Space".to_string(),
            },
            cycle_sort: KeyDef {
                key: "s".to_string(),
            },
            go_up: KeyDef {
                key: "Backspace".to_string(),
            },
            repl_overlay: KeyDef {
                key: ":".to_string(),
            },
            duplicate_key: KeyDef {
                key: "d".to_string(),
            },
            confirm_yes: KeyDef {
                key: "y".to_string(),
            },
            confirm_no: KeyDef {
                key: "n".to_string(),
            },
            inspector_list_push: KeyDef {
                key: "a".to_string(),
            },
            inspector_list_prepend: KeyDef {
                key: "p".to_string(),
            },
            inspector_list_insert: KeyDef {
                key: "I".to_string(),
            },
            inspector_hash_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_hash_filter: KeyDef {
                key: "ctrl+f".to_string(),
            },
            inspector_value_search: KeyDef {
                // `ctrl+shift+f` could never fire: legacy terminals encode
                // Ctrl+Shift+F as plain 0x06, i.e. `Char('f')`+CONTROL with
                // no SHIFT bit. Mirrors the emacs preset's working binding.
                key: "alt+/".to_string(),
            },
            delete_selected: KeyDef {
                key: "ctrl+d".to_string(),
            },
            inspector_set_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_zset_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_zset_range_query: KeyDef {
                key: "Q".to_string(),
            },
            inspector_stream_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_stream_groups: KeyDef {
                key: "G".to_string(),
            },
            inspector_stream_trim: KeyDef {
                key: "ctrl+t".to_string(),
            },
            inspector_stream_follow: KeyDef {
                key: "F".to_string(),
            },
            inspector_toggle_view: KeyDef {
                key: "f".to_string(),
            },
            inspector_goto_end: KeyDef {
                key: "g".to_string(),
            },
            set_union: KeyDef {
                key: "u".to_string(),
            },
            set_inter: KeyDef {
                key: "i".to_string(),
            },
            set_diff: KeyDef {
                key: "x".to_string(),
            },
            search_execute: KeyDef {
                key: "Enter".to_string(),
            },
            search_close: KeyDef {
                key: "Esc".to_string(),
            },
            search_toggle_mode: KeyDef {
                key: "ctrl+r".to_string(),
            },
            search_history_prev: KeyDef {
                key: "alt+Up".to_string(),
            },
            search_history_next: KeyDef {
                key: "alt+Down".to_string(),
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
            new_key: KeyDef {
                key: "n".to_string(),
            },
            rename_key: KeyDef {
                // `ctrl+r` was unreachable: the global reconnect arm in
                // `App::handle_key_event` claims that chord before any tab
                // handler runs.
                key: "alt+r".to_string(),
            },
            set_ttl: KeyDef {
                key: "ctrl+t".to_string(),
            },
            expire: KeyDef {
                key: "ctrl+e".to_string(),
            },
            select_all: KeyDef {
                key: "ctrl+a".to_string(),
            },
            toggle_select: KeyDef {
                key: "Space".to_string(),
            },
            cycle_sort: KeyDef {
                key: "ctrl+o".to_string(),
            },
            go_up: KeyDef {
                // Was `ctrl+b`, which `nav_left` claims first in the
                // browser's dispatch chain — the alias never fired. Aligns
                // with the vim preset and the hardcoded Backspace fallback.
                key: "Backspace".to_string(),
            },
            repl_overlay: KeyDef {
                key: "alt+;".to_string(),
            },
            duplicate_key: KeyDef {
                key: "alt+d".to_string(),
            },
            confirm_yes: KeyDef {
                key: "y".to_string(),
            },
            confirm_no: KeyDef {
                key: "n".to_string(),
            },
            // The inspector bindings previously used `ctrl+shift+<letter>`
            // chords that no legacy terminal can deliver — Ctrl+Shift+X is
            // byte-identical to Ctrl+X, which the global bindings own. The
            // emacs preset now mirrors the vim preset's contextual letters:
            // the inspector is a modal context that swallows them before
            // the browser sees them, and the emacs globals are all
            // ctrl/alt chords, so nothing collides.
            inspector_list_push: KeyDef {
                key: "a".to_string(),
            },
            inspector_list_prepend: KeyDef {
                key: "p".to_string(),
            },
            inspector_list_insert: KeyDef {
                key: "I".to_string(),
            },
            inspector_hash_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_hash_filter: KeyDef {
                key: "alt+f".to_string(),
            },
            inspector_value_search: KeyDef {
                key: "alt+/".to_string(),
            },
            delete_selected: KeyDef {
                key: "ctrl+d".to_string(),
            },
            inspector_set_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_zset_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_zset_range_query: KeyDef {
                key: "Q".to_string(),
            },
            inspector_stream_add: KeyDef {
                key: "a".to_string(),
            },
            inspector_stream_groups: KeyDef {
                key: "G".to_string(),
            },
            inspector_stream_trim: KeyDef {
                key: "ctrl+t".to_string(),
            },
            inspector_stream_follow: KeyDef {
                key: "F".to_string(),
            },
            inspector_toggle_view: KeyDef {
                key: "f".to_string(),
            },
            inspector_goto_end: KeyDef {
                key: "g".to_string(),
            },
            set_union: KeyDef {
                key: "ctrl+u".to_string(),
            },
            set_inter: KeyDef {
                // `ctrl+i` decodes to `Tab` (byte 0x09): no terminal
                // delivers it as a char event.
                key: "alt+i".to_string(),
            },
            set_diff: KeyDef {
                // `ctrl+d` was claimed by the whole-key `delete` binding
                // earlier in the browser's dispatch chain, so the alias
                // never fired.
                key: "alt+x".to_string(),
            },
            search_execute: KeyDef {
                key: "Enter".to_string(),
            },
            search_close: KeyDef {
                key: "ctrl+g".to_string(),
            },
            search_toggle_mode: KeyDef {
                key: "ctrl+alt+r".to_string(),
            },
            search_history_prev: KeyDef {
                key: "alt+Up".to_string(),
            },
            search_history_next: KeyDef {
                key: "alt+Down".to_string(),
            },
        }
    }

    /// The key definition bound to `action`, if the action is known.
    ///
    /// Exposed so the help overlay can be generated from the keymap actually
    /// in use rather than from a hardcoded string. The previous hardcoded text
    /// listed the vim bindings unconditionally, so it was simply wrong for
    /// anyone using the emacs preset.
    pub fn binding(&self, action: &str) -> Option<&str> {
        Some(self.resolve(action)?.key.as_str())
    }

    fn resolve(&self, action: &str) -> Option<&KeyDef> {
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
            "inspector_list_insert" => &self.inspector_list_insert,
            "inspector_hash_add" => &self.inspector_hash_add,
            "inspector_hash_filter" => &self.inspector_hash_filter,
            "inspector_value_search" => &self.inspector_value_search,
            "delete_selected" => &self.delete_selected,
            "inspector_set_add" => &self.inspector_set_add,
            "inspector_zset_add" => &self.inspector_zset_add,
            "inspector_zset_range_query" => &self.inspector_zset_range_query,
            "inspector_stream_add" => &self.inspector_stream_add,
            "inspector_stream_groups" => &self.inspector_stream_groups,
            "inspector_stream_trim" => &self.inspector_stream_trim,
            "inspector_stream_follow" => &self.inspector_stream_follow,
            "inspector_toggle_view" => &self.inspector_toggle_view,
            "inspector_goto_end" => &self.inspector_goto_end,
            "set_union" => &self.set_union,
            "set_inter" => &self.set_inter,
            "set_diff" => &self.set_diff,
            "search_execute" => &self.search_execute,
            "search_close" => &self.search_close,
            "search_toggle_mode" => &self.search_toggle_mode,
            "search_history_prev" => &self.search_history_prev,
            "search_history_next" => &self.search_history_next,
            _ => return None,
        };
        Some(def)
    }

    pub fn matches(&self, action: &str, event: &KeyEvent) -> bool {
        let Some(def) = self.resolve(action) else {
            return false;
        };
        let (expected_code, expected_mods) = parse_keydef(&def.key);
        event.code == expected_code && event.modifiers == expected_mods
    }
}

/// Every action name the keymap can resolve. Public so validation and tests
/// can walk the whole surface instead of a hand-picked subset.
pub const ACTIONS: &[&str] = &[
    "nav_up",
    "nav_down",
    "nav_left",
    "nav_right",
    "confirm",
    "cancel",
    "delete",
    "refresh",
    "filter",
    "edit",
    "quit",
    "copy",
    "help",
    "palette",
    "tab_keys",
    "tab_repl",
    "tab_info",
    "tab_pubsub",
    "new_key",
    "rename_key",
    "set_ttl",
    "expire",
    "select_all",
    "toggle_select",
    "cycle_sort",
    "go_up",
    "repl_overlay",
    "duplicate_key",
    "confirm_yes",
    "confirm_no",
    "inspector_list_push",
    "inspector_list_prepend",
    "inspector_list_insert",
    "inspector_hash_add",
    "inspector_hash_filter",
    "inspector_value_search",
    "delete_selected",
    "inspector_set_add",
    "inspector_zset_add",
    "inspector_zset_range_query",
    "inspector_stream_add",
    "inspector_stream_groups",
    "inspector_stream_trim",
    "inspector_stream_follow",
    "inspector_toggle_view",
    "inspector_goto_end",
    "set_union",
    "set_inter",
    "set_diff",
    "search_execute",
    "search_close",
    "search_toggle_mode",
    "search_history_prev",
    "search_history_next",
];

/// Parse a key spelling into the exact `KeyEvent` a terminal must deliver
/// for `matches()` to fire.
///
/// The result deliberately mirrors what crossterm 0.29 actually produces in
/// legacy input mode (the app never pushes kitty keyboard-protocol flags),
/// because a binding that can never be delivered is indistinguishable from
/// a missing one: `Ctrl+\` shipped dead for months because it was matched
/// as `Char('\\')`+CONTROL while every real terminal delivered
/// `Char('4')`+CONTROL (byte `0x1C`).
///
/// Normalizations, each tied to the decode table in `is_deliverable_legacy`:
/// - A bare uppercase letter implies SHIFT: crossterm decorates uppercase
///   input with `KeyModifiers::SHIFT` (`char_code_to_event`, parse.rs
///   128-135), and the old spelling `"D"` produced modifiers that no
///   terminal ever sends — the whole vim uppercase family was dead.
/// - `shift+x` is normalized to `X`+SHIFT for the same reason.
/// - `ctrl+\`, `ctrl+]`, `ctrl+^`, `ctrl+_` translate to the digits those
///   bytes decode to (`0x1C..=0x1F` → `Char('4'..='7')`+CONTROL).
/// - `shift+Tab` parses to `BackTab`: legacy terminals send `ESC[Z`.
/// - An unrecognized modifier (`meta+`, `super+`, a typo like `ctra+`)
///   yields `KeyCode::Null` so the binding cannot fire, instead of the old
///   behaviour of silently rebinding the bare key.
fn parse_keydef(key: &str) -> (KeyCode, KeyModifiers) {
    let parts: Vec<&str> = key.split('+').collect();
    let mut modifiers = KeyModifiers::empty();
    let code_str = parts.last().unwrap();
    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            // Never degrade to the bare key: "meta+x" must not rebind `x`.
            _ => return (KeyCode::Null, KeyModifiers::empty()),
        }
    }
    let code = if code_str.len() == 1 {
        let mut c = code_str.chars().next().unwrap();
        // The case of the spelling decides the SHIFT bit, exactly as the
        // terminal delivers it. Applies under `alt` too: ESC + 'D' arrives
        // as ALT|SHIFT.
        if c.is_uppercase() {
            modifiers |= KeyModifiers::SHIFT;
        }
        // An explicit `shift+d` must expect what the wire carries: the
        // uppercase byte 'D' (crossterm decorates it with SHIFT). Without
        // this normalization the binding could never fire.
        if c.is_ascii_lowercase() && modifiers.contains(KeyModifiers::SHIFT) {
            c = c.to_ascii_uppercase();
        }
        KeyCode::Char(c)
    } else {
        match code_str.to_ascii_lowercase().as_str() {
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "space" => KeyCode::Char(' '),
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "backspace" => KeyCode::Backspace,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "delete" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            other => match other.strip_prefix('f') {
                Some(num) => match num.parse::<u8>() {
                    Ok(n) if (1..=12).contains(&n) => KeyCode::F(n),
                    _ => KeyCode::Null,
                },
                None => KeyCode::Null,
            },
        }
    };
    // Byte-level aliases: what the terminal actually transmits.
    let (code, modifiers) = match (code, modifiers) {
        (KeyCode::Char('\\'), m) if m.contains(KeyModifiers::CONTROL) => (KeyCode::Char('4'), m),
        (KeyCode::Char(']'), m) if m.contains(KeyModifiers::CONTROL) => (KeyCode::Char('5'), m),
        (KeyCode::Char('^'), m) if m.contains(KeyModifiers::CONTROL) => (KeyCode::Char('6'), m),
        (KeyCode::Char('_'), m) if m.contains(KeyModifiers::CONTROL) => (KeyCode::Char('7'), m),
        // Shift+Tab arrives as ESC[Z, never as Tab with SHIFT set.
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => {
            (KeyCode::BackTab, m & !KeyModifiers::SHIFT)
        }
        other => other,
    };
    (code, modifiers)
}

/// Whether a terminal can deliver this exact `KeyEvent` in legacy input
/// mode. Mirrors crossterm 0.29's unix decode table
/// (`event::sys::unix::parse::parse_event`), which is the reality for this
/// app: `init_terminal` never pushes kitty keyboard-enhancement flags.
///
/// - `0x00` → `Char(' ')`+CONTROL (parse.rs 114-117)
/// - `0x01..=0x1A` → `Char('a'..='z')`+CONTROL (parse.rs 106-109), except
///   the bytes with dedicated arms: `0x09` → `Tab` (parse.rs 102),
///   `0x0D` → `Enter` (parse.rs 92) — so `ctrl+i` and `ctrl+m` never arrive
/// - `0x1C..=0x1F` → `Char('4'..='7')`+CONTROL (parse.rs 110-113)
/// - `0x7F` → `Backspace` (parse.rs 103-105)
/// - uppercase chars arrive SHIFT-decorated (`char_code_to_event`,
///   parse.rs 128-135); ESC-prefixed input ORs in ALT (parse.rs 78-88)
/// - Shift+Tab arrives as `BackTab` via `ESC[Z` (parse.rs 163-167)
/// - CSI sequences carry SHIFT/ALT/CONTROL modifiers on arrows, Home/End,
///   Delete/Insert/PageUp/PageDown and F-keys (`CSI 1;5C` etc.)
pub fn is_deliverable_legacy(code: KeyCode, mods: KeyModifiers) -> bool {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    let shift = mods.contains(KeyModifiers::SHIFT);
    match code {
        KeyCode::Null => false,
        // Raw bytes with fixed decodes; no modifier reaches them.
        KeyCode::Enter | KeyCode::Esc | KeyCode::Tab | KeyCode::Backspace => {
            !ctrl && !alt && !shift
        }
        // ESC[Z: the shift is implied by the sequence itself.
        KeyCode::BackTab => !ctrl && !alt,
        KeyCode::Char(c) => match (ctrl, alt, shift) {
            (true, false, false) => {
                c == ' '
                    || ('4'..='7').contains(&c)
                    // 0x09 (ctrl+i) and 0x0D (ctrl+m) decode as Tab/Enter.
                    || (c.is_ascii_lowercase() && c != 'i' && c != 'm')
            }
            (true, true, false) => c.is_ascii_lowercase() && c != 'i' && c != 'm',
            // ESC + printable char: uppercase additionally carries SHIFT.
            (false, true, false) => !c.is_uppercase() && !c.is_control(),
            (false, true, true) => c.is_uppercase(),
            (false, false, true) => c.is_uppercase(),
            (false, false, false) => !c.is_uppercase() && !c.is_control(),
            // CONTROL|SHIFT on a letter is byte-identical to CONTROL alone:
            // no legacy terminal can express it.
            _ => false,
        },
        // CSI-delivered keys carry their modifiers in the sequence.
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End
        | KeyCode::Delete
        | KeyCode::Insert
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::F(_) => {
            mods <= (KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL)
        }
        KeyCode::Media(_)
        | KeyCode::Modifier(_)
        | KeyCode::CapsLock
        | KeyCode::ScrollLock
        | KeyCode::NumLock
        | KeyCode::PrintScreen
        | KeyCode::Pause
        | KeyCode::Menu
        | KeyCode::KeypadBegin => false,
    }
}

/// A human-readable reason a config binding can never fire as spelled, if
/// any. Printed at startup so a bad `[keybindings]` entry announces itself
/// instead of silently doing nothing.
pub fn keydef_problem(key: &str) -> Option<String> {
    let (code, mods) = parse_keydef(key);
    if code == KeyCode::Null {
        return Some(format!(
            "'{key}' does not parse to a key (unknown name or modifier)"
        ));
    }
    if !is_deliverable_legacy(code, mods) {
        return Some(format!(
            "'{key}' parses to {code:?}+{mods:?}, which no terminal \
             delivers in legacy input mode"
        ));
    }
    None
}

impl Keymap {
    /// Startup warnings for every binding that cannot fire. Empty means the
    /// whole keymap is deliverable.
    pub fn validation_warnings(&self) -> Vec<String> {
        ACTIONS
            .iter()
            .filter_map(|action| {
                let key = self.binding(action)?;
                keydef_problem(key).map(|problem| format!("{action}: {problem}"))
            })
            .collect()
    }
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
    KeyDef {
        key: "n".to_string(),
    }
}
fn default_rename_key() -> KeyDef {
    KeyDef {
        key: "r".to_string(),
    }
}
fn default_set_ttl() -> KeyDef {
    KeyDef {
        key: "t".to_string(),
    }
}
fn default_expire() -> KeyDef {
    KeyDef {
        key: "e".to_string(),
    }
}
fn default_select_all() -> KeyDef {
    KeyDef {
        key: "ctrl+a".to_string(),
    }
}
fn default_toggle_select() -> KeyDef {
    KeyDef {
        key: "Space".to_string(),
    }
}
fn default_cycle_sort() -> KeyDef {
    KeyDef {
        key: "s".to_string(),
    }
}
fn default_go_up() -> KeyDef {
    KeyDef {
        key: "Backspace".to_string(),
    }
}
fn default_repl_overlay() -> KeyDef {
    KeyDef {
        key: ":".to_string(),
    }
}
fn default_duplicate_key() -> KeyDef {
    KeyDef {
        key: "d".to_string(),
    }
}
fn default_confirm_yes() -> KeyDef {
    KeyDef {
        key: "y".to_string(),
    }
}
fn default_confirm_no() -> KeyDef {
    KeyDef {
        key: "n".to_string(),
    }
}
fn default_inspector_list_push() -> KeyDef {
    KeyDef {
        key: "a".to_string(),
    }
}
fn default_inspector_list_prepend() -> KeyDef {
    KeyDef {
        key: "p".to_string(),
    }
}
fn default_inspector_list_insert() -> KeyDef {
    KeyDef {
        key: "I".to_string(),
    }
}
fn default_inspector_hash_add() -> KeyDef {
    KeyDef {
        key: "a".to_string(),
    }
}
fn default_inspector_hash_filter() -> KeyDef {
    KeyDef {
        key: "ctrl+f".to_string(),
    }
}
fn default_inspector_value_search() -> KeyDef {
    KeyDef {
        key: "alt+/".to_string(),
    }
}
fn default_search_history_prev() -> KeyDef {
    KeyDef {
        key: "alt+Up".to_string(),
    }
}
fn default_search_history_next() -> KeyDef {
    KeyDef {
        key: "alt+Down".to_string(),
    }
}
fn default_delete_selected() -> KeyDef {
    KeyDef {
        key: "ctrl+d".to_string(),
    }
}
fn default_inspector_set_add() -> KeyDef {
    KeyDef {
        key: "a".to_string(),
    }
}
fn default_inspector_zset_add() -> KeyDef {
    KeyDef {
        key: "a".to_string(),
    }
}
fn default_inspector_zset_range_query() -> KeyDef {
    KeyDef {
        key: "Q".to_string(),
    }
}
fn default_inspector_stream_add() -> KeyDef {
    KeyDef {
        key: "a".to_string(),
    }
}
fn default_inspector_stream_groups() -> KeyDef {
    KeyDef {
        key: "G".to_string(),
    }
}
fn default_inspector_stream_trim() -> KeyDef {
    KeyDef {
        key: "ctrl+t".to_string(),
    }
}
fn default_inspector_stream_follow() -> KeyDef {
    KeyDef {
        key: "F".to_string(),
    }
}
fn default_inspector_toggle_view() -> KeyDef {
    KeyDef {
        key: "f".to_string(),
    }
}
fn default_inspector_goto_end() -> KeyDef {
    KeyDef {
        key: "g".to_string(),
    }
}
fn default_set_union() -> KeyDef {
    KeyDef {
        key: "u".to_string(),
    }
}
fn default_set_inter() -> KeyDef {
    KeyDef {
        key: "i".to_string(),
    }
}
fn default_set_diff() -> KeyDef {
    KeyDef {
        key: "x".to_string(),
    }
}
fn default_search_execute() -> KeyDef {
    KeyDef {
        key: "Enter".to_string(),
    }
}
fn default_search_close() -> KeyDef {
    KeyDef {
        key: "Esc".to_string(),
    }
}
fn default_search_toggle_mode() -> KeyDef {
    KeyDef {
        key: "ctrl+r".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Every action name `matches()` knows about. The single source of truth
    /// is the module-level `ACTIONS`; a private copy could drift and hide
    /// an untested action.
    const ALL_ACTIONS: &[&str] = ACTIONS;

    /// Pairs that legitimately share a key because they are dispatched in
    /// different modes or value types. Any overlap NOT listed here is a bug.
    const ALLOWED_OVERLAPS: &[(&str, &str)] = &[
        // Inspector keys share letters; the active Redis value type decides
        // which handler sees them.
        ("inspector_hash_add", "inspector_list_push"),
        ("inspector_hash_add", "inspector_list_prepend"),
        ("inspector_hash_add", "inspector_set_add"),
        ("inspector_hash_add", "inspector_stream_add"),
        ("inspector_hash_add", "inspector_zset_add"),
        ("inspector_list_prepend", "inspector_list_push"),
        ("inspector_list_prepend", "inspector_stream_add"),
        ("inspector_list_prepend", "inspector_zset_add"),
        ("inspector_list_push", "inspector_set_add"),
        ("inspector_list_push", "inspector_stream_add"),
        ("inspector_list_push", "inspector_zset_add"),
        ("inspector_set_add", "inspector_stream_add"),
        ("inspector_set_add", "inspector_zset_add"),
        ("inspector_stream_add", "inspector_zset_add"),
        // Global keys that intentionally alias an inspector action.
        ("filter", "inspector_hash_filter"),
        ("refresh", "inspector_goto_end"),
        ("help", "set_diff"),
        ("edit", "expire"),
        ("delete_selected", "set_inter"),
        ("select_all", "delete_selected"),
        // Esc cancels in every mode; 'y'/'n' answer a confirm dialog but
        // copy/open elsewhere.
        ("cancel", "search_close"),
        ("confirm_yes", "copy"),
        ("confirm", "search_execute"),
        ("confirm_no", "new_key"),
        // Emacs: whole-key delete is ctrl+d in the browser while
        // delete-selected is ctrl+d in the inspector's modal context.
        ("delete", "delete_selected"),
        // Emacs: set_ttl is ctrl+t in the browser while stream trim is
        // ctrl+t in the inspector's modal context.
        ("inspector_stream_trim", "set_ttl"),
    ];

    fn action_key<'a>(km: &'a Keymap, action: &str) -> &'a str {
        match action {
            "nav_up" => &km.nav_up.key,
            "nav_down" => &km.nav_down.key,
            "nav_left" => &km.nav_left.key,
            "nav_right" => &km.nav_right.key,
            "confirm" => &km.confirm.key,
            "cancel" => &km.cancel.key,
            "delete" => &km.delete.key,
            "refresh" => &km.refresh.key,
            "filter" => &km.filter.key,
            "edit" => &km.edit.key,
            "quit" => &km.quit.key,
            "copy" => &km.copy.key,
            "help" => &km.help.key,
            "palette" => &km.palette.key,
            "tab_keys" => &km.tab_keys.key,
            "tab_repl" => &km.tab_repl.key,
            "tab_info" => &km.tab_info.key,
            "tab_pubsub" => &km.tab_pubsub.key,
            "new_key" => &km.new_key.key,
            "rename_key" => &km.rename_key.key,
            "set_ttl" => &km.set_ttl.key,
            "expire" => &km.expire.key,
            "select_all" => &km.select_all.key,
            "toggle_select" => &km.toggle_select.key,
            "cycle_sort" => &km.cycle_sort.key,
            "go_up" => &km.go_up.key,
            "repl_overlay" => &km.repl_overlay.key,
            "duplicate_key" => &km.duplicate_key.key,
            "confirm_yes" => &km.confirm_yes.key,
            "confirm_no" => &km.confirm_no.key,
            "inspector_list_push" => &km.inspector_list_push.key,
            "inspector_list_prepend" => &km.inspector_list_prepend.key,
            "inspector_list_insert" => &km.inspector_list_insert.key,
            "inspector_hash_add" => &km.inspector_hash_add.key,
            "inspector_hash_filter" => &km.inspector_hash_filter.key,
            "inspector_value_search" => &km.inspector_value_search.key,
            "delete_selected" => &km.delete_selected.key,
            "inspector_set_add" => &km.inspector_set_add.key,
            "inspector_zset_add" => &km.inspector_zset_add.key,
            "inspector_zset_range_query" => &km.inspector_zset_range_query.key,
            "inspector_stream_add" => &km.inspector_stream_add.key,
            "inspector_stream_groups" => &km.inspector_stream_groups.key,
            "inspector_stream_trim" => &km.inspector_stream_trim.key,
            "inspector_stream_follow" => &km.inspector_stream_follow.key,
            "inspector_toggle_view" => &km.inspector_toggle_view.key,
            "inspector_goto_end" => &km.inspector_goto_end.key,
            "set_union" => &km.set_union.key,
            "set_inter" => &km.set_inter.key,
            "set_diff" => &km.set_diff.key,
            "search_execute" => &km.search_execute.key,
            "search_close" => &km.search_close.key,
            "search_toggle_mode" => &km.search_toggle_mode.key,
            "search_history_prev" => &km.search_history_prev.key,
            "search_history_next" => &km.search_history_next.key,
            other => panic!("unmapped action {other}"),
        }
    }

    fn assert_no_unexpected_collisions(km: &Keymap, preset: &str) {
        let mut by_key: HashMap<(KeyCode, KeyModifiers), Vec<&'static str>> = HashMap::new();
        for &action in ALL_ACTIONS {
            by_key
                .entry(parse_keydef(action_key(km, action)))
                .or_default()
                .push(action);
        }
        for (key, mut actions) in by_key {
            if actions.len() < 2 {
                continue;
            }
            actions.sort_unstable();
            for i in 0..actions.len() {
                for j in (i + 1)..actions.len() {
                    let pair = (actions[i], actions[j]);
                    assert!(
                        ALLOWED_OVERLAPS.contains(&pair),
                        "{preset} preset: {pair:?} collide on {key:?}; \
                         add it to ALLOWED_OVERLAPS if intentional"
                    );
                }
            }
        }
    }

    #[test]
    fn vim_preset_has_no_unexpected_key_collisions() {
        assert_no_unexpected_collisions(&Keymap::vim(), "vim");
    }

    #[test]
    fn emacs_preset_has_no_unexpected_key_collisions() {
        assert_no_unexpected_collisions(&Keymap::emacs(), "emacs");
    }

    #[test]
    fn every_binding_in_every_preset_parses_to_a_real_key() {
        for (name, km) in [("vim", Keymap::vim()), ("emacs", Keymap::emacs())] {
            for &action in ALL_ACTIONS {
                let (code, _) = parse_keydef(action_key(&km, action));
                assert_ne!(
                    code,
                    KeyCode::Null,
                    "{name}: {action} has an unparseable key string"
                );
            }
        }
    }

    /// The guard the shipped bugs evaded: every preset binding must be an
    /// event a terminal can actually deliver. `ctrl+shift+f`, `ctrl+i`,
    /// `ctrl+m`, `ctrl+\" and the un-shifted uppercase spellings all parse
    /// fine — and none of them ever arrive. Parsing is not enough; the
    /// decoded event has to be reachable on the wire.
    #[test]
    fn every_binding_in_every_preset_is_deliverable() {
        for (name, km) in [("vim", Keymap::vim()), ("emacs", Keymap::emacs())] {
            for &action in ALL_ACTIONS {
                let (code, mods) = parse_keydef(action_key(&km, action));
                assert!(
                    is_deliverable_legacy(code, mods),
                    "{name}: {action} is bound to {code:?}+{mods:?}, which no \
                     terminal delivers in legacy input mode"
                );
            }
        }
    }

    /// `validation_warnings` must be empty for both shipped presets: it is
    /// printed at startup, and a preset that warns about itself on every
    /// launch would train users to ignore the warnings.
    #[test]
    fn shipped_presets_produce_no_validation_warnings() {
        assert!(Keymap::vim().validation_warnings().is_empty());
        assert!(Keymap::emacs().validation_warnings().is_empty());
    }

    /// The spelling→event normalization, pinned case by case. Each row
    /// documents why: terminals, not this repo, define the right-hand side
    /// (crossterm 0.29 unix parse.rs).
    #[test]
    fn parse_keydef_mirrors_what_terminals_deliver() {
        use crossterm::event::{KeyCode, KeyModifiers as M};

        // Uppercase letters arrive SHIFT-decorated; the old parser dropped
        // the bit, which is what killed the vim `D`/`R`/`Q`/`G`/`F`/`I`
        // bindings for months.
        assert_eq!(
            parse_keydef("D"),
            (KeyCode::Char('D'), M::SHIFT),
            "crossterm char_code_to_event: uppercase implies SHIFT"
        );
        assert_eq!(parse_keydef("shift+d"), (KeyCode::Char('D'), M::SHIFT));
        assert_eq!(parse_keydef("shift+D"), (KeyCode::Char('D'), M::SHIFT));
        // The backslash family arrives as digits (0x1C..=0x1F).
        assert_eq!(parse_keydef("ctrl+\\"), (KeyCode::Char('4'), M::CONTROL));
        assert_eq!(parse_keydef("ctrl+4"), (KeyCode::Char('4'), M::CONTROL));
        assert_eq!(parse_keydef("ctrl+]"), (KeyCode::Char('5'), M::CONTROL));
        assert_eq!(parse_keydef("ctrl+^"), (KeyCode::Char('6'), M::CONTROL));
        assert_eq!(parse_keydef("ctrl+_"), (KeyCode::Char('7'), M::CONTROL));
        // 0x00 decodes as Char(' ')+CONTROL; both spellings must work.
        assert_eq!(parse_keydef("ctrl+Space"), (KeyCode::Char(' '), M::CONTROL));
        assert_eq!(parse_keydef("ctrl+space"), (KeyCode::Char(' '), M::CONTROL));
        // Shift+Tab arrives as BackTab via ESC[Z, never Tab+SHIFT.
        assert_eq!(parse_keydef("shift+Tab"), (KeyCode::BackTab, M::empty()));
        // Named keys are case-insensitive; single letters are not, because
        // case carries the SHIFT bit.
        assert_eq!(parse_keydef("enter"), (KeyCode::Enter, M::empty()));
        assert_eq!(parse_keydef("ENTER"), (KeyCode::Enter, M::empty()));
        assert_eq!(parse_keydef("backtab"), (KeyCode::BackTab, M::empty()));
        assert_eq!(parse_keydef("pageup"), (KeyCode::PageUp, M::empty()));
        assert_eq!(parse_keydef("f5"), (KeyCode::F(5), M::empty()));
        assert_ne!(parse_keydef("d"), parse_keydef("D"));
        // Alt + uppercase carries ALT|SHIFT (ESC + 'D').
        assert_eq!(
            parse_keydef("alt+D"),
            (KeyCode::Char('D'), M::ALT | M::SHIFT)
        );
    }

    /// A config typo must never silently rebind the bare key, and an
    /// undeliverable combo must be reported rather than look configured.
    #[test]
    fn undeliverable_and_unknown_spellings_are_flagged() {
        // Unknown modifier: must parse to Null (cannot fire) AND report.
        assert_eq!(
            parse_keydef("meta+x"),
            (KeyCode::Null, KeyModifiers::empty())
        );
        assert_eq!(
            parse_keydef("ctra+x"),
            (KeyCode::Null, KeyModifiers::empty())
        );
        assert!(keydef_problem("meta+x").is_some());
        assert!(keydef_problem("no-such-key").is_some());
        // Parses fine, but no terminal sends CONTROL|SHIFT on a letter,
        // ctrl+i, or ctrl+m: the exact class that shipped dead.
        assert!(keydef_problem("ctrl+shift+x").is_some());
        assert!(keydef_problem("ctrl+i").is_some());
        assert!(keydef_problem("ctrl+m").is_some());
        assert!(
            keydef_problem("ctrl+j").is_none(),
            "ctrl+j is 0x0A and is delivered"
        );
        // Healthy spellings report nothing.
        assert!(keydef_problem("D").is_none());
        assert!(keydef_problem("ctrl+\\").is_none());
        assert!(keydef_problem("alt+Up").is_none());
        assert!(keydef_problem("Backspace").is_none());
    }

    /// The deliverability table itself, pinned to the crossterm 0.29
    /// decode facts with the byte that produces each event.
    #[test]
    fn the_legacy_deliverability_table_is_correct() {
        use crossterm::event::{KeyCode, KeyModifiers as M};

        let yes = [
            (KeyCode::Char('a'), M::CONTROL),          // 0x01
            (KeyCode::Char('z'), M::CONTROL),          // 0x1A
            (KeyCode::Char('j'), M::CONTROL),          // 0x0A
            (KeyCode::Char(' '), M::CONTROL),          // 0x00
            (KeyCode::Char('4'), M::CONTROL),          // 0x1C
            (KeyCode::Char('7'), M::CONTROL),          // 0x1F
            (KeyCode::Char('D'), M::SHIFT),            // 'D'
            (KeyCode::Char('d'), M::empty()),          // 'd'
            (KeyCode::Char('/'), M::empty()),          // '/'
            (KeyCode::Char('1'), M::ALT),              // ESC '1'
            (KeyCode::Char('/'), M::ALT),              // ESC '/'
            (KeyCode::Char('D'), M::ALT | M::SHIFT),   // ESC 'D'
            (KeyCode::Char('r'), M::CONTROL | M::ALT), // ESC 0x12
            (KeyCode::Enter, M::empty()),              // 0x0D
            (KeyCode::Esc, M::empty()),                // 0x1B
            (KeyCode::Tab, M::empty()),                // 0x09
            (KeyCode::Backspace, M::empty()),          // 0x7F
            (KeyCode::BackTab, M::empty()),            // ESC [ Z
            (KeyCode::Up, M::CONTROL),                 // CSI 1;5A
            (KeyCode::Down, M::ALT),                   // CSI 1;3B
            (KeyCode::PageDown, M::empty()),           // CSI 6~
            (KeyCode::F(3), M::empty()),               // CSI/SS3
        ];
        for (code, mods) in yes {
            assert!(
                is_deliverable_legacy(code, mods),
                "{code:?}+{mods:?} should be deliverable"
            );
        }

        let no = [
            (KeyCode::Char('i'), M::CONTROL), // 0x09 is Tab, not ctrl+i
            (KeyCode::Char('m'), M::CONTROL), // 0x0D is Enter, not ctrl+m
            (KeyCode::Char('f'), M::CONTROL | M::SHIFT), // byte-identical to ctrl+f
            (KeyCode::Char('\\'), M::CONTROL), // 0x1C arrives as Char('4')
            (KeyCode::Char('x'), M::SHIFT),   // lowercase never carries SHIFT
            (KeyCode::Char('D'), M::empty()), // uppercase always carries SHIFT
            (KeyCode::Tab, M::SHIFT),         // Shift+Tab arrives as BackTab
            (KeyCode::Enter, M::CONTROL),     // 0x0D has no modifier form
            (KeyCode::Null, M::empty()),      // unparseable
        ];
        for (code, mods) in no {
            assert!(
                !is_deliverable_legacy(code, mods),
                "{code:?}+{mods:?} should NOT be deliverable"
            );
        }
    }

    #[test]
    fn alt_arrow_history_keys_parse_and_are_distinct_from_plain_arrows() {
        let km = Keymap::vim();
        assert_eq!(
            parse_keydef(&km.search_history_prev.key),
            (KeyCode::Up, KeyModifiers::ALT)
        );
        assert_eq!(
            parse_keydef(&km.search_history_next.key),
            (KeyCode::Down, KeyModifiers::ALT)
        );
        assert!(km.matches(
            "search_history_prev",
            &KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)
        ));
        assert!(km.matches(
            "search_history_next",
            &KeyEvent::new(KeyCode::Down, KeyModifiers::ALT)
        ));
        // Plain arrows drive result navigation and must not browse history.
        assert!(!km.matches(
            "search_history_prev",
            &KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
        ));
        assert!(!km.matches(
            "search_history_next",
            &KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
        ));
    }

    #[test]
    fn emacs_inspector_keys_do_not_shadow_toggle_view() {
        let km = Keymap::emacs();
        // All three previously resolved to ctrl+shift+f at one point.
        assert_ne!(km.inspector_hash_filter.key, km.inspector_toggle_view.key);
        assert_ne!(km.inspector_value_search.key, km.inspector_toggle_view.key);
        assert_ne!(km.inspector_hash_filter.key, km.inspector_value_search.key);
    }

    #[test]
    fn toml_override_applies_to_new_actions() {
        let km: Keymap = toml::from_str("[inspector_value_search]\nkey = \"ctrl+g\"\n").unwrap();
        assert_eq!(km.inspector_value_search.key, "ctrl+g");
    }

    #[test]
    fn defaults_match_the_vim_preset() {
        // A drifting serde default silently overrides the preset for users
        // whose config omits the key.
        let preset = Keymap::vim();
        assert_eq!(
            parse_keydef(&preset.inspector_value_search.key),
            parse_keydef(&default_inspector_value_search().key)
        );
        assert_eq!(
            parse_keydef(&preset.inspector_hash_filter.key),
            parse_keydef(&default_inspector_hash_filter().key)
        );
        assert_eq!(
            parse_keydef(&preset.search_history_prev.key),
            parse_keydef(&default_search_history_prev().key)
        );
        assert_eq!(
            parse_keydef(&preset.search_history_next.key),
            parse_keydef(&default_search_history_next().key)
        );
    }
}
