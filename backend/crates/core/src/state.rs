use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::context::PaneId;

/// `$NARDO_STATE_DIR/state.json`, else `$XDG_STATE_HOME/wez-nardo/state.json`, else `~/.local/state/wez-nardo/state.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    #[serde(default)]
    pub mru: Vec<PaneId>,
    #[serde(default)]
    pub last_query: String,
}

pub const MRU_MAX: usize = 50;

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl State {
    pub fn path() -> Option<PathBuf> {
        let dir = env_dir("NARDO_STATE_DIR")
            .or_else(|| env_dir("XDG_STATE_HOME").map(|p| p.join("wez-nardo")))
            .or_else(|| env_dir("HOME").map(|p| p.join(".local/state/wez-nardo")))?;
        Some(dir.join("state.json"))
    }

    pub fn load() -> State {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let Some(dir) = path.parent() else { return };
        let Ok(raw) = serde_json::to_string(self) else {
            return;
        };
        let tmp = dir.join(format!(".state.{}.tmp", std::process::id()));
        let _ = std::fs::create_dir_all(dir);
        if std::fs::write(&tmp, raw).is_ok() && std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    pub fn touch(&mut self, pane: PaneId) {
        self.mru.retain(|p| *p != pane);
        self.mru.insert(0, pane);
        self.mru.truncate(MRU_MAX);
    }

    pub fn rank(&self, pane: PaneId) -> Option<usize> {
        self.mru.iter().position(|p| *p == pane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_moves_to_front_and_caps_length() {
        let mut state = State::default();
        state.touch(1);
        state.touch(2);
        state.touch(1);
        assert_eq!(state.mru, [1, 2]);
        assert_eq!(state.rank(2), Some(1));
        assert_eq!(state.rank(9), None);

        for pane in 100..(100 + MRU_MAX as PaneId + 5) {
            state.touch(pane);
        }
        assert_eq!(state.mru.len(), MRU_MAX);
        assert_eq!(state.mru[0], 100 + MRU_MAX as PaneId + 4);
        assert!(!state.mru.contains(&1));
    }

    #[test]
    fn state_json_round_trips_with_defaults() {
        let state: State = serde_json::from_str("{}").unwrap();
        assert_eq!(state, State::default());
        let state = State { mru: vec![3, 1], last_query: "vim".into() };
        let raw = serde_json::to_string(&state).unwrap();
        assert_eq!(serde_json::from_str::<State>(&raw).unwrap(), state);
    }
}
