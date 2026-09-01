#!/bin/sh
# Drives a throwaway WezTerm with this plugin loaded and asserts it comes up clean.
# Extend with `wezterm cli` assertions as the plugin grows.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
# shellcheck source=scripts/lib.sh
WEZPLUG_ROOT="$root" . "$root/scripts/lib.sh"

bin=""
if has_backend; then
  bin="$(bin_for release)"
  [ -x "$bin" ] || die "backend binary not found: $bin (just build)"
fi
command -v wezterm >/dev/null 2>&1 || die "wezterm not on PATH"

log=$(mktemp -t "$ns-e2e")
home=$(mktemp -d "/tmp/$ns-e2e.XXXXXX")
mkdir -p "$home/.local/share/wezterm"

cleanup() {
  set +e
  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null
  if [ -n "${E2E_LOG:-}" ]; then cp "$log" "$E2E_LOG"; fi
  rm -rf "$home" "$log"
}

HOME="$home" WEZPLUG_ROOT="$root" WEZPLUG_BIN="$bin" WEZTERM_LOG=info \
  wezterm --config-file "$root/scripts/dev-config.lua" \
  start --always-new-process --class "$ns-e2e" >"$log" 2>&1 &
pid=$!
trap cleanup EXIT

export WEZTERM_UNIX_SOCKET="$home/.local/share/wezterm/gui-sock-$pid"
ready=0
for _ in $(seq 1 50); do
  if [ -S "$WEZTERM_UNIX_SOCKET" ] && wezterm cli --no-auto-start list >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.2
done
[ "$ready" = 1 ] || { sed -n '1,40p' "$log"; die "wezterm did not come up"; }
ok "wezterm up"

sleep 2
panes=$(wezterm cli --no-auto-start list --format json | grep -c pane_id || true)
[ "$panes" -gt 0 ] || die "no panes listed"
ok "$panes pane(s) listed"

if grep -qE "$ns:.*(error|failed)|ERROR" "$log"; then
  grep -nE "$ns:|ERROR" "$log" | tail -20
  die "plugin reported errors"
fi
ok "no plugin errors in the wezterm log"
say "all e2e checks passed"
