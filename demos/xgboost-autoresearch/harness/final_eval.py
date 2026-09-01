#!/usr/bin/env python3
"""Evaluate only the frozen baseline and final incumbent on locked test data."""

from __future__ import annotations

import json
import sys
import traceback
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from harness.artifacts import emit_json, run_dir, work_dir, write_metric  # noqa: E402


def _load_json(path: Path, default: Any) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8")) if path.exists() else default
    except (OSError, json.JSONDecodeError):
        return default


def _failure(error: str, *, log: str = "") -> int:
    payload = {
        "ok": False,
        "stage": "final_eval",
        "error": error,
        "report": f"final evaluation unavailable: {error}",
        "log": log or error,
    }
    write_metric("final", payload)
    emit_json({
        "ok": False,
        "report": payload["report"],
        "log": payload["log"],
        "metrics": json.dumps(payload, sort_keys=True),
    })
    return 1


def _model_path(name: str) -> Path:
    rd = run_dir()
    preferred = rd / "models" / f"{name}_model.json"
    return preferred if preferred.exists() else work_dir() / f"{name}_model.json"


def main() -> int:
    try:
        from harness.config import config_hash, load_baseline, load_candidate, validate_candidate
        from harness.experiment import evaluate_booster, load_booster, sha256_file

        baseline_config = load_baseline()
        best_config_path = run_dir() / "best_candidate.json"
        best_config = load_candidate(best_config_path) if best_config_path.exists() else baseline_config
        for name, config in (("baseline", baseline_config), ("best", best_config)):
            errors = validate_candidate(config)
            if errors:
                return _failure(f"{name} configuration schema: {'; '.join(errors)}")

        baseline_model = _model_path("baseline")
        best_model = _model_path("best")
        if not baseline_model.exists():
            return _failure("baseline model missing; no valid baseline was adopted")
        if not best_model.exists():
            return _failure("best incumbent model missing; no valid baseline was adopted")

        baseline_hash = sha256_file(baseline_model)
        best_hash = sha256_file(best_model)
        baseline_metrics = evaluate_booster(load_booster(baseline_model), baseline_config, split="final")
        same_model = baseline_hash == best_hash
        best_metrics = baseline_metrics if same_model else evaluate_booster(
            load_booster(best_model), best_config, split="final"
        )

        state = _load_json(run_dir() / "state.json", {})
        baseline_state = state.get("baseline", {}) if isinstance(state, dict) else {}
        incumbent_state = state.get("incumbent", {}) if isinstance(state, dict) else {}
        result: dict[str, Any] = {
            "ok": True,
            "stage": "final_eval",
            "split": "final",
            "baseline": {
                "candidate_hash": config_hash(baseline_config),
                "model_sha256": baseline_hash,
                "validation_combined_score": baseline_state.get("score"),
                **baseline_metrics,
            },
            "best": {
                "candidate_hash": config_hash(best_config),
                "model_sha256": best_hash,
                "validation_combined_score": incumbent_state.get("score"),
                "same_as_baseline": same_model,
                **best_metrics,
            },
        }
        result["delta"] = {
            "macro_f1": round(float(best_metrics["macro_f1"]) - float(baseline_metrics["macro_f1"]), 6),
            "accuracy": round(float(best_metrics["accuracy"]) - float(baseline_metrics["accuracy"]), 6),
            "best_validation_to_test_gap": (
                round(float(best_metrics["macro_f1"]) - float(incumbent_state["score"]), 6)
                if isinstance(incumbent_state, dict) and incumbent_state.get("score") is not None
                else None
            ),
        }
        result["score"] = float(best_metrics["macro_f1"])
        result["report"] = (
            f"final baseline_macro_f1={baseline_metrics['macro_f1']:.6f}; "
            f"incumbent_macro_f1={best_metrics['macro_f1']:.6f}; "
            f"same_model={same_model}"
        )
        write_metric("final", result)
        emit_json({
            "ok": True,
            "score": result["score"],
            "report": result["report"],
            "log": "",
            "metrics": json.dumps(result, sort_keys=True),
        })
        return 0
    except Exception as exc:
        return _failure(f"{type(exc).__name__}: {exc}", log=traceback.format_exc(limit=8))


if __name__ == "__main__":
    raise SystemExit(main())
