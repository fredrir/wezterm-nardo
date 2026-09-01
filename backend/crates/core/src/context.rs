use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub type PaneId = u64;
pub type TabId = u64;
pub type WindowId = u64;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    #[serde(default = "one")]
    pub v: u32,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub domains: Vec<Domain>,
    #[serde(default)]
    pub panes: BTreeMap<PaneId, PaneExtra>,
    #[serde(default)]
    pub workspaces: Workspaces,
    #[serde(default)]
    pub theme: ThemeSpec,
    #[serde(default)]
    pub presentation: Presentation,
    #[serde(default)]
    pub options: serde_json::Value,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Origin {
    pub pane_id: Option<PaneId>,
    pub tab_id: Option<TabId>,
    pub window_id: Option<WindowId>,
    pub workspace: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DomainKind {
    Local,
    Unix,
    Tls,
    Ssh,
    Exec,
    Wsl,
    Serial,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DomainState {
    Attached,
    #[default]
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: DomainKind,
    #[serde(default)]
    pub state: DomainState,
    #[serde(default = "yes")]
    pub spawnable: bool,
    #[serde(default)]
    pub has_panes: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaneExtra {
    pub domain: Option<String>,
    pub process: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub unseen: bool,
    #[serde(default)]
    pub alt_screen: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspaces {
    pub active: Option<String>,
    #[serde(default)]
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeSpec {
    pub background: Option<String>,
    pub foreground: Option<String>,
    #[serde(default)]
    pub ansi: Vec<String>,
    #[serde(default)]
    pub brights: Vec<String>,
    pub selection_bg: Option<String>,
    pub selection_fg: Option<String>,
    pub cursor_bg: Option<String>,
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Overlay,
    Tab,
    Window,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    #[default]
    Dim,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presentation {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default = "default_max_width")]
    pub max_width: u16,
    #[serde(default = "default_max_height")]
    pub max_height: u16,
    #[serde(default)]
    pub backdrop: Backdrop,
    #[serde(default = "yes")]
    pub animations: bool,
}

fn default_width() -> f32 {
    0.72
}
fn default_height() -> f32 {
    0.7
}
fn default_max_width() -> u16 {
    128
}
fn default_max_height() -> u16 {
    42
}

impl Default for Presentation {
    fn default() -> Self {
        serde_json::from_str("{}").expect("all fields default")
    }
}

impl Context {
    /// `path` → file, else `$NARDO_CONTEXT`, else empty context.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let path = match path {
            Some(p) => Some(p.to_path_buf()),
            None => std::env::var_os("NARDO_CONTEXT").map(Into::into),
        };
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let raw = std::fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("context {}: {e}", path.display()))?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn domain(&self, name: &str) -> Option<&Domain> {
        self.domains.iter().find(|d| d.name == name)
    }

    pub fn own_pane_id() -> Option<PaneId> {
        std::env::var("WEZTERM_PANE").ok()?.parse().ok()
    }
}
