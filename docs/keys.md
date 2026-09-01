# Keys

## Open

| action    | macOS       | other            | option           |
| --------- | ----------- | ---------------- | ---------------- |
| sessions  | `CMD+k`     | `CTRL+SHIFT+k`   | `sessions.key`   |
| palette   | `CMD+SHIFT+p` | `CTRL+SHIFT+ALT+p` | `palette.key` |
| quick     | `ALT+CMD+t` | `CTRL+ALT+t`     | `quick.key`      |

## Sessions

Override with `sessions.keys = { <action> = "<key>" | { "<key>", ... } | false }`.

| action        | keys                        | effect                                              |
| ------------- | --------------------------- | --------------------------------------------------- |
| `switch`      | `enter`                     | activate selected pane / tab / window; attach domain |
| `close`       | `esc`, `CMD+k` (forwarded)  | close launcher                                      |
| `down`        | `down`, `ctrl+n`, `ctrl+j`  |                                                     |
| `up`          | `up`, `ctrl+p`, `ctrl+k`    |                                                     |
| `page_down`   | `pagedown`, `ctrl+d`        |                                                     |
| `page_up`     | `pageup`, `ctrl+u`          |                                                     |
| `first`       | `home`                      |                                                     |
| `last`        | `end`                       |                                                     |
| `scope_next`  | `tab`                       | all → windows → tabs → panes → domains              |
| `scope_prev`  | `backtab`                   |                                                     |
| `kill`        | `D`                         | kill selected pane / tab / window (confirm)         |
| `kill_all`    | `ctrl+shift+d`, `CMD+SHIFT+d` (forwarded) | kill every listed pane (confirm)      |
| `new_tab`     | `ctrl+t`                    | new tab in selected window / domain                 |
| `new_window`  | `ctrl+w`                    | new window in selected domain                       |
| `split`       | `ctrl+s`                    | split selected pane (bottom)                        |
| `rename`      | `ctrl+r`, `f2`              | rename tab / window / workspace                     |
| `move`        | `ctrl+m`                    | move pane → tab / new tab / window; tab → window    |
| `zoom`        | `ctrl+z`                    | toggle zoom on selected pane                        |
| `preview`     | `ctrl+space`                | toggle preview panel                                |
| `preview_up`  | `alt+up`                    | scroll preview                                      |
| `preview_down`| `alt+down`                  | scroll preview                                      |
| `clear`       | `ctrl+l`                    | clear query                                         |
| `help`        | `?` (empty query), `f1`     | key overlay                                         |

`ctrl+d` pages when the query is empty and deletes forward otherwise.

## Mouse

| gesture                    | effect                                  |
| -------------------------- | --------------------------------------- |
| hover row                  | select + preview                        |
| click row                  | select                                  |
| click selected / double    | switch                                  |
| scroll on list             | move selection                          |
| scroll on preview          | scroll preview                          |
| click chip                 | set scope                               |
| click outside modal        | close                                   |

## Query syntax

| token       | filter                    |
| ----------- | ------------------------- |
| `d:archie`  | domain name contains      |
| `w:main`    | window title contains     |
| `ws:dev`    | workspace                 |
| `#12`       | pane id                   |
| other       | fuzzy over `domain workspace window tab title process cwd` |
