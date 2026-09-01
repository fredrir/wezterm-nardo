use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::ui::fill;
use crate::ui::theme::Theme;

const GAP: u16 = 1;

/// Horizontal segmented control (scopes / filters). Returns hitboxes via `hitboxes`.
pub struct Chips<'a> {
    pub theme: &'a Theme,
    pub labels: &'a [&'a str],
    pub selected: usize,
}

impl Chips<'_> {
    /// One rect per label in order; labels that do not fit get an empty rect.
    pub fn hitboxes(&self, area: Rect) -> Vec<Rect> {
        let mut x = area.x;
        self.labels
            .iter()
            .map(|label| {
                let width = pill_width(label);
                let pill = Rect::new(x, area.y, width, area.height.min(1)).intersection(area);
                x = x.saturating_add(width + GAP);
                pill
            })
            .collect()
    }
}

fn pill_width(label: &str) -> u16 {
    u16::try_from(label.width() + 2).unwrap_or(u16::MAX)
}

impl Widget for Chips<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        fill(buf, area, theme.base());
        let active = Style::new().fg(theme.accent_fg).bg(theme.accent).add_modifier(Modifier::BOLD);
        let idle = Style::new().fg(theme.text_muted).bg(theme.surface_hi);
        for (index, (label, pill)) in self.labels.iter().zip(self.hitboxes(area)).enumerate() {
            if pill.is_empty() {
                break;
            }
            let style = if index == self.selected { active } else { idle };
            buf.set_style(pill, style);
            if pill.width > 1 {
                buf.set_stringn(pill.x + 1, pill.y, label, usize::from(pill.width - 2), style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    const LABELS: [&str; 3] = ["All", "Windows", "Tabs"];

    #[test]
    fn hitboxes_follow_labels() {
        let theme = Theme::dark();
        let chips = Chips { theme: &theme, labels: &LABELS, selected: 0 };
        let boxes = chips.hitboxes(Rect::new(2, 1, 40, 1));
        assert_eq!(boxes, vec![Rect::new(2, 1, 5, 1), Rect::new(8, 1, 9, 1), Rect::new(18, 1, 6, 1)]);
        let clipped = chips.hitboxes(Rect::new(0, 0, 10, 1));
        assert_eq!(clipped[..2], [Rect::new(0, 0, 5, 1), Rect::new(6, 0, 4, 1)]);
        assert!(clipped[2].is_empty());
        assert!(!clipped[2].contains(ratatui::layout::Position::new(16, 0)));
        assert_eq!(chips.hitboxes(Rect::ZERO).len(), 3);
    }

    #[test]
    fn renders_pills_with_selected_accent() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = buffer(area);
        Chips { theme: &theme, labels: &LABELS, selected: 1 }.render(area, &mut buf);
        assert_eq!(row(&buf, 0), " All   Windows   Tabs         ");
        assert_eq!(buf[(0, 0)].bg, theme.surface_hi);
        assert_eq!(buf[(6, 0)].bg, theme.accent);
        assert_eq!(buf[(7, 0)].fg, theme.accent_fg);
        assert_eq!(buf[(16, 0)].bg, theme.surface_hi);
        assert_eq!(buf[(29, 0)].bg, theme.surface);
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        for area in tiny_areas() {
            let mut buf = buffer(area);
            Chips { theme: &theme, labels: &LABELS, selected: 9 }.render(area, &mut buf);
            Chips { theme: &theme, labels: &[], selected: 0 }.render(area, &mut buf);
        }
    }
}
