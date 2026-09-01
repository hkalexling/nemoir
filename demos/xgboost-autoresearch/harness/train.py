#!/usr/bin/env python3
"""Train the current declarative XGBoost candidate with frozen protocol."""

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

from harness.artifacts import compact_metrics, emit_json, work_dir, write_metric  # noqa: E402


def _failure(error: str, *, candidate_hash: str = "", log: str = "") -> int:
    payload = {
        "ok": False,
        "stage": "train",
        "candidate_hash": candidate_hash,
        "error": error,
        "report": f"training failed: {error}",
        "log": log or error,
    }
    write_metric("train", payload)
    emit_json({
        "ok": False,
        "candidate_hash": candidate_hash,
        "report": payload["report"],
        "log": payload["log"],
        "metrics": compact_metrics(payload),
    })
    return 1


def main() -> int:
    try:
        from harness.config import config_hash, load_candidate, validate_candidate

        candidate = load_candidate()
        errors = validate_candidate(candidate)
        if errors:
            return _failure("candidate schema: " + "; ".join(errors))
        candidate_hash = config_hash(candidate)
    except Exception as exc:
        return _failure(f"candidate load: {type(exc).__name__}: {exc}")

    try:
        from harness.experiment import evaluate_booster, model_runtime_summary, train_booster

        model_path = work_dir() / "current_model.json"
        booster, facts = train_booster(candidate, model_path=model_path)
        train_metrics = evaluate_booster(booster, candidate, split="fit")
        payload: dict[str, Any] = {
            **train_metrics,
            **facts,
            "ok": True,
            "stage": "train",
            "candidate_hash": candidate_hash,
            "candidate_id": candidate.get("candidate_id"),
            "report": (
                f"trained candidate {candidate_hash}: iteration={facts['best_iteration']} "
                f"fit_macro_f1={train_metrics['macro_f1']:.6f}"
            ),
            "runtime": model_runtime_summary(),
        }
        write_metric("train", payload)
        (work_dir() / "current_model.meta.json").write_text(
            json.dumps({
                "candidate": candidate,
                "candidate_hash": candidate_hash,
                "facts": facts,
            }, indent=2, sort_keys=True, default=str) + "\n",
            encoding="utf-8",
        )
        emit_json({
            "ok": True,
            "candidate_hash": candidate_hash,
            "score": train_metrics["macro_f1"],
            "report": payload["report"],
            "log": "",
            "metrics": compact_metrics(payload),
        })
        return 0
    except Exception as exc:
        return _failure(
            f"{type(exc).__name__}: {exc}",
            candidate_hash=candidate_hash,
            log=traceback.format_exc(limit=8),
        )


if __name__ == "__main__":
    raise SystemExit(main())
