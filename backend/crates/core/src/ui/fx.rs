use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use tachyonfx::{Effect, EffectRenderer, Interpolation, fx};

use crate::ui::theme::Theme;

const OPEN_MS: u32 = 180;
const CLOSE_MS: u32 = 120;
const HOVER_MS: u32 = 140;

/// `$NARDO_REDUCE_MOTION` / `$REDUCE_MOTION` set to anything but `0`/`false`/`no`/`off`.
pub fn reduce_motion() -> bool {
    ["NARDO_REDUCE_MOTION", "REDUCE_MOTION"].iter().any(|name| std::env::var(name).is_ok_and(|v| truthy(&v)))
}

fn truthy(value: &str) -> bool {
    !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "no" | "off")
}

/// Entrance/exit effects. All no-ops when `enabled` is false (`presentation.animations`,
/// headless, or `reduce_motion`).
pub struct Effects {
    pub enabled: bool,
    open: Option<Effect>,
    close: Option<Effect>,
    hover: Option<Effect>,
}

impl Effects {
    pub fn new(enabled: bool) -> Self {
        Self { enabled: enabled && !reduce_motion(), open: None, close: None, hover: None }
    }

    /// Modal pops in: coalesce + fade from bg over ~180ms.
    pub fn open(&mut self, theme: &Theme) {
        if !self.enabled {
            return;
        }
        let timer = (OPEN_MS, Interpolation::QuadOut);
        self.close = None;
        self.open = Some(fx::parallel(&[fx::coalesce(timer), fx::fade_from_fg(theme.bg, timer)]));
    }

    /// Modal dissolves out over ~120ms; `closing()` reports true until done.
    pub fn close(&mut self, theme: &Theme) {
        if !self.enabled {
            return;
        }
        let timer = (CLOSE_MS, Interpolation::QuadOut);
        self.open = None;
        self.hover = None;
        self.close = Some(fx::parallel(&[fx::dissolve(timer), fx::fade_to_fg(theme.bg, timer)]));
    }

    /// Subtle highlight sweep on the newly selected row.
    pub fn hover(&mut self, theme: &Theme, row: Rect) {
        if !self.enabled || row.is_empty() {
            return;
        }
        let timer = (HOVER_MS, Interpolation::QuadOut);
        self.hover = Some(fx::fade_from_fg(theme.accent, timer).with_area(row));
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, elapsed: Duration) {
        if !self.enabled || area.is_empty() {
            self.open = None;
            self.hover = None;
            self.close = None;
            return;
        }
        let tick: tachyonfx::Duration = elapsed.into();
        for slot in [&mut self.open, &mut self.hover, &mut self.close] {
            let Some(effect) = slot else { continue };
            frame.render_effect(effect, area, tick);
            if effect.done() {
                *slot = None;
            }
        }
    }

    pub fn running(&self) -> bool {
        [&self.open, &self.close, &self.hover].iter().any(|e| e.as_ref().is_some_and(|e| e.running()))
    }

    pub fn closing(&self) -> bool {
        self.close.as_ref().is_some_and(|e| e.running())
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;

    use super::*;

    fn effects(enabled: bool) -> Effects {
        Effects { enabled, open: None, close: None, hover: None }
    }

    fn drive(effects: &mut Effects, area: Rect, frames: usize, step_ms: u64) {
        let mut terminal = Terminal::new(TestBackend::new(30, 8)).unwrap();
        for _ in 0..frames {
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new("some text to animate"), area);
                    effects.render(frame, area, Duration::from_millis(step_ms));
                })
                .unwrap();
        }
    }

    #[test]
    fn truthy_values() {
        assert!(truthy("1"));
        assert!(truthy("yes"));
        assert!(!truthy("0"));
        assert!(!truthy("false"));
        assert!(!truthy(" OFF "));
        assert!(!truthy(""));
    }

    #[test]
    fn disabled_effects_are_noops() {
        let theme = Theme::dark();
        let mut fx = effects(false);
        fx.open(&theme);
        fx.hover(&theme, Rect::new(0, 0, 5, 1));
        fx.close(&theme);
        assert!(!fx.running());
        assert!(!fx.closing());
        drive(&mut fx, Rect::new(0, 0, 20, 4), 2, 16);
        assert!(!fx.running());
    }

    #[test]
    fn open_runs_then_finishes() {
        let theme = Theme::dark();
        let mut fx = effects(true);
        fx.open(&theme);
        fx.hover(&theme, Rect::new(2, 1, 10, 1));
        assert!(fx.running());
        assert!(!fx.closing());
        drive(&mut fx, Rect::new(0, 0, 20, 4), 3, 16);
        assert!(fx.running());
        drive(&mut fx, Rect::new(0, 0, 20, 4), 20, 16);
        assert!(!fx.running());
        assert!(fx.open.is_none());
        assert!(fx.hover.is_none());
    }

    #[test]
    fn close_reports_closing_until_done() {
        let theme = Theme::dark();
        let mut fx = effects(true);
        fx.open(&theme);
        fx.close(&theme);
        assert!(fx.closing());
        assert!(fx.open.is_none());
        drive(&mut fx, Rect::new(0, 0, 20, 4), 1, 200);
        assert!(!fx.closing());
        assert!(!fx.running());
    }

    #[test]
    fn render_survives_tiny_areas() {
        let theme = Theme::dark();
        for area in crate::ui::test_util::tiny_areas() {
            let mut fx = effects(true);
            fx.open(&theme);
            fx.hover(&theme, area);
            drive(&mut fx, area, 3, 100);
            fx.close(&theme);
            drive(&mut fx, area, 3, 100);
        }
    }
}
