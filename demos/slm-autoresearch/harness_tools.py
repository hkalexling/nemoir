"""Custom NemoIR tools for the SLM autoresearch harness.

The generated workflow calls the generic `os.shell` capability from
`exec:` stages.  We register three concrete wrappers with distinct output
schemas so the runtime can deterministically select the correct wrapper by
stage writes:

- run_harness: preflight/train -> {ok, log}
- run_eval:    eval           -> {ok, score, metrics}
- run_state:   state.py       -> {state_json, trial_count, best_score, ...}
"""

from __future__ import annotations

import asyncio
import json
import os
import signal
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from nemoir_runtime import ToolContext, tool


@dataclass
class HarnessResult:
    ok: bool
    log: str


@dataclass
class EvalResult:
    ok: bool
    score: float
    metrics: str


@dataclass
class StateResult:
    # Deliberately no `ok`/`log` fields: this avoids deterministic-tool
    # selection ambiguity with run_harness's {ok, log} schema.
    state_json: str
    trial_count: int
    best_score: float
    score: float
    continue_search: bool
    report: str


DEFAULT_TRAIN_TIMEOUT_SECONDS = 19 * 60  # under 20 minutes by default
TRAIN_COMMAND = "python harness/train.py"


async def _run_shell(command: str) -> tuple[int, str, str]:
    proc = await asyncio.create_subprocess_shell(
        command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()
    return (
        int(proc.returncode or 0),
        stdout.decode(errors="replace"),
        stderr.decode(errors="replace"),
    )


async def _run_shell_with_timeout(command: str, timeout_s: float) -> tuple[int, str, str, bool]:
    """Run command with a wall-clock timeout and kill the process group."""
    proc = await asyncio.create_subprocess_shell(
        command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = await asyncio.wait_for(proc.communicate(), timeout=timeout_s)
        return int(proc.returncode or 0), stdout.decode(errors="replace"), stderr.decode(errors="replace"), False
    except asyncio.TimeoutError:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except Exception:
            try:
                proc.kill()
            except ProcessLookupError:
                pass
        stdout, stderr = await proc.communicate()
        out = stdout.decode(errors="replace")
        err = stderr.decode(errors="replace")
        err += f"\nTRAIN_TIMEOUT: command exceeded {timeout_s:.1f} seconds and was killed."
        return 124, out, err, True


def _train_timeout_seconds() -> float:
    raw = os.environ.get("NEMOIR_TRAIN_TIMEOUT_SECONDS", str(DEFAULT_TRAIN_TIMEOUT_SECONDS))
    try:
        val = float(raw)
    except ValueError:
        return float(DEFAULT_TRAIN_TIMEOUT_SECONDS)
    if val <= 0:
        return float(DEFAULT_TRAIN_TIMEOUT_SECONDS)
    # User requested <20 min; cap accidental larger values at 19m59s.
    return min(val, 19 * 60 + 59)


def _write_train_failure_result(*, error: str, command: str, log: str, timeout_s: float | None = None) -> None:
    payload = {
        "ok": False,
        "score": 0.0,
        "accuracy": 0.0,
        "macro_f1": 0.0,
        "error": error,
        "stage": "TrainAdapter",
        "command": command,
        "timestamp": datetime.now().isoformat(),
        "log_tail": log[-4000:],
    }
    if timeout_s is not None:
        payload["timeout_seconds"] = timeout_s
    Path("results.json").write_text(json.dumps(payload, indent=2, sort_keys=True))


def _last_json_line(out: str) -> dict:
    for line in reversed(out.strip().split("\n")):
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                continue
    return {}


@tool(
    capability="os.shell",
    description="Run a harness shell command (preflight or train). Returns ok and log.",
)
async def run_harness(ctx: ToolContext, command: str) -> HarnessResult:
    if command.strip() == TRAIN_COMMAND:
        timeout_s = _train_timeout_seconds()
        code, out, err, timed_out = await _run_shell_with_timeout(command, timeout_s)
        log = out
        if err:
            log += "\n[stderr]\n" + err
        if timed_out:
            _write_train_failure_result(
                error="train_timeout",
                command=command,
                log=log,
                timeout_s=timeout_s,
            )
            return HarnessResult(ok=False, log=log)
        if code != 0:
            _write_train_failure_result(error="train_failed", command=command, log=log)
        return HarnessResult(ok=code == 0, log=log)

    code, out, err = await _run_shell(command)
    log = out
    if err:
        log += "\n[stderr]\n" + err
    return HarnessResult(ok=code == 0, log=log)


@tool(
    capability="os.shell",
    description="Run harness evaluation and extract the SLMScore JSON result.",
    returns={"ok": bool, "score": float, "metrics": str},
)
async def run_eval(ctx: ToolContext, command: str) -> EvalResult:
    code, out, err = await _run_shell(command)
    data = _last_json_line(out)
    score = float(data.get("score", 0.0) or 0.0)
    ok = bool(data.get("ok", False)) and code == 0
    metrics = json.dumps(data, sort_keys=True) if data else out
    if err:
        metrics += "\n[stderr]\n" + err
    return EvalResult(ok=ok, score=score, metrics=metrics)


@tool(
    capability="os.shell",
    description="Run deterministic autoresearch state management commands.",
    returns={
        "state_json": str,
        "trial_count": int,
        "best_score": float,
        "score": float,
        "continue_search": bool,
        "report": str,
    },
)
async def run_state(ctx: ToolContext, command: str) -> StateResult:
    code, out, err = await _run_shell(command)
    data = _last_json_line(out)
    if not data:
        data = {
            "state_json": "{}",
            "trial_count": 0,
            "best_score": 0.0,
            "score": 0.0,
            "continue_search": False,
            "report": out + ("\n[stderr]\n" + err if err else ""),
        }
    report = str(data.get("report", ""))
    if code != 0:
        report = (report + "\n" if report else "") + f"state command exited {code}"
    if err:
        report = (report + "\n[stderr]\n" + err) if report else "[stderr]\n" + err
    return StateResult(
        state_json=str(data.get("state_json", "{}")),
        trial_count=int(data.get("trial_count", 0) or 0),
        best_score=float(data.get("best_score", 0.0) or 0.0),
        score=float(data.get("score", 0.0) or 0.0),
        continue_search=bool(data.get("continue_search", False)),
        report=report,
    )
