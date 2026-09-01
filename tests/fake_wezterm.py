#!/usr/bin/env python3
"""Stand-in for `wezterm cli`: state in FAKE_WEZTERM_STATE, one json line per call in FAKE_WEZTERM_LOG."""

import json
import os
import sys

VALUE_FLAGS = {
    "--class",
    "--pane-id",
    "--tab-id",
    "--window-id",
    "--workspace",
    "--domain-name",
    "--cwd",
    "--move-pane-id",
    "--start-line",
    "--end-line",
    "--format",
    "--percent",
    "--cells",
    "--tab-index",
}

SIZE_DEFAULTS = {"rows": 40, "cols": 120, "pixel_width": 1920, "pixel_height": 1080, "dpi": 96}

PANE_DEFAULTS = {
    "workspace": "default",
    "title": "",
    "cwd": "",
    "cursor_x": 0,
    "cursor_y": 0,
    "cursor_shape": "Default",
    "cursor_visibility": "Visible",
    "left_col": 0,
    "top_row": 0,
    "tab_title": "",
    "window_title": "",
    "is_active": False,
    "is_zoomed": False,
    "tty_name": None,
}

EMPTY_STATE = {"panes": [], "text": {}, "next_pane_id": 100, "fail": {}}

MUTABLE_FIELDS = ("workspace", "title", "cwd", "tab_title", "window_title", "is_active", "is_zoomed")


class CliError(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


def load_state():
    path = os.environ.get("FAKE_WEZTERM_STATE")
    if not path or not os.path.exists(path):
        return json.loads(json.dumps(EMPTY_STATE))
    with open(path, encoding="utf-8") as fh:
        raw = fh.read()
    state = json.loads(raw) if raw.strip() else {}
    for key, default in EMPTY_STATE.items():
        state.setdefault(key, json.loads(json.dumps(default)))
    for pane in state["panes"]:
        for field in MUTABLE_FIELDS:
            pane.setdefault(field, PANE_DEFAULTS[field])
    return state


def save_state(state):
    path = os.environ.get("FAKE_WEZTERM_STATE")
    if not path:
        return
    tmp = f"{path}.{os.getpid()}.tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(state, fh, indent=1)
    os.replace(tmp, path)


def log_call(args, cls):
    path = os.environ.get("FAKE_WEZTERM_LOG")
    if not path:
        return
    entry = {"args": args}
    if cls is not None:
        entry["class"] = cls
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry) + "\n")


def parse(argv):
    opts, positional = {}, []
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg == "--":
            positional.extend(argv[i + 1 :])
            break
        if arg.startswith("--"):
            if "=" in arg:
                key, value = arg.split("=", 1)
                opts[key] = value
            elif arg in VALUE_FLAGS:
                if i + 1 >= len(argv):
                    raise CliError(f"{arg} needs a value", 2)
                opts[arg] = argv[i + 1]
                i += 1
            else:
                opts[arg] = True
        else:
            positional.append(arg)
        i += 1
    return opts, positional


def require_int(opts, flag):
    if flag not in opts:
        raise CliError(f"missing {flag}", 2)
    try:
        return int(opts[flag])
    except ValueError:
        raise CliError(f"{flag}: expected an integer, got {opts[flag]!r}", 2) from None


def find_pane(state, pane_id):
    for pane in state["panes"]:
        if pane["pane_id"] == pane_id:
            return pane
    raise CliError(f"pane {pane_id} not found")


def next_id(state, key):
    return max((p[key] for p in state["panes"]), default=0) + 1


def take_pane_id(state):
    pane_id = state.get("next_pane_id") or next_id(state, "pane_id")
    state["next_pane_id"] = pane_id + 1
    return pane_id


def record(pane):
    out = dict(PANE_DEFAULTS)
    out.update(pane)
    out["size"] = {**SIZE_DEFAULTS, **pane.get("size", {})}
    return out


def program_title(positional):
    return os.path.basename(positional[0]) if positional else "zsh"


def add_pane(state, window_id, tab_id, workspace, opts, positional, extra=None):
    pane = {
        "window_id": window_id,
        "tab_id": tab_id,
        "pane_id": take_pane_id(state),
        "workspace": workspace,
        "size": dict(SIZE_DEFAULTS),
        "title": program_title(positional),
        "cwd": opts.get("--cwd", ""),
        "tab_title": "",
        "window_title": "",
        "is_active": True,
        "is_zoomed": False,
    }
    if extra:
        pane.update(extra)
    for other in state["panes"]:
        if other["tab_id"] == tab_id:
            other["is_active"] = False
    state["panes"].append(pane)
    return pane


def cmd_list(state, opts, positional):
    print(json.dumps([record(p) for p in state["panes"]]))


def cmd_get_text(state, opts, positional):
    pane_id = require_int(opts, "--pane-id")
    sys.stdout.write(state["text"].get(str(pane_id), ""))


def cmd_activate_pane(state, opts, positional):
    pane = find_pane(state, require_int(opts, "--pane-id"))
    for other in state["panes"]:
        if other["tab_id"] == pane["tab_id"]:
            other["is_active"] = other is pane
    save_state(state)


def cmd_activate_tab(state, opts, positional):
    tab_id = require_int(opts, "--tab-id")
    if not any(p["tab_id"] == tab_id for p in state["panes"]):
        raise CliError(f"tab {tab_id} not found")


def cmd_kill_pane(state, opts, positional):
    pane = find_pane(state, require_int(opts, "--pane-id"))
    state["panes"].remove(pane)
    save_state(state)


def cmd_move_pane_to_new_tab(state, opts, positional):
    pane = find_pane(state, require_int(opts, "--pane-id"))
    pane["tab_id"] = next_id(state, "tab_id")
    if opts.get("--new-window"):
        pane["window_id"] = next_id(state, "window_id")
        pane["workspace"] = opts.get("--workspace", pane["workspace"])
        pane["window_title"] = ""
    elif "--window-id" in opts:
        pane["window_id"] = require_int(opts, "--window-id")
        pane["workspace"] = window_workspace(state, pane["window_id"], pane["workspace"])
    pane["tab_title"] = ""
    save_state(state)


def cmd_split_pane(state, opts, positional):
    target = find_pane(state, require_int(opts, "--pane-id"))
    if "--move-pane-id" in opts:
        moved = find_pane(state, require_int(opts, "--move-pane-id"))
        for key in ("window_id", "tab_id", "workspace", "window_title", "tab_title"):
            moved[key] = target[key]
        save_state(state)
        print(moved["pane_id"])
        return
    opts.setdefault("--cwd", target.get("cwd", ""))
    pane = add_pane(state, target["window_id"], target["tab_id"], target["workspace"], opts, positional)
    pane["window_title"] = target.get("window_title", "")
    pane["tab_title"] = target.get("tab_title", "")
    save_state(state)
    print(pane["pane_id"])


def window_workspace(state, window_id, fallback):
    for pane in state["panes"]:
        if pane["window_id"] == window_id:
            return pane.get("workspace", fallback)
    return fallback


def window_title(state, window_id):
    for pane in state["panes"]:
        if pane["window_id"] == window_id:
            return pane.get("window_title", "")
    return ""


def own_window(state):
    own = os.environ.get("WEZTERM_PANE")
    if own and own.isdigit():
        for pane in state["panes"]:
            if pane["pane_id"] == int(own):
                return pane["window_id"]
    return state["panes"][0]["window_id"] if state["panes"] else None


def cmd_spawn(state, opts, positional):
    if opts.get("--new-window"):
        window_id = next_id(state, "window_id")
        workspace = opts.get("--workspace", "default")
    else:
        window_id = require_int(opts, "--window-id") if "--window-id" in opts else own_window(state)
        if window_id is None:
            window_id = next_id(state, "window_id")
        workspace = window_workspace(state, window_id, "default")
    extra = {"window_title": window_title(state, window_id)}
    if "--domain-name" in opts:
        extra["domain_name"] = opts["--domain-name"]
    pane = add_pane(state, window_id, next_id(state, "tab_id"), workspace, opts, positional, extra)
    save_state(state)
    print(pane["pane_id"])


def retitle(state, key, wanted, field, title):
    hits = [p for p in state["panes"] if p[key] == wanted]
    if not hits:
        raise CliError(f"{key} {wanted} not found")
    for pane in hits:
        pane[field] = title
    save_state(state)


def cmd_set_tab_title(state, opts, positional):
    if not positional:
        raise CliError("missing TITLE", 2)
    retitle(state, "tab_id", require_int(opts, "--tab-id"), "tab_title", " ".join(positional))


def cmd_set_window_title(state, opts, positional):
    if not positional:
        raise CliError("missing TITLE", 2)
    retitle(state, "window_id", require_int(opts, "--window-id"), "window_title", " ".join(positional))


def cmd_rename_workspace(state, opts, positional):
    if not positional:
        raise CliError("missing NEW_WORKSPACE", 2)
    if "--workspace" not in opts:
        raise CliError("missing --workspace", 2)
    retitle(state, "workspace", opts["--workspace"], "workspace", positional[0])


def cmd_zoom_pane(state, opts, positional):
    pane = find_pane(state, require_int(opts, "--pane-id"))
    if opts.get("--zoom"):
        pane["is_zoomed"] = True
    elif opts.get("--unzoom"):
        pane["is_zoomed"] = False
    else:
        pane["is_zoomed"] = not pane.get("is_zoomed", False)
    save_state(state)


HANDLERS = {
    "list": cmd_list,
    "get-text": cmd_get_text,
    "activate-pane": cmd_activate_pane,
    "activate-tab": cmd_activate_tab,
    "kill-pane": cmd_kill_pane,
    "move-pane-to-new-tab": cmd_move_pane_to_new_tab,
    "split-pane": cmd_split_pane,
    "spawn": cmd_spawn,
    "set-tab-title": cmd_set_tab_title,
    "set-window-title": cmd_set_window_title,
    "rename-workspace": cmd_rename_workspace,
    "zoom-pane": cmd_zoom_pane,
}


def main(argv):
    argv = list(argv)
    if argv and argv[0] == "cli":
        argv = argv[1:]
    cls = None
    while argv and argv[0].startswith("--"):
        if argv[0] == "--class" and len(argv) > 1:
            cls = argv[1]
            argv = argv[2:]
        else:
            argv = argv[1:]
    if not argv:
        print("usage: wezterm cli [--class X] <subcommand> [args]", file=sys.stderr)
        return 2
    sub, rest = argv[0], argv[1:]
    log_call([sub, *rest], cls)
    handler = HANDLERS.get(sub)
    if handler is None:
        print(f"unknown subcommand {sub!r}", file=sys.stderr)
        return 2
    state = load_state()
    message = state["fail"].get(sub)
    if message:
        print(message, file=sys.stderr)
        return 1
    try:
        opts, positional = parse(rest)
        handler(state, opts, positional)
    except CliError as err:
        print(f"{sub}: {err}", file=sys.stderr)
        return err.code
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
