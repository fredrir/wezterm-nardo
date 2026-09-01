import shutil
import subprocess

import pytest

from conftest import ROOT


def test_lua_plugin_suite_passes():
    lua = shutil.which("lua") or shutil.which("lua5.4")
    if not lua:
        pytest.skip("lua not installed")
    proc = subprocess.run([lua, "tests/run.lua"], cwd=ROOT / "plugin", capture_output=True, text=True, timeout=60)
    assert proc.returncode == 0, f"--- stdout ---\n{proc.stdout}\n--- stderr ---\n{proc.stderr}"
