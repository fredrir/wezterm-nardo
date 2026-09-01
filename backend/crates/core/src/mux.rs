use crate::context::{Context, Domain, DomainKind, DomainState, PaneId, TabId, WindowId};
use crate::wezterm::{PaneRecord, PaneSize};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mux {
    pub domains: Vec<Domain>,
    pub windows: Vec<Window>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub workspace: String,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub id: TabId,
    pub window_id: WindowId,
    pub title: String,
    pub panes: Vec<Pane>,
    pub active_pane: Option<PaneId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub id: PaneId,
    pub tab_id: TabId,
    pub window_id: WindowId,
    pub title: String,
    pub domain: String,
    pub workspace: String,
    pub cwd: Option<String>,
    pub process: Option<String>,
    pub size: PaneSize,
    pub left_col: u32,
    pub top_row: u32,
    pub is_active: bool,
    pub is_zoomed: bool,
    pub unseen: bool,
    pub alt_screen: bool,
}

impl Mux {
    /// Joins `cli list` rows with context extras; order follows the cli (window, tab, pane) order.
    /// Unknown domains fall back to `origin.domain`, else `"local"`. Domains without panes keep
    /// their context entry so detached tls/ssh domains stay listable.
    pub fn build(records: &[PaneRecord], cx: &Context) -> Mux {
        let fallback_domain = cx.origin.domain.clone().unwrap_or_else(|| "local".to_string());
        let mut windows: Vec<Window> = Vec::new();
        let mut seen_domains: Vec<String> = Vec::new();

        for record in records {
            let extra = cx.panes.get(&record.pane_id);
            let domain = record
                .domain_name
                .clone()
                .filter(|d| !d.is_empty())
                .or_else(|| extra.and_then(|e| e.domain.clone()))
                .unwrap_or_else(|| fallback_domain.clone());
            if !seen_domains.contains(&domain) {
                seen_domains.push(domain.clone());
            }
            let pane = Pane {
                id: record.pane_id,
                tab_id: record.tab_id,
                window_id: record.window_id,
                title: record.title.clone(),
                domain,
                workspace: record.workspace.clone(),
                cwd: non_empty(&record.cwd).or_else(|| extra.and_then(|e| e.cwd.clone())),
                process: extra.and_then(|e| e.process.clone()),
                size: record.size,
                left_col: record.left_col,
                top_row: record.top_row,
                is_active: record.is_active,
                is_zoomed: record.is_zoomed,
                unseen: extra.is_some_and(|e| e.unseen),
                alt_screen: extra.is_some_and(|e| e.alt_screen),
            };

            let window = window_slot(&mut windows, record);
            let tab = tab_slot(&mut window.tabs, record);
            if record.is_active {
                tab.active_pane = Some(pane.id);
            }
            tab.panes.push(pane);
        }

        for tab in windows.iter_mut().flat_map(|w| &mut w.tabs) {
            if tab.title.is_empty() {
                let active = tab.active_pane.and_then(|id| tab.panes.iter().find(|p| p.id == id));
                tab.title = active.or(tab.panes.first()).map(|p| p.title.clone()).unwrap_or_default();
            }
        }

        let mut domains = cx.domains.clone();
        for name in seen_domains {
            if !domains.iter().any(|d| d.name == name) {
                domains.push(Domain {
                    label: name.clone(),
                    kind: if name == "local" { DomainKind::Local } else { DomainKind::Unknown },
                    name,
                    state: DomainState::Attached,
                    spawnable: true,
                    has_panes: true,
                });
            }
        }

        Mux { domains, windows }
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.windows.iter().flat_map(|w| &w.tabs).find(|t| t.id == id)
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes().find(|p| p.id == id)
    }

    pub fn panes(&self) -> impl Iterator<Item = &Pane> {
        self.windows.iter().flat_map(|w| &w.tabs).flat_map(|t| &t.panes)
    }

    pub fn domain(&self, name: &str) -> Option<&Domain> {
        self.domains.iter().find(|d| d.name == name)
    }

    pub fn is_attached(&self, name: &str) -> bool {
        self.domain(name).is_some_and(|d| d.state == DomainState::Attached)
    }

    pub fn domain_kind(&self, name: &str) -> DomainKind {
        self.domain(name).map(|d| d.kind).unwrap_or_default()
    }
}

fn window_slot<'a>(windows: &'a mut Vec<Window>, record: &PaneRecord) -> &'a mut Window {
    let index = match windows.iter().position(|w| w.id == record.window_id) {
        Some(i) => i,
        None => {
            windows.push(Window {
                id: record.window_id,
                title: record.window_title.clone(),
                workspace: record.workspace.clone(),
                tabs: Vec::new(),
            });
            windows.len() - 1
        }
    };
    let window = &mut windows[index];
    if window.title.is_empty() {
        window.title = record.window_title.clone();
    }
    window
}

fn tab_slot<'a>(tabs: &'a mut Vec<Tab>, record: &PaneRecord) -> &'a mut Tab {
    let index = match tabs.iter().position(|t| t.id == record.tab_id) {
        Some(i) => i,
        None => {
            tabs.push(Tab {
                id: record.tab_id,
                window_id: record.window_id,
                title: record.tab_title.clone(),
                panes: Vec::new(),
                active_pane: None,
            });
            tabs.len() - 1
        }
    };
    let tab = &mut tabs[index];
    if tab.title.is_empty() {
        tab.title = record.tab_title.clone();
    }
    tab
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

/// `file://host/path` → `(host, path)`; plain paths → `(None, path)`.
pub fn split_cwd(cwd: &str) -> (Option<&str>, &str) {
    let Some(rest) = cwd.strip_prefix("file://") else {
        return (None, cwd);
    };
    match rest.find('/') {
        Some(0) => (None, rest),
        Some(i) => (Some(&rest[..i]), &rest[i..]),
        None => ((!rest.is_empty()).then_some(rest), ""),
    }
}

/// `/home/f/projects/x` → `~/projects/x` when `home` matches; last two components otherwise unchanged.
pub fn short_path(path: &str, home: Option<&str>) -> String {
    let trimmed = path.trim_end_matches('/');
    let path = if trimmed.is_empty() && path.starts_with('/') { "/" } else { trimmed };
    if let Some(home) = home.map(|h| h.trim_end_matches('/')).filter(|h| !h.is_empty()) {
        if path == home {
            return "~".to_string();
        }
        if let Some(rest) = path.strip_prefix(home).filter(|r| r.starts_with('/')) {
            return format!("~{rest}");
        }
    }
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() > 2 { parts[parts.len() - 2..].join("/") } else { path.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::PaneExtra;

    const LIST: &str = r#"[
      {"window_id":10,"tab_id":30,"pane_id":46,"workspace":"default","size":{"rows":40,"cols":120},"title":"zsh","cwd":"file://archie/home/f","tab_title":"","window_title":"main","is_active":true,"is_zoomed":false},
      {"window_id":10,"tab_id":30,"pane_id":47,"workspace":"default","size":{"rows":40,"cols":60},"title":"vim","cwd":"","tab_title":"","window_title":"main","is_active":false,"is_zoomed":false},
      {"window_id":10,"tab_id":31,"pane_id":48,"workspace":"default","size":{"rows":40,"cols":120},"title":"htop","cwd":"file://archie/home/f","tab_title":"monitor","window_title":"main","is_active":true,"is_zoomed":true,"domain_name":"archie-cable"},
      {"window_id":11,"tab_id":32,"pane_id":49,"workspace":"dev","size":{"rows":40,"cols":120},"title":"ssh","cwd":"file:///tmp","tab_title":"","window_title":"","is_active":true,"is_zoomed":false}
    ]"#;

    fn context() -> Context {
        let mut cx: Context = serde_json::from_str(
            r#"{
              "origin": { "pane_id": 46, "domain": "localmux" },
              "domains": [
                { "name": "local", "label": "local", "kind": "local", "state": "Attached", "has_panes": true },
                { "name": "remote", "label": "remote via tls", "kind": "tls", "state": "Detached", "has_panes": false }
              ]
            }"#,
        )
        .unwrap();
        cx.panes.insert(
            47,
            PaneExtra {
                domain: Some("local".into()),
                process: Some("vim".into()),
                cwd: Some("file://archie/home/f/x".into()),
                unseen: true,
                alt_screen: true,
            },
        );
        cx
    }

    #[test]
    fn build_groups_in_cli_order_and_joins_extras() {
        let records: Vec<PaneRecord> = serde_json::from_str(LIST).unwrap();
        let mux = Mux::build(&records, &context());

        let ids: Vec<WindowId> = mux.windows.iter().map(|w| w.id).collect();
        assert_eq!(ids, [10, 11]);
        let tabs: Vec<TabId> = mux.windows[0].tabs.iter().map(|t| t.id).collect();
        assert_eq!(tabs, [30, 31]);
        assert_eq!(mux.windows[0].title, "main");
        assert_eq!(mux.windows[1].title, "");
        assert_eq!(mux.windows[1].workspace, "dev");

        let tab = mux.tab(30).unwrap();
        assert_eq!(tab.title, "zsh", "empty tab_title falls back to the active pane's title");
        assert_eq!(tab.active_pane, Some(46));
        assert_eq!(mux.tab(31).unwrap().title, "monitor");

        let vim = mux.pane(47).unwrap();
        assert_eq!(vim.domain, "local", "context pane extra wins when cli has no domain_name");
        assert_eq!(vim.process.as_deref(), Some("vim"));
        assert_eq!(vim.cwd.as_deref(), Some("file://archie/home/f/x"), "empty cli cwd falls back to extra");
        assert!(vim.unseen && vim.alt_screen);

        assert_eq!(mux.pane(48).unwrap().domain, "archie-cable", "cli domain_name wins");
        assert!(mux.pane(48).unwrap().is_zoomed);
        assert_eq!(mux.pane(46).unwrap().domain, "localmux", "origin.domain is the fallback");
        assert_eq!(mux.pane(46).unwrap().cwd.as_deref(), Some("file://archie/home/f"));
    }

    #[test]
    fn build_keeps_context_domains_and_synthesizes_missing_ones() {
        let records: Vec<PaneRecord> = serde_json::from_str(LIST).unwrap();
        let mux = Mux::build(&records, &context());
        let names: Vec<&str> = mux.domains.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["local", "remote", "localmux", "archie-cable"]);
        assert!(!mux.is_attached("remote"));
        assert_eq!(mux.domain_kind("remote"), DomainKind::Tls);
        let synthesized = mux.domain("archie-cable").unwrap();
        assert_eq!(synthesized.state, DomainState::Attached);
        assert!(synthesized.has_panes && synthesized.spawnable);
        assert_eq!(synthesized.label, "archie-cable");
        assert_eq!(mux.domain_kind("nope"), DomainKind::Unknown);
    }

    #[test]
    fn build_without_origin_domain_defaults_to_local() {
        let records: Vec<PaneRecord> = serde_json::from_str(LIST).unwrap();
        let mux = Mux::build(&records, &Context::default());
        assert_eq!(mux.pane(46).unwrap().domain, "local");
        assert_eq!(mux.domain("local").unwrap().kind, DomainKind::Local);
    }

    #[test]
    fn split_cwd_handles_hosts_and_plain_paths() {
        assert_eq!(split_cwd("file://archie/home/f"), (Some("archie"), "/home/f"));
        assert_eq!(split_cwd("file:///tmp"), (None, "/tmp"));
        assert_eq!(split_cwd("/var/log"), (None, "/var/log"));
        assert_eq!(split_cwd("file://host"), (Some("host"), ""));
    }

    #[test]
    fn short_path_collapses_home_and_long_paths() {
        assert_eq!(short_path("/home/f/projects/x", Some("/home/f")), "~/projects/x");
        assert_eq!(short_path("/home/f", Some("/home/f/")), "~");
        assert_eq!(short_path("/home/fred/x", Some("/home/f")), "fred/x");
        assert_eq!(short_path("/etc/nginx/conf.d/", None), "nginx/conf.d");
        assert_eq!(short_path("/tmp", None), "/tmp");
        assert_eq!(short_path("/", None), "/");
    }
}
