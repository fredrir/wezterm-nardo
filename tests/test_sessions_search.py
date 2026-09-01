def test_query_narrows_to_matching_pane_without_headers(launcher):
    r = launcher.run(keys='"nvim" settle')
    assert r.snapshot["query"] == "nvim"
    assert [(row["kind"], row["id"]) for row in r.rows()] == [("pane", 51)]
    assert r.selected == {"kind": "pane", "id": 51}


def test_process_name_matches(launcher):
    r = launcher.run(keys='"claude" settle')
    assert r.ids("pane") == [47]


def test_cwd_matches(launcher):
    r = launcher.run(keys='"projects" settle')
    assert set(r.ids("pane")) == {51, 47}


def test_domain_filter_keeps_only_that_domains_panes(launcher):
    r = launcher.run(keys='"d:archie" settle')
    assert r.rows() and {row["kind"] for row in r.rows()} == {"pane"}
    assert set(r.ids("pane")) == {60, 61}
    assert {row["domain"] for row in r.rows("pane")} == {"archie-cable"}


def test_pane_id_filter_selects_that_pane(launcher):
    r = launcher.run(keys='"#47" settle')
    assert r.ids("pane") == [47]
    assert r.selected == {"kind": "pane", "id": 47}


def test_workspace_filter(launcher):
    r = launcher.run(keys='"ws:dev" settle')
    assert set(r.ids("pane")) == {60, 61}


def test_window_title_filter(launcher):
    r = launcher.run(keys='"w:fredrir" settle')
    assert set(r.ids("pane")) == {46, 51, 47}


def test_filters_combine_with_fuzzy_text(launcher):
    r = launcher.run(keys='"d:archie htop" settle')
    assert r.ids("pane") == [61]


def test_ctrl_l_clears_query_and_restores_headers(launcher):
    r = launcher.run(keys='"nvim" ctrl+l settle')
    assert r.snapshot["query"] == ""
    assert set(r.ids("window")) == {10, 12}


def test_backspace_edits_query(launcher):
    r = launcher.run(keys='"nvimx" backspace settle')
    assert r.snapshot["query"] == "nvim"
    assert r.ids("pane") == [51]


def test_no_match_yields_zero_rows(launcher):
    r = launcher.run(keys='"zzqxjv" settle')
    assert r.rows() == []
    assert not r.selected


def test_enter_with_no_match_does_nothing(launcher):
    r = launcher.run(keys='"zzqxjv" enter settle')
    assert r.exit == "open"
    assert r.calls("activate-pane") == [] and r.calls("activate-tab") == []


def test_query_applies_within_the_current_scope(launcher):
    r = launcher.run(keys='"archie" settle', options={"scope": "domains"})
    assert {row["kind"] for row in r.rows()} == {"domain"}
    assert {row["domain"] for row in r.rows()} == {"archie-cable", "archie-wifi", "archie-tailscale"}
