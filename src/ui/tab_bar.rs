use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Keys,
    Repl,
    Info,
    PubSub,
}

/// Labels rendered in the tab bar, in display order.
///
/// The separator is two spaces, so a tab's hit region spans its label plus
/// the padding and the gap that follows it.
const TAB_LABELS: [(Tab, &str); 4] = [
    (Tab::Keys, "[1:Keys]"),
    (Tab::Repl, "[2:REPL]"),
    (Tab::Info, "[3:Info]"),
    (Tab::PubSub, "[4:PubSub]"),
];

const TAB_SEPARATOR: &str = "  ";

/// Horizontal extent of each tab within `area`, in tab-bar columns.
///
/// Derived from the same labels `render_tab_bar` uses, so a click maps to the
/// tab the user actually sees rather than to hardcoded column thresholds.
pub fn tab_hit_regions(area: Rect) -> Vec<(Tab, ratatui::layout::Rect)> {
    let mut regions = Vec::with_capacity(TAB_LABELS.len());
    let mut x = area.x;
    for (tab, label) in TAB_LABELS {
        let mut width = label.chars().count() as u16;
        // Active tabs render as ">label<", one column wider than the label.
        width += 2;
        regions.push((
            tab,
            Rect {
                x,
                y: area.y,
                width,
                height: area.height,
            },
        ));
        x += width + TAB_SEPARATOR.chars().count() as u16;
    }
    regions
}

/// The tab whose hit region contains `(column, row)`, if any.
pub fn tab_at(area: Rect, column: u16, row: u16) -> Option<Tab> {
    if row < area.y || row >= area.y + area.height {
        return None;
    }
    tab_hit_regions(area)
        .into_iter()
        .find(|(_, rect)| column >= rect.x && column < rect.x + rect.width)
        .map(|(tab, _)| tab)
}

pub fn render_tab_bar(frame: &mut Frame, area: Rect, active: Tab) {
    let text: String = TAB_LABELS
        .iter()
        .map(|(t, label)| {
            if *t == active {
                format!(">{}<", label)
            } else {
                format!(" {} ", label)
            }
        })
        .collect::<Vec<_>>()
        .join(TAB_SEPARATOR);

    let style = Style::default().fg(Color::White).bg(Color::Black);
    let paragraph = Paragraph::new(text).style(style);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 1,
        }
    }

    #[test]
    fn four_regions_are_produced_in_order() {
        let regions = tab_hit_regions(bar());
        assert_eq!(regions.len(), 4);
        assert_eq!(
            regions.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
            vec![Tab::Keys, Tab::Repl, Tab::Info, Tab::PubSub]
        );
    }

    #[test]
    fn regions_do_not_overlap_and_increase() {
        let regions = tab_hit_regions(bar());
        for pair in regions.windows(2) {
            assert!(
                pair[0].1.x + pair[0].1.width <= pair[1].1.x,
                "regions overlap: {:?} {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn regions_fit_within_the_bar() {
        let regions = tab_hit_regions(bar());
        let last = regions.last().unwrap().1;
        assert!(last.x + last.width <= bar().width);
    }

    #[test]
    fn click_inside_a_label_selects_that_tab() {
        let regions = tab_hit_regions(bar());
        for (tab, rect) in regions {
            assert_eq!(tab_at(bar(), rect.x, 0), Some(tab));
            assert_eq!(tab_at(bar(), rect.x + rect.width - 1, 0), Some(tab));
        }
    }

    #[test]
    fn click_in_the_separator_selects_nothing() {
        let regions = tab_hit_regions(bar());
        // Column just past the first tab is the gap before the second.
        let gap = regions[0].1.x + regions[0].1.width;
        assert_eq!(tab_at(bar(), gap, 0), None);
    }

    #[test]
    fn click_outside_the_bar_selects_nothing() {
        let bar = bar();
        assert_eq!(tab_at(bar, 0, 5), None, "wrong row");
        assert_eq!(tab_at(bar, 0, 200), None);
    }

    #[test]
    fn click_past_the_last_tab_selects_nothing() {
        let regions = tab_hit_regions(bar());
        let after = regions[3].1.x + regions[3].1.width;
        assert_eq!(tab_at(bar(), after, 0), None);
    }

    #[test]
    fn bar_offset_shifts_regions() {
        let bar = Rect {
            x: 10,
            y: 3,
            width: 80,
            height: 1,
        };
        let regions = tab_hit_regions(bar);
        assert_eq!(regions[0].1.x, 10);
        assert_eq!(tab_at(bar, 10, 3), Some(Tab::Keys));
        assert_eq!(tab_at(bar, 10, 2), None);
    }
}
