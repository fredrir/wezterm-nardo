def test_ctrl_t_spawns_a_tab_in_the_selected_panes_window_and_domain(launcher):
    r = launcher.run(keys="ctrl+t settle", mru=[51])
    spawn = r.single("spawn")
    assert spawn.value("--window-id") == "10"
    assert spawn.value("--domain-name") == "localmux"
    assert not spawn.has("--new-window")
    assert r.pane(100)["window_id"] == 10


def test_ctrl_t_on_a_detached_domain_never_spawns(launcher):
    r = launcher.run(keys='"wifi" ctrl+t settle', options={"scope": "domains"})
    assert r.calls("spawn") == []
    if r.exit == "handed_off":
        assert (r.actions[-1]["t"], r.actions[-1]["domain"]) == ("attach_domain", "archie-wifi")
    else:
        assert r.exit == "open"


def test_ctrl_t_on_an_attached_domain_row_spawns_in_that_domain(launcher):
    r = launcher.run(keys='"cable" ctrl+t settle', options={"scope": "domains"})
    spawn = r.single("spawn")
    assert spawn.value("--domain-name") == "archie-cable"


def test_ctrl_w_spawns_a_new_window_in_the_selected_domain(launcher):
    r = launcher.run(keys="ctrl+w settle", mru=[51])
    spawn = r.single("spawn")
    assert spawn.has("--new-window") and not spawn.has("--window-id")
    assert spawn.value("--domain-name") == "localmux"
    assert r.pane(100)["window_id"] not in {10, 12}


def test_ctrl_s_splits_the_selected_pane_at_the_bottom(launcher):
    r = launcher.run(keys="ctrl+s settle", mru=[51])
    split = r.single("split-pane")
    assert split.value("--pane-id") == "51" and split.has("--bottom")
    assert not split.has("--move-pane-id")
    assert r.pane(100)["tab_id"] == 30


def test_ctrl_r_opens_the_rename_overlay(launcher):
    r = launcher.run(keys="ctrl+r settle", mru=[51])
    assert r.overlay["kind"] == "rename"


def test_f2_also_opens_the_rename_overlay(launcher):
    r = launcher.run(keys="f2 settle", mru=[51])
    assert r.overlay["kind"] == "rename"


def test_rename_sets_the_tab_title(launcher):
    r = launcher.run(keys='ctrl+r "work" enter settle', mru=[51])
    assert r.single("set-tab-title") == ["set-tab-title", "--tab-id", "30", "work"]
    assert r.pane(51)["tab_title"] == "work"
    assert r.overlay is None


def test_esc_closes_rename_without_renaming(launcher):
    r = launcher.run(keys='ctrl+r "work" esc settle', mru=[51])
    assert r.overlay is None and r.exit == "open"
    assert r.calls("set-tab-title") == []


def test_rename_on_a_window_row_sets_the_window_title(launcher):
    r = launcher.run(keys='home ctrl+r "main" enter settle', options={"scope": "windows"})
    assert r.single("set-window-title") == ["set-window-title", "--window-id", "10", "main"]


def test_ctrl_m_opens_the_move_overlay(launcher):
    r = launcher.run(keys="ctrl+m settle", mru=[51])
    assert r.overlay["kind"] == "move"


def test_move_first_choice_moves_the_pane_to_a_new_tab_in_its_window(launcher):
    r = launcher.run(keys="ctrl+m enter settle", mru=[51])
    move = r.single("move-pane-to-new-tab")
    assert move.value("--pane-id") == "51" and move.value("--window-id") == "10"
    assert r.pane(51)["window_id"] == 10
    assert r.pane(51)["tab_id"] not in {30, 31, 35}


def test_ctrl_z_toggles_zoom_on_the_selected_pane(launcher):
    r = launcher.run(keys="ctrl+z settle", mru=[51])
    zoom = r.single("zoom-pane")
    assert zoom.value("--pane-id") == "51" and zoom.has("--toggle")
    assert r.pane(51)["is_zoomed"] is True


def test_management_actions_keep_the_launcher_open(launcher):
    r = launcher.run(keys="ctrl+z ctrl+s settle", mru=[51])
    assert r.exit == "open"
    assert r.calls("zoom-pane") and r.calls("split-pane")
