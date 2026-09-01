def preview_calls(result, pane_id=None):
    calls = [c for c in result.calls("get-text") if c.has("--start-line")]
    return calls if pane_id is None else [c for c in calls if c.value("--pane-id") == str(pane_id)]


def backdrop_calls(result):
    return [c for c in result.calls("get-text") if not c.has("--start-line")]


def test_selected_pane_preview_is_fetched_once_with_scrollback(launcher):
    r = launcher.run(keys="settle", mru=[51])
    calls = preview_calls(r, 51)
    assert len(calls) == 1
    assert calls[0].has("--escapes") and calls[0].value("--start-line") == "-200"


def test_snapshot_reports_the_loaded_preview(launcher):
    r = launcher.run(keys="settle", mru=[51])
    assert r.snapshot["preview"]["pane_id"] == 51
    assert r.snapshot["preview"]["loaded"] is True


def test_preview_lines_option_sets_the_scrollback_depth(launcher):
    r = launcher.run(keys="settle", mru=[51], options={"preview_lines": 50})
    assert [c.value("--start-line") for c in preview_calls(r, 51)] == ["-50"]


def test_returning_to_a_pane_reuses_the_cached_preview(launcher):
    r = launcher.run(keys="down up settle", mru=[51])
    assert r.selected == {"kind": "pane", "id": 51}
    assert len(preview_calls(r, 51)) == 1


def test_moving_selection_previews_the_new_pane(launcher):
    r = launcher.run(keys="down settle", mru=[51])
    assert r.selected["kind"] == "pane"
    assert len(preview_calls(r, r.selected["id"])) == 1
    assert r.snapshot["preview"]["pane_id"] == r.selected["id"]


def test_preview_option_off_fetches_no_previews(launcher):
    r = launcher.run(keys="settle", mru=[51], options={"preview": False})
    assert preview_calls(r) == []
    assert r.snapshot["preview"] is None


def test_ctrl_space_hides_the_preview_panel(launcher):
    r = launcher.run(keys="ctrl+space settle", mru=[51])
    assert r.snapshot["preview"] is None


def test_backdrop_fetches_the_origin_pane_in_overlay_mode(launcher):
    r = launcher.run(keys="settle", mru=[51])
    calls = backdrop_calls(r)
    assert [c.value("--pane-id") for c in calls] == ["46"]
    assert calls[0].has("--escapes")


def test_no_backdrop_in_tab_mode(launcher):
    ctx = launcher.context()
    ctx["presentation"]["mode"] = "tab"
    r = launcher.run(keys="settle", mru=[51], context=ctx)
    assert backdrop_calls(r) == []


def test_no_backdrop_when_backdrop_is_none(launcher):
    ctx = launcher.context()
    ctx["presentation"]["backdrop"] = "none"
    r = launcher.run(keys="settle", mru=[51], context=ctx)
    assert backdrop_calls(r) == []
