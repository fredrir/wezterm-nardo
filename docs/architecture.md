# Architecture

One Lua plugin, one Rust binary. Lua owns WezTerm integration (keys, spawning, domains, config).
Rust owns everything the user sees (ratatui + tachyonfx) and every mux query/action (`wezterm cli`).

```
keybinding ─▶ nardo.launcher.open(app)     Lua: snapshot context → spawn `wez-nardo <app>` (local domain)
                    │
                    ▼
            wez-nardo <app>                Rust: TUI; reads context, drives `wezterm cli`
                    │  OSC 1337 SetUserVar=nardo=<json>
                    ▼
        wezterm.on "user-var-changed"      Lua: attach/detach domain, focus window, plugin actions
```

## Plugins

| plugin     | Lua module            | Rust crate        | status   |
| ---------- | --------------------- | ----------------- | -------- |
| launcher   | `nardo.launcher`      | `nardo-core`      | base     |
| sessions   | `nardo.sessions`      | `nardo-sessions`  | v1       |
| palette    | `nardo.palette`       | `nardo-palette`   | skeleton |
| quick      | `nardo.quick`         | —  (Lua only)     | skeleton |

## Rust workspace `backend/`

| crate            | path              | owns                                                                 |
| ---------------- | ----------------- | -------------------------------------------------------------------- |
| `wez-nardo`      | `crates/cli`      | clap entry: `wez-nardo sessions|palette [--headless --keys ..]`      |
| `nardo-core`     | `crates/core`     | runtime, events, headless driver, ui kit, theme, fx, search, wezterm |
| `nardo-sessions` | `crates/sessions` | `SessionsApp`: mux model, scopes, actions, preview, view             |
| `nardo-palette`  | `crates/palette`  | `PaletteApp`: command list from context, run via action              |

### `nardo-core` modules

| module          | responsibility                                                            |
| --------------- | ------------------------------------------------------------------------- |
| `app`           | `App` trait (`Msg`, `update`, `view`, `snapshot`), `Flow`, `Outcome`      |
| `runtime`       | terminal guard, input thread, tick/fx clock, jobs, `run` / `run_headless` |
| `event`         | `Event<Msg>`: `Key`, `Mouse`, `Resize`, `Tick`, `Msg`, `Forwarded`        |
| `keys`          | key names ↔ crossterm, `--keys` script parser, in-band forwarded keys     |
| `context`       | `Context` (JSON from Lua): origin, domains, pane extras, theme, options   |
| `wezterm`       | `Wezterm` trait + `Cli` impl, `PaneRecord` (cli list json), user vars     |
| `mux`           | `Mux` tree: domains › windows › tabs › panes, built from cli + context    |
| `search`        | nucleo matcher, `Ranked<T>` with match indices                            |
| `ui::theme`     | `Theme` from WezTerm palette                                              |
| `ui::modal`     | centered modal rect, backdrop (dimmed origin pane), frame                 |
| `ui::widgets`   | `SearchInput`, `FuzzyList`, `Preview`, `Confirm`, `Hints`, `Chips`        |
| `ui::fx`        | open/close/hover effects (tachyonfx), `reduce_motion`                     |
| `state`         | persisted state (`$XDG_STATE_HOME/wez-nardo/state.json`): MRU, last query |
| `log`           | `WEZPLUG_LOG` file logger                                                 |

## Lua modules `plugin/nardo/`

| module       | responsibility                                                            |
| ------------ | ------------------------------------------------------------------------- |
| `init`       | `apply_to_config`, exports `launcher`, `sessions`, `palette`, `quick`     |
| `config`     | defaults, validation                                                      |
| `launcher`   | `open(window, pane, spec)`: context → presentation → spawn; `on_action`   |
| `context`    | snapshot: origin, domains, pane extras, workspaces, theme                 |
| `present`    | modes: `overlay` (tab + backdrop), `tab`, `window` (floating), `split`    |
| `actions`    | `user-var-changed` dispatch: `attach_domain`, `focus`, `done`, custom     |
| `sessions`   | key binding, options → `launcher.open(.., { app = "sessions" })`          |
| `palette`    | key binding, command registry → `launcher.open(.., { app = "palette" })`  |
| `quick`      | dropdown window toggle (pure WezTerm)                                     |
| `backend`    | binary resolution, bootstrap (template)                                   |
| `platform`   | triple, `SUPER` mods per OS                                               |

## Rules

| rule                        | value                                                                         |
| --------------------------- | ----------------------------------------------------------------------------- |
| launcher spawn domain       | `local` (GUI mux) — `wezterm cli` then sees local + unix + tls + ssh panes    |
| mux queries / actions       | `wezterm cli` (`NARDO_WEZTERM`), never Lua polling                            |
| Lua-only actions            | attach/detach domain, gui focus, plugin custom actions (user var)             |
| tests                       | pytest + fake `wezterm` on `NARDO_WEZTERM`; `--headless --keys` driver        |
| UI tests                    | none until design settles                                                     |
| deps                        | ratatui 0.30, tachyonfx 0.25, crossterm 0.29, nucleo-matcher 0.3, ansi-to-tui 8 |
