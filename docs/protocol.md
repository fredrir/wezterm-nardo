# Protocol

## Process

`wez-nardo <app>` runs in a pane of the GUI's `local` domain. Exit sequence: perform final action → `wezterm cli kill-pane --pane-id $WEZTERM_PANE` → exit 0.

| env               | value                                                         |
| ----------------- | ------------------------------------------------------------- |
| `NARDO_CONTEXT`   | path to context json (below)                                  |
| `NARDO_WEZTERM`   | wezterm executable `(fallback: WEZTERM_EXECUTABLE, PATH)`     |
| `WEZTERM_PANE`    | own pane id, set by WezTerm                                   |
| `WEZPLUG_USERVAR` | user var name for actions `(fallback: nardo)`                 |
| `WEZPLUG_LOG`     | debug log file `(fallback: none)`                             |
| `NARDO_STATE_DIR` | persisted state dir `(fallback: $XDG_STATE_HOME/wez-nardo)`   |

## CLI

```sh
wez-nardo sessions                                  # interactive
wez-nardo sessions --headless --keys 'vim enter'    # scripted, prints outcome json
wez-nardo sessions --headless --keys 'j' --dump     # + view snapshot
wez-nardo sessions --context ctx.json --size 120x40
```

| flag         | value                                                      |
| ------------ | ---------------------------------------------------------- |
| `--headless` | `TestBackend`, no tty, actions still run through `wezterm` |
| `--keys`     | whitespace separated tokens, see key script                |
| `--dump`     | include `snapshot` (view model) in outcome                 |
| `--size`     | `COLSxROWS` `(fallback: 120x40)`                           |
| `--context`  | overrides `NARDO_CONTEXT`                                  |

### Key script

| token                          | event                       |
| ------------------------------ | --------------------------- |
| `a` `Z` `/`                    | char (shift = uppercase)    |
| `"vim ~/x"`                    | chars                       |
| `enter esc tab backtab space`  | named key                   |
| `up down left right home end pageup pagedown backspace delete f1..f12` | named key |
| `ctrl+d alt+k ctrl+shift+d`    | modifiers                   |
| `mouse:move:X,Y` `mouse:click:X,Y` `mouse:scroll:up|down` | mouse; wheel without a position targets the list |
| `settle`                       | wait for in-flight jobs     |

Every token settles jobs before the next one unless it is a char.

### Outcome json (stdout, headless)

```json
{ "exit": "activated", "pane_id": 51, "actions": [{"t":"attach_domain","domain":"archie-cable"}], "snapshot": {...} }
```

| `exit`       | meaning                                  |
| ------------ | ---------------------------------------- |
| `activated`  | pane switched (`pane_id`)                |
| `cancelled`  | esc / click outside                      |
| `handed_off` | Lua action owns the rest (`actions[-1]`) |
| `open`       | script ended, launcher still open        |

`actions` = user-var payloads that would have been emitted (headless captures instead of writing OSC).

### Snapshot json (`--dump`, sessions)

Scope `all` + empty query → tree (window headers › tab rows › pane rows). Non-empty query → flat pane rows, no headers. Detached domains only in scope `domains`.

```json
{
  "app": "sessions",
  "query": "",
  "scope": "all",
  "selected": { "kind": "pane", "id": 51 },
  "rows": [
    { "kind": "window", "id": 10, "label": "fredrir@archie", "domain": "localmux", "workspace": "default", "selected": false },
    { "kind": "tab", "id": 30, "label": "1 · nvim", "window_id": 10, "selected": false },
    { "kind": "pane", "id": 51, "label": "nvim ~/projects/x", "window_id": 10, "tab_id": 30, "domain": "localmux", "selected": true }
  ],
  "overlay": null,
  "preview": { "pane_id": 51, "loaded": true }
}
```

Scope `domains` row: `{ "kind": "domain", "id": 0, "label": "archie-wifi", "domain": "archie-wifi", "state": "Detached", "selected": false }`.

| field      | value                                                                    |
| ---------- | ------------------------------------------------------------------------ |
| `scope`    | `all` `windows` `tabs` `panes` `domains`                                 |
| `rows[]`   | visible rows in order; `kind` `window` `tab` `pane` `domain`; headers included with `selected: false` |
| `overlay`  | `null` or `{ "kind": "confirm" | "rename" | "move" | "help", "title": ".." , "value": ".." }` |
| `preview`  | `null` when hidden                                                       |

### Sessions options (`context.options`)

| key             | default  |
| --------------- | -------- |
| `confirm_kill`  | `true`   |
| `preview`       | `true`   |
| `preview_lines` | `200`    |
| `mru`           | `true`   |
| `scope`         | `"all"`  |
| `show_self`     | `false`  |
| `keys`          | `{}` — `{ action = "key" | ["key", ...] | false }`, see docs/keys.md |

## Context json (Lua → Rust)

```json
{
  "v": 1,
  "app": "sessions",
  "origin": { "pane_id": 46, "tab_id": 30, "window_id": 10, "workspace": "default", "domain": "localmux" },
  "domains": [
    { "name": "local", "label": "local", "kind": "local", "state": "Attached", "spawnable": true, "has_panes": true },
    { "name": "archie-cable", "label": "archie via cable", "kind": "tls", "state": "Detached", "spawnable": true, "has_panes": false }
  ],
  "panes": { "46": { "domain": "localmux", "process": "zsh", "cwd": "file://archie/home/f", "unseen": false, "alt_screen": false } },
  "workspaces": { "active": "default", "names": ["default"] },
  "theme": { "background": "#1e1e2e", "foreground": "#cdd6f4", "ansi": ["#..", "..8"], "brights": ["#..", "..8"], "selection_bg": "#..", "selection_fg": "#..", "cursor_bg": "#..", "accent": "#89b4fa" },
  "presentation": { "mode": "overlay", "width": 0.72, "height": 0.7, "max_width": 128, "max_height": 42, "backdrop": "dim", "animations": true },
  "options": {}
}
```

| field            | value                                                                        |
| ---------------- | ---------------------------------------------------------------------------- |
| `domains[].kind` | `local` `unix` `tls` `ssh` `exec` `wsl` `serial` `unknown`                   |
| `domains[].state`| `Attached` `Detached`                                                        |
| `panes`          | keyed by pane id; fields `wezterm cli list` lacks                            |
| `theme`          | `window:effective_config().resolved_palette`, `accent` = ansi blue (fallback) |
| `presentation`   | mode `overlay` `tab` `window` `split`; width/height ≤ 1 = fraction, > 1 = cells |
| `options`        | app options table from Lua config, app defines schema                        |

## Actions (Rust → Lua)

`ESC ] 1337 ; SetUserVar=nardo=<base64(json)> BEL` on the launcher pane. `nardo_role=launcher` on start.

| `t`             | args                                  | Lua                                                   |
| --------------- | ------------------------------------- | ----------------------------------------------------- |
| `attach_domain` | `domain`                              | `mux.get_domain(domain):attach()`                     |
| `detach_domain` | `domain`                              | `mux.get_domain(domain):detach()`                     |
| `focus`         | `pane_id`                             | `pane:activate()`, `gui_window:focus()`               |
| `run`           | `name`, `args`                        | `nardo.launcher.on_action(name, fn)` handlers         |
| `done`          | `exit`                                | close `window` presentation, restore origin           |
| `error`         | `message`                             | `wezterm.log_error`, toast                            |

`n` (monotonic counter) is included in every payload: WezTerm only fires `user-var-changed` on change.

## Forwarded keys (Lua → Rust)

WezTerm never delivers `CMD` chords to terminal apps. Lua binds them and forwards via `pane:send_text` when the active pane has `nardo_role=launcher`.

| bytes                 | meaning                 |
| --------------------- | ----------------------- |
| `U+E000` + `D`        | kill all (`CMD+SHIFT+d`)|
| `U+E000` + `k`        | close (`CMD+k` toggle)  |
| `U+E000` + `<char>`   | app defined             |

## Fake wezterm (tests)

`NARDO_WEZTERM` points at `tests/fake_wezterm.py`. Same argv as the real cli: `cli [--class X] <sub> [args]`.

| env                  | value                                                              |
| -------------------- | ------------------------------------------------------------------ |
| `FAKE_WEZTERM_STATE` | json file, mutated by `kill-pane` `move-pane-to-new-tab` `split-pane` `spawn` `set-tab-title` |
| `FAKE_WEZTERM_LOG`   | append one json line per call: `{"args": ["list", "--format", "json"]}` |

```json
{
  "panes": [ { "window_id": 10, "tab_id": 30, "pane_id": 46, "workspace": "default", "size": {"rows": 40, "cols": 120}, "title": "zsh", "cwd": "file://archie/home/f", "tab_title": "", "window_title": "w", "is_active": true, "is_zoomed": false } ],
  "text": { "46": "$ ls\nfoo bar\n" },
  "next_pane_id": 100,
  "fail": { "kill-pane": "pane not found" }
}
```

`spawn` / `split-pane` print `next_pane_id` and add a pane; `fail[sub]` makes that subcommand exit 1 with the message on stderr.

## `wezterm cli` used

| command                                             | purpose                        |
| --------------------------------------------------- | ------------------------------ |
| `list --format json`                                | windows/tabs/panes             |
| `get-text --pane-id N --escapes [--start-line -K]`  | preview, backdrop              |
| `activate-pane --pane-id N`                         | switch                         |
| `activate-tab --tab-id N`                           | switch tab                     |
| `kill-pane --pane-id N`                             | kill                           |
| `move-pane-to-new-tab --pane-id N [--window-id W | --new-window [--workspace S]]` | move pane |
| `split-pane --pane-id N --move-pane-id M [--right|--bottom]` | move pane into tab    |
| `spawn [--domain-name D] [--window-id W | --new-window [--workspace S]] [--cwd C]` | create |
| `set-tab-title --tab-id N TITLE`                    | rename tab                     |
| `set-window-title --window-id N TITLE`              | rename window                  |
| `rename-workspace --workspace S NEW`                | rename workspace               |
| `zoom-pane --pane-id N --toggle`                    | zoom                           |
