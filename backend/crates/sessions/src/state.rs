use std::time::{Duration, Instant};

use nardo_core::app::{App, Cx, Flow, Outcome};
use nardo_core::context::{Backdrop, Mode, PaneId, TabId, WindowId};
use nardo_core::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use nardo_core::mux::Mux;
use nardo_core::search::{Query, Searcher};
use nardo_core::state::State;
use nardo_core::ui::modal::ansi_text;
use nardo_core::ui::widgets::{Confirm, ConfirmChoice, ListStateExt, RowKind, SearchState};
use nardo_core::wezterm::Action;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::Text;
use ratatui::widgets::ListState;
use serde_json::{Value, json};

use crate::actions::MoveOp;
use crate::keys::{ActionId, Keymap};
use crate::model::{self, Build, Kind, Msg, Options, Row, Scope};
use crate::preview::{self, PreviewCache};
use crate::view;

const TOAST_FOR: Duration = Duration::from_secs(3);
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub enum Overlay {
    None,
    Confirm { what: KillWhat, title: String, body: String },
    Rename { target: RenameTarget, title: String, input: SearchState },
    Move { title: String, choices: Vec<MoveChoice>, selected: usize },
    Help,
}

pub enum KillWhat {
    Panes(Vec<PaneId>),
    Detach(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameTarget {
    Tab(TabId),
    Window(WindowId),
}

pub struct MoveChoice {
    pub label: String,
    pub op: MoveOp,
}

pub struct Toast {
    pub text: String,
    pub until: Instant,
}

/// Rects from the last frame, for mouse hit testing.
#[derive(Default)]
pub struct Hit {
    pub modal: Rect,
    pub list: Rect,
    pub preview: Rect,
    pub preview_shown: bool,
    pub chips: Vec<Rect>,
    pub dialog: Rect,
    pub choices: Rect,
    pub choice_offset: usize,
}

pub struct SessionsApp {
    pub(crate) mux: Mux,
    pub(crate) loaded: bool,
    pub(crate) rows: Vec<Row>,
    pub(crate) kinds: Vec<RowKind>,
    pub(crate) list: ListState,
    pub(crate) selected_key: Option<(Kind, u64)>,
    pub(crate) scope: Scope,
    pub(crate) query: SearchState,
    pub(crate) parsed: Query,
    pub(crate) searcher: Searcher,
    pub(crate) overlay: Overlay,
    pub(crate) preview: PreviewCache,
    pub(crate) backdrop: Option<Text<'static>>,
    pub(crate) state: State,
    pub(crate) options: Options,
    pub(crate) keymap: Keymap,
    pub(crate) toast: Option<Toast>,
    pub(crate) hit: Hit,
    pub(crate) origin: Option<PaneId>,
    pub(crate) origin_window: Option<WindowId>,
    pub(crate) last_click: Option<(Instant, usize)>,
    pub(crate) opened: bool,
    pub(crate) hover_fx: bool,
    pub(crate) last_frame: Option<Instant>,
}

impl Default for SessionsApp {
    fn default() -> Self {
        Self {
            mux: Mux::default(),
            loaded: false,
            rows: Vec::new(),
            kinds: Vec::new(),
            list: ListState::default(),
            selected_key: None,
            scope: Scope::All,
            query: SearchState::default(),
            parsed: Query::default(),
            searcher: Searcher::default(),
            overlay: Overlay::None,
            preview: PreviewCache::new(true),
            backdrop: None,
            state: State::default(),
            options: Options::default(),
            keymap: Keymap::default(),
            toast: None,
            hit: Hit::default(),
            origin: None,
            origin_window: None,
            last_click: None,
            opened: false,
            hover_fx: false,
            last_frame: None,
        }
    }
}

impl App for SessionsApp {
    type Msg = Msg;

    fn name(&self) -> &'static str {
        "sessions"
    }

    fn init(&mut self, cx: &mut Cx<Msg>) {
        self.options = Options::from_value(&cx.context.options);
        let (keymap, warnings) = Keymap::with_overrides(&self.options.keys);
        for w in warnings {
            cx.log(format!("sessions: {w}"));
        }
        self.keymap = keymap;
        self.scope = self.options.scope;
        self.preview = PreviewCache::new(self.options.preview);
        self.origin = cx.context.origin.pane_id;
        self.origin_window = cx.context.origin.window_id;
        self.state = State::load();
        self.refresh(cx);
        let wants_backdrop = cx.presentation.mode == Mode::Overlay && cx.presentation.backdrop != Backdrop::None;
        if let (true, Some(origin)) = (wants_backdrop, self.origin) {
            let wezterm = cx.wezterm.clone();
            cx.spawn(move || Msg::Backdrop(wezterm.get_text(origin, None, true)));
        }
    }

    fn update(&mut self, event: Event<Msg>, cx: &mut Cx<Msg>) -> Flow {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Release => Flow::Continue,
            Event::Key(key) => self.on_key(key, cx),
            Event::Mouse(mouse) => self.on_mouse(mouse, cx),
            Event::Paste(text) if matches!(self.overlay, Overlay::None) => {
                let value = format!("{}{}", self.query.value, text.replace(['\n', '\r'], " "));
                self.query.set(value);
                self.on_query_changed(cx)
            }
            Event::Paste(_) | Event::Resize(..) | Event::Focus(_) => Flow::Continue,
            Event::Tick(_) => self.on_tick(cx),
            Event::Forwarded('D') => self.kill_all(cx),
            Event::Forwarded('k') => self.close(cx),
            Event::Forwarded(_) => Flow::Continue,
            Event::Msg(msg) => self.on_msg(msg, cx),
        }
    }

    fn view(&mut self, frame: &mut Frame, cx: &mut Cx<Msg>) {
        view::draw(self, frame, cx);
    }

    fn snapshot(&self) -> Value {
        let selected = self.list.selected();
        let rows: Vec<Value> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut v = json!({
                    "kind": r.kind,
                    "id": r.id,
                    "label": r.label,
                    "selected": Some(i) == selected && !r.header,
                });
                if let Some(w) = r.window_id {
                    v["window_id"] = w.into();
                }
                if let Some(t) = r.tab_id {
                    v["tab_id"] = t.into();
                }
                if !r.domain.is_empty() {
                    v["domain"] = r.domain.clone().into();
                }
                if r.kind == Kind::Window {
                    v["workspace"] = r.workspace.clone().into();
                }
                if let Some(state) = r.domain_state() {
                    v["state"] = json!(state);
                }
                v
            })
            .collect();
        let overlay = match &self.overlay {
            Overlay::None => Value::Null,
            Overlay::Confirm { title, body, .. } => {
                json!({ "kind": "confirm", "title": title, "value": body })
            }
            Overlay::Rename { title, input, .. } => {
                json!({ "kind": "rename", "title": title, "value": input.value })
            }
            Overlay::Move { title, choices, selected } => {
                json!({ "kind": "move", "title": title, "value": choices.get(*selected).map(|c| c.label.clone()) })
            }
            Overlay::Help => json!({ "kind": "help", "title": "Keys", "value": "" }),
        };
        let preview = match (self.preview_shown(), self.selected_pane()) {
            (true, Some(pane)) => {
                json!({ "pane_id": pane, "loaded": self.preview.get(pane).is_some() })
            }
            _ => Value::Null,
        };
        json!({
            "app": "sessions",
            "query": self.query.value,
            "scope": self.scope.name(),
            "selected": self.selected_row().map(|r| json!({ "kind": r.kind, "id": r.id })),
            "rows": rows,
            "overlay": overlay,
            "preview": preview,
        })
    }
}

impl SessionsApp {
    pub(crate) fn selected_row(&self) -> Option<&Row> {
        self.list.selected().and_then(|i| self.rows.get(i)).filter(|r| !r.header)
    }

    pub(crate) fn selected_pane(&self) -> Option<PaneId> {
        self.selected_row().and_then(|r| r.pane)
    }

    fn preview_shown(&self) -> bool {
        if self.hit.modal.is_empty() { self.preview.visible && self.options.preview } else { self.hit.preview_shown }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Cx<Msg>) {
        let wezterm = cx.wezterm.clone();
        let context = cx.context.clone();
        cx.spawn(move || Msg::Mux(wezterm.list().map(|records| Mux::build(&records, &context))));
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>, cx: &mut Cx<Msg>) -> Flow {
        let text = text.into();
        cx.log(format!("sessions: {text}"));
        self.toast = Some(Toast { text, until: Instant::now() + TOAST_FOR });
        cx.request_redraw();
        Flow::Continue
    }

    /// Rows for the current mux / scope / query; keeps the selection by identity when possible.
    pub(crate) fn rebuild(&mut self, cx: &mut Cx<Msg>) {
        let build = Build {
            mux: &self.mux,
            scope: self.scope,
            query: &self.parsed,
            flat: !self.query.is_empty(),
            mru: &self.state.mru,
            options: &self.options,
            own_pane: cx.own_pane,
        };
        self.rows = build.rows(&mut self.searcher);
        self.kinds = self.rows.iter().map(|r| if r.header { RowKind::Header } else { RowKind::Item }).collect();
        let previous = self.list.selected();
        let index = if !self.loaded {
            model::initial_index(&self.rows, &self.state.mru, self.origin, self.options.mru)
        } else {
            let by_key = self.selected_key.and_then(|k| self.rows.iter().position(|r| r.key() == k && !r.header));
            by_key.or_else(|| self.nearest_item(previous.unwrap_or(0)))
        };
        self.loaded = true;
        self.list.select(index);
        self.after_select(cx, false);
        self.ensure_preview(cx);
    }

    pub(crate) fn select_first(&mut self, cx: &mut Cx<Msg>) {
        let index = self.rows.iter().position(|r| !r.header);
        self.list.select(index);
        self.after_select(cx, false);
    }

    fn nearest_item(&self, from: usize) -> Option<usize> {
        let items: Vec<usize> = (0..self.rows.len()).filter(|i| !self.rows[*i].header).collect();
        items.iter().copied().find(|i| *i >= from).or_else(|| items.last().copied())
    }

    pub(crate) fn step(&mut self, cx: &mut Cx<Msg>, f: impl FnOnce(&mut ListState, &[RowKind])) -> Flow {
        if self.rows.is_empty() {
            return Flow::Continue;
        }
        f(&mut self.list, &self.kinds);
        self.after_select(cx, true);
        Flow::Continue
    }

    pub(crate) fn page(&self) -> i32 {
        (self.hit.list.height as i32).max(1)
    }

    /// Records the selection identity; on change requests a preview and a hover effect.
    pub(crate) fn after_select(&mut self, cx: &mut Cx<Msg>, animate: bool) {
        let key = self.selected_row().map(Row::key);
        let pane = self.selected_pane();
        let changed = key != self.selected_key;
        self.selected_key = key;
        if changed {
            self.hover_fx = animate;
            if let Some(pane) = pane {
                self.preview.want(pane, Instant::now());
                self.pump_preview(cx);
            }
            cx.request_redraw();
        }
    }

    fn ensure_preview(&mut self, cx: &mut Cx<Msg>) {
        if let Some(pane) = self.selected_pane()
            && self.preview.get(pane).is_none()
            && !self.preview.is_loading(pane)
        {
            self.preview.want(pane, Instant::now());
            self.pump_preview(cx);
        }
    }

    fn pump_preview(&mut self, cx: &mut Cx<Msg>) {
        if !(self.preview.visible && self.options.preview) {
            return;
        }
        if let Some(pane) = self.preview.due(Instant::now(), cx.headless) {
            let wezterm = cx.wezterm.clone();
            let lines = self.options.preview_lines;
            let token = cx.spawn_cancellable(move || Msg::Preview {
                pane_id: pane,
                text: wezterm.get_text(pane, Some(lines), true),
            });
            self.preview.start(pane, token);
        }
        if self.preview.has_pending() {
            cx.request_redraw();
        }
    }

    fn on_tick(&mut self, cx: &mut Cx<Msg>) -> Flow {
        self.pump_preview(cx);
        if let Some(toast) = &self.toast {
            if toast.until <= Instant::now() {
                self.toast = None;
            }
            cx.request_redraw();
        }
        Flow::Continue
    }

    fn on_msg(&mut self, msg: Msg, cx: &mut Cx<Msg>) -> Flow {
        match msg {
            Msg::Mux(Ok(mux)) => {
                self.mux = mux;
                self.rebuild(cx);
                Flow::Continue
            }
            Msg::Mux(Err(e)) => self.toast(format!("list: {e}"), cx),
            Msg::Backdrop(Ok(raw)) => {
                self.backdrop = Some(ansi_text(&preview::normalize(&raw)));
                Flow::Continue
            }
            Msg::Backdrop(Err(e)) => {
                cx.log(format!("sessions: backdrop: {e}"));
                Flow::Continue
            }
            Msg::Preview { pane_id, text: Ok(raw) } => {
                self.preview.finish(pane_id, Some(ansi_text(&preview::normalize(&raw))));
                Flow::Continue
            }
            Msg::Preview { pane_id, text: Err(e) } => {
                self.preview.finish(pane_id, None);
                cx.log(format!("sessions: preview {pane_id}: {e}"));
                Flow::Continue
            }
            Msg::Done(Ok(())) => {
                self.preview.invalidate();
                self.refresh(cx);
                Flow::Continue
            }
            Msg::Done(Err(e)) => self.toast(e.to_string(), cx),
            Msg::Switched { pane_id, result: Ok(()) } => self.finish_switch(pane_id, false, cx),
            Msg::Switched { result: Err(e), .. } => self.toast(e.to_string(), cx),
            Msg::Created(Ok(_)) => {
                self.preview.invalidate();
                self.refresh(cx);
                Flow::Continue
            }
            Msg::Created(Err(e)) => self.toast(e.to_string(), cx),
        }
    }

    /// Cross-window targets (and freshly spawned panes) need the GUI to focus them: `Action::Focus`.
    fn finish_switch(&mut self, pane: PaneId, focus: bool, cx: &mut Cx<Msg>) -> Flow {
        self.state.touch(pane);
        self.state.save();
        let cross_window = self.mux.pane(pane).is_none_or(|p| Some(p.window_id) != self.origin_window);
        if focus || cross_window {
            cx.emit(Action::Focus { pane_id: pane });
        }
        cx.fx.close(&cx.theme);
        Flow::Exit(Outcome::activated(pane))
    }

    pub(crate) fn on_query_changed(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let parsed = Query::parse(&self.query.value);
        let text_changed = parsed.text != self.parsed.text;
        self.parsed = parsed;
        self.rebuild(cx);
        if text_changed {
            self.select_first(cx);
        }
        Flow::Continue
    }

    fn on_key(&mut self, key: KeyEvent, cx: &mut Cx<Msg>) -> Flow {
        match &mut self.overlay {
            Overlay::Rename { input, .. } if !matches!(key.code, KeyCode::Enter | KeyCode::Esc) => {
                input.handle(&key);
                return Flow::Continue;
            }
            Overlay::Move { choices, selected, .. } if !matches!(key.code, KeyCode::Enter | KeyCode::Esc) => {
                let last = choices.len().saturating_sub(1);
                match (key.code, key.modifiers) {
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        *selected = selected.saturating_sub(1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        *selected = (*selected + 1).min(last);
                    }
                    (KeyCode::Home, _) => *selected = 0,
                    (KeyCode::End, _) => *selected = last,
                    (KeyCode::Char(c), KeyModifiers::NONE) if c.is_ascii_digit() => {
                        let n = c.to_digit(10).unwrap_or(0) as usize;
                        if (1..=choices.len()).contains(&n) {
                            *selected = n - 1;
                        }
                    }
                    _ => {}
                }
                return Flow::Continue;
            }
            _ => {}
        }
        match self.overlay {
            Overlay::None => self.on_main_key(key, cx),
            Overlay::Help => self.dismiss(),
            Overlay::Confirm { .. } => match Confirm::choice_from_key(&key) {
                Some(ConfirmChoice::Yes) => self.confirm_yes(cx),
                Some(ConfirmChoice::No) => self.dismiss(),
                None => Flow::Continue,
            },
            Overlay::Rename { .. } if key.code == KeyCode::Enter => self.rename_commit(cx),
            Overlay::Move { .. } if key.code == KeyCode::Enter => self.move_commit(cx),
            Overlay::Rename { .. } | Overlay::Move { .. } => self.dismiss(),
        }
    }

    pub(crate) fn dismiss(&mut self) -> Flow {
        self.overlay = Overlay::None;
        Flow::Continue
    }

    /// Bindings win over typing; while a query exists, editing keys and `?` still reach the input.
    fn on_main_key(&mut self, key: KeyEvent, cx: &mut Cx<Msg>) -> Flow {
        let editing = !self.query.is_empty();
        match self.keymap.action_for(&key) {
            Some(ActionId::Help) if editing && matches!(key.code, KeyCode::Char(_)) => self.edit_query(key, cx),
            Some(_) if editing && is_edit_key(&key) => self.edit_query(key, cx),
            Some(action) => self.run_action(action, cx),
            None => self.edit_query(key, cx),
        }
    }

    fn edit_query(&mut self, key: KeyEvent, cx: &mut Cx<Msg>) -> Flow {
        let key = if key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::CONTROL {
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)
        } else {
            key
        };
        if self.query.handle(&key) {
            return self.on_query_changed(cx);
        }
        Flow::Continue
    }

    fn on_mouse(&mut self, mouse: MouseEvent, cx: &mut Cx<Msg>) -> Flow {
        let pos = Position::new(mouse.column, mouse.row);
        let click = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left));
        match &mut self.overlay {
            Overlay::None => return self.on_main_mouse(mouse, pos, click, cx),
            Overlay::Help => {
                if click {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Confirm { .. } if click => match Confirm::choice_from_mouse(self.hit.dialog, &mouse) {
                Some(ConfirmChoice::Yes) => return self.confirm_yes(cx),
                Some(ConfirmChoice::No) => self.overlay = Overlay::None,
                None if !self.hit.dialog.contains(pos) => self.overlay = Overlay::None,
                None => {}
            },
            Overlay::Rename { .. } if click && !self.hit.dialog.contains(pos) => self.overlay = Overlay::None,
            Overlay::Move { choices, selected, .. } => {
                let last = choices.len().saturating_sub(1);
                match mouse.kind {
                    MouseEventKind::ScrollUp => *selected = selected.saturating_sub(1),
                    MouseEventKind::ScrollDown => *selected = (*selected + 1).min(last),
                    MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left)
                        if self.hit.choices.contains(pos) =>
                    {
                        let index = self.hit.choice_offset + (mouse.row - self.hit.choices.y) as usize;
                        if index <= last {
                            *selected = index;
                            if click {
                                return self.move_commit(cx);
                            }
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) if !self.hit.dialog.contains(pos) => {
                        self.overlay = Overlay::None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Flow::Continue
    }

    fn on_main_mouse(&mut self, mouse: MouseEvent, pos: Position, click: bool, cx: &mut Cx<Msg>) -> Flow {
        let in_list = self.hit.list.contains(pos);
        let in_preview = self.hit.preview_shown && self.hit.preview.contains(pos);
        match mouse.kind {
            MouseEventKind::Moved if in_list => {
                if let Some(i) = self.list.row_at(self.hit.list, mouse.row, &self.kinds)
                    && self.list.selected() != Some(i)
                {
                    self.list.select(Some(i));
                    self.after_select(cx, true);
                }
                Flow::Continue
            }
            MouseEventKind::ScrollDown if in_preview => {
                self.preview.scroll_by(-3);
                Flow::Continue
            }
            MouseEventKind::ScrollUp if in_preview => {
                self.preview.scroll_by(3);
                Flow::Continue
            }
            MouseEventKind::ScrollDown => self.step(cx, |l, k| l.select_next_item(k)),
            MouseEventKind::ScrollUp => self.step(cx, |l, k| l.select_prev_item(k)),
            MouseEventKind::Down(MouseButton::Left) if !self.hit.modal.contains(pos) => self.close(cx),
            MouseEventKind::Down(MouseButton::Left) if in_list => {
                let Some(i) = self.list.row_at(self.hit.list, mouse.row, &self.kinds) else {
                    return Flow::Continue;
                };
                let now = Instant::now();
                let again = self.list.selected() == Some(i);
                let double = self.last_click.is_some_and(|(at, row)| row == i && now - at <= DOUBLE_CLICK);
                self.last_click = Some((now, i));
                if again || double {
                    return self.switch(cx);
                }
                self.list.select(Some(i));
                self.after_select(cx, true);
                Flow::Continue
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(i) = self.hit.chips.iter().position(|r| r.contains(pos)) {
                    return self.set_scope(Scope::ALL[i], cx);
                }
                let _ = click;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    pub(crate) fn set_scope(&mut self, scope: Scope, cx: &mut Cx<Msg>) -> Flow {
        if scope != self.scope {
            self.scope = scope;
            self.rebuild(cx);
            if self.selected_row().is_none() {
                self.select_first(cx);
            }
        }
        Flow::Continue
    }

    pub(crate) fn run_action(&mut self, action: ActionId, cx: &mut Cx<Msg>) -> Flow {
        match action {
            ActionId::Switch => self.switch(cx),
            ActionId::Close => self.close(cx),
            ActionId::Down => self.step(cx, |l, k| l.select_next_item(k)),
            ActionId::Up => self.step(cx, |l, k| l.select_prev_item(k)),
            ActionId::PageDown => {
                let page = self.page();
                self.step(cx, |l, k| l.select_page(k, page))
            }
            ActionId::PageUp => {
                let page = self.page();
                self.step(cx, |l, k| l.select_page(k, -page))
            }
            ActionId::First => self.step(cx, |l, k| l.select_first_item(k)),
            ActionId::Last => self.step(cx, |l, k| l.select_last_item(k)),
            ActionId::ScopeNext => self.set_scope(self.scope.next(), cx),
            ActionId::ScopePrev => self.set_scope(self.scope.prev(), cx),
            ActionId::Kill => self.kill(cx),
            ActionId::KillAll => self.kill_all(cx),
            ActionId::NewTab => self.new_tab(cx),
            ActionId::NewWindow => self.new_window(cx),
            ActionId::Split => self.split(cx),
            ActionId::Rename => self.rename(cx),
            ActionId::Move => self.move_(cx),
            ActionId::Zoom => self.zoom(cx),
            ActionId::Preview => {
                self.preview.visible = !self.preview.visible;
                self.ensure_preview(cx);
                Flow::Continue
            }
            ActionId::PreviewUp => {
                self.preview.scroll_by(3);
                Flow::Continue
            }
            ActionId::PreviewDown => {
                self.preview.scroll_by(-3);
                Flow::Continue
            }
            ActionId::Clear => {
                self.query.clear();
                self.on_query_changed(cx)
            }
            ActionId::Help => {
                self.overlay = Overlay::Help;
                Flow::Continue
            }
        }
    }

    pub(crate) fn close(&mut self, cx: &mut Cx<Msg>) -> Flow {
        cx.fx.close(&cx.theme);
        Flow::Exit(Outcome::cancelled())
    }
}

fn is_edit_key(key: &KeyEvent) -> bool {
    matches!(
        (key.code, key.modifiers),
        (KeyCode::Backspace | KeyCode::Delete | KeyCode::Left | KeyCode::Right, _)
            | (KeyCode::Char('u' | 'w' | 'd' | 'a' | 'e' | 'h'), KeyModifiers::CONTROL)
            | (KeyCode::Char('b' | 'f' | 'd'), KeyModifiers::ALT)
    )
}
