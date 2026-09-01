import json
import subprocess

from conftest import FAKE_WEZTERM

REAL_LIST_FIELDS = {
    "window_id",
    "tab_id",
    "pane_id",
    "workspace",
    "size",
    "title",
    "cwd",
    "cursor_x",
    "cursor_y",
    "cursor_shape",
    "cursor_visibility",
    "left_col",
    "top_row",
    "tab_title",
    "window_title",
    "is_active",
    "is_zoomed",
    "tty_name",
}


def pane(pane_id, window=1, tab=1, **fields):
    return {"window_id": window, "tab_id": tab, "pane_id": pane_id, **fields}


def two_panes():
    return {
        "panes": [pane(1, title="zsh", is_active=True), pane(2, title="nvim", workspace="dev")],
        "text": {"1": "$ ls\nfoo\n"},
        "next_pane_id": 100,
        "fail": {},
    }


def listed(fake):
    return json.loads(fake.call("list", "--format", "json").stdout)


def test_list_prints_every_pane_with_the_real_cli_shape(fake):
    fake.seed(two_panes())
    panes = listed(fake)
    assert [p["pane_id"] for p in panes] == [1, 2]
    assert set(panes[0]) == REAL_LIST_FIELDS
    assert panes[0]["size"] == {"rows": 40, "cols": 120, "pixel_width": 1920, "pixel_height": 1080, "dpi": 96}
    assert panes[0]["title"] == "zsh" and panes[0]["is_active"] is True
    assert panes[1]["workspace"] == "dev" and panes[1]["is_active"] is False
    assert panes[1]["cursor_shape"] == "Default" and panes[1]["tty_name"] is None


def test_list_without_state_file_is_empty(fake):
    assert listed(fake) == []


def test_get_text_returns_pane_text_or_nothing(fake):
    fake.seed(two_panes())
    assert fake.call("get-text", "--pane-id", "1", "--escapes", "--start-line", "-200").stdout == "$ ls\nfoo\n"
    assert fake.call("get-text", "--pane-id", "2", "--escapes").stdout == ""


def test_activate_pane_flips_active_within_the_tab(fake):
    fake.seed(two_panes())
    fake.call("activate-pane", "--pane-id", "2")
    assert [p["is_active"] for p in fake.state()["panes"]] == [False, True]


def test_activate_tab_accepts_known_tab_and_rejects_unknown(fake):
    fake.seed(two_panes())
    assert fake.call("activate-tab", "--tab-id", "1").returncode == 0
    assert fake.call("activate-tab", "--tab-id", "9", check=False).returncode == 1


def test_kill_pane_removes_it_from_state_and_list(fake):
    fake.seed(two_panes())
    fake.call("kill-pane", "--pane-id", "1")
    assert [p["pane_id"] for p in fake.state()["panes"]] == [2]
    assert [p["pane_id"] for p in listed(fake)] == [2]


def test_kill_unknown_pane_fails_without_mutation(fake):
    fake.seed(two_panes())
    proc = fake.call("kill-pane", "--pane-id", "7", check=False)
    assert proc.returncode == 1 and "not found" in proc.stderr
    assert len(fake.state()["panes"]) == 2


def test_spawn_prints_next_pane_id_and_creates_a_tab_in_the_window(fake):
    fake.seed(two_panes())
    assert fake.call("spawn", "--domain-name", "localmux", "--window-id", "1").stdout.strip() == "100"
    assert fake.call("spawn", "--window-id", "1", "--cwd", "/tmp", "--", "htop").stdout.strip() == "101"
    state = fake.state()
    new = {p["pane_id"]: p for p in state["panes"]}
    assert new[100]["window_id"] == 1 and new[100]["tab_id"] == 2 and new[100]["domain_name"] == "localmux"
    assert new[101]["tab_id"] == 3 and new[101]["cwd"] == "/tmp" and new[101]["title"] == "htop"
    assert state["next_pane_id"] == 102


def test_spawn_new_window_allocates_window_and_workspace(fake):
    fake.seed(two_panes())
    fake.call("spawn", "--new-window", "--workspace", "scratch")
    new = fake.state()["panes"][-1]
    assert new["window_id"] == 2 and new["tab_id"] == 2 and new["workspace"] == "scratch"


def test_spawn_without_target_uses_own_pane_window(fake):
    fake.seed({"panes": [pane(1, window=1), pane(9, window=5, tab=7)], "next_pane_id": 20})
    fake.call("spawn", env={"WEZTERM_PANE": "9"})
    assert fake.state()["panes"][-1]["window_id"] == 5


def test_split_pane_adds_a_pane_to_the_same_tab(fake):
    fake.seed(two_panes())
    assert fake.call("split-pane", "--pane-id", "1", "--bottom").stdout.strip() == "100"
    new = fake.state()["panes"][-1]
    assert (new["window_id"], new["tab_id"], new["is_active"]) == (1, 1, True)


def test_split_pane_move_joins_target_tab(fake):
    fake.seed({"panes": [pane(1, window=1, tab=1), pane(2, window=2, tab=2, workspace="dev")], "next_pane_id": 5})
    assert fake.call("split-pane", "--pane-id", "1", "--move-pane-id", "2", "--right").stdout.strip() == "2"
    moved = fake.state()["panes"][1]
    assert (moved["window_id"], moved["tab_id"], moved["workspace"]) == (1, 1, "default")
    assert fake.state()["next_pane_id"] == 5


def test_move_pane_to_new_tab_in_same_or_given_window(fake):
    fake.seed({"panes": [pane(1, window=1, tab=1), pane(2, window=1, tab=1), pane(3, window=4, tab=8)]})
    fake.call("move-pane-to-new-tab", "--pane-id", "2")
    assert fake.state()["panes"][1]["tab_id"] == 9 and fake.state()["panes"][1]["window_id"] == 1
    fake.call("move-pane-to-new-tab", "--pane-id", "1", "--window-id", "4")
    assert (fake.state()["panes"][0]["window_id"], fake.state()["panes"][0]["tab_id"]) == (4, 10)


def test_move_pane_to_new_window(fake):
    fake.seed({"panes": [pane(1, window=1, tab=1), pane(2, window=1, tab=1)]})
    fake.call("move-pane-to-new-tab", "--pane-id", "2", "--new-window", "--workspace", "w2")
    moved = fake.state()["panes"][1]
    assert (moved["window_id"], moved["tab_id"], moved["workspace"]) == (2, 2, "w2")


def test_titles_and_workspace_renames(fake):
    fake.seed(two_panes())
    fake.call("set-tab-title", "--tab-id", "1", "work")
    fake.call("set-window-title", "--window-id", "1", "main")
    fake.call("rename-workspace", "--workspace", "dev", "play")
    panes = fake.state()["panes"]
    assert {p["tab_title"] for p in panes} == {"work"}
    assert {p["window_title"] for p in panes} == {"main"}
    assert [p["workspace"] for p in panes] == ["default", "play"]


def test_zoom_pane_toggle_zoom_unzoom(fake):
    fake.seed(two_panes())
    fake.call("zoom-pane", "--pane-id", "1", "--toggle")
    assert fake.state()["panes"][0]["is_zoomed"] is True
    fake.call("zoom-pane", "--pane-id", "1", "--unzoom")
    assert fake.state()["panes"][0]["is_zoomed"] is False
    fake.call("zoom-pane", "--pane-id", "1", "--zoom")
    assert fake.state()["panes"][0]["is_zoomed"] is True


def test_fail_map_exits_one_with_message_and_no_mutation(fake):
    state = two_panes()
    state["fail"] = {"kill-pane": "pane not found"}
    fake.seed(state)
    proc = fake.call("kill-pane", "--pane-id", "1", check=False)
    assert proc.returncode == 1 and proc.stderr.strip() == "pane not found"
    assert len(fake.state()["panes"]) == 2
    assert fake.log()[-1].args == ["kill-pane", "--pane-id", "1"]


def test_unknown_subcommand_exits_two(fake):
    proc = fake.call("teleport", check=False)
    assert proc.returncode == 2


def test_every_call_appends_one_log_line(fake):
    fake.seed(two_panes())
    fake.call("list", "--format", "json")
    fake.call("--class", "sandbox", "get-text", "--pane-id", "1")
    entries = [json.loads(line) for line in fake.log_path.read_text().splitlines()]
    assert entries[0] == {"args": ["list", "--format", "json"]}
    assert entries[1] == {"args": ["get-text", "--pane-id", "1"], "class": "sandbox"}


def test_runs_without_cli_prefix(fake):
    fake.seed(two_panes())
    proc = subprocess.run(
        [str(FAKE_WEZTERM), "list", "--format", "json"],
        env={**fake.env(), "PATH": "/usr/bin:/bin"},
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert proc.returncode == 0
    assert len(json.loads(proc.stdout)) == 2
