use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub border: Color,
    pub title: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub ttl_ok: Color,
    pub ttl_warn: Color,
    pub ttl_crit: Color,
    /// Key-size metadata shown alongside the TTL.
    pub memory: Color,
    /// An open input prompt, which asks the user to act.
    pub prompt: Color,
    pub type_string: Color,
    pub type_list: Color,
    pub type_hash: Color,
    pub type_set: Color,
    pub type_zset: Color,
    pub type_stream: Color,
}

pub const DEFAULT: Theme = Theme {
    name: "default",
    bg: Color::Black,
    fg: Color::White,
    highlight_bg: Color::Blue,
    highlight_fg: Color::White,
    border: Color::Gray,
    title: Color::Cyan,
    error: Color::Red,
    warning: Color::Yellow,
    success: Color::Green,
    ttl_ok: Color::Green,
    ttl_warn: Color::Yellow,
    ttl_crit: Color::Red,
    memory: Color::Magenta,
    prompt: Color::Yellow,
    type_string: Color::Green,
    type_list: Color::Yellow,
    type_hash: Color::Magenta,
    type_set: Color::Cyan,
    type_zset: Color::Blue,
    type_stream: Color::LightRed,
};

pub const DRACULA: Theme = Theme {
    name: "dracula",
    bg: Color::Rgb(40, 42, 54),
    fg: Color::Rgb(248, 248, 242),
    highlight_bg: Color::Rgb(68, 71, 90),
    highlight_fg: Color::Rgb(255, 255, 255),
    border: Color::Rgb(98, 114, 164),
    title: Color::Rgb(139, 233, 253),
    error: Color::Rgb(255, 85, 85),
    warning: Color::Rgb(241, 250, 140),
    success: Color::Rgb(80, 250, 123),
    ttl_ok: Color::Rgb(80, 250, 123),
    ttl_warn: Color::Rgb(241, 250, 140),
    ttl_crit: Color::Rgb(255, 85, 85),
    memory: Color::Rgb(255, 121, 198),
    prompt: Color::Rgb(241, 250, 140),
    type_string: Color::Rgb(80, 250, 123),
    type_list: Color::Rgb(241, 250, 140),
    type_hash: Color::Rgb(255, 121, 198),
    type_set: Color::Rgb(139, 233, 253),
    type_zset: Color::Rgb(189, 147, 249),
    type_stream: Color::Rgb(255, 184, 108),
};

pub const NORD: Theme = Theme {
    name: "nord",
    bg: Color::Rgb(46, 52, 64),
    fg: Color::Rgb(216, 222, 233),
    highlight_bg: Color::Rgb(59, 66, 82),
    highlight_fg: Color::Rgb(236, 239, 244),
    border: Color::Rgb(76, 86, 106),
    title: Color::Rgb(136, 192, 208),
    error: Color::Rgb(191, 97, 106),
    warning: Color::Rgb(235, 203, 139),
    success: Color::Rgb(163, 190, 140),
    ttl_ok: Color::Rgb(163, 190, 140),
    ttl_warn: Color::Rgb(235, 203, 139),
    ttl_crit: Color::Rgb(191, 97, 106),
    memory: Color::Rgb(180, 142, 173),
    prompt: Color::Rgb(235, 203, 139),
    type_string: Color::Rgb(163, 190, 140),
    type_list: Color::Rgb(235, 203, 139),
    type_hash: Color::Rgb(180, 142, 173),
    type_set: Color::Rgb(136, 192, 208),
    type_zset: Color::Rgb(129, 161, 193),
    type_stream: Color::Rgb(208, 135, 112),
};

pub const SOLARIZED_DARK: Theme = Theme {
    name: "solarized_dark",
    bg: Color::Rgb(0, 43, 54),
    fg: Color::Rgb(131, 148, 150),
    highlight_bg: Color::Rgb(7, 54, 66),
    highlight_fg: Color::Rgb(147, 161, 161),
    border: Color::Rgb(88, 110, 117),
    title: Color::Rgb(42, 161, 152),
    error: Color::Rgb(220, 50, 47),
    warning: Color::Rgb(181, 137, 0),
    success: Color::Rgb(133, 153, 0),
    ttl_ok: Color::Rgb(133, 153, 0),
    ttl_warn: Color::Rgb(181, 137, 0),
    ttl_crit: Color::Rgb(220, 50, 47),
    memory: Color::Rgb(211, 54, 130),
    prompt: Color::Rgb(181, 137, 0),
    type_string: Color::Rgb(133, 153, 0),
    type_list: Color::Rgb(181, 137, 0),
    type_hash: Color::Rgb(211, 54, 130),
    type_set: Color::Rgb(42, 161, 152),
    type_zset: Color::Rgb(38, 139, 210),
    type_stream: Color::Rgb(203, 75, 22),
};

pub const GRUVBOX_DARK: Theme = Theme {
    name: "gruvbox_dark",
    bg: Color::Rgb(40, 40, 40),
    fg: Color::Rgb(235, 219, 178),
    highlight_bg: Color::Rgb(60, 56, 54),
    highlight_fg: Color::Rgb(251, 241, 199),
    border: Color::Rgb(102, 92, 84),
    title: Color::Rgb(104, 157, 106),
    error: Color::Rgb(204, 36, 29),
    warning: Color::Rgb(215, 153, 33),
    success: Color::Rgb(152, 151, 26),
    ttl_ok: Color::Rgb(152, 151, 26),
    ttl_warn: Color::Rgb(215, 153, 33),
    ttl_crit: Color::Rgb(204, 36, 29),
    memory: Color::Rgb(177, 98, 134),
    prompt: Color::Rgb(215, 153, 33),
    type_string: Color::Rgb(152, 151, 26),
    type_list: Color::Rgb(215, 153, 33),
    type_hash: Color::Rgb(177, 98, 134),
    type_set: Color::Rgb(104, 157, 106),
    type_zset: Color::Rgb(69, 133, 136),
    type_stream: Color::Rgb(214, 93, 14),
};

pub static THEMES: &[Theme] = &[DEFAULT, DRACULA, NORD, SOLARIZED_DARK, GRUVBOX_DARK];

impl Theme {
    pub fn by_name(name: &str) -> Option<&'static Theme> {
        THEMES.iter().find(|t| t.name == name)
    }

    /// Case-insensitive lookup, so `--theme NORD` works.
    pub fn by_name_ci(name: &str) -> Option<&'static Theme> {
        THEMES
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
    }

    /// Names of every available theme, for error messages.
    pub fn names() -> Vec<&'static str> {
        THEMES.iter().map(|t| t.name).collect()
    }
}

/// Index of the active theme within [`THEMES`].
///
/// An index rather than a stored `Theme` so [`current`] can hand out a
/// `&'static Theme`, which is what the render sites want. Storing the theme
/// itself would need a lock on the read path, and the alternative — a
/// `OnceLock` that silently ignores every call after the first — makes the
/// theme impossible to switch or to test.
static CURRENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Install `theme` as the active theme.
///
/// Ignores a theme that is not in [`THEMES`] rather than panicking: the set is
/// fixed, and a caller passing something else is a bug that should not take
/// the process down mid-render.
pub fn set_current(theme: &Theme) {
    if let Some(index) = THEMES.iter().position(|t| t.name == theme.name) {
        CURRENT.store(index, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The active theme.
///
/// Defaults to [`DEFAULT`], so a widget can always render without checking
/// whether startup ran.
pub fn current() -> &'static Theme {
    let index = CURRENT.load(std::sync::atomic::Ordering::Relaxed);
    THEMES.get(index).unwrap_or(&DEFAULT)
}

/// Comma-separated list of theme names, for `--theme` error messages.
pub fn names_joined() -> String {
    Theme::names().join(", ")
}

/// Look up a theme by name, or explain what the valid names are.
///
/// Lives here rather than in `main` so it can be tested directly: driving the
/// validation through the binary would mean running the render benchmark once
/// per case, since `--help` and `--version` both short-circuit in clap before
/// any of this runs.
pub fn resolve(name: &str) -> Result<&'static Theme, String> {
    Theme::by_name_ci(name)
        .ok_or_else(|| format!("Unknown theme {name:?}. Available: {}", names_joined()))
}

/// Style for a panel's border.
///
/// Panel chrome is the most visible part of a theme, so it is worth styling
/// explicitly rather than leaving every `Block` on ratatui's default, which is
/// what made the themes look interchangeable.
pub fn border_style() -> ratatui::style::Style {
    ratatui::style::Style::default().fg(current().border)
}

/// Style for a panel's title text.
pub fn title_style() -> ratatui::style::Style {
    ratatui::style::Style::default().fg(current().title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the module: a named theme must be findable.
    #[test]
    fn every_theme_is_reachable_by_name() {
        for theme in THEMES {
            assert_eq!(
                Theme::by_name(theme.name).map(|t| t.name),
                Some(theme.name),
                "{} is in THEMES but not findable",
                theme.name
            );
        }
    }

    #[test]
    fn by_name_ci_ignores_case_and_surrounding_space() {
        assert_eq!(Theme::by_name_ci("NORD").map(|t| t.name), Some("nord"));
        assert_eq!(Theme::by_name_ci("  Nord  ").map(|t| t.name), Some("nord"));
        assert!(Theme::by_name_ci("nope").is_none());
    }

    #[test]
    fn names_joined_lists_every_theme() {
        let joined = names_joined();
        for theme in THEMES {
            assert!(
                joined.contains(theme.name),
                "{} missing from {joined:?}",
                theme.name
            );
        }
    }

    /// With no theme installed, rendering must still work.
    #[test]
    fn current_falls_back_to_default() {
        // The atomic starts at 0, which is DEFAULT's index.
        assert_eq!(current().name, DEFAULT.name);
    }

    /// A theme must actually take effect, not be silently ignored.
    #[test]
    fn set_current_replaces_the_previous_theme() {
        for theme in THEMES {
            set_current(theme);
            assert_eq!(
                current().name,
                theme.name,
                "set_current did not take effect for {}",
                theme.name
            );
        }
        set_current(&DEFAULT);
    }

    #[test]
    fn set_current_ignores_an_unknown_theme_instead_of_panicking() {
        set_current(&DRACULA);
        let alien = Theme {
            name: "not-in-the-list",
            ..DEFAULT.clone()
        };
        set_current(&alien);
        assert_eq!(
            current().name,
            DRACULA.name,
            "an unknown theme must leave the current one in place"
        );
        set_current(&DEFAULT);
    }

    /// Every field must be set on every theme; a missing one would render as
    /// `Reset` and quietly fall back to the terminal default.
    #[test]
    fn no_theme_field_is_left_unset() {
        for theme in THEMES {
            let fields: [(&str, ratatui::style::Color); 21] = [
                ("bg", theme.bg),
                ("fg", theme.fg),
                ("highlight_bg", theme.highlight_bg),
                ("highlight_fg", theme.highlight_fg),
                ("border", theme.border),
                ("title", theme.title),
                ("error", theme.error),
                ("warning", theme.warning),
                ("success", theme.success),
                ("ttl_ok", theme.ttl_ok),
                ("ttl_warn", theme.ttl_warn),
                ("ttl_crit", theme.ttl_crit),
                ("memory", theme.memory),
                ("prompt", theme.prompt),
                ("type_string", theme.type_string),
                ("type_list", theme.type_list),
                ("type_hash", theme.type_hash),
                ("type_set", theme.type_set),
                ("type_zset", theme.type_zset),
                ("type_stream", theme.type_stream),
                ("name_is_not_a_color", theme.title),
            ];
            for (field, color) in fields {
                assert_ne!(
                    color,
                    ratatui::style::Color::Reset,
                    "{}.{} is unset",
                    theme.name,
                    field
                );
            }
        }
    }

    /// Distinct themes must actually differ, or selecting one is meaningless.
    #[test]
    fn themes_are_visually_distinguishable() {
        let default = Theme::by_name("default").unwrap();
        for theme in THEMES {
            if theme.name == "default" {
                continue;
            }
            assert_ne!(
                theme.bg, default.bg,
                "{} shares the default background",
                theme.name
            );
            assert_ne!(
                theme.title, default.title,
                "{} shares the default title colour",
                theme.name
            );
        }
    }

    #[test]
    fn set_current_is_idempotent() {
        set_current(&DEFAULT);
        set_current(&DEFAULT);
        assert_eq!(current().name, DEFAULT.name);
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    #[test]
    fn resolve_accepts_every_shipped_theme() {
        for theme in THEMES {
            let resolved = resolve(theme.name).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(resolved.name, theme.name);
        }
    }

    #[test]
    fn resolve_accepts_any_casing_and_padding() {
        assert_eq!(resolve("NORD").unwrap().name, "nord");
        assert_eq!(resolve("  Nord ").unwrap().name, "nord");
    }

    /// An unknown theme must fail loudly. The previous behaviour was to fall
    /// back silently, so `--theme nord` and `--theme typo` looked identical.
    #[test]
    fn resolve_rejects_an_unknown_name() {
        let err = resolve("solarized").unwrap_err();
        assert!(err.contains("Unknown theme"), "got: {err}");
        assert!(
            err.contains("\"solarized\""),
            "the error should quote what was typed: {err}"
        );
    }

    /// The error must list the real names, so a user can fix the typo without
    /// consulting the docs.
    #[test]
    fn resolve_error_lists_every_valid_name() {
        let err = resolve("nope").unwrap_err();
        for theme in THEMES {
            assert!(
                err.contains(theme.name),
                "{} missing from the error: {err}",
                theme.name
            );
        }
    }

    /// The short names the old `--help` text advertised must not resolve.
    #[test]
    fn the_previously_advertised_short_names_do_not_resolve() {
        for bogus in ["solarized", "gruvbox"] {
            assert!(
                resolve(bogus).is_err(),
                "{bogus} should not be a valid theme name"
            );
        }
    }
}
