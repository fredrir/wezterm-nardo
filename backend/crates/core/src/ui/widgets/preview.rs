use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::Widget;

use crate::ui::fill;
use crate::ui::theme::Theme;

const SEPARATOR: &str = "│";
const LOADING: &str = "loading…";
const EMPTY: &str = "no preview";

/// Pane content panel. `scroll` counts lines from the bottom (0 = tail visible).
pub struct Preview<'a> {
    pub theme: &'a Theme,
    pub title: &'a str,
    pub text: Option<&'a Text<'a>>,
    pub loading: bool,
    pub scroll: u16,
}

impl Widget for Preview<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        fill(buf, area, theme.base());
        for y in area.top()..area.bottom() {
            buf[(area.x, y)].set_symbol(SEPARATOR).set_fg(theme.border);
        }
        let inner_x = area.x.saturating_add(2).min(area.right());
        let inner = Rect { x: inner_x, width: area.right() - inner_x, ..area };
        if inner.is_empty() {
            return;
        }
        Line::styled(self.title, theme.dim()).render(Rect { height: 1, ..inner }, buf);
        let body = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };
        if body.is_empty() {
            return;
        }
        let text = match (self.loading, self.text) {
            (false, Some(text)) => text,
            (loading, _) => {
                let status = if loading { LOADING } else { EMPTY };
                let middle = Rect { y: body.y + body.height / 2, height: 1, ..body };
                Line::styled(status, theme.dim()).centered().render(middle, buf);
                return;
            }
        };
        let lines = &text.lines;
        let height = usize::from(body.height);
        let end = lines.len().saturating_sub(usize::from(self.scroll)).max(height.min(lines.len()));
        let start = end.saturating_sub(height);
        for (line, row) in lines[start..end].iter().zip(body.rows()) {
            line.render(row, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    fn text(n: usize) -> Text<'static> {
        Text::from((0..n).map(|i| Line::from(format!("line {i}"))).collect::<Vec<_>>())
    }

    fn preview<'a>(theme: &'a Theme, text: Option<&'a Text<'a>>, loading: bool, scroll: u16) -> Preview<'a> {
        Preview { theme, title: "pane 51 · vim", text, loading, scroll }
    }

    #[test]
    fn tail_aligned_with_scroll() {
        let theme = Theme::dark();
        let text = text(10);
        let area = Rect::new(0, 0, 14, 4);
        let mut buf = buffer(area);
        preview(&theme, Some(&text), false, 0).render(area, &mut buf);
        assert_eq!(row(&buf, 0), "│ pane 51 · vi");
        assert_eq!(row(&buf, 1), "│ line 7      ");
        assert_eq!(row(&buf, 3), "│ line 9      ");
        assert_eq!(buf[(0, 0)].fg, theme.border);
        assert_eq!(buf[(2, 0)].fg, theme.text_dim);
        assert_eq!(buf[(2, 1)].fg, theme.text);
        assert_eq!(buf[(13, 3)].bg, theme.surface);

        let mut buf = buffer(area);
        preview(&theme, Some(&text), false, 2).render(area, &mut buf);
        assert_eq!(row(&buf, 1), "│ line 5      ");
        assert_eq!(row(&buf, 3), "│ line 7      ");

        let mut buf = buffer(area);
        preview(&theme, Some(&text), false, 200).render(area, &mut buf);
        assert_eq!(row(&buf, 1), "│ line 0      ");
        assert_eq!(row(&buf, 3), "│ line 2      ");

        let short = self::text(2);
        let mut buf = buffer(area);
        preview(&theme, Some(&short), false, 0).render(area, &mut buf);
        assert_eq!(row(&buf, 1), "│ line 0      ");
        assert_eq!(row(&buf, 2), "│ line 1      ");
        assert_eq!(row(&buf, 3), "│             ");
    }

    #[test]
    fn states_loading_and_empty() {
        let theme = Theme::dark();
        let text = text(3);
        let area = Rect::new(0, 0, 16, 5);
        let mut buf = buffer(area);
        preview(&theme, Some(&text), true, 0).render(area, &mut buf);
        assert_eq!(row(&buf, 3), "│    loading…   ");
        assert_eq!(buf[(5, 3)].fg, theme.text_dim);
        let mut buf = buffer(area);
        preview(&theme, None, false, 0).render(area, &mut buf);
        assert_eq!(row(&buf, 3), "│   no preview  ");
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        let text = text(3);
        for area in tiny_areas() {
            let mut buf = buffer(area);
            preview(&theme, Some(&text), false, 0).render(area, &mut buf);
            preview(&theme, None, true, 3).render(area, &mut buf);
            preview(&theme, Some(&text), false, 0).render(Rect::new(0, 0, 30, 30), &mut buf);
        }
    }
}
