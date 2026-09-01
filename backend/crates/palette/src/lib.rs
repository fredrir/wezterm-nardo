//! Command palette: fuzzy list of commands from `context.options.commands`, run via `Action::Run`.

use nardo_core::app::{App, Cx, Flow, Outcome};
use nardo_core::event::{Event, KeyCode, KeyModifiers, MouseEventKind};
use nardo_core::search::{Ranked, Searcher};
use nardo_core::ui::modal::{Backdrop as BackdropWidget, Chrome, ansi_text, is_overlay, modal_area};
use nardo_core::ui::widgets::{
    FuzzyList, Hint, Hints, ListRow, ListStateExt, RowKind, SearchInput, SearchState, highlight,
};
use nardo_core::wezterm::{Action, Result as WezResult};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::ListState;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize, Default)]
struct Options {
    #[serde(default)]
    commands: Vec<Command>,
}

#[derive(Debug, Clone, Deserialize)]
struct Command {
    id: String,
    label: String,
    #[serde(default)]
    hint: String,
}

pub enum Msg {
    Backdrop(WezResult<String>),
}

#[derive(Default)]
pub struct PaletteApp {
    commands: Vec<Command>,
    query: SearchState,
    list: ListState,
    searcher: Searcher,
    ranked: Vec<Ranked<usize>>,
    backdrop: Option<Text<'static>>,
    list_area: Rect,
    opened: bool,
}

impl PaletteApp {
    fn rerank(&mut self) {
        let items = self.commands.iter().enumerate().map(|(i, c)| (i, c.label.clone()));
        self.ranked = self.searcher.rank(&self.query.value, items);
        self.list.select(if self.ranked.is_empty() { None } else { Some(0) });
        *self.list.offset_mut() = 0;
    }

    fn kinds(&self) -> Vec<RowKind> {
        vec![RowKind::Item; self.ranked.len()]
    }

    fn selected(&self) -> Option<&Command> {
        self.ranked.get(self.list.selected()?).map(|r| &self.commands[r.item])
    }

    fn run_selected(&mut self, cx: &mut Cx<Msg>) -> Flow {
        let Some(cmd) = self.selected() else {
            return Flow::Continue;
        };
        cx.emit(Action::Run { name: "command".into(), args: json!({ "id": cmd.id }) });
        Flow::Exit(Outcome::handed_off())
    }
}

impl App for PaletteApp {
    type Msg = Msg;

    fn name(&self) -> &'static str {
        "palette"
    }

    fn init(&mut self, cx: &mut Cx<Msg>) {
        let options: Options = serde_json::from_value(cx.context.options.clone()).unwrap_or_default();
        self.commands = options.commands;
        self.rerank();
        let overlay_backdrop =
            is_overlay(&cx.presentation) && cx.presentation.backdrop == nardo_core::context::Backdrop::Dim;
        if let Some(origin) = cx.context.origin.pane_id
            && overlay_backdrop
        {
            let wezterm = cx.wezterm.clone();
            cx.spawn(move || Msg::Backdrop(wezterm.get_text(origin, None, true)));
        }
    }

    fn update(&mut self, event: Event<Msg>, cx: &mut Cx<Msg>) -> Flow {
        match event {
            Event::Msg(Msg::Backdrop(Ok(raw))) => {
                self.backdrop = Some(ansi_text(&raw));
                cx.request_redraw();
            }
            Event::Msg(Msg::Backdrop(Err(e))) => cx.log(format!("backdrop: {e}")),
            Event::Forwarded('p') | Event::Forwarded('k') => return Flow::Exit(Outcome::cancelled()),
            Event::Key(key) => {
                let kinds = self.kinds();
                match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) => return Flow::Exit(Outcome::cancelled()),
                    (KeyCode::Enter, _) => return self.run_selected(cx),
                    (KeyCode::Down, _) | (KeyCode::Char('n' | 'j'), KeyModifiers::CONTROL) => {
                        self.list.select_next_item(&kinds)
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('p' | 'k'), KeyModifiers::CONTROL) => {
                        self.list.select_prev_item(&kinds)
                    }
                    (KeyCode::PageDown, _) => self.list.select_page(&kinds, 1),
                    (KeyCode::PageUp, _) => self.list.select_page(&kinds, -1),
                    (KeyCode::Home, _) => self.list.select_first_item(&kinds),
                    (KeyCode::End, _) => self.list.select_last_item(&kinds),
                    _ => {
                        if self.query.handle(&key) {
                            self.rerank();
                        }
                    }
                }
                cx.request_redraw();
            }
            Event::Mouse(mouse) => {
                let kinds = self.kinds();
                let at = Position::new(mouse.column, mouse.row);
                match mouse.kind {
                    MouseEventKind::ScrollDown => self.list.select_next_item(&kinds),
                    MouseEventKind::ScrollUp => self.list.select_prev_item(&kinds),
                    MouseEventKind::Down(_) if cx.area.contains(at) => {
                        if let Some(row) = self.list.row_at(self.list_area, mouse.row, &kinds) {
                            if self.list.selected() == Some(row) {
                                return self.run_selected(cx);
                            }
                            self.list.select(Some(row));
                        }
                    }
                    MouseEventKind::Down(_) => return Flow::Exit(Outcome::cancelled()),
                    _ => return Flow::Continue,
                }
                cx.request_redraw();
            }
            _ => {}
        }
        Flow::Continue
    }

    fn view(&mut self, frame: &mut Frame, cx: &mut Cx<Msg>) {
        let theme = cx.theme;
        let full = frame.area();
        if let Some(text) = &self.backdrop {
            frame.render_widget(BackdropWidget { text, theme: &theme }, full);
        } else if is_overlay(&cx.presentation) {
            frame.render_widget(BackdropWidget { text: &Text::default(), theme: &theme }, full);
        }
        let modal = modal_area(full, &cx.presentation);
        cx.area = modal;
        if !self.opened {
            self.opened = true;
            cx.fx.open(&theme);
        }
        let inner = Chrome { theme: &theme, title: Some("Palette"), focused: true }.render(modal, frame.buffer_mut());
        let [input_area, list_area, footer] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        self.list_area = list_area;

        let input = SearchInput { theme: &theme, placeholder: "Run a command", icon: "⌕" };
        frame.render_stateful_widget(input, input_area, &mut self.query);
        if let Some(pos) = self.query.cursor_pos {
            frame.set_cursor_position(pos);
        }

        let rows: Vec<ListRow> = self
            .ranked
            .iter()
            .map(|r| {
                let cmd = &self.commands[r.item];
                let trailing = (!cmd.hint.is_empty()).then(|| Line::from(Span::styled(cmd.hint.clone(), theme.dim())));
                ListRow {
                    kind: RowKind::Item,
                    indent: 0,
                    line: highlight(&cmd.label, &r.indices, theme.base(), &theme),
                    trailing,
                    style: theme.base(),
                }
            })
            .collect();
        let list = FuzzyList { theme: &theme, rows, empty_text: "no matching command", scrollbar: true };
        frame.render_stateful_widget(list, list_area, &mut self.list);

        let hints = [Hint::new("↵", "run"), Hint::new("esc", "close")];
        frame.render_widget(Hints { theme: &theme, hints: &hints }, footer);
    }

    fn snapshot(&self) -> serde_json::Value {
        let selected = self.list.selected();
        json!({
            "app": "palette",
            "query": self.query.value,
            "selected": self.selected().map(|c| json!({ "id": c.id, "label": c.label })),
            "rows": self
                .ranked
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let cmd = &self.commands[r.item];
                    json!({ "id": cmd.id, "label": cmd.label, "selected": selected == Some(i) })
                })
                .collect::<Vec<_>>(),
        })
    }
}
