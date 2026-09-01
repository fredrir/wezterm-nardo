# Development

## Layout

| path                      | what                                                     |
| ------------------------- | -------------------------------------------------------- |
| `backend/`                | Cargo workspace → `target/release/wez-nardo`             |
| `backend/crates/core`     | `nardo-core` launcher base                               |
| `backend/crates/sessions` | `nardo-sessions`                                         |
| `backend/crates/palette`  | `nardo-palette`                                          |
| `backend/crates/cli`      | `wez-nardo` binary                                       |
| `plugin/`                 | Lua plugin (`init.lua`, `nardo/*.lua`, `tests/`)         |
| `tests/`                  | pytest behaviour tests + fake `wezterm`                  |
| `scripts/`                | dev rig: sandbox WezTerm, deploy, doctor                 |

## Commands

```sh
just                      # recipes
just build                # release binary
just build debug
just test                 # rust + lua + pytest
just test-py -k kill      # pytest subset (builds debug first)
just lint                 # fmt, clippy -D warnings, luacheck, stylua, shellcheck
just run sessions --headless --keys 'j enter' --dump   # drive the binary by hand
just dev                  # sandbox WezTerm, rebuild + hot-swap on change
just dev --live           # hot-swap in your running WezTerm
just deploy               # build release, hot-swap running backends
just doctor
```

## Behaviour tests

| piece                       | value                                                        |
| --------------------------- | ------------------------------------------------------------ |
| runner                      | `uv run pytest` (`pyproject.toml`, dev group)                |
| binary                      | `NARDO_BIN` `(fallback: backend/target/debug/wez-nardo)`     |
| fake wezterm                | `tests/fake_wezterm.py`, selected through `NARDO_WEZTERM`    |
| fake state                  | `FAKE_WEZTERM_STATE` json: panes list + get-text per pane    |
| fake call log               | `FAKE_WEZTERM_LOG` one json line per `wezterm cli` call      |
| driver                      | `wez-nardo <app> --headless --keys ... --dump`               |

Tests assert on the outcome json and the call log — never on layout or rendering.

## Requirements

| tool        | version           |
| ----------- | ----------------- |
| rust        | ≥ 1.88 (edition 2024) |
| python      | ≥ 3.12, `uv`      |
| lua         | 5.4 / 5.5, `luacheck`, `stylua` |
| shellcheck  | v0.11.0 (CI pin)  |
| just, watchexec | dev loop only |

## Local config

```lua
package.path = "/path/to/nardo/plugin/?.lua;" .. package.path
local nardo = dofile "/path/to/nardo/plugin/init.lua"
nardo.apply_to_config(config, {
  backend = { path = "/path/to/nardo/backend/target/release/wez-nardo" },
  debug = true,
})
```

## Identity

`plugin.conf` is the source of truth for the shell side; `plugin/nardo/id.lua` mirrors it. CI asserts they agree.

| field    | value                       | derived                          |
| -------- | --------------------------- | -------------------------------- |
| `ns`     | `nardo`                     | Lua module dir, user-var name    |
| `name`   | `wez-nardo`                 | binary, release asset names      |
| `repo`   | `fredrir/wezterm-nardo`     | release URL, plugin-dir mangling |
| *prefix* | `NARDO`                     | bootstrap env vars               |

## Bootstrap environment

`sh bootstrap.sh <name> <PREFIX>` reads:

| env               | value                                  |
| ----------------- | -------------------------------------- |
| `NARDO_BIN`       | explicit binary; skips every fallback  |
| `NARDO_TARGET`    | rust triple                            |
| `NARDO_VERSION`   | release tag without `v`                |
| `NARDO_REPO`      | `owner/name` for release downloads     |
| `NARDO_SRC`       | workspace for the cargo fallback       |
| `NARDO_BUILD`     | `0` disables the cargo fallback        |

## Template

`.template-files` lists files owned by `wez-starter-template`; `just template-check` reports drift.
`justfile`, CI and `backend/` are plugin-specific here and no longer tracked by the template.
