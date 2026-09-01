#!/bin/sh
# Keeps the files listed in .template-files identical to starter-template's copies.
#
#   WEZPLUG_TEMPLATE   path to a starter-template checkout, or a git URL.
#                      Defaults to a ../starter-template sibling, else clones upstream.
set -eu
# `sync` re-execs a copy of this script from /tmp, where $0 no longer sits next
# to lib.sh; WEZPLUG_ROOT carries the real location across that hop.
if [ -n "${WEZPLUG_ROOT:-}" ]; then
  lib="$WEZPLUG_ROOT/scripts/lib.sh"
else
  lib="$(dirname "$0")/lib.sh"
fi
# shellcheck source=scripts/lib.sh
. "$lib"

# `template_repo` in plugin.conf overrides this; scaffold writes it for new plugins.
if [ -n "$template_repo" ]; then
  upstream="https://github.com/$template_repo"
else
  upstream=https://github.com/fredrir/wezterm-starter-template
fi

tmp=
trap 'if [ -n "$tmp" ]; then rm -rf "$tmp"; fi' EXIT

# Sets $template; returns 1 and leaves $wanted set when it cannot be reached.
# Assigns globals rather than echoing, so $tmp reaches the cleanup trap.
template=
wanted=
resolve_template() {
  wanted="${WEZPLUG_TEMPLATE:-}"
  if [ -z "$wanted" ] && [ -f "$root/../starter-template/.template-files" ]; then
    wanted=$(cd "$root/../starter-template" && pwd)
  fi
  [ -n "$wanted" ] || wanted=$upstream
  case "$wanted" in
    http*://* | git@*)
      tmp=$(mktemp -d /tmp/wezplug-tpl.XXXXXX)
      git clone --quiet --depth 1 "$wanted" "$tmp" 2>/dev/null || return 1
      template=$tmp
      ;;
    *)
      [ -d "$wanted" ] || return 1
      template=$(cd "$wanted" && pwd)
      ;;
  esac
}

files() { grep -v '^[[:space:]]*#' "$manifest" | grep -v '^[[:space:]]*$'; }

sum() { [ -f "$1" ] || { printf 'missing'; return 0; }; shasum -a 256 "$1" 2>/dev/null | cut -d' ' -f1; }

# Wrapped in a function so the whole body is parsed before any of it runs: `sync`
# rewrites this very script, and sh reads scripts incrementally.
main() {
  cmd=${1:-check}
  if [ $# -gt 0 ]; then shift; fi
  quiet=0
  for a in "$@"; do
    [ "$a" = --quiet ] && quiet=1
  done

  manifest="$root/.template-files"
  [ -f "$manifest" ] || die "no .template-files manifest in $root"

  # Belt and braces: even wrapped, a longer replacement leaves unread bytes at the
  # old offset. Running `sync` from a copy makes that impossible.
  if [ "$cmd" = sync ] && [ "${WEZPLUG_SELFCOPY:-}" != 1 ]; then
    self=$(mktemp /tmp/wezplug-self.XXXXXX)
    cat "$0" >"$self"
    status=0
    WEZPLUG_SELFCOPY=1 WEZPLUG_ROOT="$root" sh "$self" "$cmd" "$@" || status=$?
    rm -f "$self"
    exit $status
  fi

  # The template's own repo has nothing upstream of it. Decided before resolving,
  # so the template's own CI never tries to clone itself.
  if [ "$url" = "$upstream" ]; then
    if [ "$quiet" = 0 ]; then ok "this is the template itself; nothing to sync"; fi
    exit 0
  fi

  if ! resolve_template; then
    # Drift detection is advisory: an unreachable template must not fail a build.
    # An explicit sync/push/diff asked for the template, so that is a hard error.
    case "$cmd" in
      check)
        # Quiet callers (doctor) need to tell "unreachable" from "in sync"; CI runs
        # non-quiet and must stay green when the template is simply not published.
        if [ "$quiet" = 1 ]; then exit 2; fi
        warn "template not reachable ($wanted); skipping drift check"
        exit 0
        ;;
      *) die "template not reachable: $wanted" ;;
    esac
  fi

  if [ "$template" = "$root" ]; then
    if [ "$quiet" = 0 ]; then ok "this is the template itself; nothing to sync"; fi
    exit 0
  fi

  drift=0
  changed=0
  for f in $(files); do
    # Backend-owned shared files do not apply to pure-Lua plugins.
    if ! has_backend; then
      case "$f" in
        backend/* | plugin/bin/bootstrap.*) continue ;;
      esac
    fi
    a="$template/$f"
    b="$root/$f"
    # A file the template dropped, or one this plugin scaffolded without.
    [ -f "$a" ] || continue
    if [ ! -f "$b" ]; then
      drift=$((drift + 1))
      case "$cmd" in
        check | diff) if [ "$quiet" = 0 ]; then warn "MISSING  $f"; fi ;;
        sync) mkdir -p "$(dirname "$b")"; cp "$a" "$b"; say "  added    $f" ;;
        push) ;;
      esac
      continue
    fi
    [ "$(sum "$a")" = "$(sum "$b")" ] && continue
    changed=$((changed + 1))
    drift=$((drift + 1))
    case "$cmd" in
      check) if [ "$quiet" = 0 ]; then warn "DRIFT    $f"; fi ;;
      diff) say "${dim}--- template/$f${off}"; diff -u "$a" "$b" || true ;;
      sync) cp "$a" "$b"; say "  updated  $f" ;;
      push) cp "$b" "$a"; say "  pushed   $f" ;;
    esac
  done

  total=$(files | wc -l | tr -d ' ')
  case "$cmd" in
    check)
      if [ "$quiet" = 1 ]; then
        [ "$drift" -eq 0 ]
        exit $?
      fi
      if [ "$drift" -eq 0 ]; then
        ok "in sync with $dim$template$off ($total files)"
      else
        say ""
        warn "$drift of $total shared file(s) differ — just template-sync (pull) / just template-push (send up)"
        exit 1
      fi
      ;;
    sync)
      if [ "$drift" -eq 0 ]; then ok "already in sync ($total files)"; else ok "synced $drift file(s) from $dim$template$off"; fi
      ;;
    push)
      if [ "$changed" -eq 0 ]; then ok "nothing to push"; else ok "pushed $changed file(s) to $dim$template$off — commit them there"; fi
      ;;
    diff)
      if [ "$drift" -eq 0 ]; then ok "no differences ($total files)"; fi
      ;;
    *) die "usage: template.sh [check|sync|push|diff] [--quiet]" ;;
  esac
}

main "$@"; exit $?
