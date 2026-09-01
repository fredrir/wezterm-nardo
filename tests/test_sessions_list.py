import pytest

ALL_PANES = {46, 51, 47, 60, 61}


def test_every_pane_across_domains_is_listed(launcher):
    r = launcher.run(keys="settle")
    assert set(r.ids("pane")) == ALL_PANES


def test_panes_come_from_wezterm_cli_list(launcher):
    r = launcher.run(keys="settle")
    assert any(c.value("--format") == "json" for c in r.calls("list"))


def test_own_pane_is_hidden_by_default(launcher):
    r = launcher.run(keys="settle")
    assert 99 not in r.ids("pane")


def test_show_self_lists_own_pane(launcher):
    r = launcher.run(keys="settle", options={"show_self": True})
    assert 99 in r.ids("pane")


def test_scope_all_with_empty_query_has_window_and_tab_headers(launcher):
    r = launcher.run(keys="settle")
    assert (r.snapshot["scope"], r.snapshot["query"]) == ("all", "")
    assert set(r.ids("window")) == {10, 12}
    assert {30, 31, 40, 41} <= set(r.ids("tab"))
    assert not any(row["selected"] for row in r.rows("window") + r.rows("tab"))


def test_pane_rows_carry_window_tab_and_domain(launcher):
    r = launcher.run(keys="settle")
    row = next(row for row in r.rows("pane") if row["id"] == 60)
    assert (row["window_id"], row["tab_id"], row["domain"]) == (12, 40, "archie-cable")


def test_detached_domains_are_hidden_in_scope_all(launcher):
    r = launcher.run(keys="settle")
    assert not [row for row in r.rows("domain") if row["state"] == "Detached"]


def test_detached_domains_appear_in_scope_domains_via_tab(launcher):
    r = launcher.run(keys="tab tab tab tab settle")
    assert r.snapshot["scope"] == "domains"
    detached = {row["domain"] for row in r.rows("domain") if row["state"] == "Detached"}
    assert detached == {"archie-wifi", "archie-tailscale"}


def test_scope_option_starts_in_that_scope(launcher):
    r = launcher.run(keys="settle", options={"scope": "domains"})
    assert r.snapshot["scope"] == "domains"
    assert {"archie-wifi", "archie-tailscale"} <= {row["domain"] for row in r.rows("domain")}


@pytest.mark.parametrize(
    "presses,scope",
    [(1, "windows"), (2, "tabs"), (3, "panes"), (4, "domains"), (5, "all")],
)
def test_tab_cycles_scopes(launcher, presses, scope):
    r = launcher.run(keys=" ".join(["tab"] * presses) + " settle")
    assert r.snapshot["scope"] == scope


def test_backtab_cycles_scopes_backwards(launcher):
    r = launcher.run(keys="backtab settle")
    assert r.snapshot["scope"] == "domains"


def test_scope_panes_is_flat(launcher):
    r = launcher.run(keys="settle", options={"scope": "panes"})
    assert {row["kind"] for row in r.rows()} == {"pane"}
    assert set(r.ids("pane")) == ALL_PANES


def test_initial_selection_avoids_origin_pane_without_mru(launcher):
    r = launcher.run(keys="settle")
    assert r.selected["kind"] == "pane"
    assert r.selected["id"] in ALL_PANES - {46}


def test_initial_selection_is_the_mru_pane(launcher):
    r = launcher.run(keys="settle", mru=[51])
    assert r.selected == {"kind": "pane", "id": 51}


def test_selected_flag_marks_exactly_the_selected_row(launcher):
    r = launcher.run(keys="settle", mru=[51])
    assert [row["id"] for row in r.rows() if row["selected"]] == [51]


def test_stale_mru_entries_are_ignored(launcher):
    r = launcher.run(keys="settle", mru=[12345, 47])
    assert r.selected == {"kind": "pane", "id": 47}
