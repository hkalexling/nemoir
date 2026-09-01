#!/usr/bin/env python3
"""Trusted state manager for the Covertype XGBoost autoresearch workflow.

The compiled NemoIR workflow calls this module only from deterministic ``exec:``
stages.  It owns incumbent restoration, trial accounting, evidence snapshots,
and the aggregate values consumed by compiled numeric guards.  It deliberately
*does not* decide whether a score clears epsilon; that decision lives in the
workflow IR's transition guard.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import shutil
import sys
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CANDIDATE = ROOT / "candidate.json"
BASELINE_CONFIG = ROOT / "configs" / "baseline.json"


def _now() -> str:
    return datetime.now(tz=UTC).isoformat()


def _run_dir() -> Path:
    raw = os.environ.get("NEMOIR_RUN_DIR", "runs/current")
    path = Path(raw)
    return path if path.is_absolute() else ROOT / path


def _max_trials() -> int:
    try:
        return max(0, int(os.environ.get("NEMOIR_MAX_TRIALS", "10")))
    except ValueError:
        return 10


def _max_repairs() -> int:
    try:
        return max(0, int(os.environ.get("NEMOIR_MAX_REPAIRS", "2")))
    except ValueError:
        return 2


def _epsilon() -> float:
    """Return the compiled guard's configured material-improvement threshold."""
    try:
        value = float(os.environ.get("NEMOIR_EPS", "0.002"))
    except ValueError:
        return 0.002
    return value if math.isfinite(value) and value >= 0.0 else 0.002


def _paths() -> dict[str, Path]:
    run_dir = _run_dir()
    return {
        "run_dir": run_dir,
        "state": run_dir / "state.json",
        "manifest": run_dir / "run_manifest.json",
        "history": run_dir / "agent_view" / "history.jsonl",
        "trials": run_dir / "trials",
        "work": run_dir / "work",
        "models": run_dir / "models",
        "metrics": run_dir / "metrics",
        "best_candidate": run_dir / "best_candidate.json",
        "initial_candidate": run_dir / "initial_candidate.json",
        "seen_configs": run_dir / "seen_configs.json",
        "baseline_model": run_dir / "models" / "baseline_model.json",
        "best_model": run_dir / "models" / "best_model.json",
    }


def _ensure_layout() -> None:
    paths = _paths()
    for key in ("run_dir", "trials", "work", "models", "metrics"):
        paths[key].mkdir(parents=True, exist_ok=True)
    paths["history"].parent.mkdir(parents=True, exist_ok=True)


def _atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w", encoding="utf-8", dir=path.parent, prefix=f".{path.name}.", delete=False
    ) as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
        tmp = Path(handle.name)
    tmp.replace(path)


def _write_json(path: Path, payload: Any) -> None:
    _atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def _load_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return default


def _candidate_hash(path: Path = CANDIDATE) -> str:
    if not path.exists():
        return "missing"
    try:
        parsed = json.loads(path.read_text(encoding="utf-8"))
        # candidate_id is an optional human label, not a search-space dimension.
        if isinstance(parsed, dict):
            parsed.pop("candidate_id", None)
        canonical = json.dumps(parsed, sort_keys=True, separators=(",", ":")).encode("utf-8")
    except (OSError, json.JSONDecodeError):
        canonical = path.read_bytes()
    return hashlib.sha256(canonical).hexdigest()


def _file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _seen_configs() -> list[str]:
    """Return all semantically-evaluated config hashes (accepted or rejected)."""
    data = _load_json(_paths()["seen_configs"], [])
    if isinstance(data, list):
        return [str(h) for h in data]
    return []


def _record_seen_config(candidate_hash: str) -> None:
    """Append a config hash to the seen-set (idempotent, preserves order)."""
    seen = _seen_configs()
    if candidate_hash not in seen:
        seen.append(candidate_hash)
    _write_json(_paths()["seen_configs"], seen)


def _frozen_file_hashes() -> dict[str, str]:
    """Hash reviewable evaluator inputs, excluding mutable/run artifacts."""
    files: list[Path] = []
    for relative in ("autoresearch.nemo", "requirements.txt", "configs/baseline.json"):
        path = ROOT / relative
        if path.exists() and path.is_file():
            files.append(path)
    if HERE.exists():
        files.extend(
            path
            for path in HERE.rglob("*")
            if path.is_file() and "__pycache__" not in path.parts and path.suffix in {".py", ".json", ".yml", ".yaml"}
        )
    return {str(path.relative_to(ROOT)): _file_hash(path) for path in sorted(set(files))}


def _default_state() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "started_at": _now(),
        "max_trials": _max_trials(),
        "max_repairs": _max_repairs(),
        "trial_count": 0,
        "accepted_count": 0,
        "current_trial": None,
        "incumbent": None,
        "baseline": None,
        "last_event": "init",
    }


def _load_state() -> dict[str, Any]:
    return _load_json(_paths()["state"], _default_state())


def _save_state(state: dict[str, Any]) -> None:
    _ensure_layout()
    _write_json(_paths()["state"], state)


def _trial_number(state: dict[str, Any]) -> int:
    current = state.get("current_trial")
    if isinstance(current, dict):
        return int(current.get("id") or state.get("trial_count") or 0)
    return int(state.get("trial_count") or 0)


def _trial_dir(state: dict[str, Any]) -> Path:
    return _paths()["trials"] / f"{_trial_number(state):03d}"


def _metric_path(name: str) -> Path:
    return _paths()["metrics"] / f"{name}.json"


def _copy_file(source: Path, destination: Path) -> bool:
    if not source.exists() or not source.is_file():
        return False
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return True


def _append_agent_history(entry: dict[str, Any]) -> None:
    """Write only aggregate, agent-safe information; never raw labels/test data."""
    _ensure_layout()
    with _paths()["history"].open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")


def _candidate_summary() -> dict[str, Any]:
    try:
        value = json.loads(CANDIDATE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {"valid_json": False}
    if not isinstance(value, dict):
        return {"valid_json": False}
    # XGBoost candidate.json has flat params (no nested "model" dict).
    param_keys = (
        "max_depth",
        "learning_rate",
        "n_estimators",
        "subsample",
        "colsample_bytree",
        "min_child_weight",
        "gamma",
        "reg_alpha",
        "reg_lambda",
        "early_stopping_rounds",
        "class_weight_mode",
        "feature_recipe",
        "max_delta_step",
        "grow_policy",
    )
    return {
        "valid_json": True,
        **{key: value.get(key) for key in param_keys if key in value},
    }


def _result(
    *,
    ok: bool,
    state: dict[str, Any] | None = None,
    score: float = 0.0,
    best_score: float = 0.0,
    selection_score: float = 0.0,
    confirmation_score: float = 0.0,
    repair_allowed: bool = False,
    continue_search: bool = False,
    report: str = "",
    metrics: str = "",
    log: str = "",
    candidate_hash: str | None = None,
    history_summary: str = "",
) -> dict[str, Any]:
    actual_state = state if state is not None else _load_state()
    return {
        "ok": bool(ok),
        "state_json": json.dumps(actual_state, sort_keys=True),
        "trial_count": int(actual_state.get("trial_count") or 0),
        "score": float(score),
        "best_score": float(best_score),
        "selection_score": float(selection_score),
        "confirmation_score": float(confirmation_score),
        "repair_allowed": bool(repair_allowed),
        "continue_search": bool(continue_search),
        "report": report,
        "metrics": metrics,
        "log": log,
        "candidate_hash": candidate_hash or _candidate_hash(),
        "history_summary": history_summary,
        "mutable_candidate_path": str(CANDIDATE.resolve()),
        "agent_view_path": str(_paths()["history"].parent.resolve()),
        "history_path": str(_paths()["history"].resolve()),
    }


def _emit(payload: dict[str, Any], exit_code: int = 0) -> int:
    print(json.dumps(payload, sort_keys=True), flush=True)
    return exit_code


def _require_ok_metric(name: str) -> tuple[dict[str, Any] | None, str | None]:
    value = _load_json(_metric_path(name), None)
    if not isinstance(value, dict):
        return None, f"missing metric artifact: {name}.json"
    if not bool(value.get("ok", False)):
        return value, str(value.get("error") or f"{name} failed")
    return value, None


def _float_metric(value: dict[str, Any], key: str = "score") -> float:
    try:
        return float(value.get(key, 0.0))
    except (TypeError, ValueError):
        return 0.0


def _optional_float(value: dict[str, Any], key: str) -> float | None:
    raw = value.get(key)
    if raw is None:
        return None
    try:
        result = float(raw)
    except (TypeError, ValueError):
        return None
    return result if math.isfinite(result) else None


def _artifact_error(artifact: dict[str, Any], fallback: str) -> str:
    """Extract a concise, single-line deterministic failure explanation."""
    raw = artifact.get("error") or artifact.get("report") or artifact.get("log_tail") or fallback
    return " ".join(str(raw).split())[:600]


def _format_metric(value: float | None) -> str:
    return "unavailable" if value is None else f"{value:.6f}"


def _rejection_evidence(
    state: dict[str, Any],
    *,
    preflight: dict[str, Any],
    train: dict[str, Any],
    selection: dict[str, Any],
    confirmation: dict[str, Any],
    primary: dict[str, Any],
    confirmed: dict[str, Any],
) -> dict[str, Any]:
    """Classify every workflow rejection and attach the relevant numeric facts."""
    incumbent = state.get("incumbent") if isinstance(state.get("incumbent"), dict) else {}
    current = state.get("current_trial") if isinstance(state.get("current_trial"), dict) else {}
    incumbent_selection = _optional_float(incumbent, "selection_score")
    incumbent_confirmation = _optional_float(incumbent, "confirmation_score")
    incumbent_combined = _optional_float(incumbent, "score")
    epsilon = _epsilon()

    evidence: dict[str, Any] = {
        "reason_code": "INCOMPLETE_TRIAL_EVIDENCE",
        "failed_stage": None,
        "comparison": None,
        "score": None,
        "best_score": incumbent_combined,
        "selection_score": None,
        "incumbent_selection_score": incumbent_selection,
        "confirmation_score": None,
        "incumbent_confirmation_score": incumbent_confirmation,
        "delta": None,
        "epsilon": epsilon,
        "repair_attempts": int(current.get("repair_attempts") or 0),
        "max_repairs": int(state.get("max_repairs") or 0),
    }

    if preflight and not bool(preflight.get("ok", False)):
        detail = _artifact_error(preflight, "preflight failed")
        evidence.update(
            reason_code="PREFLIGHT_FAILED_REPAIRS_EXHAUSTED",
            failed_stage="Preflight",
            reason=(
                "PREFLIGHT_FAILED_REPAIRS_EXHAUSTED: candidate was not evaluated; "
                f"score=unavailable; incumbent_combined={_format_metric(incumbent_combined)}; "
                f"repair_gate_attempt={evidence['repair_attempts']}, "
                f"allowed_repairs={evidence['max_repairs']}; error={detail}"
            ),
        )
        return evidence

    if train and not bool(train.get("ok", False)):
        detail = _artifact_error(train, "training failed")
        evidence.update(
            reason_code="TRAINING_FAILED",
            failed_stage="TrainCandidate",
            reason=(
                "TRAINING_FAILED: candidate score=unavailable because training did not complete; "
                f"incumbent_combined={_format_metric(incumbent_combined)}; error={detail}"
            ),
        )
        return evidence

    if selection and not bool(selection.get("ok", False)):
        detail = _artifact_error(selection, "selection evaluation failed")
        evidence.update(
            reason_code="SELECTION_EVALUATION_FAILED",
            failed_stage="EvaluateSelection",
            reason=(
                "SELECTION_EVALUATION_FAILED: candidate selection score=unavailable; "
                f"incumbent_selection={_format_metric(incumbent_selection)}; error={detail}"
            ),
        )
        return evidence

    if primary and not bool(primary.get("ok", False)):
        detail = _artifact_error(primary, "primary judge failed")
        evidence.update(
            reason_code="PRIMARY_JUDGE_FAILED",
            failed_stage="JudgePrimary",
            reason=(
                "PRIMARY_JUDGE_FAILED: candidate selection comparison unavailable; "
                f"incumbent_selection={_format_metric(incumbent_selection)}; error={detail}"
            ),
        )
        return evidence

    if confirmation and not bool(confirmation.get("ok", False)):
        candidate_selection = _optional_float(primary, "selection_score")
        detail = _artifact_error(confirmation, "confirmation evaluation failed")
        evidence.update(
            reason_code="CONFIRMATION_EVALUATION_FAILED",
            failed_stage="ConfirmCandidate",
            score=candidate_selection,
            best_score=incumbent_selection,
            selection_score=candidate_selection,
            comparison="confirmation_evaluation",
            reason=(
                "CONFIRMATION_EVALUATION_FAILED: primary selection improved "
                f"(candidate_selection={_format_metric(candidate_selection)}, "
                f"incumbent_selection={_format_metric(incumbent_selection)}) but confirmation score is unavailable; "
                f"error={detail}"
            ),
        )
        return evidence

    if confirmed and not bool(confirmed.get("ok", False)):
        detail = _artifact_error(confirmed, "confirmation judge failed")
        evidence.update(
            reason_code="CONFIRMATION_JUDGE_FAILED",
            failed_stage="JudgeConfirmed",
            selection_score=_optional_float(primary, "selection_score"),
            confirmation_score=_optional_float(confirmation, "score"),
            reason=(
                "CONFIRMATION_JUDGE_FAILED: combined candidate/incumbent comparison unavailable; "
                f"incumbent_combined={_format_metric(incumbent_combined)}; error={detail}"
            ),
        )
        return evidence

    if confirmed and bool(confirmed.get("ok", False)):
        candidate_score = _optional_float(confirmed, "score")
        best_score = _optional_float(confirmed, "best_score")
        candidate_selection = _optional_float(confirmed, "selection_score")
        candidate_confirmation = _optional_float(confirmed, "confirmation_score")
        incumbent_sel = _optional_float(confirmed, "incumbent_selection_score")
        incumbent_conf = _optional_float(confirmed, "incumbent_confirmation_score")
        delta = _optional_float(confirmed, "delta")
        if delta is None and candidate_score is not None and best_score is not None:
            delta = candidate_score - best_score
        threshold = best_score + epsilon if best_score is not None else None
        evidence.update(
            reason_code="CONFIRMED_IMPROVEMENT_BELOW_EPS",
            failed_stage="JudgeConfirmed",
            comparison="confirmed_combined",
            score=candidate_score,
            best_score=best_score,
            selection_score=candidate_selection,
            incumbent_selection_score=incumbent_sel,
            confirmation_score=candidate_confirmation,
            incumbent_confirmation_score=incumbent_conf,
            delta=delta,
            required_score_exclusive=threshold,
            reason=(
                "CONFIRMED_IMPROVEMENT_BELOW_EPS: compiled guard requires "
                "candidate_combined - incumbent_combined > epsilon; "
                f"candidate_combined={_format_metric(candidate_score)}, "
                f"incumbent_combined={_format_metric(best_score)}, delta={_format_metric(delta)}, "
                f"epsilon={epsilon:.6f}, required_candidate_combined>{_format_metric(threshold)}; "
                f"candidate_selection={_format_metric(candidate_selection)}, "
                f"incumbent_selection={_format_metric(incumbent_sel)}, "
                f"candidate_confirmation={_format_metric(candidate_confirmation)}, "
                f"incumbent_confirmation={_format_metric(incumbent_conf)}"
            ),
        )
        return evidence

    if primary and bool(primary.get("ok", False)):
        candidate_selection = _optional_float(primary, "selection_score")
        incumbent_sel = _optional_float(primary, "incumbent_selection_score")
        delta = _optional_float(primary, "delta")
        if delta is None and candidate_selection is not None and incumbent_sel is not None:
            delta = candidate_selection - incumbent_sel
        evidence.update(
            reason_code="PRIMARY_SELECTION_NOT_IMPROVED",
            failed_stage="JudgePrimary",
            comparison="primary_selection",
            score=candidate_selection,
            best_score=incumbent_sel,
            selection_score=candidate_selection,
            incumbent_selection_score=incumbent_sel,
            delta=delta,
            reason=(
                "PRIMARY_SELECTION_NOT_IMPROVED: compiled guard requires candidate_selection > "
                "incumbent_selection; "
                f"candidate_selection={_format_metric(candidate_selection)}, "
                f"incumbent_selection={_format_metric(incumbent_sel)}, "
                f"delta={_format_metric(delta)}; confirmation was not run"
            ),
        )
        return evidence

    evidence["reason"] = (
        "INCOMPLETE_TRIAL_EVIDENCE: rejection reached without a successful numeric judge or a "
        "recorded stage failure; candidate score=unavailable; "
        f"incumbent_combined={_format_metric(incumbent_combined)}"
    )
    return evidence


def cmd_init(_args: argparse.Namespace) -> int:
    paths = _paths()
    # The driver creates a unique run directory before event streaming begins.
    # Never recursively delete it: doing so would unlink events.jsonl.
    _ensure_layout()
    if not BASELINE_CONFIG.exists():
        return _emit(_result(ok=False, report=f"missing frozen baseline: {BASELINE_CONFIG}"), 1)

    state = _default_state()
    state["run_id"] = paths["run_dir"].name
    state["candidate_path"] = str(CANDIDATE)
    state["last_event"] = "initialized"

    # Start each run from the known frozen baseline, not a prior interactive edit.
    _copy_file(BASELINE_CONFIG, CANDIDATE)
    _copy_file(CANDIDATE, paths["initial_candidate"])
    state["initial_candidate_hash"] = _candidate_hash()
    _save_state(state)

    dataset_manifest = _load_json(Path(os.environ.get("NEMOIR_DATA_DIR", ROOT / "data")) / "dataset_manifest.json", {})
    split_manifest = _load_json(Path(os.environ.get("NEMOIR_DATA_DIR", ROOT / "data")) / "split_manifest.json", {})
    manifest = {
        "schema_version": 1,
        "created_at": _now(),
        "run_id": state["run_id"],
        "python": sys.version,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "max_trials": state["max_trials"],
        "max_repairs": state["max_repairs"],
        "eps": os.environ.get("NEMOIR_EPS"),
        "device": os.environ.get("NEMOIR_DEVICE", "cpu"),
        "n_jobs": os.environ.get("NEMOIR_N_JOBS", "1"),
        "train_timeout_seconds": os.environ.get("NEMOIR_TRAIN_TIMEOUT_SECONDS"),
        "eval_timeout_seconds": os.environ.get("NEMOIR_EVAL_TIMEOUT_SECONDS"),
        "data_dir": os.environ.get("NEMOIR_DATA_DIR", str(ROOT / "data")),
        "dataset_manifest": dataset_manifest,
        "split_manifest": split_manifest,
        "frozen_file_sha256": _frozen_file_hashes(),
        "candidate_sha256": state["initial_candidate_hash"],
    }
    _write_json(paths["manifest"], manifest)
    _append_agent_history({
        "time": _now(),
        "event": "init",
        "trial": 0,
        "candidate_hash": state["initial_candidate_hash"],
        "max_trials": state["max_trials"],
    })
    return _emit(_result(ok=True, state=state, report=f"initialized run {paths['run_dir']}"))


def cmd_adopt_baseline(_args: argparse.Namespace) -> int:
    state = _load_state()
    baseline, error = _require_ok_metric("baseline")
    if error or baseline is None:
        return _emit(_result(ok=False, state=state, report=f"cannot adopt baseline: {error}"), 1)

    selection = _float_metric(baseline, "selection_score")
    confirmation = _float_metric(baseline, "confirmation_score")
    combined = _float_metric(baseline, "score")
    if combined == 0.0 and (selection != 0.0 or confirmation != 0.0):
        combined = (selection + confirmation) / 2.0
    paths = _paths()
    source_model = paths["work"] / "baseline_model.json"
    if not source_model.exists():
        source_model = paths["models"] / "baseline_model.json"
    if not source_model.exists():
        return _emit(_result(ok=False, state=state, report="baseline model artifact missing"), 1)

    _copy_file(CANDIDATE, paths["best_candidate"])
    _copy_file(source_model, paths["baseline_model"])
    _copy_file(source_model, paths["best_model"])
    baseline_hash = _candidate_hash()
    _record_seen_config(baseline_hash)
    baseline_record = {
        "trial": 0,
        "candidate_hash": baseline_hash,
        "selection_score": selection,
        "confirmation_score": confirmation,
        "score": combined,
        "model": str(paths["baseline_model"].relative_to(paths["run_dir"])),
    }
    incumbent = dict(baseline_record)
    incumbent["model"] = str(paths["best_model"].relative_to(paths["run_dir"]))
    state["baseline"] = baseline_record
    state["incumbent"] = incumbent
    state["last_event"] = "adopt_baseline"
    _save_state(state)

    tdir = _paths()["trials"] / "000"
    tdir.mkdir(parents=True, exist_ok=True)
    _copy_file(CANDIDATE, tdir / "candidate.json")
    _copy_file(_metric_path("baseline"), tdir / "baseline_metrics.json")
    _copy_file(paths["baseline_model"], tdir / "model.json")
    _write_json(tdir / "decision.json", {"decision": "baseline", "time": _now(), "incumbent": incumbent})
    _append_agent_history({
        "time": _now(),
        "event": "baseline_adopted",
        "trial": 0,
        "candidate_hash": baseline_hash,
        "selection_score": selection,
        "confirmation_score": confirmation,
        "score": combined,
        "candidate": _candidate_summary(),
    })
    return _emit(_result(
        ok=True,
        state=state,
        score=combined,
        best_score=combined,
        selection_score=selection,
        confirmation_score=confirmation,
        report=f"adopted baseline score={combined:.6f}",
    ))


def _build_history_summary(state: dict[str, Any], *, max_entries: int = 10) -> str:
    """Build a compact agent-safe summary of recent trial history."""
    history_path = _paths()["history"]
    if not history_path.exists():
        return "No trials recorded yet."
    try:
        entries = [
            json.loads(line)
            for line in history_path.read_text(encoding="utf-8").strip().splitlines()
            if line.strip()
        ]
    except (OSError, json.JSONDecodeError):
        return "History unavailable."
    # Keep only accept/reject entries (the ones with trial/score)
    trial_entries = [e for e in entries if e.get("event") in ("accept", "reject")]
    recent = trial_entries[-max_entries:]
    if not recent:
        return "No trial results yet (baseline only)."
    parts: list[str] = []
    for e in recent:
        trial = e.get("trial", "?")
        event = e.get("event", "?")
        reason = str(e.get("reason") or "")
        candidate = e.get("candidate", {})
        # _candidate_summary stores flat params for XGBoost candidates.
        param_keys = (
            "max_depth", "learning_rate", "n_estimators", "subsample",
            "colsample_bytree", "reg_alpha", "reg_lambda", "gamma",
            "min_child_weight", "class_weight_mode", "feature_recipe",
            "max_delta_step", "grow_policy",
        )
        changed = [k for k in param_keys if k in candidate and candidate[k] is not None]
        params_str = ", ".join(f"{k}={candidate[k]}" for k in changed[:4]) if changed else ""

        if event == "reject":
            reason_code = str(e.get("reason_code") or "UNCLASSIFIED_REJECTION")
            comparison = e.get("comparison")
            score = _optional_float(e, "score")
            best = _optional_float(e, "best_score")
            delta = _optional_float(e, "delta")
            epsilon = _optional_float(e, "epsilon")
            candidate_selection = _optional_float(e, "selection_score")
            incumbent_selection = _optional_float(e, "incumbent_selection_score")
            candidate_confirmation = _optional_float(e, "confirmation_score")
            incumbent_confirmation = _optional_float(e, "incumbent_confirmation_score")
            line = f"trial {trial}: REJECT [{reason_code}]"
            if comparison == "primary_selection":
                line += (
                    f" candidate_selection={_format_metric(candidate_selection)}"
                    f" incumbent_selection={_format_metric(incumbent_selection)}"
                    f" delta={_format_metric(delta)}"
                )
            elif comparison == "confirmed_combined":
                line += (
                    f" candidate_combined={_format_metric(score)}"
                    f" incumbent_combined={_format_metric(best)}"
                    f" delta={_format_metric(delta)} epsilon={_format_metric(epsilon)}"
                    f" selection={_format_metric(candidate_selection)}/{_format_metric(incumbent_selection)}"
                    f" confirmation={_format_metric(candidate_confirmation)}/{_format_metric(incumbent_confirmation)}"
                )
            elif comparison == "confirmation_evaluation":
                line += (
                    f" candidate_selection={_format_metric(candidate_selection)}"
                    f" incumbent_selection={_format_metric(incumbent_selection)}"
                    " confirmation=unavailable"
                )
            else:
                line += f" candidate_score={_format_metric(score)} incumbent_score={_format_metric(best)}"
            if params_str:
                line += f" ({params_str})"
            if reason:
                line += f" | {reason}"
            parts.append(line)
            continue

        score = _optional_float(e, "score")
        best = _optional_float(e, "best_score")
        delta = _optional_float(e, "delta")
        line = f"trial {trial}: ACCEPT score={_format_metric(score)}"
        if best is not None:
            line += f" best={best:.6f}"
        if delta is not None:
            line += f" delta={delta:+.6f}"
        if params_str:
            line += f" ({params_str})"
        parts.append(line)
    incumbent = state.get("incumbent", {}) if isinstance(state.get("incumbent"), dict) else {}
    header = (
        f"Budget: {state.get('trial_count', 0)}/{state.get('max_trials', 0)} trials used. "
        f"Best: {float(incumbent.get('score') or 0.0):.4f} (trial {incumbent.get('trial', 0)})."
    )
    return header + "\n" + "\n".join(parts)


def cmd_start_trial(_args: argparse.Namespace) -> int:
    state = _load_state()
    incumbent = state.get("incumbent")
    if not isinstance(incumbent, dict):
        return _emit(_result(ok=False, state=state, report="no incumbent; baseline was not adopted"), 1)
    if int(state.get("trial_count") or 0) >= int(state.get("max_trials") or 0):
        return _emit(_result(ok=False, state=state, report="trial budget exhausted"), 1)

    paths = _paths()
    if not _copy_file(paths["best_candidate"], CANDIDATE):
        return _emit(_result(ok=False, state=state, report="best candidate snapshot missing"), 1)
    # Never let a failure branch consume stale metrics from a prior trial.
    for metric_name in ("preflight", "train", "selection", "confirmation", "judge_primary", "judge_confirmation"):
        stale_metric = _metric_path(metric_name)
        if stale_metric.exists():
            stale_metric.unlink()
    for work_name in ("current_model.json", "current_model.meta.json", "parent_candidate.json"):
        stale_work = paths["work"] / work_name
        if stale_work.exists():
            stale_work.unlink()
    trial_id = int(state.get("trial_count") or 0) + 1
    state["trial_count"] = trial_id
    state["current_trial"] = {
        "id": trial_id,
        "started_at": _now(),
        "parent_trial": int(incumbent.get("trial") or 0),
        "parent_candidate_hash": str(incumbent.get("candidate_hash") or ""),
        "repair_attempts": 0,
    }
    state["last_event"] = "start_trial"
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)
    _copy_file(CANDIDATE, tdir / "parent_candidate.json")
    # Preflight consumes this trusted semantic parent snapshot before training.
    _copy_file(CANDIDATE, paths["work"] / "parent_candidate.json")
    _write_json(tdir / "parent.json", dict(state["current_trial"]))
    _write_json(tdir / "environment.json", {
        "run_manifest": "../../run_manifest.json",
        "run_manifest_sha256": _file_hash(paths["manifest"]) if paths["manifest"].exists() else None,
    })
    _save_state(state)
    _append_agent_history({
        "time": _now(),
        "event": "start_trial",
        "trial": trial_id,
        "parent_trial": state["current_trial"]["parent_trial"],
        "parent_candidate_hash": state["current_trial"]["parent_candidate_hash"],
        "best_score": float(incumbent.get("score") or 0.0),
    })
    return _emit(_result(
        ok=True,
        state=state,
        best_score=float(incumbent.get("score") or 0.0),
        history_summary=_build_history_summary(state),
        report=f"started trial {trial_id}",
    ))


def cmd_repair_gate(_args: argparse.Namespace) -> int:
    state = _load_state()
    current = state.get("current_trial")
    if not isinstance(current, dict):
        return _emit(_result(ok=False, state=state, report="repair requested without active trial"), 1)
    current["repair_attempts"] = int(current.get("repair_attempts") or 0) + 1
    allowed = current["repair_attempts"] <= int(state.get("max_repairs") or 0)
    state["last_event"] = "repair_gate"
    _save_state(state)
    preflight = _load_json(_metric_path("preflight"), {})
    report = str(preflight.get("error") or preflight.get("report") or "preflight failed")
    _append_agent_history({
        "time": _now(),
        "event": "repair_gate",
        "trial": _trial_number(state),
        "attempt": current["repair_attempts"],
        "allowed": allowed,
        "reason": report,
    })
    return _emit(_result(
        ok=False,
        state=state,
        repair_allowed=allowed,
        report=f"repair {current['repair_attempts']}/{state['max_repairs']}: {report}",
    ))


def cmd_judge_primary(_args: argparse.Namespace) -> int:
    state = _load_state()
    incumbent = state.get("incumbent")
    metric, error = _require_ok_metric("selection")
    if not isinstance(incumbent, dict) or metric is None or error:
        report = error or "missing incumbent"
        _write_json(_metric_path("judge_primary"), {"ok": False, "error": report, "time": _now()})
        return _emit(_result(ok=False, state=state, report=f"primary judge failed: {report}"), 1)

    selection = _float_metric(metric)
    incumbent_selection = float(incumbent.get("selection_score") or 0.0)
    current = state.get("current_trial")
    if isinstance(current, dict):
        current["selection_score"] = selection
        current["selection_metrics"] = metric
    state["last_event"] = "judge_primary"
    _save_state(state)
    payload = {
        "ok": True,
        "time": _now(),
        "trial": _trial_number(state),
        "selection_score": selection,
        "incumbent_selection_score": incumbent_selection,
        "delta": selection - incumbent_selection,
    }
    _write_json(_metric_path("judge_primary"), payload)
    return _emit(_result(
        ok=True,
        state=state,
        score=selection,
        best_score=incumbent_selection,
        selection_score=selection,
        report=(
            f"primary score={selection:.6f} incumbent_selection={incumbent_selection:.6f} "
            f"delta={selection - incumbent_selection:+.6f}"
        ),
        metrics=json.dumps(payload, sort_keys=True),
    ))


def cmd_judge_confirm(_args: argparse.Namespace) -> int:
    state = _load_state()
    incumbent = state.get("incumbent")
    primary = _load_json(_metric_path("judge_primary"), {})
    confirmation, error = _require_ok_metric("confirmation")
    if not isinstance(incumbent, dict) or not bool(primary.get("ok")) or confirmation is None or error:
        report = error or "primary judge was not successful" if isinstance(incumbent, dict) else "missing incumbent"
        _write_json(_metric_path("judge_confirmation"), {"ok": False, "error": report, "time": _now()})
        return _emit(_result(ok=False, state=state, report=f"confirmation judge failed: {report}"), 1)

    selection = float(primary.get("selection_score") or 0.0)
    candidate_confirmation = _float_metric(confirmation)
    incumbent_selection = float(incumbent.get("selection_score") or 0.0)
    incumbent_confirmation = float(incumbent.get("confirmation_score") or 0.0)
    candidate_score = (selection + candidate_confirmation) / 2.0
    incumbent_score = (incumbent_selection + incumbent_confirmation) / 2.0
    current = state.get("current_trial")
    if isinstance(current, dict):
        current["confirmation_score"] = candidate_confirmation
        current["score"] = candidate_score
        current["confirmation_metrics"] = confirmation
    state["last_event"] = "judge_confirmation"
    _save_state(state)
    payload = {
        "ok": True,
        "time": _now(),
        "trial": _trial_number(state),
        "selection_score": selection,
        "confirmation_score": candidate_confirmation,
        "score": candidate_score,
        "incumbent_selection_score": incumbent_selection,
        "incumbent_confirmation_score": incumbent_confirmation,
        "best_score": incumbent_score,
        "delta": candidate_score - incumbent_score,
    }
    _write_json(_metric_path("judge_confirmation"), payload)
    return _emit(_result(
        ok=True,
        state=state,
        score=candidate_score,
        best_score=incumbent_score,
        selection_score=selection,
        confirmation_score=candidate_confirmation,
        report=(
            f"combined score={candidate_score:.6f} incumbent={incumbent_score:.6f} "
            f"delta={candidate_score - incumbent_score:+.6f}"
        ),
        metrics=json.dumps(payload, sort_keys=True),
    ))


def _snapshot_trial_metrics(destination: Path) -> None:
    for name in ("preflight", "train", "selection", "confirmation", "judge_primary", "judge_confirmation"):
        source = _metric_path(name)
        if source.exists():
            _copy_file(source, destination / f"{name}.json")


def cmd_accept(_args: argparse.Namespace) -> int:
    state = _load_state()
    incumbent = state.get("incumbent")
    current = state.get("current_trial")
    judge, error = _require_ok_metric("judge_confirmation")
    if not isinstance(incumbent, dict) or not isinstance(current, dict) or judge is None or error:
        report = error or "accept requires an active, confirmed trial and incumbent"
        return _emit(_result(ok=False, state=state, report=report), 1)

    paths = _paths()
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)
    current_model = paths["work"] / "current_model.json"
    if not current_model.exists():
        return _emit(_result(ok=False, state=state, report="current model artifact missing"), 1)
    _copy_file(CANDIDATE, paths["best_candidate"])
    _copy_file(CANDIDATE, tdir / "candidate.json")
    _copy_file(current_model, paths["best_model"])
    _copy_file(current_model, tdir / "model.json")
    _snapshot_trial_metrics(tdir)

    accepted_hash = _candidate_hash()
    _record_seen_config(accepted_hash)

    new_incumbent = {
        "trial": _trial_number(state),
        "candidate_hash": _candidate_hash(),
        "selection_score": float(judge.get("selection_score") or 0.0),
        "confirmation_score": float(judge.get("confirmation_score") or 0.0),
        "score": float(judge.get("score") or 0.0),
        "model": str(paths["best_model"].relative_to(paths["run_dir"])),
    }
    previous = dict(incumbent)
    state["incumbent"] = new_incumbent
    state["accepted_count"] = int(state.get("accepted_count") or 0) + 1
    state["last_event"] = "accept"
    _save_state(state)
    decision = {
        "time": _now(),
        "decision": "accept",
        "trial": _trial_number(state),
        "parent": previous,
        "incumbent": new_incumbent,
        "reason": "compiled numeric guard selected AcceptCandidate",
    }
    _write_json(tdir / "decision.json", decision)
    _append_agent_history({
        "time": _now(),
        "event": "accept",
        "trial": _trial_number(state),
        "candidate_hash": new_incumbent["candidate_hash"],
        "selection_score": new_incumbent["selection_score"],
        "confirmation_score": new_incumbent["confirmation_score"],
        "score": new_incumbent["score"],
        "candidate": _candidate_summary(),
    })
    return _emit(_result(
        ok=True,
        state=state,
        score=new_incumbent["score"],
        best_score=new_incumbent["score"],
        selection_score=new_incumbent["selection_score"],
        confirmation_score=new_incumbent["confirmation_score"],
        report=f"accepted trial {_trial_number(state)} score={new_incumbent['score']:.6f}",
    ))


def cmd_reject(_args: argparse.Namespace) -> int:
    state = _load_state()
    incumbent = state.get("incumbent")
    paths = _paths()
    tdir = _trial_dir(state)
    tdir.mkdir(parents=True, exist_ok=True)
    _copy_file(CANDIDATE, tdir / "rejected_candidate.json")
    rejected_summary = _candidate_summary()
    _snapshot_trial_metrics(tdir)

    rejected_hash = _candidate_hash(tdir / "rejected_candidate.json")
    _record_seen_config(rejected_hash)

    restored = isinstance(incumbent, dict) and _copy_file(paths["best_candidate"], CANDIDATE)
    # A trial model must never masquerade as the retained incumbent.
    for work_name in ("current_model.json", "current_model.meta.json"):
        current_model = paths["work"] / work_name
        if current_model.exists():
            current_model.unlink()
    preflight = _load_json(_metric_path("preflight"), {})
    train = _load_json(_metric_path("train"), {})
    selection = _load_json(_metric_path("selection"), {})
    confirmation = _load_json(_metric_path("confirmation"), {})
    primary = _load_json(_metric_path("judge_primary"), {})
    confirmed = _load_json(_metric_path("judge_confirmation"), {})
    evidence = _rejection_evidence(
        state,
        preflight=preflight,
        train=train,
        selection=selection,
        confirmation=confirmation,
        primary=primary,
        confirmed=confirmed,
    )
    reason = str(evidence["reason"])
    output_score = evidence.get("score")
    output_best = evidence.get("best_score")
    output_selection = evidence.get("selection_score")
    output_confirmation = evidence.get("confirmation_score")
    state["last_event"] = "reject"
    _save_state(state)
    decision = {
        "time": _now(),
        "decision": "reject",
        "trial": _trial_number(state),
        **evidence,
        "restored_incumbent": restored,
        "candidate_hash": rejected_hash,
    }
    _write_json(tdir / "decision.json", decision)
    _append_agent_history({
        "time": _now(),
        "event": "reject",
        "trial": _trial_number(state),
        **evidence,
        "restored_incumbent": restored,
        "candidate_hash": rejected_hash,
        "candidate": rejected_summary,
    })
    report = f"rejected trial {_trial_number(state)}: {reason}; incumbent_restored={restored}"
    return _emit(_result(
        ok=True,
        state=state,
        score=float(output_score) if output_score is not None else 0.0,
        best_score=float(output_best) if output_best is not None else 0.0,
        selection_score=float(output_selection) if output_selection is not None else 0.0,
        confirmation_score=float(output_confirmation) if output_confirmation is not None else 0.0,
        report=report,
        metrics=json.dumps(decision, sort_keys=True),
    ))


def cmd_should_continue(_args: argparse.Namespace) -> int:
    state = _load_state()
    continue_search = int(state.get("trial_count") or 0) < int(state.get("max_trials") or 0)
    state["last_event"] = "should_continue"
    _save_state(state)
    incumbent = state.get("incumbent") if isinstance(state.get("incumbent"), dict) else {}
    _append_agent_history({
        "time": _now(),
        "event": "should_continue",
        "trial_count": int(state.get("trial_count") or 0),
        "max_trials": int(state.get("max_trials") or 0),
        "continue_search": continue_search,
        "best_score": float(incumbent.get("score") or 0.0),
    })
    return _emit(_result(
        ok=True,
        state=state,
        best_score=float(incumbent.get("score") or 0.0),
        continue_search=continue_search,
        report=f"continue_search={continue_search}",
    ))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command", required=True)
    for name, func in (
        ("init", cmd_init),
        ("adopt-baseline", cmd_adopt_baseline),
        ("start-trial", cmd_start_trial),
        ("repair-gate", cmd_repair_gate),
        ("judge-primary", cmd_judge_primary),
        ("judge-confirm", cmd_judge_confirm),
        ("accept", cmd_accept),
        ("reject", cmd_reject),
        ("should-continue", cmd_should_continue),
    ):
        command = subs.add_parser(name)
        command.set_defaults(func=func)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        return int(args.func(args))
    except Exception as exc:  # Fail closed while preserving a machine-readable terminal line.
        state = _load_state()
        payload = _result(ok=False, state=state, report=f"state error: {type(exc).__name__}: {exc}")
        return _emit(payload, 1)


if __name__ == "__main__":
    raise SystemExit(main())
