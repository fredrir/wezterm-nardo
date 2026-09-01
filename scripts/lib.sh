# Shared paths and helpers for the dev scripts. Sourced, never executed.
# shellcheck shell=sh disable=SC2034  # consumed by the scripts that source this
root="${WEZPLUG_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"

[ -f "$root/plugin.conf" ] || { printf 'plugin.conf not found in %s\n' "$root" >&2; exit 1; }
# shellcheck source=/dev/null
. "$root/plugin.conf"
: "${ns:?plugin.conf must set ns}"
: "${repo:?plugin.conf must set repo}"

name="${name:-wez-$ns}"
template_repo="${template_repo:-}"
prefix=$(printf '%s' "$ns" | tr '[:lower:]-' '[:upper:]_')
url="https://github.com/$repo"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
plugins="$data/wezterm/plugins"
cache="$data/$name/bin"
has_backend() { [ -f "$root/backend/Cargo.toml" ]; }

# WezTerm mangles the repo URL into the plugin dir name: / -> sZs, : -> sCs, . -> sDs.
component=$(printf '%s' "$url" | sed -e 's|/|sZs|g' -e 's|:|sCs|g' -e 's|\.|sDs|g')
checkout="$plugins/$component"

version=$(sed -n 's/^return "\(.*\)"$/\1/p' "$root/plugin/$ns/version.lua" 2>/dev/null || true)
[ -n "$version" ] || version=0.0.0

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) triple=aarch64-apple-darwin ;;
  Darwin-x86_64) triple=x86_64-apple-darwin ;;
  Linux-x86_64) triple=x86_64-unknown-linux-gnu ;;
  Linux-aarch64 | Linux-arm64) triple=aarch64-unknown-linux-gnu ;;
  *) triple=unknown ;;
esac

if [ -t 1 ]; then
  dim=$(printf '\033[2m'); red=$(printf '\033[31m'); grn=$(printf '\033[32m')
  ylw=$(printf '\033[33m'); off=$(printf '\033[0m')
else
  dim=; red=; grn=; ylw=; off=
fi

say() { printf '%s\n' "$*"; }
ok() { printf '%s%s%s\n' "$grn" "$*" "$off"; }
warn() { printf '%s%s%s\n' "$ylw" "$*" "$off"; }
die() { printf '%s%s%s\n' "$red" "$*" "$off" >&2; exit 1; }

build() {
  has_backend || { printf '%sno backend crate; nothing to build%s\n' "$dim" "$off"; return 0; }
  profile=$1
  start=$(date +%s)
  status=0
  if [ "$profile" = release ]; then
    cargo build --release --locked --manifest-path "$root/backend/Cargo.toml" \
      >/dev/null 2>"$root/.dev-build.log" || status=$?
  else
    cargo build --locked --manifest-path "$root/backend/Cargo.toml" \
      >/dev/null 2>"$root/.dev-build.log" || status=$?
  fi
  elapsed=$(( $(date +%s) - start ))
  if [ $status -ne 0 ]; then
    printf '%sbuild failed%s (%ss)\n' "$red" "$off" "$elapsed"
    tail -20 "$root/.dev-build.log"
    return 1
  fi
  printf '%sbuild ok%s %s(%ss, %s)%s\n' "$grn" "$off" "$dim" "$elapsed" "$profile" "$off"
}

bin_for() {
  [ "$1" = release ] && printf '%s' "$root/backend/target/release/$name" \
    || printf '%s' "$root/backend/target/debug/$name"
}

# Backend panes respawn on the next poll, so killing the process is a hot-swap.
restart_backends() {
  has_backend || return 0
  n=$(pgrep -xf "$1" 2>/dev/null | wc -l | tr -d ' ')
  if [ "$n" -gt 0 ]; then
    pkill -xf "$1" 2>/dev/null || true
    printf '%s  swapped %s backend(s)%s\n' "$dim" "$n" "$off"
  else
    printf '%s  no running backends%s\n' "$dim" "$off"
  fi
}
