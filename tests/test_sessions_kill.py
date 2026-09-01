ALL_PANES = {46, 51, 47, 60, 61}


def test_D_on_a_pane_opens_the_confirm_overlay(launcher):
    r = launcher.run(keys="D settle", mru=[51])
    assert r.overlay["kind"] == "confirm"
    assert r.calls("kill-pane") == []


def test_n_closes_the_confirm_without_killing(launcher):
    r = launcher.run(keys="D n settle", mru=[51])
    assert r.overlay is None
    assert r.calls("kill-pane") == []
    assert 51 in r.pane_ids()
    assert r.exit == "open"


def test_y_kills_the_pane_and_refreshes_the_list(launcher):
    r = launcher.run(keys="D y settle", mru=[51])
    assert r.killed() == [51]
    assert 51 not in r.pane_ids()
    assert (r.exit, r.overlay) == ("open", None)
    subs = [c.sub for c in r.log]
    assert "list" in subs[subs.index("kill-pane") + 1 :]
    assert 51 not in r.ids("pane")


def test_kill_after_a_query_targets_the_matched_pane(launcher):
    r = launcher.run(keys='"nvim" D y settle')
    assert r.killed() == [51]


def test_confirm_kill_false_kills_immediately(launcher):
    r = launcher.run(keys="D settle", mru=[51], options={"confirm_kill": False})
    assert r.overlay is None
    assert r.killed() == [51]


def test_kill_all_asks_first(launcher):
    r = launcher.run(keys="ctrl+shift+d settle")
    assert r.overlay["kind"] == "confirm"
    assert r.calls("kill-pane") == []


def test_kill_all_kills_every_listed_pane_but_never_self(launcher):
    r = launcher.run(keys="ctrl+shift+d y settle")
    assert set(r.killed()) == ALL_PANES
    assert 99 in r.pane_ids()


def test_kill_all_respects_the_current_filter(launcher):
    r = launcher.run(keys='"d:archie" ctrl+shift+d y settle')
    assert set(r.killed()) == {60, 61}


def test_D_on_a_tab_row_kills_all_its_panes(launcher):
    r = launcher.run(keys="home D y settle", options={"scope": "tabs"})
    assert set(r.killed()) == {46, 51}
    assert r.pane_ids() == {47, 99, 60, 61}


def test_kill_failure_keeps_the_launcher_open_and_the_pane_listed(launcher):
    r = launcher.run(keys="D y settle", mru=[51], fail={"kill-pane": "boom"})
    assert r.killed() == [51]
    assert 51 in r.pane_ids()
    assert r.exit == "open"
    assert 51 in r.ids("pane")
