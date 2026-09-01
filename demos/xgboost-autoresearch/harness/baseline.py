#!/usr/bin/env python3
"""Train and measure the immutable Covertype XGBoost baseline."""

from __future__ import annotations

import json
import sys
import traceback
from typing import Any

from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from harness.artifacts import compact_metrics, emit_json, work_dir, write_metric  # noqa: E402


def _failure(error: str, *, log: str = "") -> int:
    payload = {
        "ok": False,
        "stage": "baseline",
        "error": error,
        "report": f"baseline failed: {error}",
        "log": log or error,
    }
    write_metric("baseline", payload)
    emit_json({
        "ok": False,
        "score": 0.0,
        "selection_score": 0.0,
        "confirmation_score": 0.0,
        "report": payload["report"],
        "log": payload["log"],
        "metrics": compact_metrics(payload),
    })
    return 1


def main() -> int:
    try:
        from harness.config import config_hash, load_baseline, validate_candidate

        baseline = load_baseline()
        errors = validate_candidate(baseline)
        if errors:
            return _failure("baseline schema: " + "; ".join(errors))
        baseline_hash = config_hash(baseline)
    except Exception as exc:
        return _failure(f"baseline load: {type(exc).__name__}: {exc}")

    try:
        from harness.experiment import evaluate_booster, model_runtime_summary, train_booster

        model_path = work_dir() / "baseline_model.json"
        booster, facts = train_booster(baseline, model_path=model_path)
        selection = evaluate_booster(booster, baseline, split="selection")
        confirmation = evaluate_booster(booster, baseline, split="confirmation")
        selection_score = float(selection["macro_f1"])
        confirmation_score = float(confirmation["macro_f1"])
        combined_score = (selection_score + confirmation_score) / 2.0
        payload: dict[str, Any] = {
            "ok": True,
            "stage": "baseline",
            "baseline_hash": baseline_hash,
            "candidate_hash": baseline_hash,
            "candidate_id": baseline.get("candidate_id"),
            "selection_score": selection_score,
            "confirmation_score": confirmation_score,
            "score": combined_score,
            "selection_metrics": selection,
            "confirmation_metrics": confirmation,
            **facts,
            "runtime": model_runtime_summary(),
            "report": (
                f"baseline selection={selection_score:.6f} "
                f"confirmation={confirmation_score:.6f} combined={combined_score:.6f}"
            ),
        }
        write_metric("baseline", payload)
        (work_dir() / "baseline_model.meta.json").write_text(
            json.dumps({"candidate": baseline, "candidate_hash": baseline_hash, "facts": facts}, indent=2, sort_keys=True, default=str) + "\n",
            encoding="utf-8",
        )
        emit_json({
            "ok": True,
            "candidate_hash": baseline_hash,
            "score": combined_score,
            "selection_score": selection_score,
            "confirmation_score": confirmation_score,
            "report": payload["report"],
            "log": "",
            "metrics": compact_metrics(payload),
        })
        return 0
    except Exception as exc:
        return _failure(f"{type(exc).__name__}: {exc}", log=traceback.format_exc(limit=8))


if __name__ == "__main__":
    raise SystemExit(main())
