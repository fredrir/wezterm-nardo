use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Clear, Widget};
use unicode_width::UnicodeWidthStr;

use crate::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::ui::theme::Theme;

const WIDTH: u16 = 60;
const HEIGHT: u16 = 7;
const BUTTON_MAX: u16 = 14;
const BUTTON_GAP: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Yes,
    No,
}

/// Centred dialog on top of the modal: `y`/`enter` = yes, `n`/`esc` = no, click on buttons.
pub struct Confirm<'a> {
    pub theme: &'a Theme,
    pub title: &'a str,
    pub body: &'a str,
    pub yes: &'a str,
    pub no: &'a str,
    pub danger: bool,
}

impl Confirm<'_> {
    pub fn area(area: Rect) -> Rect {
        let width = WIDTH.min(area.width.saturating_sub(4));
        let height = HEIGHT.min(area.height);
        Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
    }

    pub fn choice_from_key(key: &KeyEvent) -> Option<ConfirmChoice> {
        if key.kind == KeyEventKind::Release || key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return None;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => Some(ConfirmChoice::Yes),
            KeyCode::Esc | KeyCode::Char('n' | 'N') => Some(ConfirmChoice::No),
            _ => None,
        }
    }

    /// `area` is the dialog rect from `Confirm::area`.
    pub fn choice_from_mouse(area: Rect, mouse: &MouseEvent) -> Option<ConfirmChoice> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return None;
        }
        let (yes, no) = button_areas(area);
        let at = Position::new(mouse.column, mouse.row);
        if yes.contains(at) {
            Some(ConfirmChoice::Yes)
        } else if no.contains(at) {
            Some(ConfirmChoice::No)
        } else {
            None
        }
    }
}

/// Two equal pills on the last inner row, centred as a pair.
fn button_areas(area: Rect) -> (Rect, Rect) {
    let inner = area.inner(Margin::new(2, 1));
    let width = (inner.width.saturating_sub(BUTTON_GAP) / 2).min(BUTTON_MAX);
    if inner.is_empty() || width == 0 {
        return (Rect::ZERO, Rect::ZERO);
    }
    let total = width * 2 + BUTTON_GAP;
    let x = inner.x + (inner.width - total) / 2;
    let y = inner.bottom() - 1;
    (Rect::new(x, y, width, 1), Rect::new(x + width + BUTTON_GAP, y, width, 1))
}

impl Widget for Confirm<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        let bg = theme.surface_hi;
        let accent = if self.danger { theme.danger } else { theme.accent };
        Clear.render(area, buf);
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(accent).bg(bg))
            .style(Style::new().fg(theme.text).bg(bg))
            .render(area, buf);

        let inner = area.inner(Margin::new(2, 1));
        let (yes, no) = button_areas(area);
        let text_rows = if yes.is_empty() { inner.bottom() } else { yes.y };
        let width = usize::from(inner.width);
        if inner.y < text_rows {
            let title = Style::new().fg(theme.text).bg(bg).add_modifier(Modifier::BOLD);
            buf.set_stringn(inner.x, inner.y, self.title, width, title);
        }
        if inner.y + 2 < text_rows {
            buf.set_stringn(inner.x, inner.y + 2, self.body, width, Style::new().fg(theme.text_muted).bg(bg));
        }
        let yes_style = Style::new().fg(theme.accent_fg).bg(accent).add_modifier(Modifier::BOLD);
        let no_style = Style::new().fg(theme.text).bg(theme.surface);
        render_button(buf, yes, self.yes, 'y', yes_style);
        render_button(buf, no, self.no, 'n', no_style);
    }
}

fn render_button(buf: &mut Buffer, pill: Rect, label: &str, hotkey: char, style: Style) {
    let pill = pill.intersection(buf.area);
    if pill.is_empty() {
        return;
    }
    buf.set_style(pill, style);
    let label_width = u16::try_from(label.width()).unwrap_or(u16::MAX);
    let pad = pill.width.saturating_sub(label_width) / 2;
    let x = pill.x + pad;
    let (end, _) = buf.set_stringn(x, pill.y, label, usize::from(pill.width - pad), style);
    let underlined = label.chars().next().is_some_and(|c| c.eq_ignore_ascii_case(&hotkey));
    if underlined && end > x {
        buf[(x, pill.y)].modifier.insert(Modifier::UNDERLINED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE }
    }

    fn confirm(theme: &Theme) -> Confirm<'_> {
        Confirm { theme, title: "Kill pane 51?", body: "vim ~/x will be closed", yes: "Yes", no: "No", danger: true }
    }

    #[test]
    fn area_is_centred_and_capped() {
        assert_eq!(Confirm::area(Rect::new(0, 0, 120, 40)), Rect::new(30, 16, 60, 7));
        assert_eq!(Confirm::area(Rect::new(0, 0, 30, 5)), Rect::new(2, 0, 26, 5));
        assert_eq!(Confirm::area(Rect::new(0, 0, 3, 1)), Rect::new(1, 0, 0, 1));
        assert_eq!(Confirm::area(Rect::ZERO), Rect::ZERO);
    }

    #[test]
    fn choice_from_key_hotkeys() {
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Char('y'))), Some(ConfirmChoice::Yes));
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Char('Y'))), Some(ConfirmChoice::Yes));
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Enter)), Some(ConfirmChoice::Yes));
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Char('n'))), Some(ConfirmChoice::No));
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Esc)), Some(ConfirmChoice::No));
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Char('x'))), None);
        assert_eq!(Confirm::choice_from_key(&key(KeyCode::Tab)), None);
        let ctrl_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL);
        assert_eq!(Confirm::choice_from_key(&ctrl_y), None);
        let mut release = key(KeyCode::Enter);
        release.kind = KeyEventKind::Release;
        assert_eq!(Confirm::choice_from_key(&release), None);
    }

    #[test]
    fn choice_from_mouse_hits_buttons() {
        let area = Confirm::area(Rect::new(0, 0, 120, 40));
        let (yes, no) = button_areas(area);
        assert_eq!(yes, Rect::new(45, 21, 14, 1));
        assert_eq!(no, Rect::new(61, 21, 14, 1));
        assert_eq!(Confirm::choice_from_mouse(area, &click(yes.x, yes.y)), Some(ConfirmChoice::Yes));
        assert_eq!(Confirm::choice_from_mouse(area, &click(no.right() - 1, no.y)), Some(ConfirmChoice::No));
        assert_eq!(Confirm::choice_from_mouse(area, &click(yes.right(), yes.y)), None);
        assert_eq!(Confirm::choice_from_mouse(area, &click(yes.x, yes.y - 1)), None);
        let moved = MouseEvent { kind: MouseEventKind::Moved, ..click(yes.x, yes.y) };
        assert_eq!(Confirm::choice_from_mouse(area, &moved), None);
        assert_eq!(Confirm::choice_from_mouse(Rect::ZERO, &click(0, 0)), None);
    }

    #[test]
    fn renders_title_body_and_buttons() {
        let theme = Theme::dark();
        let area = Confirm::area(Rect::new(0, 0, 70, 12));
        let mut buf = buffer(Rect::new(0, 0, 70, 12));
        confirm(&theme).render(area, &mut buf);
        assert!(row(&buf, area.y + 1).contains("Kill pane 51?"));
        assert!(row(&buf, area.y + 3).contains("vim ~/x will be closed"));
        let buttons = row(&buf, area.y + 5);
        assert!(buttons.contains("Yes"));
        assert!(buttons.contains("No"));
        assert_eq!(buf[(area.x, area.y)].fg, theme.danger);
        let (yes, no) = button_areas(area);
        assert_eq!(buf[(yes.x, yes.y)].bg, theme.danger);
        assert_eq!(buf[(no.x, no.y)].bg, theme.surface);
        let y_cell = (yes.x..yes.right()).find(|&x| buf[(x, yes.y)].symbol() == "Y").unwrap();
        assert!(buf[(y_cell, yes.y)].modifier.contains(Modifier::UNDERLINED));
        let mut buf = buffer(Rect::new(0, 0, 70, 12));
        Confirm { danger: false, yes: "Kill", ..confirm(&theme) }.render(area, &mut buf);
        assert_eq!(buf[(area.x, area.y)].fg, theme.accent);
        let k_cell = (yes.x..yes.right()).find(|&x| buf[(x, yes.y)].symbol() == "K").unwrap();
        assert!(!buf[(k_cell, yes.y)].modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        for area in tiny_areas() {
            let mut buf = buffer(area);
            confirm(&theme).render(Confirm::area(area), &mut buf);
            confirm(&theme).render(area, &mut buf);
        }
        let mut buf = buffer(Rect::new(0, 0, 8, 4));
        confirm(&theme).render(Rect::new(0, 0, 8, 4), &mut buf);
    }
}
