COMMANDS = [
    {"id": "p0", "label": "Reload configuration", "hint": "wezterm"},
    {"id": "p1", "label": "Toggle fullscreen", "hint": ""},
    {"id": "p2", "label": "Show debug overlay", "hint": "wezterm"},
]


def run_palette(launcher, keys, **kwargs):
    return launcher.run(app="palette", keys=keys, options={"commands": COMMANDS}, **kwargs)


def test_lists_all_commands_with_empty_query(launcher):
    r = run_palette(launcher, "settle")
    assert [row["label"] for row in r.snapshot["rows"]] == [c["label"] for c in COMMANDS]
    assert r.selected == {"id": "p0", "label": "Reload configuration"}


def test_query_narrows_and_enter_emits_run_action(launcher):
    r = run_palette(launcher, '"debug" enter')
    assert r.exit == "handed_off"
    assert r.actions[-1]["t"] == "run"
    assert r.actions[-1]["name"] == "command"
    assert r.actions[-1]["args"] == {"id": "p2"}


def test_esc_cancels_without_actions(launcher):
    r = run_palette(launcher, "esc")
    assert r.exit == "cancelled"
    assert r.actions == []


def test_navigation_wraps_and_selects(launcher):
    r = run_palette(launcher, "down down settle")
    assert r.selected["id"] == "p2"
    r = run_palette(launcher, "up settle")
    assert r.selected["id"] == "p2"


def test_no_match_makes_enter_a_noop(launcher):
    r = run_palette(launcher, '"zzzz" enter settle')
    assert r.exit == "open"
    assert r.snapshot["rows"] == []
    assert r.actions == []


def test_options_without_commands_is_empty_not_a_crash(launcher):
    r = launcher.run(app="palette", keys="settle")
    assert r.exit == "open"
    assert r.snapshot["rows"] == []
