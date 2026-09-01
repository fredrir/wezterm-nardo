#!/bin/sh
# Locates a plugin backend: explicit path, cached download, verified GitHub release, or cargo build.
#
#   sh bootstrap.sh <binary-name> <ENV_PREFIX>
#
# Reads <PREFIX>_{BIN,TARGET,VERSION,REPO,SRC,BUILD}. WEZPLUG_* are passed through
# to the binary untouched, so this script never needs to know what they mean.
set -u

name=${1:-}
prefix=${2:-}
if [ -z "$name" ] || [ -z "$prefix" ]; then
  printf 'usage: bootstrap.sh <name> <PREFIX>\n'
  exit 1
fi
printf '%s' "$prefix" | grep -Eq '^[A-Z][A-Z0-9_]*$' || { printf 'invalid prefix\n'; exit 1; }

# Indirect lookup of <PREFIX>_<suffix>; the prefix is validated above.
env_of() { eval "printf '%s' \"\${${prefix}_$1:-}\""; }

data="${XDG_DATA_HOME:-$HOME/.local/share}/$name"
target=$(env_of TARGET)
version=$(env_of VERSION)
[ -n "$version" ] || version=dev
repo=$(env_of REPO)
src=$(env_of SRC)
PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export PATH

explicit=$(env_of BIN)
if [ -n "$explicit" ] && [ -x "$explicit" ]; then
  exec "$explicit"
fi

if [ -z "$target" ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) target=aarch64-apple-darwin ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
    Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
    *) target=unknown ;;
  esac
fi

safe() { printf '%s' "$1" | grep -Eq '^[A-Za-z0-9._-]+$'; }
if ! safe "$target" || ! safe "$version"; then
  printf 'invalid %s_TARGET or %s_VERSION\n' "$prefix" "$prefix"
  exit 1
fi

bin="$data/bin/$name-$target-$version"
[ -x "$bin" ] && exec "$bin"
mkdir -p "$data/bin"

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

download() {
  base="https://github.com/$repo/releases/download/v$version"
  tmp=$(mktemp "$data/bin/.$name.XXXXXX") || return 1
  sums="$tmp.sums"
  printf 'downloading %s\n' "$base/$name-$target"
  if curl -fsSL -o "$tmp" "$base/$name-$target" && curl -fsSL -o "$sums" "$base/SHA256SUMS"; then
    expected=$(grep " $name-$target\$" "$sums" | cut -d' ' -f1)
    actual=$(sha256 "$tmp")
    if [ -n "$expected" ] && [ "$expected" = "$actual" ]; then
      chmod +x "$tmp" && mv "$tmp" "$bin" && rm -f "$sums" && return 0
    fi
    printf 'checksum mismatch\n'
  fi
  rm -f "$tmp" "$sums"
  return 1
}

if [ "$version" != dev ] && [ -n "$repo" ] && command -v curl >/dev/null 2>&1; then
  download && exec "$bin"
  printf 'download failed\n'
fi

if [ "$(env_of BUILD)" != 0 ] && [ -f "$src/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  printf 'building backend\n'
  if cargo build --release --manifest-path "$src/Cargo.toml" --target-dir "$data/target"; then
    cp "$data/target/release/$name" "$bin" && exec "$bin"
  fi
  printf 'build failed\n'
fi

printf 'backend not found: install cargo, publish a release, or set backend.path\n'
exit 1
