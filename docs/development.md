# Development

## Backend

```sh
cd backend
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release      # target/release/wez-nardo
```

## Plugin

```sh
cd plugin
lua tests/run.lua
luacheck init.lua nardo tests
stylua --check init.lua nardo tests
```

## Shell

```sh
just lint-sh      # what CI runs
```

CI pins **shellcheck v0.11.0**. SC2015 (`A && B || C`) fires on materially more
code in 0.10.0 and earlier, so an unpinned runner can fail a tree that is clean
on your machine. If you bump the pin in `.github/workflows/ci.yml`, bump it here.

## End-to-end

```sh
sh plugin/tests/e2e.sh
```

## Dev loop

Needs [`just`](https://github.com/casey/just) and [`watchexec`](https://github.com/watchexec/watchexec).

```sh
just              # list recipes
just dev          # sandbox WezTerm, rebuild + hot-swap on change
just dev --live   # hot-swap in your running WezTerm instead
just doctor       # what is installed, what is running, whether it agrees
```

## Applying a build

```sh
just deploy                  # build release, hot-swap running backends
just deploy --from-prd       # install into WezTerm's plugin dir as a real plugin
just deploy --from-release   # download the published assets and install those
```

## Local development config

```lua
package.path = "/path/to/wezterm-nardo/plugin/?.lua;" .. package.path
local nardo = dofile "/path/to/wezterm-nardo/plugin/init.lua"
nardo.apply_to_config(config, {
  backend = { path = "/path/to/wezterm-nardo/backend/target/release/wez-nardo" },
  debug = true,
})
```

## Staying in sync with the template

`.template-files` lists the files starter-template owns. They hold no
plugin-specific strings, so they are identical in every plugin.

```sh
just template-check   # report drift
just template-diff    # show it
just template-sync    # pull the template's versions
just template-push    # send local fixes upstream
```

`WEZPLUG_TEMPLATE` picks the template to compare against: a path, or a git URL.
It defaults to a `../starter-template` sibling, else clones upstream.

## Identity

`plugin.conf` is the single source of truth for the shell side; `plugin/nardo/id.lua`
mirrors it for the Lua side. CI asserts they agree.

| field    | example                            | derived                              |
| -------- | ---------------------------------- | ------------------------------------ |
| `ns`     | `nardo`                          | Lua module dir, user-var name        |
| `name`   | `wez-nardo`                      | backend binary, release asset names  |
| `repo`   | `fredrir/wezterm-nardo` | release URL, plugin-dir mangling     |
| *prefix* | `NARDO`                          | uppercased `ns`; bootstrap env vars  |

## Bootstrap environment

The bootstrap is invoked as `sh bootstrap.sh <name> <PREFIX>` and reads these,
where `PREFIX` is the uppercased `ns`:

| Variable          | Description                          |
| ----------------- | ------------------------------------ |
| `<PREFIX>_BIN`    | Explicit binary; skips every fallback |
| `<PREFIX>_TARGET` | Rust triple                          |
| `<PREFIX>_VERSION`| Release tag without `v`              |
| `<PREFIX>_REPO`   | `owner/name` for release downloads   |
| `<PREFIX>_SRC`    | Backend crate for the cargo fallback |
| `<PREFIX>_BUILD`  | `0` disables the cargo fallback      |

The backend binary itself reads only neutral names, so it never needs to know its
own prefix:

| Variable          | Description                                             |
| ----------------- | ------------------------------------------------------- |
| `WEZPLUG_USERVAR` | user var name for events (default `wezplug`)            |
| `WEZPLUG_LOG`     | append debug lines here (0600, symlinks refused)        |
