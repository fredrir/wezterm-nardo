use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListState, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget};

use crate::ui::fill;
use crate::ui::theme::Theme;

const SCROLL_PADDING: usize = 2;
const MARKER: &str = "▎";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Non-selectable group header (domain / window / tab).
    Header,
    Item,
}

/// One row: pre-styled columns, match indices already applied by the app.
#[derive(Debug, Clone)]
pub struct ListRow<'a> {
    pub kind: RowKind,
    pub indent: u16,
    pub line: Line<'a>,
    /// Right-aligned column (ids, badges, counts).
    pub trailing: Option<Line<'a>>,
    pub style: Style,
}

pub struct FuzzyList<'a> {
    pub theme: &'a Theme,
    pub rows: Vec<ListRow<'a>>,
    pub empty_text: &'a str,
    pub scrollbar: bool,
}

impl StatefulWidget for FuzzyList<'_> {
    type State = ListState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = area.intersection(buf.area);
        let theme = self.theme;
        fill(buf, area, theme.base());
        if area.is_empty() {
            return;
        }
        if self.rows.is_empty() {
            state.select(None);
            *state.offset_mut() = 0;
            let middle = Rect { y: area.y + area.height / 2, height: 1, ..area };
            Line::styled(self.empty_text, theme.dim()).centered().render(middle, buf);
            return;
        }

        let len = self.rows.len();
        let height = usize::from(area.height);
        let selected = state.selected().map(|s| s.min(len - 1));
        state.select(selected);
        let first = window_start(len, height, state.offset(), selected);
        *state.offset_mut() = first;

        let overflow = self.scrollbar && len > height;
        let content_width = if overflow { area.width - 1 } else { area.width };
        let last = (first + height).min(len);
        for (i, row) in self.rows[first..last].iter().enumerate() {
            let row_area = Rect { x: area.x, y: area.y + i as u16, width: content_width, height: 1 };
            render_row(row, row_area, buf, theme, selected == Some(first + i));
        }
        if overflow {
            let bar = Rect { x: area.right() - 1, width: 1, ..area };
            let mut bar_state = ScrollbarState::new(len - height + 1).position(first).viewport_content_length(height);
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("┃")
                .track_style(Style::new().fg(theme.border).bg(theme.surface))
                .thumb_style(theme.muted())
                .render(bar, buf, &mut bar_state);
        }
    }
}

/// First visible row so that `selected` stays `SCROLL_PADDING` rows away from the edges.
fn window_start(len: usize, height: usize, offset: usize, selected: Option<usize>) -> usize {
    if len == 0 || height == 0 {
        return 0;
    }
    let max_start = len.saturating_sub(height);
    let mut first = offset.min(max_start);
    if let Some(selected) = selected {
        let pad = SCROLL_PADDING.min((height - 1) / 2);
        let low = selected.saturating_sub(pad);
        let high = (selected + pad).min(len - 1);
        if high >= first + height {
            first = high + 1 - height;
        }
        if low < first {
            first = low;
        }
    }
    first.min(max_start)
}

fn render_row(row: &ListRow, area: Rect, buf: &mut Buffer, theme: &Theme, selected: bool) {
    if area.is_empty() {
        return;
    }
    let header = row.kind == RowKind::Header;
    let base = if header { theme.dim() } else { theme.base() };
    buf.set_style(area, base.patch(row.style));
    let selected = selected && !header;
    if selected {
        buf.set_style(area, theme.selected());
        buf[(area.x, area.y)].set_symbol(MARKER).set_fg(theme.accent);
    }

    let text_x = area.x.saturating_add(1 + row.indent).min(area.right());
    let mut text = Rect { x: text_x, width: area.right() - text_x, ..area };
    if let Some(trailing) = &row.trailing {
        let width = u16::try_from(trailing.width()).unwrap_or(u16::MAX);
        if width > 0 && width + 2 <= text.width {
            let x = text.right() - width;
            trailing.render(Rect { x, width, ..text }, buf);
            text.width = x - 1 - text.x;
        }
    }
    (&row.line).render(text, buf);
    if selected {
        buf.set_style(area, Style::new().bg(theme.selection_bg));
    }
}

/// Screen rect of row `index` under the current offset, if it is visible in `area`.
pub fn row_rect(state: &ListState, area: Rect, index: usize) -> Option<Rect> {
    let visible = index.checked_sub(state.offset())?;
    let y = area.y.checked_add(u16::try_from(visible).ok()?)?;
    (y < area.bottom()).then_some(Rect { y, height: 1, ..area })
}

/// Selection movement that skips headers and wraps.
pub trait ListStateExt {
    fn select_next_item(&mut self, kinds: &[RowKind]);
    fn select_prev_item(&mut self, kinds: &[RowKind]);
    fn select_first_item(&mut self, kinds: &[RowKind]);
    fn select_last_item(&mut self, kinds: &[RowKind]);
    fn select_page(&mut self, kinds: &[RowKind], delta: i32);
    /// Row under `y` for `area` given the current offset, headers excluded.
    fn row_at(&self, area: Rect, y: u16, kinds: &[RowKind]) -> Option<usize>;
}

fn items(kinds: &[RowKind]) -> impl DoubleEndedIterator<Item = usize> + '_ {
    kinds.iter().enumerate().filter(|(_, kind)| **kind == RowKind::Item).map(|(i, _)| i)
}

impl ListStateExt for ListState {
    fn select_next_item(&mut self, kinds: &[RowKind]) {
        let next = match self.selected() {
            Some(current) => items(kinds).find(|&i| i > current).or_else(|| items(kinds).next()),
            None => items(kinds).next(),
        };
        self.select(next);
    }

    fn select_prev_item(&mut self, kinds: &[RowKind]) {
        let prev = match self.selected() {
            Some(current) => items(kinds).rev().find(|&i| i < current).or_else(|| items(kinds).next_back()),
            None => items(kinds).next_back(),
        };
        self.select(prev);
    }

    fn select_first_item(&mut self, kinds: &[RowKind]) {
        self.select(items(kinds).next());
    }

    fn select_last_item(&mut self, kinds: &[RowKind]) {
        self.select(items(kinds).next_back());
    }

    fn select_page(&mut self, kinds: &[RowKind], delta: i32) {
        if delta == 0 {
            return;
        }
        let steps = delta.unsigned_abs() as usize - 1;
        let target = match (self.selected(), delta > 0) {
            (Some(current), true) => {
                items(kinds).filter(|&i| i > current).nth(steps).or_else(|| items(kinds).next_back())
            }
            (Some(current), false) => {
                items(kinds).rev().filter(|&i| i < current).nth(steps).or_else(|| items(kinds).next())
            }
            (None, true) => items(kinds).next(),
            (None, false) => items(kinds).next_back(),
        };
        self.select(target);
    }

    fn row_at(&self, area: Rect, y: u16, kinds: &[RowKind]) -> Option<usize> {
        if y < area.y || y >= area.bottom() {
            return None;
        }
        let index = self.offset() + usize::from(y - area.y);
        (kinds.get(index) == Some(&RowKind::Item)).then_some(index)
    }
}

/// Applies `theme.matched()` to the chars at `indices` of `text`.
pub fn highlight<'a>(text: &'a str, indices: &[u32], base: Style, theme: &Theme) -> Line<'a> {
    if indices.is_empty() {
        return Line::from(Span::styled(text, base));
    }
    let sorted: Cow<[u32]> = if indices.is_sorted() {
        Cow::Borrowed(indices)
    } else {
        let mut owned = indices.to_vec();
        owned.sort_unstable();
        Cow::Owned(owned)
    };
    let matched = base.patch(theme.matched());
    let style_for = |hit: bool| if hit { matched } else { base };
    let mut spans = Vec::new();
    let mut start = 0;
    let mut in_match = false;
    for (i, (byte, _)) in text.char_indices().enumerate() {
        let hit = sorted.binary_search(&(i as u32)).is_ok();
        if hit != in_match && byte > start {
            spans.push(Span::styled(&text[start..byte], style_for(in_match)));
            start = byte;
        }
        in_match = hit;
    }
    if start < text.len() {
        spans.push(Span::styled(&text[start..], style_for(in_match)));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    const H: RowKind = RowKind::Header;
    const I: RowKind = RowKind::Item;

    fn rows<'a>(theme: &Theme, n: usize) -> Vec<ListRow<'a>> {
        (0..n)
            .map(|i| ListRow {
                kind: if i % 4 == 0 { H } else { I },
                indent: if i % 4 == 0 { 0 } else { 1 },
                line: Line::from(format!("row {i}")),
                trailing: (i % 4 != 0).then(|| Line::styled(format!("#{i}"), theme.muted())),
                style: Style::default(),
            })
            .collect()
    }

    fn list<'a>(theme: &'a Theme, rows: Vec<ListRow<'a>>) -> FuzzyList<'a> {
        FuzzyList { theme, rows, empty_text: "nothing here", scrollbar: true }
    }

    #[test]
    fn next_prev_skip_headers_and_wrap() {
        let kinds = [H, I, I, H, I];
        let mut state = ListState::default();
        state.select_next_item(&kinds);
        assert_eq!(state.selected(), Some(1));
        state.select_next_item(&kinds);
        state.select_next_item(&kinds);
        assert_eq!(state.selected(), Some(4));
        state.select_next_item(&kinds);
        assert_eq!(state.selected(), Some(1));
        state.select_prev_item(&kinds);
        assert_eq!(state.selected(), Some(4));
        state.select_prev_item(&kinds);
        assert_eq!(state.selected(), Some(2));
        state.select_first_item(&kinds);
        assert_eq!(state.selected(), Some(1));
        state.select_last_item(&kinds);
        assert_eq!(state.selected(), Some(4));
        let mut empty = ListState::default().with_selected(Some(3));
        empty.select_next_item(&[H, H]);
        assert_eq!(empty.selected(), None);
        empty.select_prev_item(&[]);
        assert_eq!(empty.selected(), None);
    }

    #[test]
    fn page_moves_by_items_and_clamps() {
        let kinds = [H, I, I, H, I, I, I];
        let mut state = ListState::default();
        state.select_page(&kinds, 2);
        assert_eq!(state.selected(), Some(1));
        state.select_page(&kinds, 2);
        assert_eq!(state.selected(), Some(4));
        state.select_page(&kinds, 10);
        assert_eq!(state.selected(), Some(6));
        state.select_page(&kinds, -1);
        assert_eq!(state.selected(), Some(5));
        state.select_page(&kinds, -10);
        assert_eq!(state.selected(), Some(1));
        state.select_page(&kinds, 0);
        assert_eq!(state.selected(), Some(1));
        let mut state = ListState::default();
        state.select_page(&kinds, -3);
        assert_eq!(state.selected(), Some(6));
    }

    #[test]
    fn row_at_uses_offset_and_skips_headers() {
        let kinds = [H, I, I, H, I];
        let area = Rect::new(2, 5, 20, 3);
        let state = ListState::default().with_offset(1);
        assert_eq!(state.row_at(area, 5, &kinds), Some(1));
        assert_eq!(state.row_at(area, 6, &kinds), Some(2));
        assert_eq!(state.row_at(area, 7, &kinds), None);
        assert_eq!(state.row_at(area, 4, &kinds), None);
        assert_eq!(state.row_at(area, 8, &kinds), None);
        assert_eq!(ListState::default().with_offset(4).row_at(area, 6, &kinds), None);
        assert_eq!(row_rect(&state, area, 2), Some(Rect::new(2, 6, 20, 1)));
        assert_eq!(row_rect(&state, area, 0), None);
        assert_eq!(row_rect(&state, area, 4), None);
    }

    #[test]
    fn highlight_splits_spans() {
        let theme = Theme::dark();
        let base = theme.base();
        let line = highlight("héllo", &[3, 0, 3], base, &theme);
        let parts: Vec<(&str, bool)> = line
            .spans
            .iter()
            .map(|s| (s.content.as_ref(), s.style.add_modifier.contains(Modifier::UNDERLINED)))
            .collect();
        assert_eq!(parts, vec![("h", true), ("él", false), ("l", true), ("o", false)]);
        assert_eq!(line.spans[0].style.fg, Some(theme.match_hl));
        assert_eq!(line.spans[0].style.bg, Some(theme.surface));
        assert_eq!(line.spans[1].style, base);
        let plain = highlight("abc", &[], base, &theme);
        assert_eq!(plain.spans.len(), 1);
        assert_eq!(highlight("", &[0], base, &theme).spans.len(), 0);
        let all = highlight("ab", &[0, 1, 9], base, &theme);
        assert_eq!(all.spans.len(), 1);
        assert_eq!(all.spans[0].content, "ab");
    }

    #[test]
    fn window_keeps_padding_and_clamps() {
        assert_eq!(window_start(100, 10, 0, Some(0)), 0);
        assert_eq!(window_start(100, 10, 0, Some(50)), 43);
        assert_eq!(window_start(100, 10, 43, Some(51)), 44);
        assert_eq!(window_start(100, 10, 44, Some(45)), 43);
        assert_eq!(window_start(100, 10, 0, Some(99)), 90);
        assert_eq!(window_start(100, 10, 95, None), 90);
        assert_eq!(window_start(5, 10, 3, Some(4)), 0);
        assert_eq!(window_start(100, 1, 0, Some(7)), 7);
        assert_eq!(window_start(0, 10, 3, Some(4)), 0);
        assert_eq!(window_start(10, 0, 3, Some(4)), 0);
    }

    #[test]
    fn renders_selection_bar_headers_and_trailing() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 16, 4);
        let mut buf = buffer(area);
        let mut state = ListState::default().with_selected(Some(2));
        list(&theme, rows(&theme, 3)).render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), " row 0          ");
        assert_eq!(row(&buf, 1), "  row 1       #1");
        assert_eq!(row(&buf, 2), "▎ row 2       #2");
        assert_eq!(row(&buf, 3), "                ");
        assert_eq!(buf[(1, 0)].fg, theme.text_dim);
        assert_eq!(buf[(0, 2)].fg, theme.accent);
        for x in 0..16 {
            assert_eq!(buf[(x, 2)].bg, theme.selection_bg, "x={x}");
            assert_eq!(buf[(x, 1)].bg, theme.surface, "x={x}");
        }
        assert_eq!(buf[(15, 2)].fg, theme.text_muted);
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn scrolls_to_selection_and_draws_scrollbar() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 12, 5);
        let mut buf = buffer(area);
        let mut state = ListState::default().with_selected(Some(50));
        list(&theme, rows(&theme, 100)).render(area, &mut buf, &mut state);
        assert_eq!(state.offset(), 48);
        assert_eq!(state.selected(), Some(50));
        assert!(row(&buf, 2).starts_with("▎ row 5"), "trailing #50 truncates the line at width 12");
        let bar: Vec<&str> = (0..5).map(|y| buf[(11, y)].symbol()).collect();
        assert!(bar.contains(&"┃"));
        assert!(bar.contains(&"│"));

        let mut state = ListState::default().with_selected(Some(500));
        list(&theme, rows(&theme, 100)).render(area, &mut buf, &mut state);
        assert_eq!(state.selected(), Some(99));
        assert_eq!(state.offset(), 95);
        assert_eq!(buf[(11, 4)].symbol(), "┃");

        let mut state = ListState::default();
        list(&theme, rows(&theme, 100)).render(area, &mut buf, &mut state);
        assert_eq!(buf[(11, 0)].symbol(), "┃");
        let mut plain = list(&theme, rows(&theme, 100));
        plain.scrollbar = false;
        plain.render(area, &mut buf, &mut state);
        assert_eq!(buf[(11, 0)].symbol(), " ");
    }

    #[test]
    fn empty_list_shows_placeholder() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = buffer(area);
        let mut state = ListState::default().with_selected(Some(3)).with_offset(9);
        list(&theme, vec![]).render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 1), "    nothing here    ");
        assert_eq!(buf[(4, 1)].fg, theme.text_dim);
        assert_eq!(state.selected(), None);
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        for area in tiny_areas() {
            for n in [0, 1, 9] {
                let mut buf = buffer(area);
                let mut state = ListState::default().with_selected(Some(5));
                list(&theme, rows(&theme, n)).render(area, &mut buf, &mut state);
                let mut buf = buffer(area);
                list(&theme, rows(&theme, n)).render(Rect::new(0, 0, 40, 40), &mut buf, &mut state);
            }
        }
    }
}
