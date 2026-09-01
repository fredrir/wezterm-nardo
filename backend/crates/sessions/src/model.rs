use std::collections::BTreeMap;

use nardo_core::context::{DomainKind, DomainState, PaneId, TabId, WindowId};
use nardo_core::mux::{self, Mux, Pane, Tab, Window};
use nardo_core::search::{Query, Searcher};
use nardo_core::wezterm::Result;
use serde::{Deserialize, Serialize};

/// Background job results.
pub enum Msg {
    Mux(Result<Mux>),
    Backdrop(Result<String>),
    Preview { pane_id: PaneId, text: Result<String> },
    Done(Result<()>),
    Switched { pane_id: PaneId, result: Result<()> },
    Created(Result<PaneId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    All,
    Windows,
    Tabs,
    Panes,
    Domains,
}

impl Scope {
    pub const ALL: [Scope; 5] = [Scope::All, Scope::Windows, Scope::Tabs, Scope::Panes, Scope::Domains];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn next(self) -> Scope {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Scope {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::All => "All",
            Scope::Windows => "Windows",
            Scope::Tabs => "Tabs",
            Scope::Panes => "Panes",
            Scope::Domains => "Domains",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::Windows => "windows",
            Scope::Tabs => "tabs",
            Scope::Panes => "panes",
            Scope::Domains => "domains",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Window,
    Tab,
    Pane,
    Domain,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Detail {
    Window { tabs: usize, panes: usize },
    Tab { index: usize, panes: usize },
    Pane { process: String, cwd: String, unseen: bool, zoomed: bool, kind: DomainKind },
    Domain { label: String, kind: DomainKind, state: DomainState, windows: usize },
}

/// One list row. `haystack` = `"{label} {meta}"` (+ hidden extras) so match indices map onto the
/// rendered label / meta segments directly.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub kind: Kind,
    pub header: bool,
    pub id: u64,
    pub window_id: Option<WindowId>,
    pub tab_id: Option<TabId>,
    /// Pane used for preview and switch (pane itself, tab's active pane, window's first tab's active pane).
    pub pane: Option<PaneId>,
    pub label: String,
    pub meta: String,
    pub haystack: String,
    pub domain: String,
    pub workspace: String,
    pub depth: u16,
    pub indices: Vec<u32>,
    pub detail: Detail,
}

impl Row {
    pub fn key(&self) -> (Kind, u64) {
        (self.kind, self.id)
    }

    pub fn domain_state(&self) -> Option<DomainState> {
        match &self.detail {
            Detail::Domain { state, .. } => Some(*state),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum KeySpec {
    One(String),
    Many(Vec<String>),
    Off(bool),
}

/// `context.options` for the sessions app, see docs/protocol.md "Sessions options".
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub confirm_kill: bool,
    pub preview: bool,
    pub preview_lines: u32,
    pub mru: bool,
    pub scope: Scope,
    pub show_self: bool,
    pub keys: BTreeMap<String, KeySpec>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            confirm_kill: true,
            preview: true,
            preview_lines: 200,
            mru: true,
            scope: Scope::All,
            show_self: false,
            keys: BTreeMap::new(),
        }
    }
}

impl Options {
    /// Field-by-field lenient parse: a bad value falls back to that field's default.
    pub fn from_value(value: &serde_json::Value) -> Options {
        let mut o = Options::default();
        let Some(obj) = value.as_object() else {
            return o;
        };
        let bool_or = |key: &str, default: bool| obj.get(key).and_then(|v| v.as_bool()).unwrap_or(default);
        o.confirm_kill = bool_or("confirm_kill", o.confirm_kill);
        o.preview = bool_or("preview", o.preview);
        o.mru = bool_or("mru", o.mru);
        o.show_self = bool_or("show_self", o.show_self);
        if let Some(n) = obj.get("preview_lines").and_then(|v| v.as_f64()) {
            o.preview_lines = n.max(0.0) as u32;
        }
        if let Some(scope) = obj.get("scope").and_then(|v| serde_json::from_value(v.clone()).ok()) {
            o.scope = scope;
        }
        if let Some(keys) = obj.get("keys").and_then(|v| v.as_object()) {
            o.keys = keys
                .iter()
                .filter_map(|(k, v)| serde_json::from_value(v.clone()).ok().map(|spec| (k.clone(), spec)))
                .collect();
        }
        o
    }
}

/// Everything needed to turn a mux snapshot into rows for one scope.
pub struct Build<'a> {
    pub mux: &'a Mux,
    pub scope: Scope,
    pub query: &'a Query,
    /// Raw query string non-empty: flat ranked pane rows instead of the tree.
    pub flat: bool,
    pub mru: &'a [PaneId],
    pub options: &'a Options,
    pub own_pane: Option<PaneId>,
}

impl Build<'_> {
    pub fn rows(&self, searcher: &mut Searcher) -> Vec<Row> {
        let flat = self.flat;
        let rows = match self.scope {
            Scope::All if !flat => return self.tree(),
            Scope::All => self.pane_rows(0).collect(),
            Scope::Windows => self.window_rows(false).collect(),
            Scope::Tabs => self.tab_rows().collect(),
            Scope::Panes => self.mru_sorted(self.pane_rows(0).collect()),
            Scope::Domains => self.domain_rows(|_| true).collect(),
        };
        rank(searcher, &self.query.text, rows)
    }

    fn tree(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for w in &self.mux.windows {
            let start = rows.len();
            for (index, t) in w.tabs.iter().enumerate() {
                let panes: Vec<&Pane> = t.panes.iter().filter(|p| self.visible(w, p)).collect();
                if panes.is_empty() {
                    continue;
                }
                rows.push(self.tab_row(w, t, index, panes.len(), 1, true));
                rows.extend(panes.iter().map(|p| self.pane_row(w, t, index, p, 2)));
            }
            if rows.len() > start {
                rows.insert(start, self.window_row(w, true));
            }
        }
        rows
    }

    fn visible(&self, w: &Window, p: &Pane) -> bool {
        (self.options.show_self || Some(p.id) != self.own_pane) && self.matches(w, p)
    }

    fn matches(&self, w: &Window, p: &Pane) -> bool {
        let q = self.query;
        q.domain.as_deref().is_none_or(|d| contains_ci(&p.domain, d))
            && q.window.as_deref().is_none_or(|t| contains_ci(&w.title, t))
            && q.workspace.as_deref().is_none_or(|ws| contains_ci(&p.workspace, ws))
            && q.pane_id.is_none_or(|id| p.id == id)
    }

    fn visible_panes<'m>(&'m self, w: &'m Window, t: &'m Tab) -> impl Iterator<Item = &'m Pane> + 'm {
        t.panes.iter().filter(move |p| self.visible(w, p))
    }

    fn pane_rows(&self, depth: u16) -> impl Iterator<Item = Row> + '_ {
        self.mux.windows.iter().flat_map(move |w| {
            w.tabs
                .iter()
                .enumerate()
                .flat_map(move |(index, t)| self.visible_panes(w, t).map(move |p| self.pane_row(w, t, index, p, depth)))
        })
    }

    fn tab_rows(&self) -> impl Iterator<Item = Row> + '_ {
        self.mux.windows.iter().flat_map(move |w| {
            w.tabs.iter().enumerate().filter_map(move |(index, t)| {
                let panes = self.visible_panes(w, t).count();
                (panes > 0).then(|| self.tab_row(w, t, index, panes, 0, false))
            })
        })
    }

    fn window_rows(&self, header: bool) -> impl Iterator<Item = Row> + '_ {
        self.mux
            .windows
            .iter()
            .filter(move |w| w.tabs.iter().any(|t| self.visible_panes(w, t).next().is_some()))
            .map(move |w| self.window_row(w, header))
    }

    fn domain_rows<'m>(&'m self, keep: impl Fn(&str) -> bool + 'm) -> impl Iterator<Item = Row> + 'm {
        self.mux.domains.iter().enumerate().filter_map(move |(i, d)| {
            let q = self.query;
            let wanted = q.domain.as_deref().is_none_or(|f| contains_ci(&d.name, f))
                && q.window.is_none()
                && q.workspace.is_none()
                && q.pane_id.is_none();
            (wanted && keep(&d.name)).then(|| {
                let windows = self
                    .mux
                    .windows
                    .iter()
                    .filter(|w| w.tabs.iter().any(|t| t.panes.iter().any(|p| p.domain == d.name)))
                    .count();
                let meta = [d.label.as_str(), kind_name(d.kind)]
                    .into_iter()
                    .filter(|s| !s.is_empty() && *s != d.name)
                    .collect::<Vec<_>>()
                    .join(" · ");
                Row {
                    kind: Kind::Domain,
                    header: false,
                    id: i as u64,
                    window_id: None,
                    tab_id: None,
                    pane: None,
                    haystack: joined(&d.name, &meta),
                    label: d.name.clone(),
                    meta,
                    domain: d.name.clone(),
                    workspace: String::new(),
                    depth: 0,
                    indices: Vec::new(),
                    detail: Detail::Domain { label: d.label.clone(), kind: d.kind, state: d.state, windows },
                }
            })
        })
    }

    fn window_row(&self, w: &Window, header: bool) -> Row {
        let first = w.tabs.iter().flat_map(|t| &t.panes).next();
        let domain = first.map(|p| p.domain.clone()).unwrap_or_default();
        let label = window_title(w);
        let meta = crumbs(&[&domain, &w.workspace]);
        Row {
            kind: Kind::Window,
            header,
            id: w.id,
            window_id: Some(w.id),
            tab_id: None,
            pane: w.tabs.first().and_then(tab_pane),
            haystack: joined(&label, &meta),
            label,
            meta,
            domain,
            workspace: w.workspace.clone(),
            depth: 0,
            indices: Vec::new(),
            detail: Detail::Window { tabs: w.tabs.len(), panes: w.tabs.iter().map(|t| t.panes.len()).sum() },
        }
    }

    fn tab_row(&self, w: &Window, t: &Tab, index: usize, panes: usize, depth: u16, header: bool) -> Row {
        let pane = tab_pane(t);
        let sample = pane.and_then(|id| t.panes.iter().find(|p| p.id == id)).or(t.panes.first());
        let domain = sample.map(|p| p.domain.clone()).unwrap_or_default();
        let label = format!("{} · {}", index + 1, tab_title(t));
        let meta = crumbs(&[&domain, &w.workspace, &window_title(w)]);
        Row {
            kind: Kind::Tab,
            header,
            id: t.id,
            window_id: Some(w.id),
            tab_id: Some(t.id),
            pane,
            haystack: joined(&label, &meta),
            label,
            meta,
            domain,
            workspace: w.workspace.clone(),
            depth,
            indices: Vec::new(),
            detail: Detail::Tab { index: index + 1, panes },
        }
    }

    fn pane_row(&self, w: &Window, t: &Tab, index: usize, p: &Pane, depth: u16) -> Row {
        let process = p.process.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| p.title.clone());
        let cwd = p.cwd.as_deref().map(short_cwd).unwrap_or_default();
        let label = joined(&process, &cwd);
        let meta = crumbs(&[&p.domain, &p.workspace, &window_title(w), &format!("{} · {}", index + 1, tab_title(t))]);
        let extra = if p.title == process { String::new() } else { p.title.clone() };
        Row {
            kind: Kind::Pane,
            header: false,
            id: p.id,
            window_id: Some(w.id),
            tab_id: Some(t.id),
            pane: Some(p.id),
            haystack: joined(&label, &extra),
            label,
            meta,
            domain: p.domain.clone(),
            workspace: p.workspace.clone(),
            depth,
            indices: Vec::new(),
            detail: Detail::Pane {
                process,
                cwd,
                unseen: p.unseen,
                zoomed: p.is_zoomed,
                kind: self.mux.domain_kind(&p.domain),
            },
        }
    }

    fn mru_sorted(&self, mut rows: Vec<Row>) -> Vec<Row> {
        if self.options.mru && self.query.text.is_empty() {
            rows.sort_by_key(|r| self.mru.iter().position(|p| *p == r.id).unwrap_or(usize::MAX));
        }
        rows
    }
}

fn rank(searcher: &mut Searcher, text: &str, rows: Vec<Row>) -> Vec<Row> {
    if text.is_empty() {
        return rows;
    }
    searcher
        .rank(
            text,
            rows.into_iter().map(|r| {
                let hay = r.haystack.clone();
                (r, hay)
            }),
        )
        .into_iter()
        .map(|ranked| Row { indices: ranked.indices, ..ranked.item })
        .collect()
}

/// Most recent other pane when `use_mru`, else the first non-origin pane (so ↵ always goes
/// somewhere else, alt-tab style), else the origin's row, else the first item.
pub fn initial_index(rows: &[Row], mru: &[PaneId], origin: Option<PaneId>, use_mru: bool) -> Option<usize> {
    let pane_index = |id: PaneId| rows.iter().position(|r| r.kind == Kind::Pane && r.id == id);
    use_mru
        .then(|| mru.iter().filter(|p| Some(**p) != origin).find_map(|p| pane_index(*p)))
        .flatten()
        .or_else(|| rows.iter().position(|r| r.kind == Kind::Pane && !r.header && Some(r.id) != origin))
        .or_else(|| origin.and_then(pane_index))
        .or_else(|| rows.iter().position(|r| !r.header))
}

pub fn tab_pane(t: &Tab) -> Option<PaneId> {
    t.active_pane
        .filter(|id| t.panes.iter().any(|p| p.id == *id))
        .or_else(|| t.panes.iter().find(|p| p.is_active).map(|p| p.id))
        .or_else(|| t.panes.first().map(|p| p.id))
}

pub fn tab_title(t: &Tab) -> String {
    if !t.title.is_empty() {
        return t.title.clone();
    }
    tab_pane(t)
        .and_then(|id| t.panes.iter().find(|p| p.id == id))
        .map(|p| p.title.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("tab {}", t.id))
}

pub fn window_title(w: &Window) -> String {
    if w.title.is_empty() { format!("window {}", w.id) } else { w.title.clone() }
}

pub fn kind_name(kind: DomainKind) -> &'static str {
    match kind {
        DomainKind::Local => "local",
        DomainKind::Unix => "unix",
        DomainKind::Tls => "tls",
        DomainKind::Ssh => "ssh",
        DomainKind::Exec => "exec",
        DomainKind::Wsl => "wsl",
        DomainKind::Serial => "serial",
        DomainKind::Unknown => "",
    }
}

pub fn short_cwd(cwd: &str) -> String {
    let (_, path) = mux::split_cwd(cwd);
    let env_home = std::env::var("HOME").ok();
    let home = env_home.as_deref().filter(|h| !h.is_empty() && path.starts_with(h)).or_else(|| user_home(path));
    mux::short_path(path, home)
}

/// `/home/<user>` or `/Users/<user>` prefix, so remote cwds shorten to `~` too.
fn user_home(path: &str) -> Option<&str> {
    ["/home/", "/Users/"].into_iter().find_map(|prefix| {
        let rest = path.strip_prefix(prefix)?;
        let user = rest.split('/').next().filter(|u| !u.is_empty())?;
        Some(&path[..prefix.len() + user.len()])
    })
}

pub fn crumbs(parts: &[&str]) -> String {
    parts.iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" › ")
}

fn joined(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a} {b}"),
    }
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nardo_core::context::Domain;
    use nardo_core::wezterm::PaneSize;

    fn pane(id: PaneId, tab: TabId, window: WindowId, domain: &str, title: &str, process: &str, cwd: &str) -> Pane {
        Pane {
            id,
            tab_id: tab,
            window_id: window,
            title: title.into(),
            domain: domain.into(),
            workspace: "default".into(),
            cwd: Some(cwd.into()),
            process: Some(process.into()),
            size: PaneSize::default(),
            left_col: 0,
            top_row: 0,
            is_active: true,
            is_zoomed: false,
            unseen: false,
            alt_screen: false,
        }
    }

    fn domain(name: &str, kind: DomainKind, state: DomainState) -> Domain {
        Domain { name: name.into(), label: name.into(), kind, state, spawnable: true, has_panes: false }
    }

    /// window 10 (localmux): tab 30 [46 zsh, 47 vim], tab 31 [48 htop]; window 11 (archie): tab 32 [50 ssh]
    /// own pane 99 alone in tab 33 of window 10; domains: localmux attached, archie attached, cable detached.
    fn mux() -> Mux {
        Mux {
            domains: vec![
                domain("localmux", DomainKind::Unix, DomainState::Attached),
                domain("archie", DomainKind::Tls, DomainState::Attached),
                domain("cable", DomainKind::Tls, DomainState::Detached),
            ],
            windows: vec![
                Window {
                    id: 10,
                    title: "main".into(),
                    workspace: "default".into(),
                    tabs: vec![
                        Tab {
                            id: 30,
                            window_id: 10,
                            title: "".into(),
                            panes: vec![
                                pane(46, 30, 10, "localmux", "zsh", "zsh", "file://host/home/f"),
                                pane(47, 30, 10, "localmux", "vim", "nvim", "file://host/home/f/x"),
                            ],
                            active_pane: Some(47),
                        },
                        Tab {
                            id: 31,
                            window_id: 10,
                            title: "monitor".into(),
                            panes: vec![pane(48, 31, 10, "localmux", "htop", "htop", "file://host/tmp")],
                            active_pane: Some(48),
                        },
                        Tab {
                            id: 33,
                            window_id: 10,
                            title: "".into(),
                            panes: vec![pane(99, 33, 10, "local", "wez-nardo", "wez-nardo", "file://host/home/f")],
                            active_pane: Some(99),
                        },
                    ],
                },
                Window {
                    id: 11,
                    title: "".into(),
                    workspace: "dev".into(),
                    tabs: vec![Tab {
                        id: 32,
                        window_id: 11,
                        title: "".into(),
                        panes: vec![{
                            let mut p = pane(50, 32, 11, "archie", "ssh", "zsh", "file://archie/home/f/srv");
                            p.workspace = "dev".into();
                            p
                        }],
                        active_pane: Some(50),
                    }],
                },
            ],
        }
    }

    fn build<'a>(mux: &'a Mux, scope: Scope, query: &'a Query, options: &'a Options) -> Vec<Row> {
        Build { mux, scope, query, flat: !query.text.is_empty(), mru: &[], options, own_pane: Some(99) }
            .rows(&mut Searcher::default())
    }

    fn keys(rows: &[Row]) -> Vec<(Kind, u64)> {
        rows.iter().map(Row::key).collect()
    }

    #[test]
    fn tree_groups_windows_tabs_panes_without_domain_rows() {
        let mux = mux();
        let rows = build(&mux, Scope::All, &Query::default(), &Options::default());
        assert_eq!(
            keys(&rows),
            vec![
                (Kind::Window, 10),
                (Kind::Tab, 30),
                (Kind::Pane, 46),
                (Kind::Pane, 47),
                (Kind::Tab, 31),
                (Kind::Pane, 48),
                (Kind::Window, 11),
                (Kind::Tab, 32),
                (Kind::Pane, 50),
            ]
        );
        assert!(rows[0].header && rows[1].header, "window and tab rows are headers in the tree");
        assert_eq!(rows[1].label, "1 · vim");
        assert_eq!(rows[4].label, "2 · monitor");
        assert_eq!(rows[6].label, "window 11");
        assert_eq!(rows[3].depth, 2);
    }

    #[test]
    fn show_self_lists_own_pane() {
        let mux = mux();
        let options = Options { show_self: true, ..Options::default() };
        let rows = build(&mux, Scope::Panes, &Query::default(), &options);
        assert!(rows.iter().any(|r| r.id == 99));
    }

    #[test]
    fn scopes_list_their_items_with_breadcrumbs() {
        let mux = mux();
        let q = Query::default();
        let windows = build(&mux, Scope::Windows, &q, &Options::default());
        assert_eq!(keys(&windows), vec![(Kind::Window, 10), (Kind::Window, 11)]);
        assert!(!windows[0].header);
        assert_eq!(windows[0].meta, "localmux › default");

        let tabs = build(&mux, Scope::Tabs, &q, &Options::default());
        assert_eq!(keys(&tabs), vec![(Kind::Tab, 30), (Kind::Tab, 31), (Kind::Tab, 32)]);
        assert_eq!(tabs[2].meta, "archie › dev › window 11");

        let domains = build(&mux, Scope::Domains, &q, &Options::default());
        assert_eq!(keys(&domains), vec![(Kind::Domain, 0), (Kind::Domain, 1), (Kind::Domain, 2)]);
        assert_eq!(
            domains[1].detail,
            Detail::Domain { label: "archie".into(), kind: DomainKind::Tls, state: DomainState::Attached, windows: 1 }
        );
    }

    #[test]
    fn pane_labels_and_haystack() {
        let mux = mux();
        let rows = build(&mux, Scope::Panes, &Query::default(), &Options::default());
        let vim = rows.iter().find(|r| r.id == 47).unwrap();
        assert_eq!(vim.label, format!("nvim {}", short_cwd("file://host/home/f/x")));
        assert_eq!(vim.meta, "localmux › default › main › 1 · vim");
        assert!(vim.haystack.starts_with(&vim.label));
        assert!(vim.haystack.ends_with(" vim"));
    }

    #[test]
    fn query_text_flattens_and_ranks() {
        let mux = mux();
        let rows = build(&mux, Scope::All, &Query::parse("htop"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Pane, 48)]);
        assert!(!rows[0].indices.is_empty());
        let rows = build(&mux, Scope::All, &Query::parse("cable"), &Options::default());
        assert_eq!(keys(&rows), vec![], "domains are not searchable in scope all");
        let rows = build(&mux, Scope::Domains, &Query::parse("cable"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Domain, 2)]);
    }

    #[test]
    fn query_filters_prune_the_tree() {
        let mux = mux();
        let rows = build(&mux, Scope::All, &Query::parse("d:archie"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Window, 11), (Kind::Tab, 32), (Kind::Pane, 50)]);
        let rows = build(&mux, Scope::All, &Query::parse("#48"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Window, 10), (Kind::Tab, 31), (Kind::Pane, 48)]);
        let rows = build(&mux, Scope::Windows, &Query::parse("ws:dev"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Window, 11)]);
        let rows = build(&mux, Scope::Tabs, &Query::parse("w:main"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Tab, 30), (Kind::Tab, 31)]);
        let rows = build(&mux, Scope::Domains, &Query::parse("d:cab"), &Options::default());
        assert_eq!(keys(&rows), vec![(Kind::Domain, 2)]);
    }

    #[test]
    fn panes_scope_orders_by_mru_on_empty_query() {
        let mux = mux();
        let options = Options::default();
        let q = Query::default();
        let rows = Build {
            mux: &mux,
            scope: Scope::Panes,
            query: &q,
            flat: !q.text.is_empty(),
            mru: &[50, 48],
            options: &options,
            own_pane: Some(99),
        }
        .rows(&mut Searcher::default());
        assert_eq!(keys(&rows), vec![(Kind::Pane, 50), (Kind::Pane, 48), (Kind::Pane, 46), (Kind::Pane, 47)]);
        let off = Options { mru: false, ..Options::default() };
        let rows = Build {
            mux: &mux,
            scope: Scope::Panes,
            query: &q,
            flat: !q.text.is_empty(),
            mru: &[50, 48],
            options: &off,
            own_pane: Some(99),
        }
        .rows(&mut Searcher::default());
        assert_eq!(keys(&rows), vec![(Kind::Pane, 46), (Kind::Pane, 47), (Kind::Pane, 48), (Kind::Pane, 50)]);
    }

    #[test]
    fn initial_selection_prefers_most_recent_other_pane() {
        let mux = mux();
        let rows = build(&mux, Scope::All, &Query::default(), &Options::default());
        let at = |id| rows.iter().position(|r| r.kind == Kind::Pane && r.id == id).unwrap();
        assert_eq!(initial_index(&rows, &[46, 50, 48], Some(46), true), Some(at(50)));
        assert_eq!(
            initial_index(&rows, &[46, 50, 48], Some(46), false),
            Some(at(47)),
            "mru off still avoids the origin"
        );
        assert_eq!(
            initial_index(&rows, &[46, 1234], Some(46), true),
            Some(at(47)),
            "stale mru falls back to the first non-origin pane"
        );
        assert_eq!(initial_index(&rows, &[], None, true), Some(at(46)));
        assert_eq!(initial_index(&[], &[46], Some(46), true), None);
    }

    #[test]
    fn options_parse_leniently() {
        let v = serde_json::json!({
            "confirm_kill": false, "preview_lines": 50.0, "scope": "tabs", "mru": "nope",
            "keys": { "kill": "x", "up": ["k", "ctrl+p"], "help": false, "bogus": 3 }
        });
        let o = Options::from_value(&v);
        assert!(!o.confirm_kill);
        assert_eq!(o.preview_lines, 50);
        assert_eq!(o.scope, Scope::Tabs);
        assert!(o.mru);
        assert_eq!(o.keys.get("kill"), Some(&KeySpec::One("x".into())));
        assert_eq!(o.keys.get("up"), Some(&KeySpec::Many(vec!["k".into(), "ctrl+p".into()])));
        assert_eq!(o.keys.get("help"), Some(&KeySpec::Off(false)));
        assert!(!o.keys.contains_key("bogus"));
        assert_eq!(Options::from_value(&serde_json::Value::Null), Options::default());
    }

    #[test]
    fn scope_cycles() {
        assert_eq!(Scope::All.next(), Scope::Windows);
        assert_eq!(Scope::Domains.next(), Scope::All);
        assert_eq!(Scope::All.prev(), Scope::Domains);
        assert_eq!(Scope::Panes.name(), "panes");
    }

    #[test]
    fn user_home_prefixes() {
        assert_eq!(user_home("/home/f/x"), Some("/home/f"));
        assert_eq!(user_home("/Users/f"), Some("/Users/f"));
        assert_eq!(user_home("/tmp/x"), None);
    }
}
