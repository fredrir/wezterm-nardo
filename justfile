ns := `. ./plugin.conf && printf '%s' "$ns"`

_default:
    @just --list --unsorted

# Watch, rebuild and hot-swap into a sandbox WezTerm
dev *args:
    @sh scripts/dev.sh {{args}}

deploy *args: # --from-dev (default) / --from-prd / --from-release
    @sh scripts/deploy.sh {{args}}

# What is installed, what is running, whether it all agrees
doctor:
    @sh scripts/doctor.sh

# Everything CI runs
check: test lint

test: test-rust test-lua test-py

test-rust:
    @if [ -f backend/Cargo.toml ]; then cd backend && cargo test --workspace --locked; fi

test-lua:
    cd plugin && lua tests/run.lua

# Behaviour tests: pytest drives the binary headless against a fake `wezterm`
test-py *args: (build "debug")
    uv run pytest {{args}}

lint:
    @if [ -f backend/Cargo.toml ]; then cd backend && cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings; fi
    cd plugin && luacheck init.lua {{ns}} tests && stylua --check init.lua {{ns}} tests
    @if command -v shellcheck >/dev/null 2>&1; then just lint-sh; else echo "warning: shellcheck not installed — CI still runs it (pinned v0.11.0)"; fi

# Exactly what CI runs. CI pins v0.11.0; SC2015 differs across versions.
lint-sh:
    find scripts plugin -name '*.sh' -print0 | xargs -0 shellcheck -x

e2e mode="local":
    sh plugin/tests/e2e.sh {{mode}}

build profile="release":
    @if [ -f backend/Cargo.toml ]; then cd backend && cargo build --locked -p wez-nardo {{ if profile == "release" { "--release" } else { "" } }}; else echo "no backend crate"; fi

# Run the session explorer in this pane against the live mux (needs a context file, see docs/protocol.md)
run app="sessions" *args: (build "debug")
    NARDO_WEZTERM=$(command -v wezterm) backend/target/debug/wez-nardo {{app}} {{args}}

# Stamp this template out into a new plugin
scaffold ns *args:
    @sh scripts/scaffold.sh {{ns}} {{args}}

# Report shared files that differ from starter-template
template-check:
    @sh scripts/template.sh check

# Pull shared files down from starter-template
template-sync:
    @sh scripts/template.sh sync

# Send local shared-file edits back up to starter-template
template-push:
    @sh scripts/template.sh push

template-diff:
    @sh scripts/template.sh diff
