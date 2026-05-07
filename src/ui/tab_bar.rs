use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Keys,
    Repl,
    Info,
    PubSub,
}

pub fn render_tab_bar(frame: &mut Frame, area: Rect, active: Tab) {
    let tabs = [
        (Tab::Keys, "[1:Keys]"),
        (Tab::Repl, "[2:REPL]"),
        (Tab::Info, "[3:Info]"),
        (Tab::PubSub, "[4:PubSub]"),
    ];

    let text: String = tabs
        .iter()
        .map(|(t, label)| {
            if *t == active {
                format!(">{}<", label)
            } else {
                format!(" {} ", label)
            }
        })
        .collect::<Vec<_>>()
        .join("  ");

    let style = Style::default().fg(Color::White).bg(Color::Black);
    let paragraph = Paragraph::new(text).style(style);
    frame.render_widget(paragraph, area);
}
