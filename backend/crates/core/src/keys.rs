use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Prefix byte Lua sends before a forwarded chord letter.
pub const FORWARD_PREFIX: char = '\u{E000}';

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptToken {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Settle,
}

/// Parses `--keys` scripts, see docs/protocol.md "Key script".
pub fn parse_script(script: &str) -> anyhow::Result<Vec<ScriptToken>> {
    let mut tokens = Vec::new();
    let mut chars = script.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == '"' {
                    closed = true;
                    break;
                }
                tokens.push(ScriptToken::Key(char_key(ch)));
            }
            anyhow::ensure!(closed, "keys: unterminated quote in {script:?}");
        } else {
            let mut word = String::new();
            while let Some(&ch) = chars.peek().filter(|ch| !ch.is_whitespace()) {
                word.push(ch);
                chars.next();
            }
            parse_token(&word, &mut tokens)?;
        }
    }
    Ok(tokens)
}

fn parse_token(word: &str, out: &mut Vec<ScriptToken>) -> anyhow::Result<()> {
    if word == "settle" {
        out.push(ScriptToken::Settle);
    } else if let Some(spec) = word.strip_prefix("mouse:") {
        parse_mouse(spec, out)?;
    } else {
        out.push(ScriptToken::Key(parse_key(word)?));
    }
    Ok(())
}

fn parse_mouse(spec: &str, out: &mut Vec<ScriptToken>) -> anyhow::Result<()> {
    let (kind, arg) = spec.split_once(':').ok_or_else(|| anyhow::anyhow!("keys: mouse:{spec} needs an argument"))?;
    match kind {
        "move" => {
            let (x, y) = parse_xy(arg)?;
            out.push(ScriptToken::Mouse(mouse(MouseEventKind::Moved, x, y)));
        }
        "click" => {
            let (x, y) = parse_xy(arg)?;
            out.push(ScriptToken::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), x, y)));
            out.push(ScriptToken::Mouse(mouse(MouseEventKind::Up(MouseButton::Left), x, y)));
        }
        "scroll" => {
            let (dir, (x, y)) = match arg.split_once(':') {
                Some((dir, pos)) => (dir, parse_xy(pos)?),
                None => (arg, (0, 0)),
            };
            let kind = match dir {
                "up" => MouseEventKind::ScrollUp,
                "down" => MouseEventKind::ScrollDown,
                _ => anyhow::bail!("keys: mouse:scroll wants up|down, got {dir:?}"),
            };
            out.push(ScriptToken::Mouse(mouse(kind, x, y)));
        }
        _ => anyhow::bail!("keys: unknown mouse token mouse:{spec}"),
    }
    Ok(())
}

fn parse_xy(arg: &str) -> anyhow::Result<(u16, u16)> {
    let parse = || -> Option<(u16, u16)> {
        let (x, y) = arg.split_once(',')?;
        Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
    };
    parse().ok_or_else(|| anyhow::anyhow!("keys: expected X,Y, got {arg:?}"))
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }
}

fn char_key(c: char) -> KeyEvent {
    normalize(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

/// `ctrl+shift+d` / `enter` / `X` → KeyEvent. Single chars: uppercase implies SHIFT.
pub fn parse_key(token: &str) -> anyhow::Result<KeyEvent> {
    anyhow::ensure!(!token.is_empty(), "keys: empty key token");
    if let Some(c) = single_char(token) {
        return Ok(char_key(c));
    }
    let (mods, key) = match token.strip_suffix('+').and_then(|t| t.strip_suffix('+')) {
        Some(mods) => (Some(mods), "+"),
        None => match token.rsplit_once('+') {
            Some((mods, key)) => (Some(mods), key),
            None => (None, token),
        },
    };
    let mut modifiers = KeyModifiers::NONE;
    for name in mods.into_iter().flat_map(|m| m.split('+')) {
        modifiers |= parse_modifier(name)?;
    }
    Ok(normalize(KeyEvent::new(parse_code(key)?, modifiers)))
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

fn parse_modifier(name: &str) -> anyhow::Result<KeyModifiers> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => KeyModifiers::CONTROL,
        "alt" | "opt" | "option" => KeyModifiers::ALT,
        "shift" => KeyModifiers::SHIFT,
        "super" | "cmd" | "win" => KeyModifiers::SUPER,
        "meta" => KeyModifiers::META,
        _ => anyhow::bail!("keys: unknown modifier {name:?}"),
    })
}

fn parse_code(name: &str) -> anyhow::Result<KeyCode> {
    if let Some(c) = single_char(name) {
        return Ok(KeyCode::Char(c));
    }
    let lower = name.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=24).contains(n)) {
        return Ok(KeyCode::F(n));
    }
    Ok(match lower.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        _ => anyhow::bail!("keys: unknown key {name:?}"),
    })
}

/// Same shape crossterm delivers: Char + SHIFT carries the uppercase char, shift+tab is BackTab + SHIFT.
fn normalize(mut key: KeyEvent) -> KeyEvent {
    match key.code {
        KeyCode::Char(c) if c.is_uppercase() => key.modifiers |= KeyModifiers::SHIFT,
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            key.code = KeyCode::Char(c.to_uppercase().next().unwrap_or(c));
        }
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => key.code = KeyCode::BackTab,
        KeyCode::BackTab => key.modifiers |= KeyModifiers::SHIFT,
        _ => {}
    }
    key
}

/// Canonical name for a key event, inverse of `parse_key` (`ctrl+d`, `enter`, `D`).
pub fn key_name(key: &KeyEvent) -> String {
    let key = normalize(*key);
    let mut mods = key.modifiers;
    let base = match key.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) if mods.difference(KeyModifiers::SHIFT).is_empty() => {
            mods = KeyModifiers::NONE;
            c.to_string()
        }
        KeyCode::Char(c) => c.to_lowercase().to_string(),
        KeyCode::BackTab => {
            mods.remove(KeyModifiers::SHIFT);
            "backtab".to_string()
        }
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::F(n) => format!("f{n}"),
        other => format!("{other:?}").to_lowercase(),
    };
    let mut name = String::new();
    for (flag, prefix) in [
        (KeyModifiers::CONTROL, "ctrl+"),
        (KeyModifiers::ALT, "alt+"),
        (KeyModifiers::SHIFT, "shift+"),
        (KeyModifiers::SUPER, "super+"),
        (KeyModifiers::META, "meta+"),
    ] {
        if mods.contains(flag) {
            name.push_str(prefix);
        }
    }
    name.push_str(&base);
    name
}

/// Human hint label: `↵` `esc` `⇧D` `^D` `⌥K`.
pub fn key_label(key: &KeyEvent) -> String {
    let key = normalize(*key);
    let mut mods = key.modifiers;
    if key.code == KeyCode::BackTab {
        mods.remove(KeyModifiers::SHIFT);
    }
    let shifted_symbol = matches!(key.code, KeyCode::Char(c) if !c.is_alphabetic());
    let mut label = String::new();
    if mods.contains(KeyModifiers::CONTROL) {
        label.push('^');
    }
    if mods.contains(KeyModifiers::ALT) {
        label.push('⌥');
    }
    if mods.contains(KeyModifiers::SHIFT) && !shifted_symbol {
        label.push('⇧');
    }
    if mods.contains(KeyModifiers::SUPER) {
        label.push('⌘');
    }
    let symbol = match key.code {
        KeyCode::Char(' ') => "␣".to_string(),
        KeyCode::Char(c) if mods.is_empty() => c.to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        KeyCode::Enter => "↵".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab => "⇥".to_string(),
        KeyCode::BackTab => "⇤".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Home => "↖".to_string(),
        KeyCode::End => "↘".to_string(),
        KeyCode::PageUp => "⇞".to_string(),
        KeyCode::PageDown => "⇟".to_string(),
        KeyCode::Backspace => "⌫".to_string(),
        KeyCode::Delete => "⌦".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => key_name(&KeyEvent::new(other, KeyModifiers::NONE)),
    };
    label.push_str(&symbol);
    label
}

pub fn is(key: &KeyEvent, code: KeyCode, mods: KeyModifiers) -> bool {
    key.code == code && key.modifiers == mods
}

/// Printable char for search input: plain or shifted letters/symbols, never control chords.
pub fn printable(key: &KeyEvent) -> Option<char> {
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    let chord =
        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER;
    (!key.modifiers.intersects(chord) && !c.is_control() && c != FORWARD_PREFIX).then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parse_key_covers_chars_named_keys_and_modifiers() {
        assert_eq!(parse_key("a").unwrap(), key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(parse_key("Z").unwrap(), key(KeyCode::Char('Z'), KeyModifiers::SHIFT));
        assert_eq!(parse_key("/").unwrap(), key(KeyCode::Char('/'), KeyModifiers::NONE));
        assert_eq!(parse_key("?").unwrap(), key(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(parse_key("enter").unwrap(), key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(parse_key("ESC").unwrap(), key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(parse_key("space").unwrap(), key(KeyCode::Char(' '), KeyModifiers::NONE));
        assert_eq!(parse_key("f2").unwrap(), key(KeyCode::F(2), KeyModifiers::NONE));
        assert_eq!(parse_key("f12").unwrap(), key(KeyCode::F(12), KeyModifiers::NONE));
        assert_eq!(parse_key("ctrl+d").unwrap(), key(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(
            parse_key("ctrl+shift+d").unwrap(),
            key(KeyCode::Char('D'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        );
        assert_eq!(parse_key("shift+d").unwrap(), key(KeyCode::Char('D'), KeyModifiers::SHIFT));
        assert_eq!(parse_key("alt+up").unwrap(), key(KeyCode::Up, KeyModifiers::ALT));
        assert_eq!(parse_key("ctrl+space").unwrap(), key(KeyCode::Char(' '), KeyModifiers::CONTROL));
        assert_eq!(parse_key("backtab").unwrap(), key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(parse_key("shift+tab").unwrap(), key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(parse_key("ctrl++").unwrap(), key(KeyCode::Char('+'), KeyModifiers::CONTROL));
        assert!(parse_key("").is_err());
        assert!(parse_key("bogus").is_err());
        assert!(parse_key("hyper+x").is_err());
        assert!(parse_key("ctrl+").is_err());
    }

    #[test]
    fn key_name_round_trips() {
        for name in [
            "a",
            "Z",
            "?",
            "/",
            "space",
            "enter",
            "esc",
            "tab",
            "backtab",
            "up",
            "down",
            "left",
            "right",
            "home",
            "end",
            "pageup",
            "pagedown",
            "backspace",
            "delete",
            "insert",
            "f1",
            "f12",
            "ctrl+d",
            "ctrl+shift+d",
            "alt+up",
            "alt+shift+x",
            "ctrl+space",
            "ctrl+alt+shift+super+k",
            "ctrl+?",
        ] {
            assert_eq!(key_name(&parse_key(name).unwrap()), name, "round trip {name}");
        }
        assert_eq!(key_name(&parse_key("shift+tab").unwrap()), "backtab");
        assert_eq!(key_name(&parse_key("shift+d").unwrap()), "D");
        assert_eq!(key_name(&key(KeyCode::Char('D'), KeyModifiers::NONE)), "D");
        assert_eq!(key_name(&key(KeyCode::Char('d'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)), "ctrl+shift+d");
    }

    #[test]
    fn key_label_uses_symbols() {
        let label = |s: &str| key_label(&parse_key(s).unwrap());
        assert_eq!(label("enter"), "↵");
        assert_eq!(label("esc"), "esc");
        assert_eq!(label("tab"), "⇥");
        assert_eq!(label("backtab"), "⇤");
        assert_eq!(label("D"), "⇧D");
        assert_eq!(label("d"), "d");
        assert_eq!(label("?"), "?");
        assert_eq!(label("ctrl+d"), "^D");
        assert_eq!(label("ctrl+shift+d"), "^⇧D");
        assert_eq!(label("alt+up"), "⌥↑");
        assert_eq!(label("alt+k"), "⌥K");
        assert_eq!(label("pageup"), "⇞");
        assert_eq!(label("pagedown"), "⇟");
        assert_eq!(label("ctrl+space"), "^␣");
        assert_eq!(label("f2"), "F2");
    }

    #[test]
    fn parse_script_tokenizes_words_quotes_mouse_and_settle() {
        let tokens =
            parse_script("  v i \"m ~/X\" enter mouse:move:3,4 mouse:click:5,6 mouse:scroll:down settle ").unwrap();
        let mut expected = vec![
            ScriptToken::Key(parse_key("v").unwrap()),
            ScriptToken::Key(parse_key("i").unwrap()),
            ScriptToken::Key(parse_key("m").unwrap()),
            ScriptToken::Key(parse_key("space").unwrap()),
            ScriptToken::Key(parse_key("~").unwrap()),
            ScriptToken::Key(parse_key("/").unwrap()),
            ScriptToken::Key(parse_key("X").unwrap()),
            ScriptToken::Key(parse_key("enter").unwrap()),
            ScriptToken::Mouse(mouse(MouseEventKind::Moved, 3, 4)),
            ScriptToken::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 6)),
            ScriptToken::Mouse(mouse(MouseEventKind::Up(MouseButton::Left), 5, 6)),
            ScriptToken::Mouse(mouse(MouseEventKind::ScrollDown, 0, 0)),
            ScriptToken::Settle,
        ];
        assert_eq!(tokens, expected);
        expected.clear();
        assert_eq!(parse_script("").unwrap(), expected);
        assert_eq!(
            parse_script("mouse:scroll:up:7,8").unwrap(),
            [ScriptToken::Mouse(mouse(MouseEventKind::ScrollUp, 7, 8))]
        );
        assert!(parse_script("\"open").is_err());
        assert!(parse_script("mouse:click:x").is_err());
        assert!(parse_script("mouse:drag:1,1").is_err());
    }

    #[test]
    fn printable_accepts_plain_and_shifted_chars_only() {
        assert_eq!(printable(&parse_key("a").unwrap()), Some('a'));
        assert_eq!(printable(&parse_key("Z").unwrap()), Some('Z'));
        assert_eq!(printable(&parse_key("?").unwrap()), Some('?'));
        assert_eq!(printable(&parse_key("space").unwrap()), Some(' '));
        assert_eq!(printable(&parse_key("ctrl+d").unwrap()), None);
        assert_eq!(printable(&parse_key("alt+k").unwrap()), None);
        assert_eq!(printable(&parse_key("enter").unwrap()), None);
        assert_eq!(printable(&key(KeyCode::Char(FORWARD_PREFIX), KeyModifiers::NONE)), None);
        assert_eq!(printable(&key(KeyCode::Char('\u{7}'), KeyModifiers::NONE)), None);
    }
}
