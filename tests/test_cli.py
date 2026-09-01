import json
import subprocess

EMPTY_MUX = {"panes": [], "text": {}, "next_pane_id": 1, "fail": {}}


def test_help_lists_the_apps(binary):
    proc = subprocess.run([str(binary), "--help"], capture_output=True, text=True, timeout=30)
    assert proc.returncode == 0
    assert "sessions" in proc.stdout and "palette" in proc.stdout


def test_app_help_lists_headless_flags(binary):
    proc = subprocess.run([str(binary), "sessions", "--help"], capture_output=True, text=True, timeout=30)
    assert proc.returncode == 0
    for flag in ("--headless", "--keys", "--dump", "--size", "--context"):
        assert flag in proc.stdout


def test_headless_without_context_on_an_empty_mux_opens_with_no_rows(launcher):
    r = launcher.run(keys="settle", context=False, mux=EMPTY_MUX)
    assert r.exit == "open"
    assert r.rows() == []
    assert r.calls("list")


def test_outcome_without_dump_has_no_snapshot(launcher):
    r = launcher.run(keys="esc", dump=False)
    assert r.exit == "cancelled"
    assert "snapshot" not in r.outcome


def test_outcome_always_carries_an_actions_list(launcher):
    r = launcher.run(keys="esc", dump=False)
    assert r.actions == []


def test_tiny_size_does_not_crash(launcher):
    r = launcher.run(keys="down settle", size="40x10")
    assert r.exit == "open"


def test_bad_key_token_fails_with_a_message(launcher):
    r = launcher.run(keys="ctrl+notakey", check=False)
    assert r.returncode != 0
    assert r.stderr.strip()


def test_bad_size_fails_with_a_message(launcher):
    r = launcher.run(keys="settle", size="wide", check=False)
    assert r.returncode != 0
    assert "size" in r.stderr.lower()


def test_missing_context_file_is_an_error(launcher):
    r = launcher.run(keys="settle", context=False, env={"NARDO_CONTEXT": "/nonexistent/ctx.json"}, check=False)
    assert r.returncode != 0
    assert "context" in r.stderr.lower()


def test_context_flag_overrides_the_environment(launcher):
    ctx = launcher.context()
    ctx["options"] = {"scope": "panes"}
    path = launcher.root / "explicit-context.json"
    path.write_text(json.dumps(ctx), encoding="utf-8")
    r = launcher.run(keys="settle", options={"scope": "tabs"}, args=("--context", str(path)))
    assert r.snapshot["scope"] == "panes"
