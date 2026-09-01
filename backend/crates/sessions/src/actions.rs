use nardo_core::app::{Cx, Flow, Outcome};
use nardo_core::context::{DomainState, PaneId, WindowId};
use nardo_core::mux::split_cwd;
use nardo_core::ui::widgets::SearchState;
use nardo_core::wezterm::{Action, NewTabTarget, Result, SpawnSpec, SplitDirection, Wezterm};

use crate::model::{self, Kind, Msg, Row};
use crate::state::{KillWhat, MoveChoice, Overlay, RenameTarget, SessionsApp};

pub enum MoveOp {
    PaneToTab { pane: PaneId, window: WindowId },
    PaneToWindow { pane: PaneId, workspace: Option<String> },
    PaneIntoSplit { pane: PaneId, next_to: PaneId },
    TabToWindow { panes: Vec<PaneId>, window: Option<WindowId>, workspace: Option<String> },
}

impl MoveOp {
    fn run(&self, wezterm: &dyn Wezterm) -> Result<()> {
        match self {
            MoveOp::PaneToTab { pane, window } => wezterm.move_pane_to_new_tab(*pane, NewTabTarget::Window(*window)),
            MoveOp::PaneToWindow { pane, workspace } => {
                wezterm.move_pane_to_new_tab(*pane, NewTabTarget::NewWindow { workspace: workspace.clone() })
            }
            MoveOp::PaneIntoSplit { pane, next_to } => {
                wezterm.move_pane_into_split(*pane, *next_to, SplitDirection::Bottom)
            }
            MoveOp::TabToWindow { panes, window, workspace } => {
                let Some((first, rest)) = panes.split_first() else {
                    return Ok(());
                };
                let target = match window {
                    Some(w) => NewTabTarget::Window(*w),
                    None => NewTabTarget::NewWindow { workspace: workspace.clone() },
                };
                wezterm.move_pane_to_new_tab(*first, target)?;
                // WezTerm has no move-tab cli: the split layout flattens into bottom splits next to the first pane.
                rest.iter().try_for_each(|p| wezterm.move_pane_into_split(*p, *first, SplitDirection::Bottom))
            }
        }
    }
}

impl SessionsApp {
    fn job(&self, cx: &mut Cx<Msg>, f: impl FnOnce(&dyn Wezterm) -> Result<()> + Send + 'static) -> Flow {
        let wezterm = cx.wezterm.clone();
        cx.spawn(move || Msg::Done(f(wezterm.as_ref())));
        Flow::Continue
    }

    fn create(&self, cx: &mut Cx<Msg>, f: impl FnOnce(&dyn Wezterm) -> Result<PaneId> + Send + 'static) -> Flow {
        let wezterm = cx.wezterm.clone();
        cx.spawn(move || Msg::Created(f(wezterm.as_ref())));
        Flow::Continue
    }

    fn activate(&self, pane: PaneId, cx: &mut Cx<Msg>) -> Flow {
        let wezterm = cx.wezterm.clone();
        cx.spawn(move || Msg::Switched { pane_id: pane, result: wezterm.activate_pane(pane) });
        Flow::Continue
    }

    fn hand_off(&self, action: Action, cx: &mut Cx<Msg>) -> Flow {
        cx.emit(action);
        cx.fx.close(&cx.theme);
        Flow::Exit(Outcome::handed_off())
    }

    fn selected(&self) -> Option<Row> {
        self.selected_row().cloned()
    }

    fn pane_cwd(&self, pane: PaneId) -> Option<String> {
        self.mux.pane(pane).and_then(|p| p.cwd.as_deref()).map(|c| split_cwd(c).1.to_string())
    }

    /// Panes of the selected row's subtree, never the launcher's own pane.
    fn subtree_panes(&self, row: &Row, cx: &Cx<Msg>) -> Vec<PaneId> {
        let ids: Vec<PaneId> = match row.kind {
            Kind::Pane => vec![row.id],
            Kind::Tab => self.mux.tab(row.id).map(|t| t.panes.iter().map(|p| p.id).collect()).unwrap_or_default(),
            Kind::Window => self
                .mux
                .window(row.id)
                .map(|w| w.tabs.iter().flat_map(|t| &t.panes).map(|p| p.id).collect())
                .unwrap_or_default(),
            Kind::Domain => Vec::new(),
        };
        ids.into_iter().filter(|id| Some(*id) != cx.own_pane).collect()
    }

    fn window_for_domain(&self, domain: &str) -> Option<WindowId> {
        self.mux.windows.iter().find(|w| w.tabs.iter().flat_map(|t| &t.panes).any(|p| p.domain == domain)).map(|w| w.id)
    }

    pub(crate) fn switch(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        match row.kind {
            Kind::Domain if row.domain_state() == Some(DomainState::Detached) => {
                self.hand_off(Action::AttachDomain { domain: row.domain }, cx)
            }
            Kind::Domain => {
                let first =
                    self.mux.panes().find(|p| p.domain == row.domain && Some(p.id) != cx.own_pane).map(|p| p.id);
                match first {
                    Some(pane) => self.activate(pane, cx),
                    None => {
                        let spec = SpawnSpec {
                            domain: Some(row.domain),
                            target: Some(NewTabTarget::NewWindow { workspace: None }),
                            ..SpawnSpec::default()
                        };
                        self.create(cx, move |wz| wz.spawn(&spec))
                    }
                }
            }
            Kind::Tab => {
                let Some(pane) = row.pane else {
                    return Flow::Continue;
                };
                let tab = row.id;
                let wezterm = cx.wezterm.clone();
                cx.spawn(move || Msg::Switched { pane_id: pane, result: wezterm.activate_tab(tab) });
                Flow::Continue
            }
            Kind::Window | Kind::Pane => match row.pane {
                Some(pane) => self.activate(pane, cx),
                None => Flow::Continue,
            },
        }
    }

    pub(crate) fn kill(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        if row.kind == Kind::Domain {
            return match row.domain_state() {
                Some(DomainState::Attached) => {
                    self.overlay = Overlay::Confirm {
                        what: KillWhat::Detach(row.domain.clone()),
                        title: "Detach domain".into(),
                        body: format!("Detach {}? Its panes keep running on the remote side.", row.domain),
                    };
                    Flow::Continue
                }
                _ => self.toast("domain is already detached", cx),
            };
        }
        let panes = self.subtree_panes(&row, cx);
        if panes.is_empty() {
            return self.toast("nothing to kill", cx);
        }
        if !self.options.confirm_kill {
            return self.kill_panes(panes, cx);
        }
        let what = match row.kind {
            Kind::Pane => "pane",
            Kind::Tab => "tab",
            _ => "window",
        };
        let count = if panes.len() > 1 { format!(" ({} panes)", panes.len()) } else { String::new() };
        self.overlay = Overlay::Confirm {
            what: KillWhat::Panes(panes),
            title: format!("Kill {what}"),
            body: format!("Kill {what} {}{count}? Running processes are terminated.", row.label),
        };
        Flow::Continue
    }

    pub(crate) fn kill_all(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let panes: Vec<PaneId> =
            self.rows.iter().filter(|r| r.kind == Kind::Pane && Some(r.id) != cx.own_pane).map(|r| r.id).collect();
        if panes.is_empty() {
            return self.toast("nothing to kill", cx);
        }
        self.overlay = Overlay::Confirm {
            title: "Kill all panes".into(),
            body: format!("Kill all {} listed panes? Every running process in them is terminated.", panes.len()),
            what: KillWhat::Panes(panes),
        };
        Flow::Continue
    }

    fn kill_panes(&mut self, panes: Vec<PaneId>, cx: &mut Cx<Msg>) -> Flow {
        self.job(cx, move |wz| panes.iter().try_for_each(|p| wz.kill_pane(*p)))
    }

    pub(crate) fn confirm_yes(&mut self, cx: &mut Cx<Msg>) -> Flow {
        match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::Confirm { what: KillWhat::Panes(panes), .. } => self.kill_panes(panes, cx),
            Overlay::Confirm { what: KillWhat::Detach(domain), .. } => {
                self.hand_off(Action::DetachDomain { domain }, cx)
            }
            _ => Flow::Continue,
        }
    }

    pub(crate) fn new_tab(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        if row.domain_state() == Some(DomainState::Detached) {
            return self.hand_off(Action::AttachDomain { domain: row.domain }, cx);
        }
        let window = row.window_id.or_else(|| self.window_for_domain(&row.domain));
        let spec = SpawnSpec {
            domain: Some(row.domain.clone()),
            target: Some(window.map(NewTabTarget::Window).unwrap_or(NewTabTarget::NewWindow { workspace: None })),
            cwd: (row.kind == Kind::Pane).then(|| self.pane_cwd(row.id)).flatten(),
            args: Vec::new(),
        };
        self.create(cx, move |wz| wz.spawn(&spec))
    }

    pub(crate) fn new_window(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        if row.domain_state() == Some(DomainState::Detached) {
            return self.hand_off(Action::AttachDomain { domain: row.domain }, cx);
        }
        let workspace = Some(row.workspace.clone()).filter(|w| !w.is_empty());
        let spec = SpawnSpec {
            domain: Some(row.domain.clone()),
            target: Some(NewTabTarget::NewWindow { workspace }),
            cwd: (row.kind == Kind::Pane).then(|| self.pane_cwd(row.id)).flatten(),
            args: Vec::new(),
        };
        self.create(cx, move |wz| wz.spawn(&spec))
    }

    pub(crate) fn split(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        let Some(pane) = row.pane else {
            return self.toast("select a pane to split", cx);
        };
        let cwd = self.pane_cwd(pane);
        self.create(cx, move |wz| wz.split_pane(pane, SplitDirection::Bottom, cwd.as_deref()))
    }

    pub(crate) fn zoom(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        let Some(pane) = row.pane else {
            return self.toast("select a pane to zoom", cx);
        };
        self.job(cx, move |wz| wz.zoom_pane(pane, None))
    }

    pub(crate) fn rename(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        let target = match row.kind {
            Kind::Window => RenameTarget::Window(row.id),
            Kind::Tab | Kind::Pane => match row.tab_id {
                Some(tab) => RenameTarget::Tab(tab),
                None => return Flow::Continue,
            },
            Kind::Domain => return self.toast("domains cannot be renamed here", cx),
        };
        let input = SearchState::default();
        let title = match target {
            RenameTarget::Tab(id) => format!("Rename tab #{id}"),
            RenameTarget::Window(id) => format!("Rename window #{id}"),
        };
        self.overlay = Overlay::Rename { target, title, input };
        Flow::Continue
    }

    pub(crate) fn rename_commit(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Overlay::Rename { target, input, .. } = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return Flow::Continue;
        };
        let title = input.value.trim().to_string();
        match target {
            RenameTarget::Tab(tab) => self.job(cx, move |wz| wz.set_tab_title(tab, &title)),
            RenameTarget::Window(window) => self.job(cx, move |wz| wz.set_window_title(window, &title)),
        }
    }

    pub(crate) fn move_(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(row) = self.selected() else {
            return Flow::Continue;
        };
        let workspace = Some(row.workspace.clone()).filter(|w| !w.is_empty());
        let (title, choices) = match row.kind {
            Kind::Pane => (format!("Move pane #{}", row.id), self.pane_move_choices(&row, workspace, cx)),
            Kind::Tab => (format!("Move tab {}", row.label), self.tab_move_choices(&row, workspace, cx)),
            _ => return self.toast("select a pane or tab to move", cx),
        };
        if choices.is_empty() {
            return self.toast("nowhere to move to", cx);
        }
        self.overlay = Overlay::Move { title, choices, selected: 0 };
        Flow::Continue
    }

    fn pane_move_choices(&self, row: &Row, workspace: Option<String>, cx: &Cx<Msg>) -> Vec<MoveChoice> {
        let pane = row.id;
        let mut choices = Vec::new();
        if let Some(window) = row.window_id {
            choices.push(MoveChoice { label: "New tab in this window".into(), op: MoveOp::PaneToTab { pane, window } });
        }
        choices.push(MoveChoice { label: "New window".into(), op: MoveOp::PaneToWindow { pane, workspace } });
        for w in &self.mux.windows {
            for (index, t) in w.tabs.iter().enumerate() {
                let Some(next_to) = t.panes.iter().find(|p| Some(p.id) != cx.own_pane && p.id != pane).map(|p| p.id)
                else {
                    continue;
                };
                if Some(t.id) == row.tab_id {
                    continue;
                }
                let next_to =
                    model::tab_pane(t).filter(|id| *id != pane && Some(*id) != cx.own_pane).unwrap_or(next_to);
                let label = format!("Into tab {} · {}  ({})", index + 1, model::tab_title(t), model::window_title(w));
                choices.push(MoveChoice { label, op: MoveOp::PaneIntoSplit { pane, next_to } });
            }
        }
        choices
    }

    fn tab_move_choices(&self, row: &Row, workspace: Option<String>, cx: &Cx<Msg>) -> Vec<MoveChoice> {
        let panes = self.subtree_panes(row, cx);
        if panes.is_empty() {
            return Vec::new();
        }
        let mut choices = vec![MoveChoice {
            label: "New window".into(),
            op: MoveOp::TabToWindow { panes: panes.clone(), window: None, workspace },
        }];
        for w in self.mux.windows.iter().filter(|w| Some(w.id) != row.window_id) {
            choices.push(MoveChoice {
                label: format!("Into window {}  ({})", model::window_title(w), w.workspace),
                op: MoveOp::TabToWindow { panes: panes.clone(), window: Some(w.id), workspace: None },
            });
        }
        choices
    }

    pub(crate) fn move_commit(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Overlay::Move { mut choices, selected, .. } = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return Flow::Continue;
        };
        if selected >= choices.len() {
            return Flow::Continue;
        }
        let op = choices.swap_remove(selected).op;
        self.job(cx, move |wz| op.run(wz))
    }
}
