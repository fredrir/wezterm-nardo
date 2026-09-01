#!/bin/sh
# Reports which backend is actually running and whether the installs agree.
set -eu
# shellcheck source=scripts/lib.sh
. "$(dirname "$0")/lib.sh"

row() { printf '  %-22s %s\n' "$1" "$2"; }
stamp() {
  if [ -e "$1" ]; then
    printf '%s  %s' "$(wc -c <"$1" | tr -d ' ')B" "$(date -r "$1" '+%Y-%m-%d %H:%M')"
  else
    printf '%s' "-"
  fi
}

dirty() { if git -C "$1" diff --quiet 2>/dev/null; then echo clean; else echo dirty; fi; }

say "${dim}source${off}"
row "root" "$root"
row "plugin" "$ns ($name)"
row "branch" "$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '-') $(dirty "$root")"
row "version.lua" "$version"
if has_backend; then
  row "Cargo.toml" "$(sed -n 's/^version *= *"\(.*\)"$/\1/p' "$root/backend/Cargo.toml" | head -1)"
fi
row "latest tag" "$(git -C "$root" describe --tags --abbrev=0 2>/dev/null || echo none)"

if has_backend; then
  say ""
  say "${dim}builds${off}"
  row "target/debug" "$(stamp "$root/backend/target/debug/$name")"
  row "target/release" "$(stamp "$root/backend/target/release/$name")"
fi

say ""
say "${dim}installs${off}"
if [ -d "$checkout" ]; then
  row "plugin dir" "$checkout"
  row "  version.lua" "$(sed -n 's/^return "\(.*\)"$/\1/p' "$checkout/plugin/$ns/version.lua" 2>/dev/null || echo '-')"
else
  row "plugin dir" "not installed ${dim}(just deploy --from-prd)${off}"
fi
if [ -d "$cache" ] && [ -n "$(ls -A "$cache" 2>/dev/null)" ]; then
  for f in "$cache"/*; do row "  cached" "$(basename "$f")  $(stamp "$f")"; done
elif has_backend; then
  row "bootstrap cache" "empty"
fi

say ""
say "${dim}template${off}"
if [ -f "$root/.template-files" ]; then
  status=0
  sh "$root/scripts/template.sh" check --quiet || status=$?
  case $status in
    0) row "shared files" "in sync" ;;
    2) row "shared files" "${dim}template not reachable${off}" ;;
    *) row "shared files" "${ylw}drift — just template-check${off}" ;;
  esac
else
  row "shared files" "no .template-files manifest"
fi

say ""
say "${dim}your wezterm config${off}"
cfg="${WEZTERM_CONFIG_DIR:-$HOME/.config/wezterm}"
# Resolve symlinks: BSD grep -r will not descend into a symlinked directory.
cfg=$(cd "$cfg" 2>/dev/null && pwd -P) || cfg=""
if [ -n "$cfg" ] && grep -rl "$ns" "$cfg" >/dev/null 2>&1; then
  grep -rn --exclude-dir=lua_modules --exclude-dir=.luarocks --exclude-dir=types \
    -e "$repo" -e "$ns.apply_to_config" "$cfg" 2>/dev/null |
    sed "s|$cfg|<config>|g; s|$HOME|~|g" | head -8 |
    while IFS= read -r l; do printf '  %s\n' "$l"; done
else
  row "wiring" "no $ns reference found in $cfg"
fi

if has_backend; then
  say ""
  say "${dim}running backends${off}"
  found=0
  for pid in $(pgrep -f "$name" 2>/dev/null || true); do
    cmd=$(ps -p "$pid" -o args= 2>/dev/null || true)
    case "$cmd" in
      *watchexec* | *doctor.sh* | *dev.sh*) continue ;;
    esac
    [ -n "$cmd" ] || continue
    found=$((found + 1))
    row "pid $pid" "$(printf '%s' "$cmd" | sed "s|$HOME|~|g")"
  done
  [ "$found" -eq 0 ] && row "" "none running"
fi
exit 0
