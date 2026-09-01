#!/usr/bin/env python3
"""Deterministic state manager for the SLM autoresearch workflow.

The compiled NemoIR workflow calls this script from deterministic `exec:`
stages.  It owns trial counting, best-score tracking, candidate snapshots,
adapter archiving, accept/reject restore behavior, and trial history JSONL.

Commands:
    python harness/state.py init
    python harness/state.py start-trial
    python harness/state.py judge
    python harness/state.py accept
    python harness/state.py reject
    python harness/state.py should-continue

Each command prints a final JSON line with fields consumed by harness_tools.py.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CANDIDATE = ROOT / "candidate.py"
ADAPTER = ROOT / "adapter"
RESULTS = ROOT / "results.json"
BASE_RESULTS = ROOT / "base_results.json"


def _run_dir() -> Path:
    raw = os.environ.get("NEMOIR_RUN_DIR", "runs/current")
    path = Path(raw)
    return path if path.is_absolute() else ROOT / path


def _max_trials() -> int:
    return int(os.environ.get("NEMOIR_MAX_TRIALS", "5"))


def _profile() -> str:
    return os.environ.get("NEMOIR_PROFILE", "mnli_demo")


def _paths() -> dict[str, Path]:
    rd = _run_dir()
    return {
        "run_dir": rd,
        "state": rd / "state.json",
        "history": rd / "trial_history.jsonl",
        "trials": rd / "trials",
        "best_candidate": rd / "best_candidate.py",
        "initial_candidate": rd / "initial_candidate.py",
        "best_adapter": rd / "best_adapter",
    }


def _now() -> str:
    return _dt.datetime.now().isoformat()


def _candidate_hash(path: Path = CANDIDATE) -> str:
    if not path.exists():
        return "missing"
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def _load_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    return json.loads(path.read_text())


def _load_results() -> dict[str, Any]:
    return _load_json(RESULTS, {"ok": False, "score": 0.0, "error": "results_missing"})


def _default_state() -> dict[str, Any]:
    return {
        "profile": _profile(),
        "started_at": _now(),
        "trial_count": 0,
        "accepted_count": 0,
        "best_score": None,
        "best_metrics": None,
        "best_candidate_hash": None,
        "current_trial": None,
        "max_trials": _max_trials(),
    }


def _load_state() -> dict[str, Any]:
    return _load_json(_paths()["state"], _default_state())


def _save_state(state: dict[str, Any]) -> None:
    p = _paths()
    p["run_dir"].mkdir(parents=True, exist_ok=True)
    p["state"].write_text(json.dumps(state, indent=2, sort_keys=True))


def _append_history(entry: dict[str, Any]) -> None:
    p = _paths()
    p["run_dir"].mkdir(parents=True, exist_ok=True)
    with open(p["history"], "a") as f:
        f.write(json.dumps(entry, sort_keys=True) + "\n")


def _copytree(src: Path, dst: Path) -> None:
    if dst.exists():
        shutil.rmtree(dst)
    if src.exists():
        shutil.copytree(src, dst)


def _trial_dir(state: dict[str, Any] | None = None) -> Path:
    if state is None:
        state = _load_state()
    n = int(state.get("current_trial") or state.get("trial_count") or 0)
    return _paths()["trials"] / f"{n:03d}"


def _state_json(state: dict[str, Any]) -> str:
    return json.dumps(state, sort_keys=True)


def _numeric_best(state: dict[str, Any]) -> float:
    val = state.get("best_score")
    return float(val) if val is not None else 0.0


def _emit(state: dict[str, Any], *, report: str, score: float | None = None, continue_search: bool | None = None) -> int:
    payload = {
        "state_json": _state_json(state),
        "trial_count": int(state.get("trial_count") or 0),
        "best_score": _numeric_best(state),
        "score": float(score if score is not None else 0.0),
        "continue_search": bool(continue_search) if continue_search is not None else False,
        "report": report,
    }
    print(json.dumps(payload, sort_keys=True), flush=True)
    return 0


def _eps() -> float:
    raw = os.environ.get("NEMOIR_EPS", "0.02")
    try:
        return float(raw)
    except ValueError:
        return 0.02


def _extract_candidate_config() -> dict[str, Any]:
    """Extract training hyperparameters from candidate.py by executing it."""
    import importlib

    sys.path.insert(0, str(ROOT))
    config: dict[str, Any] = {}
    for attr in (
        "LORA_R", "LORA_ALPHA", "LORA_DROPOUT", "LORA_TARGET_MODULES",
        "LEARNING_RATE", "LR_SCHEDULER", "WARMUP_STEPS", "MAX_STEPS",
        "PER_DEVICE_BATCH_SIZE", "GRAD_ACCUM_STEPS", "MAX_SEQ_LENGTH",
        "TRAIN_EXAMPLES", "EVAL_EXAMPLES", "OPTIMIZER", "SEED",
    ):
        try:
            import candidate
            val = getattr(candidate, attr, None)
            if val is not None:
                config[attr] = val
        except Exception:
            pass
    return config


def _format_config_line(config: dict[str, Any]) -> str:
    """Build a compact one-line config summary."""
    return ", ".join(f"{k}={_fmt_val(config[k])}" for k in sorted(config) if k in config)


def _fmt_val(v: Any) -> str:
    if isinstance(v, float):
        return f"{v:.4g}"
    if isinstance(v, list):
        return "[" + ",".join(str(x) for x in v) + "]"
    return str(v)


def _assert_safe_run_dir(path: Path) -> None:
    resolved = path.resolve()
    forbidden = {Path("/").resolve(), ROOT.resolve(), Path.home().resolve()}
    if resolved in forbidden:
        raise ValueError(f"refusing to reset unsafe run dir: {resolved}")


def cmd_init(args: argparse.Namespace) -> int:
    p = _paths()
    _assert_safe_run_dir(p["run_dir"])
    if p["run_dir"].exists():
        shutil.rmtree(p["run_dir"])
    p["trials"].mkdir(parents=True, exist_ok=True)
    state = _default_state()
    state["max_trials"] = int(args.max_trials or _max_trials())
    if CANDIDATE.exists():
        shutil.copy2(CANDIDATE, p["initial_candidate"])
        state["initial_candidate_hash"] = _candidate_hash(CANDIDATE)
    _save_state(state)
    _append_history({"time": _now(), "event": "init", "state": state})
    return _emit(state, report=f"Initialized run dir {p['run_dir']} with max_trials={state['max_trials']}")


def cmd_start_trial(args: argparse.Namespace) -> int:
    p = _paths()
    state = _load_state()
    # Always start from the last accepted candidate, not from a rejected patch.
    if p["best_candidate"].exists():
        shutil.copy2(p["best_candidate"], CANDIDATE)
    state["trial_count"] = int(state.get("trial_count") or 0) + 1
    state["current_trial"] = state["trial_count"]
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)
    if CANDIDATE.exists():
        shutil.copy2(CANDIDATE, tdir / "parent_candidate.py")
    _save_state(state)
    _append_history({
        "time": _now(),
        "event": "start_trial",
        "trial": state["trial_count"],
        "best_score": state.get("best_score"),
        "candidate_hash": _candidate_hash(),
    })
    return _emit(state, report=f"Started trial {state['trial_count']} from best_score={state.get('best_score')}")


def cmd_judge(args: argparse.Namespace) -> int:
    state = _load_state()
    results = _load_results()
    score = float(results.get("score") or 0.0)
    best = state.get("best_score")
    delta = score - (float(best) if best is not None else 0.0)
    state["last_score"] = score
    state["last_delta"] = delta
    _save_state(state)
    return _emit(state, score=score, report=f"Judged current score={score:.6f}, best={best}, delta={delta:.6f}")


def cmd_accept(args: argparse.Namespace) -> int:
    p = _paths()
    state = _load_state()
    results = _load_results()
    score = float(results.get("score") or 0.0)
    trial = int(state.get("current_trial") or state.get("trial_count") or 0)
    trial_config = _extract_candidate_config()
    prev_best = state.get("best_score")
    prev_best_val = float(prev_best) if prev_best is not None else 0.0
    delta = score - prev_best_val
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)

    if CANDIDATE.exists():
        shutil.copy2(CANDIDATE, p["best_candidate"])
        shutil.copy2(CANDIDATE, tdir / "candidate.py")
    if RESULTS.exists():
        shutil.copy2(RESULTS, tdir / "metrics.json")
    _copytree(ADAPTER, p["best_adapter"])
    if ADAPTER.exists():
        _copytree(ADAPTER, tdir / "adapter")

    state["best_score"] = score
    state["best_metrics"] = results
    state["best_candidate_hash"] = _candidate_hash()
    state["accepted_count"] = int(state.get("accepted_count") or 0) + 1
    state["last_decision"] = "accept"
    _save_state(state)

    entry = {
        "time": _now(),
        "event": "accept",
        "trial": trial,
        "score": score,
        "prev_best": prev_best_val,
        "delta": delta,
        "candidate_hash": _candidate_hash(),
        "trial_config": trial_config,
        "metrics": results,
    }
    _append_history(entry)
    config_line = _format_config_line(trial_config)
    report = (
        f"Accepted trial {trial}: score={score:.4f} prev_best={prev_best_val:.4f} "
        f"delta=+{delta:.4f}. Config: {config_line}"
    )
    return _emit(state, score=score, report=report)


def cmd_reject(args: argparse.Namespace) -> int:
    p = _paths()
    state = _load_state()
    results = _load_results()
    score = float(results.get("score") or 0.0)
    trial = int(state.get("current_trial") or state.get("trial_count") or 0)
    trial_config = _extract_candidate_config()
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)
    if CANDIDATE.exists():
        shutil.copy2(CANDIDATE, tdir / "rejected_candidate.py")
    if RESULTS.exists():
        shutil.copy2(RESULTS, tdir / "metrics.json")

    restored = False
    if p["best_candidate"].exists():
        shutil.copy2(p["best_candidate"], CANDIDATE)
        restored = True
    if p["best_adapter"].exists():
        _copytree(p["best_adapter"], ADAPTER)

    error = str(results.get("error") or "")
    timeout_seconds = results.get("timeout_seconds")
    best = state.get("best_score")
    best_val = float(best) if best is not None else 0.0
    delta = score - best_val
    epsilon = _eps()

    if error == "train_timeout":
        reason = f"TRAIN_TIMEOUT: training exceeded {timeout_seconds} seconds and was killed"
    elif error == "train_failed":
        reason = "TRAIN_FAILED: training exited with an error before evaluation"
    elif score > best_val:
        reason = (
            f"IMPROVED_BUT_BELOW_EPS: score {score:.4f} > best {best_val:.4f} "
            f"(delta=+{delta:.4f}) but below acceptance threshold eps={epsilon:.4f}"
        )
        error = f"IMPROVED_BUT_BELOW_EPS: delta={delta:.6f} eps={epsilon:.6f}"
    else:
        reason = (
            f"NO_IMPROVEMENT: score {score:.4f} <= best {best_val:.6f} "
            f"(delta={delta:.4f})"
        )

    state["last_score"] = score
    state["last_error"] = error
    state["last_decision"] = "reject"
    _save_state(state)
    _append_history({
        "time": _now(),
        "event": "reject",
        "trial": trial,
        "score": score,
        "best_score": best_val,
        "delta": delta,
        "eps": epsilon,
        "error": error,
        "reason": reason,
        "timeout_seconds": timeout_seconds,
        "restored_best_candidate": restored,
        "trial_config": trial_config,
        "metrics": results,
    })

    config_line = _format_config_line(trial_config)
    report = (
        f"Rejected trial {trial}: {reason}. "
        f"best_score={best_val:.4f} score={score:.4f} delta={delta:+.4f} "
        f"eps={epsilon:.4f} restored={restored}. "
        f"Config: {config_line}"
    )
    return _emit(state, score=score, report=report)


def cmd_should_continue(args: argparse.Namespace) -> int:
    state = _load_state()
    max_trials = int(args.max_trials or state.get("max_trials") or _max_trials())
    state["max_trials"] = max_trials
    continue_search = int(state.get("trial_count") or 0) < max_trials
    _save_state(state)
    _append_history({
        "time": _now(),
        "event": "should_continue",
        "trial_count": state.get("trial_count"),
        "max_trials": max_trials,
        "continue_search": continue_search,
        "best_score": state.get("best_score"),
    })
    msg = (
        f"continue_search={continue_search}; "
        f"trial_count={state.get('trial_count')} max_trials={max_trials} "
        f"best_score={state.get('best_score')}"
    )
    return _emit(state, report=msg, continue_search=continue_search)


def main() -> int:
    parser = argparse.ArgumentParser(description="NemoIR autoresearch state manager")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_init = sub.add_parser("init")
    p_init.add_argument("--max-trials", type=int, default=None)
    p_init.set_defaults(func=cmd_init)

    p_start = sub.add_parser("start-trial")
    p_start.set_defaults(func=cmd_start_trial)

    p_judge = sub.add_parser("judge")
    p_judge.set_defaults(func=cmd_judge)

    p_accept = sub.add_parser("accept")
    p_accept.set_defaults(func=cmd_accept)

    p_reject = sub.add_parser("reject")
    p_reject.set_defaults(func=cmd_reject)

    p_continue = sub.add_parser("should-continue")
    p_continue.add_argument("--max-trials", type=int, default=None)
    p_continue.set_defaults(func=cmd_should_continue)

    args = parser.parse_args()
    try:
        return int(args.func(args))
    except Exception as e:
        payload = {
            "state_json": json.dumps(_load_state(), sort_keys=True),
            "trial_count": int((_load_state()).get("trial_count") or 0),
            "best_score": _numeric_best(_load_state()),
            "score": 0.0,
            "continue_search": False,
            "report": f"state.py error: {type(e).__name__}: {e}",
        }
        print(json.dumps(payload, sort_keys=True), flush=True)
        raise


if __name__ == "__main__":
    sys.exit(main())
