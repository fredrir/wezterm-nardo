def test_enter_activates_the_matched_pane(launcher):
    r = launcher.run(keys='"nvim" enter')
    assert (r.exit, r.outcome["pane_id"]) == ("activated", 51)
    assert r.single("activate-pane").value("--pane-id") == "51"
    assert r.pane(51)["is_active"] is True


def test_switch_puts_the_pane_first_in_mru(launcher):
    r = launcher.run(keys='"nvim" enter')
    assert r.mru()[:1] == [51]


def test_switch_moves_pane_to_the_front_of_an_existing_mru(launcher):
    r = launcher.run(keys='"nvim" enter', mru=[47, 51])
    assert r.mru()[:2] == [51, 47]


def test_enter_with_empty_query_switches_to_the_selected_pane(launcher):
    r = launcher.run(keys="enter", mru=[47])
    assert (r.exit, r.outcome["pane_id"]) == ("activated", 47)


def test_down_then_enter_switches_to_a_different_pane(launcher):
    r = launcher.run(keys="down enter", mru=[51])
    assert r.exit == "activated"
    assert r.outcome["pane_id"] in {46, 47, 60, 61}


def test_esc_cancels_without_activating(launcher):
    r = launcher.run(keys="esc")
    assert r.exit == "cancelled"
    assert r.calls("activate-pane") == [] and r.calls("activate-tab") == []


def test_cancel_leaves_mru_untouched(launcher):
    r = launcher.run(keys="esc", mru=[47])
    assert r.mru() == [47]


def test_enter_on_a_tab_row_activates_the_tab(launcher):
    r = launcher.run(keys="home enter", options={"scope": "tabs"})
    assert r.exit == "activated"
    assert r.single("activate-tab").value("--tab-id") == "30"


def test_enter_on_a_detached_domain_hands_off_an_attach(launcher):
    r = launcher.run(keys='"wifi" enter', options={"scope": "domains"})
    assert r.exit == "handed_off"
    last = r.actions[-1]
    assert (last["t"], last["domain"]) == ("attach_domain", "archie-wifi")
    assert r.calls("activate-pane") == []


def test_mouse_scroll_down_moves_selection_like_the_down_key(launcher):
    keyboard = launcher.run(keys="down settle", mru=[51])
    mouse = launcher.run(keys="mouse:scroll:down settle", mru=[51])
    assert keyboard.selected != {"kind": "pane", "id": 51}
    assert mouse.selected == keyboard.selected


def test_mouse_scroll_up_returns_to_previous_row(launcher):
    r = launcher.run(keys="mouse:scroll:down mouse:scroll:up settle", mru=[51])
    assert r.selected == {"kind": "pane", "id": 51}
