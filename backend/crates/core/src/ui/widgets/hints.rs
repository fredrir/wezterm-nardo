use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::ui::fill;
use crate::ui::theme::Theme;

const GAP: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub key: String,
    pub label: String,
}

impl Hint {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self { key: key.into(), label: label.into() }
    }
}

/// Footer: `↵ switch   ⇧D kill   ^N new   esc close`, drops hints from the right when narrow.
pub struct Hints<'a> {
    pub theme: &'a Theme,
    pub hints: &'a [Hint],
}

impl Widget for Hints<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        fill(buf, area, theme.base());
        let badge = Style::new().fg(theme.text).bg(theme.surface_hi);
        let label = theme.muted();
        let y = area.y;
        let mut x = usize::from(area.x);
        let right = usize::from(area.right());
        for hint in self.hints {
            let key_width = hint.key.width() + 2;
            let label_width = hint.label.width();
            if x + key_width + 1 + label_width > right {
                break;
            }
            let x0 = x as u16;
            buf.set_string(x0, y, " ", badge);
            buf.set_string(x0 + 1, y, &hint.key, badge);
            buf.set_string(x0 + 1 + hint.key.width() as u16, y, " ", badge);
            x += key_width + 1;
            buf.set_stringn(x as u16, y, &hint.label, label_width, label);
            x += label_width + GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    fn hints() -> Vec<Hint> {
        vec![Hint::new("↵", "switch"), Hint::new("⇧D", "kill"), Hint::new("esc", "close")]
    }

    #[test]
    fn renders_badges_and_labels() {
        let theme = Theme::dark();
        let hints = hints();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = buffer(area);
        Hints { theme: &theme, hints: &hints }.render(area, &mut buf);
        assert_eq!(row(&buf, 0), " ↵  switch    ⇧D  kill    esc  close    ");
        assert_eq!(buf[(0, 0)].bg, theme.surface_hi);
        assert_eq!(buf[(1, 0)].fg, theme.text);
        assert_eq!(buf[(4, 0)].fg, theme.text_muted);
        assert_eq!(buf[(4, 0)].bg, theme.surface);
        assert_eq!(buf[(39, 0)].bg, theme.surface);
    }

    #[test]
    fn drops_trailing_hints_when_narrow() {
        let theme = Theme::dark();
        let hints = hints();
        let area = Rect::new(0, 0, 22, 1);
        let mut buf = buffer(area);
        Hints { theme: &theme, hints: &hints }.render(area, &mut buf);
        assert_eq!(row(&buf, 0), " ↵  switch    ⇧D  kill");
        let area = Rect::new(0, 0, 8, 1);
        let mut buf = buffer(area);
        Hints { theme: &theme, hints: &hints }.render(area, &mut buf);
        assert_eq!(row(&buf, 0), "        ");
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        let hints = hints();
        for area in tiny_areas() {
            let mut buf = buffer(area);
            Hints { theme: &theme, hints: &hints }.render(area, &mut buf);
            Hints { theme: &theme, hints: &[] }.render(area, &mut buf);
        }
    }
}
