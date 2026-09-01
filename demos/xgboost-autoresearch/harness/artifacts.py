"""Shared artifact and JSON-line helpers for the trusted XGBoost harness."""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent


def run_dir() -> Path:
    raw = os.environ.get("NEMOIR_RUN_DIR", "")
    return Path(raw) if raw else ROOT / "runs" / "current"


def work_dir() -> Path:
    path = run_dir() / "work"
    path.mkdir(parents=True, exist_ok=True)
    return path


def metrics_dir() -> Path:
    path = run_dir() / "metrics"
    path.mkdir(parents=True, exist_ok=True)
    return path


def atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w", encoding="utf-8", dir=path.parent, prefix=f".{path.name}.", delete=False
    ) as handle:
        json.dump(payload, handle, indent=2, sort_keys=True, default=str)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
        temp_path = Path(handle.name)
    temp_path.replace(path)


def write_metric(name: str, payload: dict[str, Any]) -> Path:
    path = metrics_dir() / f"{name}.json"
    atomic_write_json(path, payload)
    return path


def emit_json(payload: dict[str, Any]) -> None:
    """Emit exactly one machine-readable terminal line for harness_tools.py."""
    print(json.dumps(payload, sort_keys=True, default=str), flush=True)


def compact_metrics(payload: dict[str, Any]) -> str:
    """Return a bounded deterministic summary suitable for workflow stage output."""
    keys = (
        "ok",
        "stage",
        "split",
        "candidate_hash",
        "macro_f1",
        "accuracy",
        "log_loss",
        "n_samples",
        "best_iteration",
        "elapsed_seconds",
        "error",
    )
    return json.dumps({key: payload[key] for key in keys if key in payload}, sort_keys=True, default=str)
