use std::time::Duration;

pub use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[derive(Debug)]
pub enum Event<M> {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize(u16, u16),
    Focus(bool),
    /// Elapsed since the previous tick; drives effects.
    Tick(Duration),
    /// Lua forwarded a chord (`U+E000` + char), see docs/protocol.md.
    Forwarded(char),
    Msg(M),
}

impl<M> Event<M> {
    pub fn key(code: KeyCode, mods: KeyModifiers) -> Self {
        Event::Key(KeyEvent::new(code, mods))
    }
}
