import copy
import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest

TESTS = Path(__file__).resolve().parent
ROOT = TESTS.parent
FAKE_WEZTERM = TESTS / "fake_wezterm.py"
FIXTURES = TESTS / "fixtures"
OWN_PANE = 99
RUN_TIMEOUT_S = 30


def load_fixture(name="mux_two_hosts"):
    return json.loads((FIXTURES / f"{name}.json").read_text(encoding="utf-8"))


class Call:
    def __init__(self, args, cls=None):
        self.args = list(args)
        self.cls = cls

    @property
    def sub(self):
        return self.args[0] if self.args else None

    def has(self, flag):
        return flag in self.args

    def value(self, flag):
        try:
            return self.args[self.args.index(flag) + 1]
        except (ValueError, IndexError):
            return None

    def __eq__(self, other):
        return self.args == (other.args if isinstance(other, Call) else list(other))

    def __repr__(self):
        return f"Call({self.args!r})"


class FakeWezterm:
    def __init__(self, directory: Path):
        self.dir = directory
        self.dir.mkdir(parents=True, exist_ok=True)
        self.state_path = self.dir / "wezterm-state.json"
        self.log_path = self.dir / "wezterm-calls.jsonl"

    def seed(self, state):
        self.state_path.write_text(json.dumps(state), encoding="utf-8")

    def env(self):
        return {"FAKE_WEZTERM_STATE": str(self.state_path), "FAKE_WEZTERM_LOG": str(self.log_path)}

    def state(self):
        return json.loads(self.state_path.read_text(encoding="utf-8"))

    def log(self):
        if not self.log_path.exists():
            return []
        lines = self.log_path.read_text(encoding="utf-8").splitlines()
        entries = [json.loads(line) for line in lines if line.strip()]
        return [Call(e["args"], e.get("class")) for e in entries]

    def call(self, *args, check=True, env=None):
        proc = subprocess.run(
            [str(FAKE_WEZTERM), "cli", *args],
            env={**os.environ, **self.env(), **(env or {})},
            capture_output=True,
            text=True,
            timeout=RUN_TIMEOUT_S,
        )
        if check and proc.returncode != 0:
            raise AssertionError(f"fake wezterm {args} exited {proc.returncode}: {proc.stderr}")
        return proc


@dataclass
class Result:
    outcome: dict
    log: list
    state: dict
    stdout: str
    stderr: str
    returncode: int
    state_dir: Path

    @property
    def exit(self):
        return self.outcome.get("exit")

    @property
    def snapshot(self) -> dict:
        snapshot = self.outcome.get("snapshot")
        assert snapshot is not None, "outcome has no snapshot: run with dump=True"
        return snapshot

    @property
    def actions(self):
        return self.outcome.get("actions", [])

    @property
    def selected(self):
        return self.snapshot["selected"]

    @property
    def overlay(self):
        return self.snapshot.get("overlay")

    def rows(self, kind=None):
        rows = self.snapshot["rows"]
        return rows if kind is None else [r for r in rows if r["kind"] == kind]

    def ids(self, kind):
        return [r["id"] for r in self.rows(kind)]

    def calls(self, sub):
        return [c for c in self.log if c.sub == sub]

    def single(self, sub):
        calls = self.calls(sub)
        assert len(calls) == 1, f"expected exactly one {sub!r} call, got {calls}"
        return calls[0]

    def killed(self):
        return [int(c.value("--pane-id")) for c in self.calls("kill-pane")]

    def pane_ids(self):
        return {p["pane_id"] for p in self.state["panes"]}

    def pane(self, pane_id):
        return next(p for p in self.state["panes"] if p["pane_id"] == pane_id)

    def mru(self):
        path = self.state_dir / "state.json"
        if not path.exists():
            return []
        return json.loads(path.read_text(encoding="utf-8")).get("mru", [])


class Launcher:
    def __init__(self, binary: Path, root: Path, fixture: dict):
        self.binary = binary
        self.root = root
        self.fixture = fixture
        self.runs = 0

    def context(self):
        return copy.deepcopy(self.fixture["context"])

    def mux(self):
        return copy.deepcopy(self.fixture["state"])

    def run(
        self,
        app="sessions",
        keys="",
        dump=True,
        context=None,
        options=None,
        size="120x40",
        env=None,
        mux=None,
        fail=None,
        mru=None,
        check=True,
        args=(),
    ):
        self.runs += 1
        run_dir = self.root / f"run{self.runs}"
        fake = FakeWezterm(run_dir)
        state = self.mux() if mux is None else copy.deepcopy(mux)
        if fail:
            state.setdefault("fail", {}).update(fail)
        fake.seed(state)

        state_dir = run_dir / "nardo-state"
        state_dir.mkdir()
        if mru is not None:
            (state_dir / "state.json").write_text(json.dumps({"mru": mru, "last_query": ""}), encoding="utf-8")

        environment = {k: v for k, v in os.environ.items() if k not in ("NARDO_CONTEXT", "WEZPLUG_LOG", "XDG_STATE_HOME")}
        environment.update(fake.env())
        environment.update(
            NARDO_WEZTERM=str(FAKE_WEZTERM),
            NARDO_STATE_DIR=str(state_dir),
            WEZTERM_PANE=str(OWN_PANE),
            NARDO_REDUCE_MOTION="1",
        )
        if context is not False:
            ctx = self.context() if context is None else copy.deepcopy(context)
            if options:
                ctx.setdefault("options", {}).update(options)
            ctx_path = run_dir / "context.json"
            ctx_path.write_text(json.dumps(ctx), encoding="utf-8")
            environment["NARDO_CONTEXT"] = str(ctx_path)
        if env:
            environment.update(env)

        cmd = [str(self.binary), app, "--headless", "--size", size, "--keys", keys, *args]
        if dump:
            cmd.append("--dump")
        proc = subprocess.run(
            cmd,
            env=environment,
            capture_output=True,
            text=True,
            timeout=RUN_TIMEOUT_S,
            stdin=subprocess.DEVNULL,
            cwd=run_dir,
        )
        parsed = parse_outcome(proc.stdout) if proc.returncode == 0 else None
        if check:
            if proc.returncode != 0:
                pytest.fail(
                    f"wez-nardo exited {proc.returncode}\n$ {' '.join(cmd)}\n"
                    f"--- stderr ---\n{proc.stderr}\n--- stdout ---\n{proc.stdout}"
                )
            if parsed is None:
                pytest.fail(f"wez-nardo printed no outcome json\n--- stdout ---\n{proc.stdout}\n--- stderr ---\n{proc.stderr}")
        return Result(
            outcome=parsed if parsed is not None else {},
            log=fake.log(),
            state=fake.state(),
            stdout=proc.stdout,
            stderr=proc.stderr,
            returncode=proc.returncode,
            state_dir=state_dir,
        )


def parse_outcome(stdout) -> dict | None:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if line.startswith("{"):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                return None
    return None


@pytest.fixture(scope="session")
def binary():
    path = Path(os.environ.get("NARDO_BIN") or ROOT / "backend" / "target" / "debug" / "wez-nardo")
    if not path.is_file():
        pytest.skip(f"wez-nardo binary not found at {path}; run `just build debug` or set NARDO_BIN")
    return path


@pytest.fixture(scope="session")
def fixture_data():
    return load_fixture()


@pytest.fixture
def fake(tmp_path):
    return FakeWezterm(tmp_path / "fake")


@pytest.fixture
def launcher(binary, tmp_path, fixture_data):
    return Launcher(binary, tmp_path, fixture_data)
