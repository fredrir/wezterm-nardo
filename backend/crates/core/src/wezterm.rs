use std::path::PathBuf;
use std::process::{Command, Stdio};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::context::{PaneId, TabId, WindowId};

/// One row of `wezterm cli list --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaneRecord {
    pub window_id: WindowId,
    pub tab_id: TabId,
    pub pane_id: PaneId,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub size: PaneSize,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub cursor_x: u32,
    #[serde(default)]
    pub cursor_y: u32,
    #[serde(default)]
    pub left_col: u32,
    #[serde(default)]
    pub top_row: u32,
    #[serde(default)]
    pub tab_title: String,
    #[serde(default)]
    pub window_title: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_zoomed: bool,
    #[serde(default)]
    pub tty_name: Option<String>,
    /// Present on newer builds only.
    #[serde(default)]
    pub domain_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaneSize {
    pub rows: u16,
    pub cols: u16,
    #[serde(default)]
    pub pixel_width: u32,
    #[serde(default)]
    pub pixel_height: u32,
    #[serde(default)]
    pub dpi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Left,
    Right,
    Top,
    Bottom,
}

impl SplitDirection {
    fn flag(self) -> &'static str {
        match self {
            SplitDirection::Left => "--left",
            SplitDirection::Right => "--right",
            SplitDirection::Top => "--top",
            SplitDirection::Bottom => "--bottom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewTabTarget {
    Window(WindowId),
    NewWindow { workspace: Option<String> },
}

impl NewTabTarget {
    fn push_args(&self, args: &mut Vec<String>) {
        match self {
            NewTabTarget::Window(id) => args.extend(["--window-id".into(), id.to_string()]),
            NewTabTarget::NewWindow { workspace } => {
                args.push("--new-window".into());
                if let Some(ws) = workspace {
                    args.extend(["--workspace".into(), ws.clone()]);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnSpec {
    pub domain: Option<String>,
    pub target: Option<NewTabTarget>,
    pub cwd: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum WeztermError {
    #[error("wezterm not found: {0}")]
    NotFound(String),
    #[error("wezterm cli {cmd}: {stderr}")]
    Failed { cmd: String, stderr: String },
    #[error("wezterm cli {cmd}: bad output: {source}")]
    BadOutput { cmd: String, source: anyhow::Error },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, WeztermError>;

/// Everything the apps need from the mux. `Cli` shells out; tests point `NARDO_WEZTERM` at a fake.
pub trait Wezterm: Send + Sync {
    fn list(&self) -> Result<Vec<PaneRecord>>;
    /// `lines`: how many lines back into scrollback (`--start-line -N`); `None` = screen only.
    fn get_text(&self, pane: PaneId, lines: Option<u32>, escapes: bool) -> Result<String>;
    fn activate_pane(&self, pane: PaneId) -> Result<()>;
    fn activate_tab(&self, tab: TabId) -> Result<()>;
    fn kill_pane(&self, pane: PaneId) -> Result<()>;
    fn move_pane_to_new_tab(&self, pane: PaneId, target: NewTabTarget) -> Result<()>;
    fn move_pane_into_split(&self, pane: PaneId, next_to: PaneId, dir: SplitDirection) -> Result<()>;
    fn split_pane(&self, pane: PaneId, dir: SplitDirection, cwd: Option<&str>) -> Result<PaneId>;
    fn spawn(&self, spec: &SpawnSpec) -> Result<PaneId>;
    fn set_tab_title(&self, tab: TabId, title: &str) -> Result<()>;
    fn set_window_title(&self, window: WindowId, title: &str) -> Result<()>;
    fn rename_workspace(&self, workspace: &str, new_name: &str) -> Result<()>;
    fn zoom_pane(&self, pane: PaneId, zoom: Option<bool>) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Cli {
    pub exe: PathBuf,
    pub class: Option<String>,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl Cli {
    /// `explicit` → `$NARDO_WEZTERM` → `$WEZTERM_EXECUTABLE`'s dir + `wezterm` → `wezterm` on PATH.
    pub fn from_env(explicit: Option<PathBuf>) -> Self {
        let sibling = || {
            env_path("WEZTERM_EXECUTABLE")
                .and_then(|gui| gui.parent().map(|dir| dir.join("wezterm")))
                .filter(|p| p.is_file())
        };
        let exe =
            explicit.or_else(|| env_path("NARDO_WEZTERM")).or_else(sibling).unwrap_or_else(|| PathBuf::from("wezterm"));
        let class = std::env::var("NARDO_WEZTERM_CLASS").ok().filter(|c| !c.is_empty());
        Self { exe, class }
    }

    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.arg("cli");
        if let Some(class) = &self.class {
            cmd.arg("--class").arg(class);
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let cmd = args.join(" ");
        let output = self.command().args(args).output().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => WeztermError::NotFound(self.exe.display().to_string()),
            _ => WeztermError::Io(e),
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stderr = if stderr.is_empty() { output.status.to_string() } else { stderr };
            return Err(WeztermError::Failed { cmd, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn run_owned(&self, args: &[String]) -> Result<String> {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&refs)
    }

    fn run_for_pane_id(&self, args: &[String]) -> Result<PaneId> {
        let out = self.run_owned(args)?;
        out.trim().parse().map_err(|e| WeztermError::BadOutput {
            cmd: args.join(" "),
            source: anyhow::anyhow!("expected a pane id, got {:?}: {e}", out.trim()),
        })
    }
}

impl Wezterm for Cli {
    fn list(&self) -> Result<Vec<PaneRecord>> {
        let args = ["list", "--format", "json"];
        let out = self.run(&args)?;
        if out.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&out).map_err(|e| WeztermError::BadOutput { cmd: args.join(" "), source: e.into() })
    }

    fn get_text(&self, pane: PaneId, lines: Option<u32>, escapes: bool) -> Result<String> {
        let pane = pane.to_string();
        let start = lines.map(|n| format!("-{n}"));
        let mut args = vec!["get-text", "--pane-id", &pane];
        if escapes {
            args.push("--escapes");
        }
        if let Some(start) = &start {
            args.extend(["--start-line", start]);
        }
        self.run(&args)
    }

    fn activate_pane(&self, pane: PaneId) -> Result<()> {
        self.run(&["activate-pane", "--pane-id", &pane.to_string()]).map(drop)
    }

    fn activate_tab(&self, tab: TabId) -> Result<()> {
        self.run(&["activate-tab", "--tab-id", &tab.to_string()]).map(drop)
    }

    fn kill_pane(&self, pane: PaneId) -> Result<()> {
        self.run(&["kill-pane", "--pane-id", &pane.to_string()]).map(drop)
    }

    fn move_pane_to_new_tab(&self, pane: PaneId, target: NewTabTarget) -> Result<()> {
        let mut args = vec!["move-pane-to-new-tab".to_string(), "--pane-id".into(), pane.to_string()];
        target.push_args(&mut args);
        self.run_owned(&args).map(drop)
    }

    fn move_pane_into_split(&self, pane: PaneId, next_to: PaneId, dir: SplitDirection) -> Result<()> {
        let args = [
            "split-pane".to_string(),
            "--pane-id".into(),
            next_to.to_string(),
            "--move-pane-id".into(),
            pane.to_string(),
            dir.flag().into(),
        ];
        self.run_owned(&args).map(drop)
    }

    fn split_pane(&self, pane: PaneId, dir: SplitDirection, cwd: Option<&str>) -> Result<PaneId> {
        let mut args = vec!["split-pane".to_string(), "--pane-id".into(), pane.to_string(), dir.flag().into()];
        if let Some(cwd) = cwd {
            args.extend(["--cwd".into(), cwd.into()]);
        }
        self.run_for_pane_id(&args)
    }

    fn spawn(&self, spec: &SpawnSpec) -> Result<PaneId> {
        let mut args = vec!["spawn".to_string()];
        if let Some(domain) = &spec.domain {
            args.extend(["--domain-name".into(), domain.clone()]);
        }
        if let Some(target) = &spec.target {
            target.push_args(&mut args);
        }
        if let Some(cwd) = &spec.cwd {
            args.extend(["--cwd".into(), cwd.clone()]);
        }
        if !spec.args.is_empty() {
            args.push("--".into());
            args.extend(spec.args.iter().cloned());
        }
        self.run_for_pane_id(&args)
    }

    fn set_tab_title(&self, tab: TabId, title: &str) -> Result<()> {
        self.run(&["set-tab-title", "--tab-id", &tab.to_string(), title]).map(drop)
    }

    fn set_window_title(&self, window: WindowId, title: &str) -> Result<()> {
        self.run(&["set-window-title", "--window-id", &window.to_string(), title]).map(drop)
    }

    fn rename_workspace(&self, workspace: &str, new_name: &str) -> Result<()> {
        self.run(&["rename-workspace", "--workspace", workspace, new_name]).map(drop)
    }

    fn zoom_pane(&self, pane: PaneId, zoom: Option<bool>) -> Result<()> {
        let mode = match zoom {
            None => "--toggle",
            Some(true) => "--zoom",
            Some(false) => "--unzoom",
        };
        self.run(&["zoom-pane", "--pane-id", &pane.to_string(), mode]).map(drop)
    }
}

pub const DEFAULT_USERVAR: &str = "nardo";
pub const ROLE: &str = "launcher";

/// Payload for the `nardo` user var; `n` makes equal payloads distinct (WezTerm fires on change only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Action {
    AttachDomain {
        domain: String,
    },
    DetachDomain {
        domain: String,
    },
    Focus {
        pane_id: PaneId,
    },
    Run {
        name: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    Done {
        exit: String,
    },
    Error {
        message: String,
    },
}

pub fn user_var(name: &str, value: &str) -> String {
    format!("\x1b]1337;SetUserVar={name}={}\x07", base64::engine::general_purpose::STANDARD.encode(value))
}

pub fn action_payload(action: &Action, n: u64) -> String {
    let mut v = serde_json::to_value(action).expect("action serializes");
    v["v"] = 1.into();
    v["n"] = n.into();
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_var_is_osc_1337() {
        assert_eq!(user_var("nardo", "x"), "\x1b]1337;SetUserVar=nardo=eA==\x07");
    }

    #[test]
    fn list_json_parses_installed_build_shape() {
        let raw = r#"[{"window_id":10,"tab_id":30,"pane_id":46,"workspace":"default","size":{"rows":58,"cols":98,"pixel_width":1470,"pixel_height":1740,"dpi":144},"title":"zsh","cwd":"file://archie/home/f","cursor_x":86,"cursor_y":54,"cursor_shape":"Default","cursor_visibility":"Visible","left_col":0,"top_row":0,"tab_title":"","window_title":"w","is_active":true,"is_zoomed":false,"tty_name":"/dev/pts/0"}]"#;
        let recs: Vec<PaneRecord> = serde_json::from_str(raw).unwrap();
        assert_eq!(recs[0].pane_id, 46);
        assert_eq!(recs[0].size.cols, 98);
    }

    #[test]
    fn action_payload_has_tag_version_and_counter() {
        let raw = action_payload(&Action::AttachDomain { domain: "archie-cable".into() }, 7);
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["t"], "attach_domain");
        assert_eq!(v["domain"], "archie-cable");
        assert_eq!(v["v"], 1);
        assert_eq!(v["n"], 7);

        let raw = action_payload(&Action::Run { name: "x".into(), args: serde_json::json!({"a": 1}) }, 8);
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["t"], "run");
        assert_eq!(v["args"]["a"], 1);
        assert_eq!(v["n"], 8);
    }

    #[test]
    fn command_is_exe_cli_with_optional_class() {
        let argv =
            |cli: &Cli| -> Vec<String> { cli.command().get_args().map(|a| a.to_string_lossy().into_owned()).collect() };
        let plain = Cli { exe: "/usr/bin/wezterm".into(), class: None };
        assert_eq!(argv(&plain), ["cli"]);
        let classed = Cli { exe: "/usr/bin/wezterm".into(), class: Some("sandbox".into()) };
        assert_eq!(argv(&classed), ["cli", "--class", "sandbox"]);
        assert_eq!(classed.command().get_program(), "/usr/bin/wezterm");
    }

    #[test]
    fn missing_executable_is_not_found() {
        let cli = Cli { exe: "/nonexistent/wezterm-nardo-test".into(), class: None };
        assert!(matches!(cli.list(), Err(WeztermError::NotFound(_))));
    }
}
