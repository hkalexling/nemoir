#!/usr/bin/env python3
"""Evaluate a frozen XGBoost model on a locked non-final split."""

from __future__ import annotations

import argparse
import sys
import traceback
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from harness.artifacts import compact_metrics, emit_json, work_dir, write_metric  # noqa: E402


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--split", choices=("selection", "confirmation"), required=True)
    parser.add_argument("--model", choices=("current", "baseline"), default="current")
    return parser


def _failure(split: str, error: str, *, log: str = "") -> int:
    payload = {
        "ok": False,
        "stage": "evaluate",
        "split": split,
        "error": error,
        "report": f"{split} evaluation failed: {error}",
        "log": log or error,
    }
    write_metric(split, payload)
    emit_json({
        "ok": False,
        "score": 0.0,
        "report": payload["report"],
        "log": payload["log"],
        "metrics": compact_metrics(payload),
    })
    return 1


def main() -> int:
    args = _parser().parse_args()
    try:
        from harness.config import config_hash, load_baseline, load_candidate, validate_candidate
        from harness.experiment import evaluate_booster, load_booster

        candidate = load_candidate() if args.model == "current" else load_baseline()
        errors = validate_candidate(candidate)
        if errors:
            return _failure(args.split, "configuration schema: " + "; ".join(errors))
        candidate_hash = config_hash(candidate)
        model_path = work_dir() / ("current_model.json" if args.model == "current" else "baseline_model.json")
        booster = load_booster(model_path)
        metrics = evaluate_booster(booster, candidate, split=args.split)
        score = float(metrics["macro_f1"])
        payload: dict[str, Any] = {
            **metrics,
            "ok": True,
            "stage": "evaluate",
            "model": args.model,
            "candidate_hash": candidate_hash,
            "candidate_id": candidate.get("candidate_id"),
            "score": score,
            "report": f"{args.split} macro_f1={score:.6f} for {candidate_hash}",
        }
        write_metric(args.split, payload)
        emit_json({
            "ok": True,
            "candidate_hash": candidate_hash,
            "score": score,
            "report": payload["report"],
            "log": "",
            "metrics": compact_metrics(payload),
        })
        return 0
    except Exception as exc:
        return _failure(args.split, f"{type(exc).__name__}: {exc}", log=traceback.format_exc(limit=8))


if __name__ == "__main__":
    raise SystemExit(main())
