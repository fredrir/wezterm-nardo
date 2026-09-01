use std::time::Instant;

use nardo_core::app::Cx;
use nardo_core::context::{Backdrop as BackdropMode, DomainKind, DomainState, PaneId};
use nardo_core::ui::modal::{Backdrop, Chrome, is_overlay, modal_area};
use nardo_core::ui::theme::Theme;
use nardo_core::ui::widgets::{
    Chips, Confirm, FuzzyList, Hint, Hints, ListRow, Preview, RowKind, SearchInput, highlight,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph};

use crate::keys::ActionId;
use crate::model::{Detail, Kind, Msg, Row, Scope, kind_name};
use crate::state::{KillWhat, Overlay, SessionsApp};

const PREVIEW_MIN_WIDTH: u16 = 80;

pub(crate) fn draw(app: &mut SessionsApp, frame: &mut Frame, cx: &mut Cx<Msg>) {
    let theme = cx.theme;
    let elapsed = app.last_frame.replace(Instant::now()).map(|t| t.elapsed()).unwrap_or_default();
    if !app.opened {
        app.opened = true;
        cx.fx.open(&theme);
    }
    let full = frame.area();
    let modal = modal_area(full, &cx.presentation);
    app.hit.modal = modal;

    let dimmed = is_overlay(&cx.presentation) && cx.presentation.backdrop != BackdropMode::None;
    match (&app.backdrop, dimmed) {
        (Some(text), true) => frame.render_widget(Backdrop { text, theme: &theme }, full),
        _ => {
            frame.render_widget(Clear, full);
            frame.buffer_mut().set_style(full, Style::new().bg(theme.bg));
        }
    }

    let inner = Chrome { theme: &theme, title: Some("Sessions"), focused: true }.render(modal, frame.buffer_mut());
    if inner.height >= 4 && inner.width >= 12 {
        draw_body(app, frame, cx, &theme, inner);
    }
    draw_overlay(app, frame, &theme, modal);
    cx.fx.render(frame, modal, elapsed);
}

fn draw_body(app: &mut SessionsApp, frame: &mut Frame, cx: &mut Cx<Msg>, theme: &Theme, inner: Rect) {
    let [search, chips, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)])
            .areas(inner);

    let input = SearchInput { theme, placeholder: "Search panes, tabs, windows, domains", icon: "⌕" };
    frame.render_stateful_widget(input, search, &mut app.query);

    let labels: Vec<&str> = Scope::ALL.iter().map(|s| s.label()).collect();
    let chips_widget = Chips { theme, labels: &labels, selected: app.scope.index() };
    app.hit.chips = chips_widget.hitboxes(chips);
    frame.render_widget(chips_widget, chips);

    let show_preview = app.preview.visible && app.options.preview && inner.width >= PREVIEW_MIN_WIDTH;
    let [list_area, preview_area] = if show_preview {
        Layout::horizontal([Constraint::Fill(1), Constraint::Percentage(45)]).areas(body)
    } else {
        [body, Rect::default()]
    };
    app.hit.list = list_area;
    app.hit.preview = preview_area;
    app.hit.preview_shown = show_preview;

    let flat = !app.parsed.text.is_empty() || app.scope != Scope::All;
    let rows: Vec<ListRow<'_>> = app.rows.iter().map(|r| list_row(r, flat, app.origin, theme)).collect();
    let empty_text = if app.loaded { "no matches" } else { "loading…" };
    frame.render_stateful_widget(FuzzyList { theme, rows, empty_text, scrollbar: true }, list_area, &mut app.list);

    if std::mem::take(&mut app.hover_fx)
        && let Some(i) = app.list.selected()
        && let Some(y) = i.checked_sub(app.list.offset())
        && (y as u16) < list_area.height
    {
        cx.fx.hover(theme, Rect::new(list_area.x, list_area.y + y as u16, list_area.width, 1));
    }

    if show_preview {
        let pane = app.selected_pane();
        let title = match (app.selected_row(), pane) {
            (Some(row), Some(pane)) if row.kind == Kind::Pane => format!("{}  #{pane}", row.label),
            (Some(row), Some(pane)) => format!("{}  › pane #{pane}", row.label),
            _ => "preview".to_string(),
        };
        let text = pane.and_then(|p| app.preview.get(p));
        let loading = pane.is_some_and(|p| app.preview.is_loading(p)) && text.is_none();
        frame.render_widget(Preview { theme, title: &title, text, loading, scroll: app.preview.scroll }, preview_area);
    }

    match &app.toast {
        Some(toast) if toast.until > Instant::now() => {
            let line = Line::from(vec![
                Span::styled("⚠ ", theme.danger()),
                Span::styled(toast.text.as_str(), Style::new().fg(theme.danger).bg(theme.surface)),
            ]);
            frame.render_widget(Paragraph::new(line).style(theme.base()), footer);
        }
        _ => {
            let hints = hints(app);
            frame.render_widget(Hints { theme, hints: &hints }, footer);
        }
    }

    if matches!(app.overlay, Overlay::None)
        && let Some(pos) = app.query.cursor_pos
    {
        frame.set_cursor_position(pos);
    }
}

fn hints(app: &SessionsApp) -> Vec<Hint> {
    let key = |a: ActionId| app.keymap.label(a);
    let hint = |a: ActionId, label: &str| Hint::new(key(a), label);
    let mut hints = Vec::new();
    match app.selected_row() {
        Some(row) => {
            match row.detail {
                Detail::Domain { state: DomainState::Detached, .. } => hints.push(hint(ActionId::Switch, "attach")),
                _ => hints.push(hint(ActionId::Switch, "switch")),
            }
            match row.kind {
                Kind::Pane => {
                    hints.push(hint(ActionId::Kill, "kill"));
                    hints.push(hint(ActionId::NewTab, "new tab"));
                    hints.push(hint(ActionId::Split, "split"));
                    hints.push(hint(ActionId::Move, "move"));
                    hints.push(hint(ActionId::Rename, "rename"));
                }
                Kind::Tab => {
                    hints.push(hint(ActionId::Kill, "kill"));
                    hints.push(hint(ActionId::NewTab, "new tab"));
                    hints.push(hint(ActionId::Move, "move"));
                    hints.push(hint(ActionId::Rename, "rename"));
                }
                Kind::Window => {
                    hints.push(hint(ActionId::Kill, "kill"));
                    hints.push(hint(ActionId::NewTab, "new tab"));
                    hints.push(hint(ActionId::NewWindow, "new window"));
                    hints.push(hint(ActionId::Rename, "rename"));
                }
                Kind::Domain => {
                    hints.push(hint(ActionId::NewWindow, "new window"));
                    if row.domain_state() == Some(DomainState::Attached) {
                        hints.push(hint(ActionId::Kill, "detach"));
                    }
                }
            }
        }
        None => {
            hints.push(hint(ActionId::ScopeNext, "scope"));
            hints.push(hint(ActionId::Help, "help"));
        }
    }
    hints.push(hint(ActionId::Close, "close"));
    hints.retain(|h| !h.key.is_empty());
    hints
}

/// Highlights `text` with the haystack indices that fall inside `[offset, offset + len)`.
fn seg<'a>(text: &'a str, indices: &[u32], offset: u32, base: Style, theme: &Theme) -> Vec<Span<'a>> {
    if text.is_empty() {
        return Vec::new();
    }
    let len = text.chars().count() as u32;
    let local: Vec<u32> = indices.iter().filter_map(|i| i.checked_sub(offset).filter(|i| *i < len)).collect();
    highlight(text, &local, base, theme).spans
}

fn chip<'a>(text: &'a str, theme: &Theme) -> Span<'a> {
    Span::styled(format!(" {text} "), Style::new().fg(theme.text_muted).bg(theme.surface_hi))
}

fn kind_chip<'a>(kind: DomainKind, theme: &Theme) -> Option<Span<'a>> {
    matches!(kind, DomainKind::Tls | DomainKind::Ssh | DomainKind::Wsl | DomainKind::Serial)
        .then(|| chip(kind_name(kind), theme))
}

fn list_row<'a>(row: &'a Row, flat: bool, origin: Option<PaneId>, theme: &'a Theme) -> ListRow<'a> {
    let base = theme.base();
    let muted = theme.muted();
    let dim = theme.dim();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut trailing: Vec<Span<'a>> = Vec::new();
    let label_len = row.label.chars().count() as u32;
    let push_meta = |spans: &mut Vec<Span<'a>>| {
        if flat && !row.meta.is_empty() {
            spans.push(Span::styled("  ", base));
            spans.extend(seg(&row.meta, &row.indices, label_len + 1, muted, theme));
        }
    };
    match &row.detail {
        Detail::Window { tabs, panes } => {
            spans.push(Span::styled("▣ ", muted));
            spans.extend(seg(&row.label, &row.indices, 0, base.add_modifier(Modifier::BOLD), theme));
            if row.header {
                if !row.workspace.is_empty() {
                    spans.push(Span::styled(format!("  {}", row.workspace), muted));
                }
                if !row.domain.is_empty() {
                    spans.push(Span::styled(" ", base));
                    spans.push(chip(&row.domain, theme));
                }
            } else {
                push_meta(&mut spans);
            }
            trailing.push(Span::styled(format!("{tabs} tabs · {panes} panes"), dim));
        }
        Detail::Tab { panes, .. } => {
            spans.push(Span::styled("▤ ", muted));
            spans.extend(seg(&row.label, &row.indices, 0, base, theme));
            push_meta(&mut spans);
            if *panes > 1 {
                trailing.push(Span::styled(format!("{panes} panes "), dim));
            }
            trailing.push(Span::styled(format!("#{}", row.id), dim));
        }
        Detail::Pane { process, cwd, unseen, zoomed, kind } => {
            spans.push(Span::styled("▪ ", muted));
            spans.extend(seg(process, &row.indices, 0, base, theme));
            if !cwd.is_empty() {
                spans.push(Span::styled(" ", base));
                spans.extend(seg(cwd, &row.indices, process.chars().count() as u32 + 1, muted, theme));
            }
            push_meta(&mut spans);
            if *unseen {
                trailing.push(Span::styled("● ", theme.accent()));
            }
            if *zoomed {
                trailing.push(Span::styled("⤢ ", muted));
            }
            if let Some(chip) = kind_chip(*kind, theme) {
                trailing.push(chip);
                trailing.push(Span::styled(" ", base));
            }
            if origin == Some(row.id) {
                trailing.push(Span::styled("here ", dim));
            }
            trailing.push(Span::styled(format!("#{}", row.id), dim));
        }
        Detail::Domain { state, kind, windows, .. } => {
            spans.push(Span::styled("◈ ", muted));
            spans.extend(seg(&row.label, &row.indices, 0, base, theme));
            if !row.meta.is_empty() {
                spans.push(Span::styled("  ", base));
                spans.extend(seg(&row.meta, &row.indices, label_len + 1, muted, theme));
            }
            if *windows > 0 {
                trailing.push(Span::styled(format!("{windows} windows "), dim));
            }
            if let Some(chip) = kind_chip(*kind, theme) {
                trailing.push(chip);
                trailing.push(Span::styled(" ", base));
            }
            let badge = match state {
                DomainState::Attached => Span::styled("attached", Style::new().fg(theme.success).bg(theme.surface)),
                DomainState::Detached => Span::styled("detached", Style::new().fg(theme.warning).bg(theme.surface)),
            };
            trailing.push(badge);
        }
    }
    ListRow {
        kind: if row.header { RowKind::Header } else { RowKind::Item },
        indent: row.depth,
        line: Line::from(spans),
        trailing: (!trailing.is_empty()).then(|| Line::from(trailing)),
        style: base,
    }
}

fn dialog(theme: &Theme, title: &str, danger: bool) -> Block<'static> {
    let border = if danger { theme.danger } else { theme.border_focus };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border).bg(theme.surface))
        .title(Line::from(Span::styled(format!(" {title} "), theme.accent())))
        .style(theme.base())
        .padding(Padding::horizontal(1))
}

fn draw_overlay(app: &mut SessionsApp, frame: &mut Frame, theme: &Theme, modal: Rect) {
    match &mut app.overlay {
        Overlay::None => {}
        Overlay::Confirm { what, title, body } => {
            let area = Confirm::area(modal);
            app.hit.dialog = area;
            let yes = match what {
                KillWhat::Panes(_) => "Kill",
                KillWhat::Detach(_) => "Detach",
            };
            frame.render_widget(Confirm { theme, title, body, yes, no: "Cancel", danger: true }, area);
        }
        Overlay::Rename { title, input, .. } => {
            let area = modal.centered(Constraint::Length(modal.width.saturating_sub(4).min(60)), Constraint::Length(3));
            app.hit.dialog = area;
            frame.render_widget(Clear, area);
            let block = dialog(theme, title, false);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_stateful_widget(SearchInput { theme, placeholder: "title", icon: "✎" }, inner, input);
            if let Some(pos) = input.cursor_pos {
                frame.set_cursor_position(pos);
            }
        }
        Overlay::Move { title, choices, selected } => {
            let visible = choices.len().clamp(1, 12) as u16;
            let width = modal.width.saturating_sub(4).min(70);
            let area = modal.centered(Constraint::Length(width), Constraint::Length(visible + 2));
            app.hit.dialog = area;
            frame.render_widget(Clear, area);
            let block = dialog(theme, title, false);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let offset =
                selected.saturating_sub(visible as usize - 1).min(choices.len().saturating_sub(visible as usize));
            app.hit.choices = inner;
            app.hit.choice_offset = offset;
            for (i, choice) in choices.iter().enumerate().skip(offset).take(visible as usize) {
                let y = inner.y + (i - offset) as u16;
                let style = if i == *selected { theme.selected() } else { theme.base() };
                let marker = if i == *selected { "▎" } else { " " };
                let line = Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(format!("{:>2}  ", i + 1), if i == *selected { style } else { theme.dim() }),
                    Span::styled(choice.label.as_str(), style),
                ]);
                frame.render_widget(Paragraph::new(line).style(style), Rect::new(inner.x, y, inner.width, 1));
            }
        }
        Overlay::Help => {
            let lines = app.keymap.help_lines();
            let height = (lines.len() as u16 + 3).min(modal.height.saturating_sub(2));
            let width = modal.width.saturating_sub(4).min(78);
            let area = modal.centered(Constraint::Length(width), Constraint::Length(height));
            app.hit.dialog = area;
            frame.render_widget(Clear, area);
            let block =
                dialog(theme, "Keys", false).title_bottom(Line::from(Span::styled(" any key closes ", theme.dim())));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let text: Vec<Line> = lines
                .iter()
                .map(|(keys, name, desc)| {
                    Line::from(vec![
                        Span::styled(format!("{keys:<24}"), theme.accent()),
                        Span::styled(format!("{name:<13}"), theme.base()),
                        Span::styled(*desc, theme.muted()),
                    ])
                })
                .collect();
            frame.render_widget(Paragraph::new(text).style(theme.base()), inner);
        }
    }
}
