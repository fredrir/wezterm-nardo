use std::collections::HashMap;
use std::time::{Duration, Instant};

use nardo_core::app::JobToken;
use nardo_core::context::PaneId;
use ratatui::text::Text;

pub const DEBOUNCE: Duration = Duration::from_millis(40);
const CACHE_MAX: usize = 64;

/// Cached `get-text` results plus the debounce / in-flight bookkeeping. A cached pane renders
/// instantly and is still refreshed once the selection has rested for `DEBOUNCE`.
#[derive(Default)]
pub struct PreviewCache {
    texts: HashMap<PaneId, Text<'static>>,
    in_flight: Option<(PaneId, JobToken)>,
    pending: Option<(PaneId, Instant)>,
    pub scroll: u16,
    pub visible: bool,
}

impl PreviewCache {
    pub fn new(visible: bool) -> Self {
        Self { visible, ..Self::default() }
    }

    pub fn get(&self, pane: PaneId) -> Option<&Text<'static>> {
        self.texts.get(&pane)
    }

    pub fn is_loading(&self, pane: PaneId) -> bool {
        self.pending.is_some_and(|(p, _)| p == pane) || self.in_flight.as_ref().is_some_and(|(p, _)| *p == pane)
    }

    /// Selection moved to `pane`: (re)start the debounce; cached or in-flight panes fetch nothing.
    pub fn want(&mut self, pane: PaneId, now: Instant) {
        self.scroll = 0;
        if self.texts.contains_key(&pane) || self.in_flight.as_ref().is_some_and(|(p, _)| *p == pane) {
            self.pending = None;
        } else {
            self.pending = Some((pane, now));
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Pane whose debounce elapsed, taken out of `pending`.
    pub fn due(&mut self, now: Instant, immediate: bool) -> Option<PaneId> {
        let (pane, since) = self.pending?;
        (immediate || now.duration_since(since) >= DEBOUNCE).then(|| {
            self.pending = None;
            pane
        })
    }

    pub fn start(&mut self, pane: PaneId, token: JobToken) {
        if let Some((_, old)) = self.in_flight.replace((pane, token)) {
            old.cancel();
        }
    }

    pub fn finish(&mut self, pane: PaneId, text: Option<Text<'static>>) {
        if self.in_flight.as_ref().is_some_and(|(p, _)| *p == pane) {
            self.in_flight = None;
        }
        if let Some(text) = text {
            if self.texts.len() >= CACHE_MAX && !self.texts.contains_key(&pane) {
                self.texts.clear();
            }
            self.texts.insert(pane, text);
        }
    }

    /// Mux changed under us (kill / move / spawn): drop everything cached.
    pub fn invalidate(&mut self) {
        self.texts.clear();
        self.pending = None;
        if let Some((_, token)) = self.in_flight.take() {
            token.cancel();
        }
    }

    pub fn scroll_by(&mut self, delta: i32) {
        self.scroll = (self.scroll as i32 + delta).clamp(0, u16::MAX as i32) as u16;
    }
}

/// Expand tabs before ansi parsing (ratatui keeps `\t` literal).
pub fn normalize(raw: &str) -> String {
    raw.replace('\t', "    ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_then_fetch_then_cache() {
        let mut cache = PreviewCache::new(true);
        let t0 = Instant::now();
        cache.want(7, t0);
        assert!(cache.is_loading(7));
        assert_eq!(cache.due(t0, false), None);
        assert_eq!(cache.due(t0 + DEBOUNCE, false), Some(7));
        assert_eq!(cache.due(t0 + DEBOUNCE, false), None);
        cache.start(7, JobToken::default());
        assert!(cache.is_loading(7));
        cache.finish(7, Some(Text::raw("hi")));
        assert!(!cache.is_loading(7));
        assert!(cache.get(7).is_some());
    }

    #[test]
    fn immediate_skips_debounce_and_restart_cancels_previous() {
        let mut cache = PreviewCache::new(true);
        let now = Instant::now();
        cache.want(1, now);
        assert_eq!(cache.due(now, true), Some(1));
        let first = JobToken::default();
        cache.start(1, first.clone());
        cache.start(2, JobToken::default());
        assert!(first.cancelled());
        cache.want(2, now);
        assert!(!cache.has_pending());
        cache.invalidate();
        assert!(!cache.is_loading(2));
    }

    #[test]
    fn scroll_clamps_and_resets_on_selection() {
        let mut cache = PreviewCache::new(true);
        cache.scroll_by(-5);
        assert_eq!(cache.scroll, 0);
        cache.scroll_by(3);
        assert_eq!(cache.scroll, 3);
        cache.want(1, Instant::now());
        assert_eq!(cache.scroll, 0);
    }
}
