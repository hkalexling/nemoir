#!/usr/bin/env python3
"""Validate a candidate and frozen Covertype experiment before training.

This program is part of the trusted evaluator.  It writes
``metrics/preflight.json`` on both success and failure and emits one final JSON
line for the deterministic NemoIR stage.
"""

from __future__ import annotations

import json
import os
import sys
import traceback
from pathlib import Path
from typing import Any, Callable

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from harness.artifacts import compact_metrics, emit_json, run_dir, write_metric  # noqa: E402


def _record(checks: list[dict[str, Any]], name: str, fn: Callable[[], str]) -> bool:
    try:
        detail = fn()
    except Exception as exc:  # A failed preflight check is data, not a process crash.
        checks.append({"check": name, "ok": False, "error": f"{type(exc).__name__}: {exc}"})
        print(f"[preflight] FAIL {name}: {type(exc).__name__}: {exc}", flush=True)
        return False
    checks.append({"check": name, "ok": True, "detail": detail})
    print(f"[preflight] OK   {name}: {detail}", flush=True)
    return True


def _candidate() -> tuple[dict[str, Any], str]:
    from harness.config import CANDIDATE_PATH, config_hash, load_candidate, validate_candidate

    if CANDIDATE_PATH.is_symlink() or not CANDIDATE_PATH.is_file():
        raise ValueError("candidate.json must be a regular, non-symlink file")
    candidate = load_candidate()
    errors = validate_candidate(candidate)
    if errors:
        raise ValueError("; ".join(errors))
    return candidate, config_hash(candidate)


def _baseline() -> str:
    from harness.config import load_baseline, validate_candidate

    errors = validate_candidate(load_baseline())
    if errors:
        raise ValueError("; ".join(errors))
    return "baseline schema valid"


def _feature_recipe(candidate: dict[str, Any]) -> str:
    import numpy as np

    from harness.features import feature_count, get_recipe

    recipe = str(candidate["feature_recipe"])
    transformed = get_recipe(recipe)(np.zeros((2, 54), dtype=np.float32))
    expected = feature_count(recipe)
    if transformed.shape != (2, expected):
        raise ValueError(f"recipe produced {transformed.shape}, expected (2, {expected})")
    return f"{recipe} ({expected} features)"


def _data_and_splits() -> str:
    import numpy as np

    from harness.data import compute_provenance, load_covtype, load_or_create_splits

    X, y = load_covtype(cache=True)
    fingerprint = compute_provenance(X, y)
    data_manifest_path = Path(os.environ.get("NEMOIR_DATA_DIR", str(ROOT / "data"))) / "dataset_manifest.json"
    data_manifest = json.loads(data_manifest_path.read_text(encoding="utf-8"))
    if data_manifest.get("content_fingerprint_sha256") != fingerprint:
        raise ValueError("cached Covertype content fingerprint differs from dataset_manifest.json")
    splits = load_or_create_splits(y)
    if X.shape[0] != len(y):
        raise ValueError(f"X/y size mismatch: {X.shape[0]} != {len(y)}")
    occupied = np.zeros(len(y), dtype=bool)
    for name in ("fit", "early_stop", "selection", "confirmation", "final"):
        mask = splits.get(name)
        if mask is None or len(mask) != len(y):
            raise ValueError(f"missing or malformed split: {name}")
        if bool((occupied & mask).any()):
            raise ValueError(f"split overlap detected at {name}")
        occupied |= mask
    if not bool(occupied.all()):
        raise ValueError(f"splits leave {int((~occupied).sum())} rows unassigned")
    return f"rows={len(y)} fingerprint={fingerprint}"


def _frozen_integrity() -> str:
    manifest_path = run_dir() / "run_manifest.json"
    if not manifest_path.exists():
        return "run manifest not created yet"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    expected = manifest.get("frozen_file_sha256")
    if not isinstance(expected, dict):
        raise ValueError("run manifest lacks frozen_file_sha256")
    # state.py is a trusted harness module and owns the frozen file list.
    from harness.state import _frozen_file_hashes

    actual = _frozen_file_hashes()
    changed = [path for path, digest in expected.items() if actual.get(path) != digest]
    if changed:
        raise ValueError(f"frozen evaluator changed after initialization: {', '.join(changed)}")
    return f"{len(expected)} frozen files match launch manifest"


def _candidate_delta(candidate: dict[str, Any]) -> str:
    from harness.config import config_hash

    parent_path = run_dir() / "work" / "parent_candidate.json"
    if not parent_path.exists():
        return "baseline/no active parent"
    parent = json.loads(parent_path.read_text(encoding="utf-8"))
    if config_hash(parent) == config_hash(candidate):
        raise ValueError("candidate is semantically identical to its incumbent parent")
    return f"parent={config_hash(parent)} candidate={config_hash(candidate)}"


def _candidate_novelty(candidate: dict[str, Any]) -> str:
    """Reject any candidate whose semantic hash matches a prior evaluated trial.

    This is the key compiled constraint: the prompt asks the model not to repeat
    rejected configurations, but only an inspectable deterministic check can
    enforce it. The seen-set is maintained by state.py (init/adopt seeds with
    the baseline; accept/reject append the evaluated hash).
    """
    from harness.config import config_hash
    from harness.state import _seen_configs

    current_hash = config_hash(candidate)
    seen = _seen_configs()
    if current_hash in seen:
        raise ValueError(
            f"candidate hash {current_hash[:16]}... already evaluated in a prior trial "
            f"(checked against {len(seen)} configs). Propose a different configuration."
        )
    return f"novel: {current_hash[:16]}... (checked against {len(seen)} prior configs)"


def main() -> int:
    checks: list[dict[str, Any]] = []
    candidate: dict[str, Any] | None = None
    candidate_hash = ""

    def check_candidate() -> str:
        nonlocal candidate, candidate_hash
        candidate, candidate_hash = _candidate()
        return candidate_hash

    all_ok = True
    all_ok &= _record(checks, "candidate schema", check_candidate)
    all_ok &= _record(checks, "baseline schema", _baseline)
    if candidate is not None:
        all_ok &= _record(checks, "feature recipe", lambda: _feature_recipe(candidate or {}))
        all_ok &= _record(checks, "candidate differs from parent", lambda: _candidate_delta(candidate or {}))
        all_ok &= _record(checks, "candidate is novel (no prior trial repeat)", lambda: _candidate_novelty(candidate or {}))
    else:
        all_ok = False
        checks.append({"check": "feature recipe", "ok": False, "error": "candidate unavailable"})
    all_ok &= _record(checks, "dataset and locked splits", _data_and_splits)
    all_ok &= _record(checks, "frozen evaluator integrity", _frozen_integrity)

    failed = [entry for entry in checks if not entry["ok"]]
    report = "preflight passed" if all_ok else f"preflight failed: {len(failed)}/{len(checks)} checks"
    result: dict[str, Any] = {
        "ok": bool(all_ok),
        "stage": "preflight",
        "candidate_hash": candidate_hash,
        "checks_total": len(checks),
        "checks_ok": len(checks) - len(failed),
        "checks_fail": len(failed),
        "checks": checks,
        "report": report,
    }
    if failed:
        result["error"] = "; ".join(str(entry["error"]) for entry in failed)
    result["log"] = result.get("error", report)
    write_metric("preflight", result)
    terminal = {
        "ok": result["ok"],
        "candidate_hash": candidate_hash,
        "report": report,
        "log": result["log"],
        "metrics": compact_metrics(result),
    }
    emit_json(terminal)
    return 0 if all_ok else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # pragma: no cover - last-resort process boundary
        traceback.print_exc()
        result = {"ok": False, "stage": "preflight", "error": f"fatal: {type(exc).__name__}: {exc}"}
        write_metric("preflight", result)
        emit_json({"ok": False, "report": result["error"], "log": result["error"], "metrics": compact_metrics(result)})
        raise SystemExit(1)
