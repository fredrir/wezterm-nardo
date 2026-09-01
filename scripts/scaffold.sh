#!/bin/sh
# Stamps this template out into a new plugin.
#
#   sh scripts/scaffold.sh <ns> [--repo owner/name] [--dest path] [--no-backend]
#
# Shared files (.template-files) are copied byte-for-byte and never rewritten, so
# the new plugin starts in sync. Everything else is seed code: renamed and yours.
set -eu
# shellcheck source=scripts/lib.sh
. "$(dirname "$0")/lib.sh"

from_ns=$ns
from_prefix=$prefix
from_name=$name

to_ns=
to_repo=
dest=
backend=1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-backend) backend=0 ;;
    --repo) to_repo=$2; shift ;;
    --dest) dest=$2; shift ;;
    -h | --help) say "usage: just scaffold <ns> [--repo owner/name] [--dest path] [--no-backend]"; exit 0 ;;
    -*) die "unknown flag: $1" ;;
    *) if [ -n "$to_ns" ]; then die "unexpected argument: $1"; fi; to_ns=$1 ;;
  esac
  shift
done

[ -n "$to_ns" ] || die "usage: just scaffold <ns> [--repo owner/name] [--dest path] [--no-backend]"
printf '%s' "$to_ns" | grep -Eq '^[a-z][a-z0-9-]*$' || die "ns must match [a-z][a-z0-9-]*"
if [ "$to_ns" = "$from_ns" ]; then die "pick an ns other than the template's own ($from_ns)"; fi

to_prefix=$(printf '%s' "$to_ns" | tr '[:lower:]-' '[:upper:]_')
to_name="wez-$to_ns"
owner=${repo%%/*}
[ -n "$to_repo" ] || to_repo="$owner/wezterm-$to_ns"
[ -n "$dest" ] || dest="$root/../$to_ns"

if [ -e "$dest" ] && [ -n "$(ls -A "$dest" 2>/dev/null)" ]; then die "destination is not empty: $dest"; fi
mkdir -p "$dest"
dest=$(cd "$dest" && pwd)

say "${dim}scaffolding $to_ns into $dest${off}"

# Build artefacts and vendored types are never part of a fresh plugin.
tar -cf - -C "$root" \
  --exclude .git --exclude target --exclude lua_modules --exclude luarocks \
  --exclude '*.src.rock' --exclude .dev-build.log . | tar -xf - -C "$dest"

cat > "$dest/plugin.conf" <<EOF
# Identity for this plugin. Every shared script derives the rest from these.
#   prefix  uppercased ns, used for the bootstrap env contract (${to_prefix}_BIN, ...)
#   url     https://github.com/\$repo
#   name    defaults to wez-\$ns
ns=$to_ns
name=$to_name
repo=$to_repo

# Where just template-sync pulls shared files from, when there is no
# ../starter-template sibling and WEZPLUG_TEMPLATE is unset.
template_repo=$repo
EOF

mv "$dest/plugin/$from_ns" "$dest/plugin/$to_ns"

shared=$(grep -v '^[[:space:]]*#' "$root/.template-files" | grep -v '^[[:space:]]*$')

# Seed files carry the template's identity in their source; shared files must not be touched.
find "$dest" -type f | while IFS= read -r f; do
  rel=${f#"$dest"/}
  case "$rel" in
    .git/* | plugin.conf) continue ;;
  esac
  if printf '%s\n' "$shared" | grep -Fxq "$rel"; then continue; fi
  case "$(basename "$f")" in
    *.lua | *.rs | *.toml | *.lock | *.md | *.sh | *.ps1 | *.yml | *.json | justfile | .luacheckrc) ;;
    *) continue ;;
  esac
  # perl, not sed: BSD sed has no \b and would silently leave the file untouched.
  FROM_NS=$from_ns TO_NS=$to_ns FROM_NAME=$from_name TO_NAME=$to_name \
    FROM_PREFIX=$from_prefix TO_PREFIX=$to_prefix FROM_REPO=$repo TO_REPO=$to_repo \
    perl -pi -e '
      s/\Q$ENV{FROM_REPO}\E/$ENV{TO_REPO}/g;
      s/\Qwezterm-$ENV{FROM_NS}-template\E/wezterm-$ENV{TO_NS}/g;
      s/\Q$ENV{FROM_NAME}\E/$ENV{TO_NAME}/g;
      s/\Q$ENV{FROM_PREFIX}\E/$ENV{TO_PREFIX}/g;
      s/(?<![A-Za-z0-9])\Q$ENV{FROM_NS}\E(?![A-Za-z0-9-])/$ENV{TO_NS}/g;
    ' "$f"
done

if [ "$backend" = 0 ]; then
  rm -rf "$dest/backend" "$dest/plugin/bin" "$dest/docs/protocol.md"
  # Drop the links that pointed at the protocol doc we just deleted.
  perl -ni -e 'print unless /protocol\.md/' "$dest/README.md" "$dest/docs/development.md"
  say "  ${dim}removed backend/, plugin/bin/, docs/protocol.md${off}"
fi

git -C "$dest" init --quiet
git -C "$dest" add -A
git -C "$dest" commit --quiet -m "scaffold $to_ns from starter-template" || true

ok "created $dim$dest$off"
say "  ns      $to_ns   ${dim}(env prefix ${to_prefix}_)${off}"
say "  repo    https://github.com/$to_repo"
if [ "$backend" = 1 ]; then say "  backend $to_name"; fi
say ""
say "  ${dim}cd $dest && just check${off}"
