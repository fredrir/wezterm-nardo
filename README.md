# wezterm-nardo

Template for a WezTerm plugin: a Lua plugin, an optional Rust backend process, and
the dev rig (sandbox WezTerm, hot-swap, deploy modes, CI, release) that goes with them.

## New plugin

```sh
just scaffold nardo                 # -> ../nardo, Lua + Rust backend
just scaffold nardo --no-backend    # pure Lua
just scaffold nardo --repo me/wezterm-nardo --dest ~/src/nardo
```

Scaffolding copies the tree, rewrites the seed code to the new namespace, writes
`plugin.conf`, and commits. Shared files are copied verbatim so the new plugin
starts in sync with the template.

## Layout

```
plugin.conf              identity: ns, name, repo — every script derives the rest
.template-files          which files the template owns
justfile                 task entry points
scripts/                 dev, deploy, doctor, scaffold, template sync
plugin/
  init.lua               root resolution, guarded events, reload watch list
  nardo/               id, config, util, platform, backend, version
  tests/                 lua runner, wezterm stub, fake mux, e2e
backend/                 Rust terminal-bridge backend (optional)
```

## Shared vs seed

`.template-files` lists the files starter-template owns. They contain **no
plugin-specific strings** — `plugin.conf` supplies identity at runtime — so they
are byte-identical in every plugin and sync is a plain copy.

```sh
just template-check   # which shared files drifted
just template-diff    # how they drifted
just template-sync    # pull the template's versions down
just template-push    # send local fixes back up to the template
```

Everything else is seed code: copied once at scaffold, yours afterwards. CI runs
`template-check`, so drift shows up on the next push rather than the next time you
go looking.

Fix a script once in `starter-template`, then `just template-sync` in each plugin.

## Development

See [docs/development.md](docs/development.md). Configuration options are in
[docs/configuration.md](docs/configuration.md), and the backend wire format is in
[docs/protocol.md](docs/protocol.md).
