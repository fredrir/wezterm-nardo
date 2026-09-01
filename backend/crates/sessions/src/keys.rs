use std::collections::BTreeMap;

use nardo_core::event::{KeyCode, KeyEvent, KeyModifiers};
use nardo_core::keys::{key_label, key_name, parse_key};

use crate::model::KeySpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    Switch,
    Close,
    Down,
    Up,
    PageDown,
    PageUp,
    First,
    Last,
    ScopeNext,
    ScopePrev,
    Kill,
    KillAll,
    NewTab,
    NewWindow,
    Split,
    Rename,
    Move,
    Zoom,
    Preview,
    PreviewUp,
    PreviewDown,
    Clear,
    Help,
}

impl ActionId {
    pub const ALL: [ActionId; 23] = [
        ActionId::Switch,
        ActionId::Close,
        ActionId::Down,
        ActionId::Up,
        ActionId::PageDown,
        ActionId::PageUp,
        ActionId::First,
        ActionId::Last,
        ActionId::ScopeNext,
        ActionId::ScopePrev,
        ActionId::Kill,
        ActionId::KillAll,
        ActionId::NewTab,
        ActionId::NewWindow,
        ActionId::Split,
        ActionId::Rename,
        ActionId::Move,
        ActionId::Zoom,
        ActionId::Preview,
        ActionId::PreviewUp,
        ActionId::PreviewDown,
        ActionId::Clear,
        ActionId::Help,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ActionId::Switch => "switch",
            ActionId::Close => "close",
            ActionId::Down => "down",
            ActionId::Up => "up",
            ActionId::PageDown => "page_down",
            ActionId::PageUp => "page_up",
            ActionId::First => "first",
            ActionId::Last => "last",
            ActionId::ScopeNext => "scope_next",
            ActionId::ScopePrev => "scope_prev",
            ActionId::Kill => "kill",
            ActionId::KillAll => "kill_all",
            ActionId::NewTab => "new_tab",
            ActionId::NewWindow => "new_window",
            ActionId::Split => "split",
            ActionId::Rename => "rename",
            ActionId::Move => "move",
            ActionId::Zoom => "zoom",
            ActionId::Preview => "preview",
            ActionId::PreviewUp => "preview_up",
            ActionId::PreviewDown => "preview_down",
            ActionId::Clear => "clear",
            ActionId::Help => "help",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            ActionId::Switch => "activate selected pane / tab / window, attach domain",
            ActionId::Close => "close launcher",
            ActionId::Down => "select next",
            ActionId::Up => "select previous",
            ActionId::PageDown => "page down (empty query)",
            ActionId::PageUp => "page up (empty query)",
            ActionId::First => "select first",
            ActionId::Last => "select last",
            ActionId::ScopeNext => "next scope",
            ActionId::ScopePrev => "previous scope",
            ActionId::Kill => "kill selected pane / tab / window",
            ActionId::KillAll => "kill every listed pane",
            ActionId::NewTab => "new tab in selected window / domain",
            ActionId::NewWindow => "new window in selected domain",
            ActionId::Split => "split selected pane (bottom)",
            ActionId::Rename => "rename tab / window",
            ActionId::Move => "move pane / tab",
            ActionId::Zoom => "toggle zoom on selected pane",
            ActionId::Preview => "toggle preview panel",
            ActionId::PreviewUp => "scroll preview up",
            ActionId::PreviewDown => "scroll preview down",
            ActionId::Clear => "clear query",
            ActionId::Help => "this overlay",
        }
    }

    pub fn from_name(name: &str) -> Option<ActionId> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }

    fn defaults(self) -> Vec<KeyEvent> {
        match self {
            ActionId::Switch => vec![key(KeyCode::Enter)],
            ActionId::Close => vec![key(KeyCode::Esc)],
            ActionId::Down => vec![key(KeyCode::Down), ctrl('n'), ctrl('j')],
            ActionId::Up => vec![key(KeyCode::Up), ctrl('p'), ctrl('k')],
            ActionId::PageDown => vec![key(KeyCode::PageDown), ctrl('d')],
            ActionId::PageUp => vec![key(KeyCode::PageUp), ctrl('u')],
            ActionId::First => vec![key(KeyCode::Home)],
            ActionId::Last => vec![key(KeyCode::End)],
            ActionId::ScopeNext => vec![key(KeyCode::Tab)],
            ActionId::ScopePrev => vec![key(KeyCode::BackTab)],
            ActionId::Kill => vec![KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT)],
            ActionId::KillAll => vec![KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)],
            ActionId::NewTab => vec![ctrl('t')],
            ActionId::NewWindow => vec![ctrl('w')],
            ActionId::Split => vec![ctrl('s')],
            ActionId::Rename => vec![ctrl('r'), key(KeyCode::F(2))],
            ActionId::Move => vec![ctrl('m')],
            ActionId::Zoom => vec![ctrl('z')],
            ActionId::Preview => vec![ctrl(' ')],
            ActionId::PreviewUp => vec![KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)],
            ActionId::PreviewDown => vec![KeyEvent::new(KeyCode::Down, KeyModifiers::ALT)],
            ActionId::Clear => vec![ctrl('l')],
            ActionId::Help => vec![key(KeyCode::Char('?')), key(KeyCode::F(1))],
        }
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Terminal-agnostic identity: BackTab implies SHIFT, letters case-fold (uppercase = SHIFT +
/// lowercase code, so `Char('D')` and `ctrl+shift+d` meet in the middle), shifted symbols don't.
fn canon(k: &KeyEvent) -> (KeyCode, KeyModifiers) {
    let mut mods = k.modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
    let mut code = k.code;
    match code {
        KeyCode::BackTab => mods.remove(KeyModifiers::SHIFT),
        KeyCode::Char(c) if c.is_uppercase() => {
            mods.insert(KeyModifiers::SHIFT);
            code = KeyCode::Char(c.to_lowercase().next().unwrap_or(c));
        }
        KeyCode::Char(c) if !c.is_alphabetic() => mods.remove(KeyModifiers::SHIFT),
        _ => {}
    }
    (code, mods)
}

pub fn same_key(a: &KeyEvent, b: &KeyEvent) -> bool {
    canon(a) == canon(b)
}

/// Action → keys; defaults from docs/keys.md, `options.keys` overrides (`false` unbinds).
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: Vec<(ActionId, Vec<KeyEvent>)>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self { bindings: ActionId::ALL.into_iter().map(|a| (a, a.defaults())).collect() }
    }
}

impl Keymap {
    /// Returns the keymap and one warning per override it could not apply.
    pub fn with_overrides(overrides: &BTreeMap<String, KeySpec>) -> (Keymap, Vec<String>) {
        let mut map = Keymap::default();
        let mut warnings = Vec::new();
        for (name, spec) in overrides {
            let Some(action) = ActionId::from_name(name) else {
                warnings.push(format!("keys.{name}: unknown action"));
                continue;
            };
            let tokens: Vec<&str> = match spec {
                KeySpec::One(k) => vec![k.as_str()],
                KeySpec::Many(ks) => ks.iter().map(String::as_str).collect(),
                KeySpec::Off(false) => Vec::new(),
                KeySpec::Off(true) => continue,
            };
            let mut keys = Vec::new();
            for token in tokens {
                match parse_key(token) {
                    Ok(k) => keys.push(k),
                    Err(e) => warnings.push(format!("keys.{name}: {token:?}: {e}")),
                }
            }
            map.set(action, keys);
        }
        (map, warnings)
    }

    fn set(&mut self, action: ActionId, keys: Vec<KeyEvent>) {
        for (_, bound) in self.bindings.iter_mut() {
            bound.retain(|b| !keys.iter().any(|k| same_key(k, b)));
        }
        if let Some((_, bound)) = self.bindings.iter_mut().find(|(a, _)| *a == action) {
            *bound = keys;
        }
    }

    pub fn action_for(&self, key: &KeyEvent) -> Option<ActionId> {
        self.bindings.iter().find(|(_, keys)| keys.iter().any(|k| same_key(k, key))).map(|(a, _)| *a)
    }

    pub fn keys(&self, action: ActionId) -> &[KeyEvent] {
        self.bindings.iter().find(|(a, _)| *a == action).map(|(_, k)| k.as_slice()).unwrap_or(&[])
    }

    /// Hint label of the primary key (`↵`, `⇧D`, `^T`), empty when unbound.
    pub fn label(&self, action: ActionId) -> String {
        self.keys(action).first().map(key_label).unwrap_or_default()
    }

    /// `(keys, action name, description)` per bound action, for the help overlay.
    pub fn help_lines(&self) -> Vec<(String, &'static str, &'static str)> {
        self.bindings
            .iter()
            .filter(|(_, keys)| !keys.is_empty())
            .map(|(a, keys)| (keys.iter().map(key_name).collect::<Vec<_>>().join("  "), a.name(), a.describe()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn defaults_match_docs() {
        let map = Keymap::default();
        assert_eq!(map.action_for(&press(KeyCode::Enter, KeyModifiers::NONE)), Some(ActionId::Switch));
        assert_eq!(map.action_for(&press(KeyCode::Esc, KeyModifiers::NONE)), Some(ActionId::Close));
        assert_eq!(map.action_for(&press(KeyCode::Char('j'), KeyModifiers::CONTROL)), Some(ActionId::Down));
        assert_eq!(map.action_for(&press(KeyCode::Char('D'), KeyModifiers::SHIFT)), Some(ActionId::Kill));
        assert_eq!(map.action_for(&press(KeyCode::Char('D'), KeyModifiers::NONE)), Some(ActionId::Kill));
        assert_eq!(map.action_for(&press(KeyCode::Char('d'), KeyModifiers::NONE)), None);
        assert_eq!(
            map.action_for(&press(KeyCode::Char('d'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)),
            Some(ActionId::KillAll)
        );
        assert_eq!(map.action_for(&press(KeyCode::BackTab, KeyModifiers::SHIFT)), Some(ActionId::ScopePrev));
        assert_eq!(map.action_for(&press(KeyCode::BackTab, KeyModifiers::NONE)), Some(ActionId::ScopePrev));
        assert_eq!(map.action_for(&press(KeyCode::Char('?'), KeyModifiers::SHIFT)), Some(ActionId::Help));
        assert_eq!(map.action_for(&press(KeyCode::F(2), KeyModifiers::NONE)), Some(ActionId::Rename));
        assert_eq!(map.action_for(&press(KeyCode::Char(' '), KeyModifiers::CONTROL)), Some(ActionId::Preview));
        assert_eq!(map.action_for(&press(KeyCode::Up, KeyModifiers::ALT)), Some(ActionId::PreviewUp));
        assert_eq!(map.action_for(&press(KeyCode::Char('x'), KeyModifiers::NONE)), None);
    }

    #[test]
    fn every_action_has_a_name_roundtrip() {
        for a in ActionId::ALL {
            assert_eq!(ActionId::from_name(a.name()), Some(a));
        }
    }

    #[test]
    fn overrides_rebind_unbind_and_steal() {
        let overrides = BTreeMap::from([
            ("kill".to_string(), KeySpec::One("ctrl+x".into())),
            ("up".to_string(), KeySpec::Many(vec!["ctrl+t".into(), "up".into()])),
            ("help".to_string(), KeySpec::Off(false)),
            ("zoom".to_string(), KeySpec::Off(true)),
            ("nope".to_string(), KeySpec::One("a".into())),
        ]);
        let (map, warnings) = Keymap::with_overrides(&overrides);
        assert_eq!(map.action_for(&press(KeyCode::Char('x'), KeyModifiers::CONTROL)), Some(ActionId::Kill));
        assert_eq!(map.action_for(&press(KeyCode::Char('D'), KeyModifiers::SHIFT)), None);
        assert_eq!(map.action_for(&press(KeyCode::Char('t'), KeyModifiers::CONTROL)), Some(ActionId::Up));
        assert_eq!(map.keys(ActionId::NewTab), &[]);
        assert_eq!(map.action_for(&press(KeyCode::Char('?'), KeyModifiers::NONE)), None);
        assert_eq!(map.action_for(&press(KeyCode::F(1), KeyModifiers::NONE)), None);
        assert_eq!(map.action_for(&press(KeyCode::Char('z'), KeyModifiers::CONTROL)), Some(ActionId::Zoom));
        assert_eq!(warnings, vec!["keys.nope: unknown action".to_string()]);
    }

    #[test]
    fn bad_key_token_warns_and_keeps_the_rest() {
        let overrides = BTreeMap::from([("down".to_string(), KeySpec::Many(vec!["ctrl+n".into(), "".into()]))]);
        let (map, warnings) = Keymap::with_overrides(&overrides);
        assert_eq!(map.keys(ActionId::Down).len(), 1);
        assert_eq!(warnings.len(), 1);
    }
}
