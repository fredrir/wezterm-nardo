use ratatui::style::{Color, Modifier, Style};

use crate::context::ThemeSpec;

const FALLBACK_BG: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
const FALLBACK_FG: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
const FALLBACK_ACCENT: Color = Color::Rgb(0x89, 0xb4, 0xfa);
const FALLBACK_MATCH: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
const FALLBACK_DANGER: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
const FALLBACK_WARNING: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
const FALLBACK_SUCCESS: Color = Color::Rgb(0xa6, 0xe3, 0xa1);

/// Resolved palette. Every colour is an explicit `Color::Rgb`; the modal never relies on the
/// terminal defaults so it looks identical on every WezTerm colour scheme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub surface_hi: Color,
    pub border: Color,
    pub border_focus: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub accent_fg: Color,
    pub match_hl: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub backdrop_dim: f32,
}

impl Theme {
    /// Missing spec fields fall back to Catppuccin Mocha-ish values; `accent` = spec.accent → ansi blue.
    pub fn from_spec(spec: &ThemeSpec) -> Theme {
        let hex = |value: &Option<String>| value.as_deref().and_then(parse_hex);
        let slot = |palette: &[String], index: usize| palette.get(index).and_then(|s| parse_hex(s));

        let bg = hex(&spec.background).unwrap_or(FALLBACK_BG);
        let text = hex(&spec.foreground).unwrap_or(FALLBACK_FG);
        let dark = luma(bg) < 0.5;
        let shade = |amount: f32| {
            if dark { lighten(bg, amount) } else { darken(bg, amount) }
        };
        let accent = hex(&spec.accent).or_else(|| slot(&spec.ansi, 4)).unwrap_or(FALLBACK_ACCENT);
        let surface = shade(0.06);

        Theme {
            bg,
            surface,
            surface_hi: shade(0.12),
            border: shade(0.14),
            border_focus: accent,
            text,
            text_muted: blend(text, bg, 0.45),
            text_dim: blend(text, bg, 0.65),
            accent,
            accent_fg: bg,
            match_hl: slot(&spec.brights, 3).unwrap_or(FALLBACK_MATCH),
            danger: slot(&spec.ansi, 1).unwrap_or(FALLBACK_DANGER),
            warning: slot(&spec.ansi, 3).unwrap_or(FALLBACK_WARNING),
            success: slot(&spec.ansi, 2).unwrap_or(FALLBACK_SUCCESS),
            selection_bg: blend(accent, surface, 0.75),
            selection_fg: text,
            backdrop_dim: 0.55,
        }
    }

    pub fn dark() -> Theme {
        Self::from_spec(&ThemeSpec::default())
    }

    pub fn is_dark(&self) -> bool {
        luma(self.bg) < 0.5
    }

    pub fn base(&self) -> Style {
        Style::new().fg(self.text).bg(self.surface)
    }
    pub fn muted(&self) -> Style {
        Style::new().fg(self.text_muted).bg(self.surface)
    }
    pub fn dim(&self) -> Style {
        Style::new().fg(self.text_dim).bg(self.surface)
    }
    pub fn accent(&self) -> Style {
        Style::new().fg(self.accent).bg(self.surface).add_modifier(Modifier::BOLD)
    }
    pub fn selected(&self) -> Style {
        Style::new().fg(self.selection_fg).bg(self.selection_bg).add_modifier(Modifier::BOLD)
    }
    pub fn matched(&self) -> Style {
        Style::new().fg(self.match_hl).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }
    pub fn danger(&self) -> Style {
        Style::new().fg(self.danger).bg(self.surface).add_modifier(Modifier::BOLD)
    }
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha ignored), `rgb:rr/gg/bb` (X11, 1–4 digits per channel).
pub fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("rgb:") {
        let mut parts = rest.split('/');
        let (r, g, b) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        return Some(Color::Rgb(x11_channel(r)?, x11_channel(g)?, x11_channel(b)?));
    }
    let hex = s.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    match hex.len() {
        3 => Some(Color::Rgb(channel(0..1)? * 17, channel(1..2)? * 17, channel(2..3)? * 17)),
        6 | 8 => Some(Color::Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?)),
        _ => None,
    }
}

fn x11_channel(s: &str) -> Option<u8> {
    if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(s, 16).ok()?;
    let max = (1u32 << (4 * s.len() as u32)) - 1;
    u8::try_from(value * 255 / max).ok()
}

/// `t` = 0 → a, 1 → b. Non-RGB colours pass through unchanged.
pub fn blend(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

pub fn lighten(c: Color, amount: f32) -> Color {
    blend(c, Color::Rgb(255, 255, 255), amount)
}

pub fn darken(c: Color, amount: f32) -> Color {
    blend(c, Color::Rgb(0, 0, 0), amount)
}

/// Perceived brightness 0..1 (Rec. 601 weights); non-RGB → 0.0.
pub fn luma(c: Color) -> f32 {
    let Color::Rgb(r, g, b) = c else {
        return 0.0;
    };
    (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) / 255.0
}

/// RGB for colours the terminal defines deterministically (RGB, xterm cube/greys); the rest
/// (named ansi, indexed 0–15, reset) become `fallback` so they can be blended.
pub fn rgb_or(c: Color, fallback: Color) -> Color {
    match c {
        Color::Rgb(..) => c,
        Color::Indexed(n @ 16..=231) => {
            let i = n - 16;
            let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color::Rgb(step(i / 36), step(i / 6 % 6), step(i % 6))
        }
        Color::Indexed(n @ 232..=255) => {
            let v = 8 + (n - 232) * 10;
            Color::Rgb(v, v, v)
        }
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(bg: &str, fg: &str) -> ThemeSpec {
        ThemeSpec { background: Some(bg.into()), foreground: Some(fg.into()), ..Default::default() }
    }

    #[test]
    fn parse_hex_variants() {
        assert_eq!(parse_hex("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(parse_hex("#1e1e2e"), Some(Color::Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(parse_hex("#1E1E2EFF"), Some(Color::Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(parse_hex(" #abc "), Some(Color::Rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(parse_hex("rgb:1e/1e/2e"), Some(Color::Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(parse_hex("rgb:f/8/0"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(parse_hex("rgb:ffff/0000/8080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(parse_hex("1e1e2e"), None);
        assert_eq!(parse_hex("rgb:1/2"), None);
        assert_eq!(parse_hex("rgb:1/2/3/4"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn blend_and_luma() {
        let black = Color::Rgb(0, 0, 0);
        let white = Color::Rgb(255, 255, 255);
        assert_eq!(blend(black, white, 0.0), black);
        assert_eq!(blend(black, white, 1.0), white);
        assert_eq!(blend(black, white, 0.5), Color::Rgb(128, 128, 128));
        assert_eq!(blend(black, white, 7.0), white);
        assert_eq!(blend(black, white, f32::NAN), black);
        assert_eq!(blend(Color::Red, white, 0.5), Color::Red);
        assert_eq!(blend(black, Color::Reset, 0.5), black);
        assert_eq!(luma(white), 1.0);
        assert_eq!(luma(black), 0.0);
        assert_eq!(luma(Color::Reset), 0.0);
        assert!(luma(Color::Rgb(0x1e, 0x1e, 0x2e)) < 0.5);
        assert!(luma(Color::Rgb(0xef, 0xf1, 0xf5)) > 0.5);
    }

    #[test]
    fn rgb_or_maps_cube_and_greys() {
        assert_eq!(rgb_or(Color::Indexed(16), Color::Reset), Color::Rgb(0, 0, 0));
        assert_eq!(rgb_or(Color::Indexed(231), Color::Reset), Color::Rgb(255, 255, 255));
        assert_eq!(rgb_or(Color::Indexed(196), Color::Reset), Color::Rgb(255, 0, 0));
        assert_eq!(rgb_or(Color::Indexed(232), Color::Reset), Color::Rgb(8, 8, 8));
        assert_eq!(rgb_or(Color::Indexed(255), Color::Reset), Color::Rgb(238, 238, 238));
        assert_eq!(rgb_or(Color::Indexed(1), Color::Reset), Color::Reset);
        assert_eq!(rgb_or(Color::Red, Color::Black), Color::Black);
        assert_eq!(rgb_or(Color::Rgb(1, 2, 3), Color::Black), Color::Rgb(1, 2, 3));
    }

    #[test]
    fn from_spec_fallbacks() {
        let theme = Theme::dark();
        assert_eq!(theme.bg, FALLBACK_BG);
        assert_eq!(theme.text, FALLBACK_FG);
        assert_eq!(theme.accent, FALLBACK_ACCENT);
        assert_eq!(theme.border_focus, FALLBACK_ACCENT);
        assert_eq!(theme.accent_fg, theme.bg);
        assert_eq!(theme.match_hl, FALLBACK_MATCH);
        assert_eq!(theme.danger, FALLBACK_DANGER);
        assert_eq!(theme.selection_fg, theme.text);
        assert!(theme.is_dark());
        assert!(luma(theme.surface) > luma(theme.bg));
        assert!(luma(theme.surface_hi) > luma(theme.surface));
        assert!(luma(theme.text_muted) < luma(theme.text));
        assert!(luma(theme.text_dim) < luma(theme.text_muted));
        assert!((theme.backdrop_dim - 0.55).abs() < f32::EPSILON);
    }

    #[test]
    fn from_spec_accent_prefers_spec_then_ansi_blue() {
        let mut spec = spec("#000000", "#ffffff");
        spec.ansi = (0..8).map(|i| format!("#0000{i:02x}")).collect();
        spec.brights = (0..8).map(|i| format!("#ff00{i:02x}")).collect();
        let theme = Theme::from_spec(&spec);
        assert_eq!(theme.accent, Color::Rgb(0, 0, 4));
        assert_eq!(theme.danger, Color::Rgb(0, 0, 1));
        assert_eq!(theme.success, Color::Rgb(0, 0, 2));
        assert_eq!(theme.warning, Color::Rgb(0, 0, 3));
        assert_eq!(theme.match_hl, Color::Rgb(255, 0, 3));

        spec.accent = Some("#123456".into());
        assert_eq!(Theme::from_spec(&spec).accent, Color::Rgb(0x12, 0x34, 0x56));

        spec.accent = Some("nope".into());
        assert_eq!(Theme::from_spec(&spec).accent, Color::Rgb(0, 0, 4));
    }

    #[test]
    fn from_spec_light_theme_darkens_surfaces() {
        let theme = Theme::from_spec(&spec("#eff1f5", "#4c4f69"));
        assert!(!theme.is_dark());
        assert!(luma(theme.surface) < luma(theme.bg));
        assert!(luma(theme.surface_hi) < luma(theme.surface));
        assert!(luma(theme.text_muted) > luma(theme.text));
    }

    #[test]
    fn from_spec_short_palettes_fall_back() {
        let mut spec = spec("#000000", "#ffffff");
        spec.ansi = vec!["#111111".into()];
        let theme = Theme::from_spec(&spec);
        assert_eq!(theme.accent, FALLBACK_ACCENT);
        assert_eq!(theme.danger, FALLBACK_DANGER);
    }
}
