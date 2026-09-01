use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, Padding, Widget};

use crate::context::{Mode, Presentation};
use crate::ui::fill;
use crate::ui::theme::{Theme, blend, rgb_or};

const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 5;
const DEFAULT_WIDTH: f32 = 0.72;
const DEFAULT_HEIGHT: f32 = 0.7;

/// Where the launcher chrome goes. `overlay` → centred box sized by presentation; other modes
/// fill the area with a 1-cell margin (`window`/`split`/`tab` already gave us the exact space).
pub fn modal_area(area: Rect, presentation: &Presentation) -> Rect {
    if !is_overlay(presentation) {
        return area.inner(Margin::new(1, 0));
    }
    let width = extent(presentation.width, presentation.max_width, area.width, MIN_WIDTH, DEFAULT_WIDTH);
    let height = extent(presentation.height, presentation.max_height, area.height, MIN_HEIGHT, DEFAULT_HEIGHT);
    area.centered(Constraint::Length(width), Constraint::Length(height))
}

/// `want` ≤ 1 is a fraction of `available`, > 1 is cells; `cap` 0 = uncapped.
fn extent(want: f32, cap: u16, available: u16, min: u16, default: f32) -> u16 {
    let want = if want > 0.0 { want } else { default };
    let cells = if want <= 1.0 { f32::from(available) * want } else { want };
    let cells = cells.round().clamp(0.0, f32::from(u16::MAX)) as u16;
    let capped = if cap > 0 { cells.min(cap) } else { cells };
    capped.max(min).min(available)
}

pub fn is_overlay(presentation: &Presentation) -> bool {
    presentation.mode == Mode::Overlay
}

/// Origin pane content (`get-text --escapes`) drawn dimmed behind the modal.
pub struct Backdrop<'a> {
    pub text: &'a Text<'a>,
    pub theme: &'a Theme,
}

impl Widget for Backdrop<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        fill(buf, area, Style::new().fg(theme.text).bg(theme.bg));
        self.text.render(area, buf);
        for pos in area.positions() {
            let cell = &mut buf[pos];
            let (mut fg, mut bg) = (rgb_or(cell.fg, theme.text), rgb_or(cell.bg, theme.bg));
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            cell.modifier.remove(Modifier::REVERSED | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK);
            cell.fg = blend(fg, theme.bg, theme.backdrop_dim);
            cell.bg = blend(bg, theme.bg, theme.backdrop_dim);
        }
    }
}

/// Rounded frame with surface background; returns the inner area.
pub struct Chrome<'a> {
    pub theme: &'a Theme,
    pub title: Option<&'a str>,
    pub focused: bool,
}

impl Chrome<'_> {
    pub fn render(self, area: Rect, buf: &mut Buffer) -> Rect {
        let theme = self.theme;
        let border = if self.focused { theme.border_focus } else { theme.border };
        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(border).bg(theme.surface))
            .style(theme.base())
            .padding(Padding::horizontal(1));
        if let Some(title) = self.title {
            block = block.title_top(Line::styled(format!(" {title} "), theme.dim()));
        }
        let inner = block.inner(area);
        Clear.render(area, buf);
        block.render(area, buf);
        inner.intersection(buf.area)
    }
}

/// Tabs become 4 spaces; `Color::Reset` from SGR 0/39/49 becomes "inherit" so the panel keeps
/// its explicit surface colours. Unparseable input falls back to the escape-stripped plain text.
pub fn ansi_text(raw: &str) -> Text<'static> {
    if raw.is_empty() {
        return Text::default();
    }
    let expanded = raw.replace('\t', "    ");
    let parsed = expanded.as_bytes().into_text().ok().filter(|t| !t.lines.is_empty());
    let Some(mut text) = parsed else {
        return Text::raw(strip_escapes(&expanded));
    };
    for span in text.lines.iter_mut().flat_map(|l| l.spans.iter_mut()) {
        if span.style.fg == Some(Color::Reset) {
            span.style.fg = None;
        }
        if span.style.bg == Some(Color::Reset) {
            span.style.bg = None;
        }
    }
    text
}

fn strip_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.next() {
                Some('[') => {
                    for n in chars.by_ref() {
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(n) = chars.next() {
                        if n == '\x07' || (n == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\n' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use super::*;
    use crate::context::Backdrop as BackdropMode;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    fn presentation(mode: Mode, width: f32, height: f32) -> Presentation {
        Presentation {
            mode,
            width,
            height,
            max_width: 128,
            max_height: 42,
            backdrop: BackdropMode::Dim,
            animations: true,
        }
    }

    #[test]
    fn modal_area_fraction_is_centred() {
        let area = Rect::new(0, 0, 100, 40);
        let modal = modal_area(area, &Presentation::default());
        assert_eq!(modal, Rect::new(14, 6, 72, 28));
    }

    #[test]
    fn modal_area_cells_and_caps() {
        let area = Rect::new(0, 0, 200, 60);
        assert_eq!(modal_area(area, &presentation(Mode::Overlay, 30.0, 10.0)).width, 30);
        assert_eq!(modal_area(area, &presentation(Mode::Overlay, 30.0, 10.0)).height, 10);
        let capped = modal_area(area, &presentation(Mode::Overlay, 1.0, 1.0));
        assert_eq!((capped.width, capped.height), (128, 42));
        let mut uncapped = presentation(Mode::Overlay, 1.0, 1.0);
        uncapped.max_width = 0;
        uncapped.max_height = 0;
        assert_eq!(modal_area(area, &uncapped), area);
        let huge = modal_area(area, &presentation(Mode::Overlay, 999.0, 999.0));
        assert_eq!((huge.width, huge.height), (128, 42));
    }

    #[test]
    fn modal_area_never_exceeds_area() {
        for area in [Rect::new(0, 0, 10, 3), Rect::new(0, 0, 0, 0), Rect::new(5, 5, 1, 1)] {
            let modal = modal_area(area, &Presentation::default());
            assert_eq!(modal, modal.intersection(area));
            let bad = modal_area(area, &presentation(Mode::Overlay, 0.0, f32::NAN));
            assert_eq!(bad, bad.intersection(area));
        }
    }

    #[test]
    fn modal_area_other_modes_fill_with_margin() {
        let area = Rect::new(2, 3, 50, 20);
        assert_eq!(modal_area(area, &presentation(Mode::Tab, 0.5, 0.5)), Rect::new(3, 3, 48, 20));
        assert_eq!(modal_area(Rect::new(0, 0, 1, 1), &presentation(Mode::Window, 0.5, 0.5)), Rect::ZERO);
    }

    #[test]
    fn backdrop_dims_cells() {
        let theme = Theme::dark();
        let text = Text::from(vec![
            Line::from(vec![Span::raw("hi"), Span::styled("!", Style::new().fg(Color::Rgb(255, 0, 0)))]),
            Line::styled("rev", Style::new().add_modifier(Modifier::REVERSED)),
        ]);
        let area = Rect::new(0, 0, 6, 3);
        let mut buf = buffer(area);
        Backdrop { text: &text, theme: &theme }.render(area, &mut buf);

        assert_eq!(row(&buf, 0), "hi!   ");
        let plain = &buf[(0, 0)];
        assert_eq!(plain.fg, blend(theme.text, theme.bg, theme.backdrop_dim));
        assert_eq!(plain.bg, theme.bg);
        assert_ne!(plain.fg, theme.text);
        assert_eq!(buf[(2, 0)].fg, blend(Color::Rgb(255, 0, 0), theme.bg, theme.backdrop_dim));
        assert_eq!(buf[(5, 2)].bg, theme.bg);
        let reversed = &buf[(0, 1)];
        assert!(!reversed.modifier.contains(Modifier::REVERSED));
        assert_eq!(reversed.bg, blend(theme.text, theme.bg, theme.backdrop_dim));
    }

    #[test]
    fn backdrop_survives_tiny_areas() {
        let theme = Theme::dark();
        let text = Text::raw("line one\nline two\nline three");
        for area in tiny_areas() {
            let mut buf = buffer(area);
            Backdrop { text: &text, theme: &theme }.render(area, &mut buf);
            Backdrop { text: &text, theme: &theme }.render(Rect::new(0, 0, 50, 50), &mut buf);
        }
    }

    #[test]
    fn chrome_returns_padded_inner() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 20, 5);
        let mut buf = buffer(area);
        let inner = Chrome { theme: &theme, title: Some("sessions"), focused: true }.render(area, &mut buf);
        assert_eq!(inner, Rect::new(2, 1, 16, 3));
        assert_eq!(buf[(0, 0)].symbol(), "╭");
        assert_eq!(buf[(0, 0)].fg, theme.border_focus);
        assert_eq!(buf[(5, 5 - 1)].bg, theme.surface);
        assert!(row(&buf, 0).contains(" sessions "));
        let mut buf = buffer(area);
        Chrome { theme: &theme, title: None, focused: false }.render(area, &mut buf);
        assert_eq!(buf[(0, 0)].fg, theme.border);
        for area in tiny_areas() {
            let mut buf = buffer(area);
            let inner = Chrome { theme: &theme, title: Some("t"), focused: false }.render(area, &mut buf);
            assert_eq!(inner, inner.intersection(area));
        }
    }

    #[test]
    fn ansi_text_parses_and_normalises() {
        let text = ansi_text("a\tb\x1b[31mred\x1b[0m plain\nsecond");
        assert_eq!(text.lines.len(), 2);
        let first: String = text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(first, "a    bred plain");
        assert!(text.lines[0].spans.iter().any(|s| s.style.fg == Some(Color::Red)));
        assert!(text.lines.iter().flat_map(|l| l.spans.iter()).all(|s| s.style.fg != Some(Color::Reset)));
        assert!(ansi_text("").lines.is_empty());
    }

    #[test]
    fn strip_escapes_drops_csi_osc_and_controls() {
        assert_eq!(strip_escapes("a\x1b[1;31mb\x1b]0;title\x07c\x1b]2;t\x1b\\d\x07e\nf"), "abcde\nf");
        assert_eq!(strip_escapes("trailing\x1b["), "trailing");
        assert_eq!(strip_escapes("lone\x1b"), "lone");
    }
}
