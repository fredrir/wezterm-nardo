# Configuration

```lua
local nardo = wezterm.plugin.require "https://github.com/fredrir/wezterm-nardo"
nardo.apply_to_config(config, {
  poll_ms = 500,
})
```

Unknown keys are merged in as-is. Invalid enum values and out-of-range numbers fall
back to the default with a warning in the WezTerm log.

| option     | default   | meaning                                             |
| ---------- | --------- | --------------------------------------------------- |
| `debug`    | `false`   | verbose logging                                     |
| `poll_ms`  | `500`     | status-update cadence; also caps `status_update_interval` |
| `position` | `"left"`  | example enum — `"left"` or `"right"`                |
| `width`    | `28`      | example bounded number — minimum 8                  |
| `keys`     | `{}`      | key bindings                                        |
| `hooks`    | `{}`      | user callbacks                                      |
| `backend`  | see below | how the backend process is located                  |

Replace `position` and `width` with your own options; they exist to show the enum
and range validation in `config.lua`.

## backend

| option    | default              | meaning                                              |
| --------- | -------------------- | ---------------------------------------------------- |
| `path`    | `nil`                | explicit binary; skips the bootstrap entirely        |
| `repo`    | from `id.lua`        | `owner/name` to download releases from               |
| `version` | `version.lua`        | release tag to fetch, without the `v`                |
| `build`   | `true`               | allow the cargo fallback when no release matches     |
| `uservar` | `ns`                 | user var the backend emits events on                 |
| `log`     | `nil`                | absolute path for the backend's debug log            |

`path` may be a string (this machine only), a table keyed by host or domain, or
`fun(domain, host): string|nil` — so one config can serve local panes, unix
domains and ssh domains with different binaries.

```lua
backend = {
  path = {
    ["local"] = "/opt/bin/wez-nardo",
    archie = "/home/me/.local/bin/wez-nardo",
  },
}
```
