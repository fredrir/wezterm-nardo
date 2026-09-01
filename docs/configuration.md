# Configuration

```lua
local nardo = wezterm.plugin.require "https://github.com/fredrir/wezterm-nardo"
nardo.apply_to_config(config, {
  sessions = { key = { key = "k", mods = "SUPER" } },
})
```

Unknown keys merge as-is. Invalid enums / out-of-range numbers fall back to the default with a warning in the WezTerm log.

## Shared

| option                      | default          | values                                              |
| --------------------------- | ---------------- | --------------------------------------------------- |
| `debug`                     | `false`          |                                                     |
| `presentation.mode`         | `"overlay"`      | `overlay` `tab` `window` `split`                    |
| `presentation.width`        | `0.72`           | ≤ 1 fraction of area, > 1 cells                     |
| `presentation.height`       | `0.7`            | ≤ 1 fraction of area, > 1 cells                     |
| `presentation.max_width`    | `128`            | cells                                               |
| `presentation.max_height`   | `42`             | cells                                               |
| `presentation.backdrop`     | `"dim"`          | `dim` `none`                                        |
| `presentation.animations`   | `true`           |                                                     |
| `presentation.split`        | `{ direction = "Bottom", size = 0.5 }` | `window` / `split` mode only   |
| `theme`                     | `"auto"`         | `auto` (WezTerm palette) or `{ accent = "#hex", ... }` |
| `backend`                   | see below        |                                                     |
| `hooks`                     | `{}`             | `{ [action_t] = fun(window, pane, action) }`, runs after the built-in handler |

## `sessions`

| option                      | default                        | values                                  |
| --------------------------- | ------------------------------ | --------------------------------------- |
| `sessions.enabled`          | `true`                         |                                         |
| `sessions.key`              | `{ key = "k", mods = SUPER }`  | `false` disables the binding            |
| `sessions.confirm_kill`     | `true`                         |                                         |
| `sessions.preview`          | `true`                         |                                         |
| `sessions.preview_lines`    | `200`                          | scrollback lines fetched                |
| `sessions.mru`              | `true`                         | most-recent panes first on empty query  |
| `sessions.scope`            | `"all"`                        | `all` `windows` `tabs` `panes` `domains`|
| `sessions.show_self`        | `false`                        | list the launcher's own pane            |
| `sessions.keys`             | see `docs/keys.md`             | per-action overrides, `false` disables  |

## `palette`

| option                      | default                        | values                                  |
| --------------------------- | ------------------------------ | --------------------------------------- |
| `palette.enabled`           | `true`                         |                                         |
| `palette.key`               | `{ key = "p", mods = SUPER2 }` |                                         |
| `palette.commands`          | `{}`                           | `{ { label, action = wezterm.action.X or fun(window, pane) } }` |

## `quick`

| option                      | default                        | values                                  |
| --------------------------- | ------------------------------ | --------------------------------------- |
| `quick.enabled`             | `true`                         |                                         |
| `quick.key`                 | `{ key = "t", mods = "ALT|CMD" }` macOS, `{ key = "t", mods = "CTRL|ALT" }` else |          |
| `quick.height`              | `0.4`                          | fraction of screen                      |
| `quick.args`                | `nil`                          | program, `nil` = default shell          |

`SUPER` = `CMD` on macOS, `CTRL|SHIFT` elsewhere. `SUPER2` = `CMD|SHIFT` on macOS, `CTRL|SHIFT|ALT` elsewhere.

## `backend`

| option    | default              | meaning                                              |
| --------- | -------------------- | ---------------------------------------------------- |
| `path`    | `nil`                | explicit binary; skips the bootstrap entirely        |
| `repo`    | from `id.lua`        | `owner/name` to download releases from               |
| `version` | `version.lua`        | release tag to fetch, without the `v`                |
| `build`   | `true`               | allow the cargo fallback when no release matches     |
| `uservar` | `nardo`              | user var the backend emits actions on                |
| `log`     | `nil`                | absolute path for the backend's debug log            |
| `class`   | `nil`                | GUI `--class`, passed as `NARDO_WEZTERM_CLASS`       |

`path` may be a string (this machine only), a table keyed by host or domain, or
`fun(domain, host): string|nil`. The launcher always spawns on the GUI host (`local` domain).
