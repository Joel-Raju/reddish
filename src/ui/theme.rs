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
}
