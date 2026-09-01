use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::StatefulWidget;
use unicode_width::UnicodeWidthChar;

use crate::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crate::ui::fill;
use crate::ui::theme::Theme;

/// Single-line editor: chars, backspace, delete, ←/→, home/end, ctrl+u (clear), ctrl+w (kill word),
/// alt+←/→ (word jumps). Unicode aware (char indices, width via unicode-width).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchState {
    pub value: String,
    pub cursor: usize,
    /// Set by render: where the terminal cursor should sit.
    pub cursor_pos: Option<Position>,
}

impl SearchState {
    /// Returns true when the value changed.
    pub fn handle(&mut self, key: &KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let chord = key.modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER,
        );
        let len = self.len();
        self.cursor = self.cursor.min(len);
        match key.code {
            KeyCode::Char(c) if !chord => {
                self.insert(c);
                true
            }
            KeyCode::Char('a') if ctrl => self.move_to(0),
            KeyCode::Char('e') if ctrl => self.move_to(len),
            KeyCode::Char('b') if alt => self.move_to(self.prev_word()),
            KeyCode::Char('f') if alt => self.move_to(self.next_word()),
            KeyCode::Char('u') if ctrl => self.delete_range(0, self.cursor),
            KeyCode::Char('w') if ctrl => self.delete_range(self.prev_word(), self.cursor),
            KeyCode::Char('d') if ctrl => self.delete_range(self.cursor, self.cursor + 1),
            KeyCode::Char('h') if ctrl => self.delete_range(self.cursor.saturating_sub(1), self.cursor),
            KeyCode::Backspace if alt || ctrl => self.delete_range(self.prev_word(), self.cursor),
            KeyCode::Backspace => self.delete_range(self.cursor.saturating_sub(1), self.cursor),
            KeyCode::Delete => self.delete_range(self.cursor, self.cursor + 1),
            KeyCode::Left if alt || ctrl => self.move_to(self.prev_word()),
            KeyCode::Right if alt || ctrl => self.move_to(self.next_word()),
            KeyCode::Left => self.move_to(self.cursor.saturating_sub(1)),
            KeyCode::Right => self.move_to(self.cursor + 1),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(len),
            _ => false,
        }
    }

    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.chars().count();
    }

    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    fn len(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_at(&self, index: usize) -> usize {
        self.value.char_indices().nth(index).map_or(self.value.len(), |(byte, _)| byte)
    }

    fn move_to(&mut self, index: usize) -> bool {
        self.cursor = index.min(self.len());
        false
    }

    fn insert(&mut self, c: char) {
        let byte = self.byte_at(self.cursor);
        self.value.insert(byte, c);
        self.cursor += 1;
    }

    fn delete_range(&mut self, from: usize, to: usize) -> bool {
        let to = to.min(self.len());
        if from >= to {
            return false;
        }
        let (start, end) = (self.byte_at(from), self.byte_at(to));
        self.value.replace_range(start..end, "");
        self.cursor = from;
        true
    }

    fn prev_word(&self) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i > 0 && !is_word(chars[i - 1]) {
            i -= 1;
        }
        while i > 0 && is_word(chars[i - 1]) {
            i -= 1;
        }
        i
    }

    fn next_word(&self) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i < chars.len() && !is_word(chars[i]) {
            i += 1;
        }
        while i < chars.len() && is_word(chars[i]) {
            i += 1;
        }
        i
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

pub struct SearchInput<'a> {
    pub theme: &'a Theme,
    pub placeholder: &'a str,
    pub icon: &'a str,
}

impl StatefulWidget for SearchInput<'_> {
    type State = SearchState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = area.intersection(buf.area);
        state.cursor_pos = None;
        if area.is_empty() {
            return;
        }
        let theme = self.theme;
        fill(buf, area, theme.base());
        let y = area.y;
        let mut x = area.x;
        if !self.icon.is_empty() {
            let icon = Style::new().fg(theme.accent).bg(theme.surface);
            let (end, _) = buf.set_stringn(x, y, self.icon, usize::from(area.width), icon);
            x = end.saturating_add(1).min(area.right());
        }
        let width = usize::from(area.right() - x);
        if width == 0 {
            return;
        }
        if state.value.is_empty() {
            buf.set_stringn(x, y, self.placeholder, width, theme.dim());
            state.cursor_pos = Some(Position::new(x, y));
            return;
        }

        state.cursor = state.cursor.min(state.len());
        let cursor_col: usize = state.value.chars().take(state.cursor).map(char_width).sum();
        let skip = cursor_col.saturating_sub(width - 1);
        let mut col = 0;
        let mut start = state.value.len();
        for (byte, c) in state.value.char_indices() {
            if col >= skip {
                start = byte;
                break;
            }
            col += char_width(c);
        }
        buf.set_stringn(x, y, &state.value[start..], width, theme.base());
        let cursor_x = x + u16::try_from(cursor_col.saturating_sub(col)).unwrap_or(u16::MAX);
        state.cursor_pos = Some(Position::new(cursor_x.min(area.right() - 1), y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_util::{buffer, row, tiny_areas};

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn press(state: &mut SearchState, code: KeyCode) -> bool {
        state.handle(&key(code, KeyModifiers::NONE))
    }

    fn ctrl(state: &mut SearchState, c: char) -> bool {
        state.handle(&key(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    fn alt(state: &mut SearchState, code: KeyCode) -> bool {
        state.handle(&key(code, KeyModifiers::ALT))
    }

    #[test]
    fn typing_and_deleting_unicode() {
        let mut state = SearchState::default();
        assert!(press(&mut state, KeyCode::Char('h')));
        assert!(press(&mut state, KeyCode::Char('é')));
        assert!(state.handle(&key(KeyCode::Char('L'), KeyModifiers::SHIFT)));
        assert_eq!(state.value, "héL");
        assert_eq!(state.cursor, 3);
        assert!(!press(&mut state, KeyCode::Left));
        assert!(press(&mut state, KeyCode::Char('日')));
        assert_eq!(state.value, "hé日L");
        assert_eq!(state.cursor, 3);
        assert!(press(&mut state, KeyCode::Backspace));
        assert_eq!(state.value, "héL");
        assert!(press(&mut state, KeyCode::Delete));
        assert_eq!(state.value, "hé");
        assert!(!press(&mut state, KeyCode::Delete));
        assert!(!press(&mut state, KeyCode::Right));
        assert_eq!(state.cursor, 2);
        assert!(!press(&mut state, KeyCode::Home));
        assert!(!press(&mut state, KeyCode::Backspace));
        assert_eq!(state.cursor, 0);
        assert!(!press(&mut state, KeyCode::End));
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn chords_are_not_inserted() {
        let mut state = SearchState::default();
        assert!(!state.handle(&key(KeyCode::Char('x'), KeyModifiers::CONTROL)));
        assert!(!state.handle(&key(KeyCode::Char('x'), KeyModifiers::ALT)));
        assert!(!state.handle(&key(KeyCode::Char('x'), KeyModifiers::SUPER)));
        assert!(!state.handle(&key(KeyCode::Tab, KeyModifiers::NONE)));
        let mut release = key(KeyCode::Char('x'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(!state.handle(&release));
        assert!(state.is_empty());
    }

    #[test]
    fn readline_editing() {
        let mut state = SearchState::default();
        state.set("d:archie vim ~/x");
        assert!(!ctrl(&mut state, 'a'));
        assert_eq!(state.cursor, 0);
        assert!(!ctrl(&mut state, 'e'));
        assert_eq!(state.cursor, 16);
        assert!(ctrl(&mut state, 'w'));
        assert_eq!(state.value, "d:archie vim ~/");
        assert!(alt(&mut state, KeyCode::Backspace));
        assert_eq!(state.value, "d:archie ");
        assert!(ctrl(&mut state, 'u'));
        assert!(state.is_empty());
        assert!(!ctrl(&mut state, 'u'));

        state.set("foo bar");
        state.cursor = 3;
        assert!(ctrl(&mut state, 'd'));
        assert_eq!(state.value, "foobar");
        assert!(ctrl(&mut state, 'h'));
        assert_eq!(state.value, "fobar");
        assert!(ctrl(&mut state, 'u'));
        assert_eq!(state.value, "bar");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn word_jumps() {
        let mut state = SearchState::default();
        state.set("d:archie  vim ~/x");
        assert!(!alt(&mut state, KeyCode::Left));
        assert_eq!(state.cursor, 16);
        assert!(!alt(&mut state, KeyCode::Left));
        assert_eq!(state.cursor, 10);
        assert!(!alt(&mut state, KeyCode::Left));
        assert_eq!(state.cursor, 2);
        assert!(!alt(&mut state, KeyCode::Left));
        assert_eq!(state.cursor, 0);
        assert!(!alt(&mut state, KeyCode::Right));
        assert_eq!(state.cursor, 1);
        assert!(!state.handle(&key(KeyCode::Right, KeyModifiers::CONTROL)));
        assert_eq!(state.cursor, 8);
        assert!(!alt(&mut state, KeyCode::Char('f')));
        assert_eq!(state.cursor, 13);
        assert!(!alt(&mut state, KeyCode::Char('b')));
        assert_eq!(state.cursor, 10);
        assert!(!alt(&mut state, KeyCode::Right));
        assert!(!alt(&mut state, KeyCode::Right));
        assert_eq!(state.cursor, 17);
    }

    #[test]
    fn stale_cursor_is_clamped() {
        let mut state = SearchState { value: "ab".into(), cursor: 9, cursor_pos: None };
        assert!(press(&mut state, KeyCode::Char('c')));
        assert_eq!(state.value, "abc");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn renders_icon_value_and_cursor() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 12, 1);
        let mut buf = buffer(area);
        let mut state = SearchState::default();
        state.set("vim");
        SearchInput { theme: &theme, placeholder: "Search", icon: ">" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "> vim       ");
        assert_eq!(buf[(0, 0)].fg, theme.accent);
        assert_eq!(buf[(2, 0)].fg, theme.text);
        assert_eq!(state.cursor_pos, Some(Position::new(5, 0)));

        state.cursor = 1;
        SearchInput { theme: &theme, placeholder: "Search", icon: "" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "vim         ");
        assert_eq!(state.cursor_pos, Some(Position::new(1, 0)));
    }

    #[test]
    fn renders_placeholder() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 12, 1);
        let mut buf = buffer(area);
        let mut state = SearchState::default();
        SearchInput { theme: &theme, placeholder: "Search panes", icon: "🔍" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "🔍  Search pa");
        assert_eq!(buf[(3, 0)].fg, theme.text_dim);
        assert_eq!(state.cursor_pos, Some(Position::new(3, 0)));
    }

    #[test]
    fn scrolls_to_keep_cursor_visible() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 6, 1);
        let mut buf = buffer(area);
        let mut state = SearchState::default();
        state.set("abcdefgh");
        SearchInput { theme: &theme, placeholder: "", icon: "" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "defgh ");
        assert_eq!(state.cursor_pos, Some(Position::new(5, 0)));

        state.cursor = 2;
        SearchInput { theme: &theme, placeholder: "", icon: "" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "abcdef");
        assert_eq!(state.cursor_pos, Some(Position::new(2, 0)));

        state.set("日本語テキスト");
        SearchInput { theme: &theme, placeholder: "", icon: "" }.render(area, &mut buf, &mut state);
        assert_eq!(row(&buf, 0), "ス ト   ");
        assert_eq!(state.cursor_pos, Some(Position::new(4, 0)));
    }

    #[test]
    fn survives_tiny_areas() {
        let theme = Theme::dark();
        let mut state = SearchState::default();
        state.set("query");
        for area in tiny_areas() {
            let mut buf = buffer(area);
            SearchInput { theme: &theme, placeholder: "Search", icon: "🔍" }.render(area, &mut buf, &mut state);
            if area.is_empty() {
                assert_eq!(state.cursor_pos, None);
            }
            let mut empty = SearchState::default();
            SearchInput { theme: &theme, placeholder: "Search", icon: "" }.render(area, &mut buf, &mut empty);
        }
    }
}
