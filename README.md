# wez-nardo

Spotlight for WezTerm. A fast, batteries-included launcher family: Lua plugin front, Rust TUI back
(ratatui + tachyonfx), native WezTerm integration everywhere it counts — including TLS/unix mux domains.

| launcher            | default key | what                                                        |
| ------------------- | ----------- | ----------------------------------------------------------- |
| Session explorer    | `⌘K`        | fuzzy-search domains › windows › tabs › panes, live preview, switch/kill/move/rename/create, attach TLS domains |
| Command palette     | `⌘⇧P`       | your commands + built-in WezTerm actions                    |
| Quick terminal      | `⌥⌘T`       | dropdown terminal window                                    |

All three sit on one shared launcher base (`nardo-core` + `nardo.launcher`) that other plugins can
build on: context snapshot in, centered overlay UI out, actions back via user vars.

## Install

```lua
local wezterm = require "wezterm"
local nardo = wezterm.plugin.require "https://github.com/fredrir/wezterm-nardo"

local config = wezterm.config_builder()
nardo.apply_to_config(config, {})
return config
```

The backend binary is resolved automatically: explicit `backend.path` → cached → GitHub release
(checksum-verified) → local `cargo build`.

## Feel

- Centered overlay above a dimmed snapshot of your pane (`presentation.mode = "overlay" | "tab" | "window" | "split"`)
- Colours derived from your WezTerm palette; subtle open/close/hover effects (`presentation.animations = false` to disable)
- Full mouse support: hover to preview, click to select, wheel to move, chips to change scope
- Are-you-sure prompts before anything destructive

## Keys, options, internals

| doc                                          | what                                  |
| -------------------------------------------- | ------------------------------------- |
| [docs/keys.md](docs/keys.md)                 | every binding, mouse, query syntax    |
| [docs/configuration.md](docs/configuration.md) | all options                         |
| [docs/architecture.md](docs/architecture.md) | crates, modules, data flow            |
| [docs/protocol.md](docs/protocol.md)         | context json, actions, headless driver |
| [docs/development.md](docs/development.md)   | dev rig, tests, release               |

## Remote / TLS mux

The launcher always runs on the GUI host in the `local` domain, so it sees every attached domain —
local, unix and TLS — and lists detached domains for one-keystroke attach. Pane moves across
different domains are a WezTerm limitation, not a nardo one.

## Development

```sh
just build     # release binary
just test      # cargo test + lua tests + pytest behaviour suite
just dev       # sandbox WezTerm with hot-swap
```

Behaviour is tested with **pytest** driving the real binary headless against a fake `wezterm`
(`tests/`); rendering is deliberately untested while the design settles.
