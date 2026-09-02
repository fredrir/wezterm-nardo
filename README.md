# wez-nardo



## Install

```lua
local wezterm = require "wezterm"
local nardo = wezterm.plugin.require "https://github.com/fredrir/wezterm-nardo"

local config = wezterm.config_builder()
nardo.apply_to_config(config, {})
return config
```

## Keys
| launcher            | default key | what                                                        |
| ------------------- | ----------- | ----------------------------------------------------------- |
| Session explorer    | `⌘K`        | fuzzy-search domains › windows › tabs › panes, live preview, switch/kill/move/rename/create, attach TLS domains |
| Command palette     | `⌘⇧P`       | your commands + built-in WezTerm actions                    |
| Quick terminal      | `⌥⌘T`       | dropdown terminal window                                    |

## Options

| doc                                          | what                                  |
| -------------------------------------------- | ------------------------------------- |
| [docs/keys.md](docs/keys.md)                 | every binding, mouse, query syntax    |
| [docs/configuration.md](docs/configuration.md) | all options                         |
| [docs/architecture.md](docs/architecture.md) | crates, modules, data flow            |
| [docs/protocol.md](docs/protocol.md)         | context json, actions, headless driver |
| [docs/development.md](docs/development.md)   | dev rig, tests, release               |

## Development

```sh
just build
just test
just dev
```
