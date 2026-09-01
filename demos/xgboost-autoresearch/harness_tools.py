"""Safe deterministic command adapter for the Covertype harness.

``exec: os.shell(...)`` is the current backend-neutral DSL primitive.  This
adapter accepts only the workflow's exact command literals, maps them to
``sys.executable`` argument vectors, and never invokes a shell.  NemoIR's
compiled policies are the first enforcement layer; this mapping is deliberate
second-line defense.
"""

from __future__ import annotations

import asyncio
import json
import os
import signal
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from nemoir_runtime import ToolContext, ToolInvocationError, tool

HERE = Path(__file__).resolve().parent
ROOT = HERE


@dataclass(frozen=True)
class AgentReadResult:
    """Bounded text-file response for model-facing reads."""

    path: str
    content: str
    offset: int
    limit: int
    lines_returned: int
    truncated: bool


@dataclass(frozen=True)
class HarnessResult:
    ok: bool
    state_json: str
    trial_count: int
    score: float
    best_score: float
    selection_score: float
    confirmation_score: float
    repair_allowed: bool
    continue_search: bool
    report: str
    metrics: str
    log: str
    candidate_hash: str
    mutable_candidate_path: str
    history_summary: str
    history_path: str
    agent_view_path: str


@tool(
    capability="fs.read",
    description=(
        "Read the mutable candidate configuration or sanitized trial history. "
        "A missing allowed path returns corrective guidance instead of aborting the workflow."
    ),
)
async def read_agent_file(
    *,
    path: Path,
    ctx: ToolContext,
    offset: int = 0,
    limit: int = 2000,
) -> AgentReadResult:
    """Read an allowed text file; unauthorized reads raise a retryable error.

    Authorization is enforced at the handler level so that a rejected read
    becomes a ToolInvocationError (fed back to the model for retry) rather
    than a fatal PolicyDeniedError.
    """
    if offset < 0 or limit <= 0:
        raise ToolInvocationError("offset must be >= 0 and limit must be > 0")
    cwd = Path(ctx.inputs.get("cwd", Path.cwd()))
    resolved = path if path.is_absolute() else (cwd / path).resolve()
    candidate = Path(ctx.inputs.get("candidate_path", cwd / "candidate.json")).resolve()
    history_dir = Path(ctx.inputs.get("agent_view_dir", cwd / "agent_view")).resolve()

    # Authorization: only the mutable candidate and files under agent_view_dir.
    is_candidate = resolved == candidate
    is_under_history = history_dir in resolved.parents or resolved == history_dir
    if not (is_candidate or is_under_history):
        raise ToolInvocationError(
            f"Read denied — {resolved} is outside the authorized scope. "
            f"Read the candidate at {candidate} or history at {history_dir / 'history.jsonl'}."
        )

    if not resolved.exists():
        content = (
            f"Requested file does not exist: {resolved}.\n"
            f"Read the candidate only at: {candidate}\n"
            f"Read aggregate history only at: {history_dir / 'history.jsonl'}\n"
            "Do not invent paths beneath the history directory."
        )
        return AgentReadResult(
            path=str(resolved),
            content=content,
            offset=offset,
            limit=limit,
            lines_returned=len(content.splitlines()),
            truncated=False,
        )
    if resolved.is_dir():
        raise ToolInvocationError(f"path is a directory: {resolved}")
    lines = resolved.read_text(encoding="utf-8").splitlines()
    selected = lines[offset : offset + limit]
    return AgentReadResult(
        path=str(resolved),
        content="\n".join(selected),
        offset=offset,
        limit=limit,
        lines_returned=len(selected),
        truncated=offset + limit < len(lines),
    )


@dataclass(frozen=True)
class AgentWriteResult:
    """Result of a model-initiated file write to the mutable candidate."""

    path: str
    bytes_written: int
    created: bool


@tool(
    capability="fs.write",
    description="Create or overwrite the candidate.json configuration only.",
)
async def write_agent_file(
    *,
    path: Path,
    content: str,
    ctx: ToolContext,
) -> AgentWriteResult:
    """Enforce candidate-only writes; all other paths raise a retryable error."""
    cwd = Path(ctx.inputs.get("cwd", Path.cwd()))
    resolved = path if path.is_absolute() else (cwd / path).resolve()
    candidate = Path(ctx.inputs.get("candidate_path", cwd / "candidate.json")).resolve()
    if resolved != candidate:
        raise ToolInvocationError(
            f"Write denied — only {candidate} may be written, got {resolved}."
        )
    created = not resolved.exists()
    resolved.parent.mkdir(parents=True, exist_ok=True)
    encoded = content.encode("utf-8")
    resolved.write_bytes(encoded)
    return AgentWriteResult(path=str(resolved), bytes_written=len(encoded), created=created)


@dataclass(frozen=True)
class AgentEditResult:
    """Result of a model-initiated file edit to the mutable candidate."""

    path: str
    occurrences_replaced: int
    bytes_written: int


@tool(
    capability="fs.write",
    description="Edit the candidate.json configuration by replacing an exact string.",
)
async def edit_agent_file(
    *,
    path: Path,
    content: str,
    new_content: str,
    ctx: ToolContext,
    replace_all: bool = False,
) -> AgentEditResult:
    """Enforce candidate-only edits; all other paths raise a retryable error."""
    cwd = Path(ctx.inputs.get("cwd", Path.cwd()))
    resolved = path if path.is_absolute() else (cwd / path).resolve()
    candidate = Path(ctx.inputs.get("candidate_path", cwd / "candidate.json")).resolve()
    if resolved != candidate:
        raise ToolInvocationError(
            f"Edit denied — only {candidate} may be edited, got {resolved}."
        )
    if not content:
        raise ToolInvocationError("content (old text) must be non-empty")
    text = resolved.read_text(encoding="utf-8")
    count = text.count(content)
    if count == 0:
        raise ToolInvocationError(f"content not found in {resolved}")
    if not replace_all and count > 1:
        raise ToolInvocationError(
            f"content found {count} times; use replace_all=True"
        )
    text = text.replace(content, new_content) if replace_all else text.replace(content, new_content, 1)
    encoded = text.encode("utf-8")
    resolved.write_bytes(encoded)
    return AgentEditResult(
        path=str(resolved),
        occurrences_replaced=count if replace_all else 1,
        bytes_written=len(encoded),
    )

# These strings are intentionally identical to the .nemo allowlist.
_COMMAND_ARGS: dict[str, tuple[str, ...]] = {
    "python harness/state.py init": ("harness/state.py", "init"),
    "python harness/baseline.py": ("harness/baseline.py",),
    "python harness/state.py adopt-baseline": ("harness/state.py", "adopt-baseline"),
    "python harness/state.py start-trial": ("harness/state.py", "start-trial"),
    "python harness/preflight.py": ("harness/preflight.py",),
    "python harness/state.py repair-gate": ("harness/state.py", "repair-gate"),
    "python harness/train.py": ("harness/train.py",),
    "python harness/evaluate.py --split selection --model current": (
        "harness/evaluate.py",
        "--split",
        "selection",
        "--model",
        "current",
    ),
    "python harness/state.py judge-primary": ("harness/state.py", "judge-primary"),
    "python harness/evaluate.py --split confirmation --model current": (
        "harness/evaluate.py",
        "--split",
        "confirmation",
        "--model",
        "current",
    ),
    "python harness/state.py judge-confirm": ("harness/state.py", "judge-confirm"),
    "python harness/state.py accept": ("harness/state.py", "accept"),
    "python harness/state.py reject": ("harness/state.py", "reject"),
    "python harness/state.py should-continue": ("harness/state.py", "should-continue"),
    "python harness/final_eval.py": ("harness/final_eval.py",),
}

# Even though a model stage that exposes os.shell can see both os.shell tools,
# this map prevents it from using run_harness to spend compute or alter state.
_DETERMINISTIC_STAGE_COMMANDS: dict[str, str] = {
    "Init": "python harness/state.py init",
    "Baseline": "python harness/baseline.py",
    "AdoptBaseline": "python harness/state.py adopt-baseline",
    "StartTrial": "python harness/state.py start-trial",
    "Preflight": "python harness/preflight.py",
    "RepairGate": "python harness/state.py repair-gate",
    "TrainCandidate": "python harness/train.py",
    "EvaluateSelection": "python harness/evaluate.py --split selection --model current",
    "JudgePrimary": "python harness/state.py judge-primary",
    "ConfirmCandidate": "python harness/evaluate.py --split confirmation --model current",
    "JudgeConfirmed": "python harness/state.py judge-confirm",
    "AcceptCandidate": "python harness/state.py accept",
    "RejectCandidate": "python harness/state.py reject",
    "CheckBudget": "python harness/state.py should-continue",
    "FinalEval": "python harness/final_eval.py",
}

_TRAIN_COMMANDS = {"python harness/baseline.py", "python harness/train.py"}
_EVAL_COMMANDS = {
    "python harness/evaluate.py --split selection --model current",
    "python harness/evaluate.py --split confirmation --model current",
    "python harness/final_eval.py",
}

_FAILURE_METRIC_FOR_COMMAND: dict[str, str] = {
    "python harness/baseline.py": "baseline",
    "python harness/preflight.py": "preflight",
    "python harness/train.py": "train",
    "python harness/evaluate.py --split selection --model current": "selection",
    "python harness/evaluate.py --split confirmation --model current": "confirmation",
    "python harness/final_eval.py": "final",
}


def _timeout_from_env(name: str, default: float) -> float:
    try:
        value = float(os.environ.get(name, str(default)))
    except ValueError:
        return default
    return value if value > 0 else default


def _timeout_for(command: str) -> float:
    if command in _TRAIN_COMMANDS:
        return _timeout_from_env("NEMOIR_TRAIN_TIMEOUT_SECONDS", 20 * 60.0)
    if command in _EVAL_COMMANDS:
        return _timeout_from_env("NEMOIR_EVAL_TIMEOUT_SECONDS", 10 * 60.0)
    return _timeout_from_env("NEMOIR_COMMAND_TIMEOUT_SECONDS", 120.0)


def _last_json_line(text: str) -> dict[str, Any]:
    for line in reversed(text.splitlines()):
        candidate = line.strip()
        if not (candidate.startswith("{") and candidate.endswith("}")):
            continue
        try:
            parsed = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            return parsed
    return {}


def _as_float(value: Any) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def _as_int(value: Any) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def _to_result(data: dict[str, Any], *, ok: bool, log: str) -> HarnessResult:
    report = str(data.get("report") or data.get("error") or "")
    metrics = data.get("metrics", "")
    if not isinstance(metrics, str):
        metrics = json.dumps(metrics, sort_keys=True, default=str)
    return HarnessResult(
        ok=bool(data.get("ok", ok)) and ok,
        state_json=str(data.get("state_json") or "{}"),
        trial_count=_as_int(data.get("trial_count")),
        score=_as_float(data.get("score")),
        best_score=_as_float(data.get("best_score")),
        selection_score=_as_float(data.get("selection_score")),
        confirmation_score=_as_float(data.get("confirmation_score")),
        repair_allowed=bool(data.get("repair_allowed", False)),
        continue_search=bool(data.get("continue_search", False)),
        report=report,
        metrics=metrics,
        log=log,
        candidate_hash=str(data.get("candidate_hash") or ""),
        mutable_candidate_path=str(data.get("mutable_candidate_path") or ""),
        history_summary=str(data.get("history_summary") or ""),
        history_path=str(data.get("history_path") or ""),
        agent_view_path=str(data.get("agent_view_path") or ""),
    )


def _record_process_failure(
    command: str,
    *,
    cwd: Path,
    exit_code: int,
    timed_out: bool,
    log: str,
) -> None:
    """Persist a structured failure when a killed process cannot do so itself."""
    metric_name = _FAILURE_METRIC_FOR_COMMAND.get(command)
    if metric_name is None:
        return
    raw_run_dir = os.environ.get("NEMOIR_RUN_DIR", "runs/current")
    run_dir = Path(raw_run_dir)
    if not run_dir.is_absolute():
        run_dir = cwd / run_dir
    metric_path = run_dir / "metrics" / f"{metric_name}.json"
    # Preserve a normal harness-generated failure artifact when available.
    if metric_path.exists() and not timed_out:
        return
    metric_path.parent.mkdir(parents=True, exist_ok=True)
    error = "command_timeout" if timed_out else f"command_exit_{exit_code}"
    metric_path.write_text(json.dumps({
        "ok": False,
        "stage": metric_name,
        "error": error,
        "command": command,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "timestamp": datetime.now(tz=UTC).isoformat(),
        "log_tail": log[-4000:],
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")


async def _invoke(argv: tuple[str, ...], *, cwd: Path, timeout_s: float) -> tuple[int, str, str, bool]:
    proc = await asyncio.create_subprocess_exec(
        sys.executable,
        *argv,
        cwd=str(cwd),
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
        except (AttributeError, ProcessLookupError, PermissionError):
            try:
                proc.kill()
            except ProcessLookupError:
                pass
        stdout, stderr = await proc.communicate()
        return 124, stdout.decode(errors="replace"), stderr.decode(errors="replace"), True


@tool(
    capability="os.shell",
    description="Run one fixed, trusted Covertype harness command without a shell.",
    returns={
        "ok": bool,
        "state_json": str,
        "trial_count": int,
        "score": float,
        "best_score": float,
        "selection_score": float,
        "confirmation_score": float,
        "repair_allowed": bool,
        "continue_search": bool,
        "report": str,
        "metrics": str,
        "log": str,
        "candidate_hash": str,
        "mutable_candidate_path": str,
        "history_summary": str,
        "history_path": str,
        "agent_view_path": str,
    },
)
async def run_harness(*, command: str, ctx: ToolContext) -> HarnessResult:
    expected_command = _DETERMINISTIC_STAGE_COMMANDS.get(ctx.stage_id)
    if expected_command != command:
        raise ToolInvocationError(
            f"run_harness expects only {expected_command!r} in stage {ctx.stage_id!r}, "
            f"got {command!r}. Use read_agent_file for authorized candidate/history reads."
        )
    argv = _COMMAND_ARGS.get(command)
    if argv is None:
        return _to_result(
            {"ok": False, "error": f"unrecognized deterministic command: {command!r}"},
            ok=False,
            log="",
        )

    cwd_value = ctx.inputs.get("cwd")
    cwd = Path(cwd_value) if cwd_value is not None else Path.cwd()
    code, stdout, stderr, timed_out = await _invoke(argv, cwd=cwd, timeout_s=_timeout_for(command))
    log = stdout
    if stderr:
        log = f"{log}\n[stderr]\n{stderr}" if log else f"[stderr]\n{stderr}"
    if timed_out:
        log = f"{log}\nTIMEOUT: {command} exceeded {_timeout_for(command):.1f} seconds"
    data = _last_json_line(stdout)
    if not data:
        data = {
            "ok": False,
            "error": f"harness produced no final JSON result (exit_code={code})",
        }
    if code != 0 or timed_out:
        data = dict(data)
        data["ok"] = False
        detail = f"command exit_code={code}" if not timed_out else "command timed out"
        data["error"] = str(data.get("error") or detail)
        _record_process_failure(command, cwd=cwd, exit_code=code, timed_out=timed_out, log=log)
    return _to_result(data, ok=code == 0 and not timed_out, log=log)
