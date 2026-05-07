use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct HelpOverlay {
    pub visible: bool,
    pub context: HelpContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HelpContext {
    KeyBrowser,
    ValueInspector,
    Repl,
    Stats,
    PubSub,
    Global,
}

impl HelpOverlay {
    pub fn new(context: HelpContext) -> Self {
        Self {
            visible: true,
            context,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(80, 80, area);
        frame.render_widget(Clear, popup_area);

        let title = format!("Keybindings — {:?}", self.context);
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let lines: Vec<(&str, &str)> = match self.context {
            HelpContext::KeyBrowser => vec![
                ("j / Down", "Navigate down"),
                ("k / Up", "Navigate up"),
                ("Enter", "Select / Expand"),
                ("d", "Delete key"),
                ("r", "Refresh"),
                ("/", "Filter"),
                ("?", "Toggle help"),
                ("q", "Quit"),
            ],
            _ => vec![
                ("Esc", "Close / Cancel"),
                ("?", "Toggle help"),
                ("q", "Quit"),
            ],
        };

        let text = lines
            .iter()
            .map(|(k, v)| format!("{:<20} {}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);
    }
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
