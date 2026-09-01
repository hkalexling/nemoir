"""Config loading, validation, and fixed-invariant envelope.

The candidate JSON is declarative and schema-bounded.  This module:
  - Defines the candidate schema with bounds and allowed values.
  - Validates a candidate dict against the schema.
  - Provides fixed model invariants that the candidate cannot override
    (objective, num_class, tree_method, eval_metric, seed, verbosity).
  - Reads device and nthread only from validated environment variables
    NEMOIR_DEVICE and NEMOIR_N_JOBS — the candidate cannot control these.
  - Loads candidate.json and baseline.json from the filesystem.
  - Computes a deterministic semantic config hash (excludes candidate_id).
"""

from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
from typing import Any

# ── Paths ────────────────────────────────────────────────────────────────────

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CANDIDATE_PATH = ROOT / "candidate.json"
BASELINE_PATH = ROOT / "configs" / "baseline.json"

# Allowed schema versions (currently only 1).
ALLOWED_SCHEMA_VERSIONS: set[int] = {1}


# ── Fixed model invariants ───────────────────────────────────────────────────
# The candidate controls only the parameters listed in CANDIDATE_SCHEMA.
# Everything below is sealed by the harness and cannot be changed by the
# candidate JSON.  device and nthread are resolved at build time from
# environment variables (see _resolve_device_nthread).

FIXED_INVARIANTS: dict[str, Any] = {
    "objective": "multi:softprob",
    "num_class": 7,
    "tree_method": "hist",
    "eval_metric": "mlogloss",
    "seed": 42,
    "verbosity": 0,
}


# ── Candidate schema ─────────────────────────────────────────────────────────
# Each key maps to a dict: {type, min, max, allowed, default, required}.
# Validated strictly — unknown keys, non-finite floats, and out-of-bound
# values are rejected.  schema_version must be present and equal to 1.

CANDIDATE_SCHEMA: dict[str, dict[str, Any]] = {
    "schema_version": {
        "type": "int",
        "required": True,
        "allowed_values": list(ALLOWED_SCHEMA_VERSIONS),
    },
    "candidate_id": {
        "type": "str",
        "required": False,
    },
    "feature_recipe": {
        "type": "enum",
        "allowed": ["raw_v1", "terrain_v1", "minimal_v1"],
        "default": "raw_v1",
    },
    "n_estimators": {
        "type": "int",
        "min": 10,
        "max": 2000,
        "default": 500,
    },
    "max_depth": {
        "type": "int",
        "min": 2,
        "max": 20,
        "default": 6,
    },
    "learning_rate": {
        "type": "float",
        "min": 0.001,
        "max": 0.5,
        "default": 0.1,
    },
    "subsample": {
        "type": "float",
        "min": 0.1,
        "max": 1.0,
        "default": 0.8,
    },
    "colsample_bytree": {
        "type": "float",
        "min": 0.1,
        "max": 1.0,
        "default": 0.8,
    },
    "colsample_bylevel": {
        "type": "float",
        "min": 0.1,
        "max": 1.0,
        "default": 1.0,
    },
    "colsample_bynode": {
        "type": "float",
        "min": 0.1,
        "max": 1.0,
        "default": 1.0,
    },
    "reg_alpha": {
        "type": "float",
        "min": 0.0,
        "max": 20.0,
        "default": 0.0,
    },
    "reg_lambda": {
        "type": "float",
        "min": 0.0,
        "max": 20.0,
        "default": 1.0,
    },
    "gamma": {
        "type": "float",
        "min": 0.0,
        "max": 10.0,
        "default": 0.0,
    },
    "min_child_weight": {
        "type": "float",
        "min": 0.0,
        "max": 50.0,
        "default": 1.0,
    },
    "max_bin": {
        "type": "int",
        "min": 32,
        "max": 512,
        "default": 256,
    },
    "grow_policy": {
        "type": "enum",
        "allowed": ["depthwise", "lossguide"],
        "default": "depthwise",
    },
    "early_stopping_rounds": {
        "type": "int",
        "min": 5,
        "max": 500,
        "default": 50,
    },
    "class_weight_mode": {
        "type": "enum",
        "allowed": ["none", "balanced", "balanced_sqrt"],
        "default": "none",
    },
    "max_delta_step": {
        "type": "float",
        "min": 0.0,
        "max": 10.0,
        "default": 0.0,
    },
}


# ── Internal helpers ─────────────────────────────────────────────────────────


def _resolve_device_nthread() -> tuple[str, int]:
    """Resolve device and nthread from environment variables.

    Returns:
        (device, nthread) where device is 'cpu' or 'cuda' and nthread is
        a positive integer.

    Raises:
        ValueError on invalid values.
    """
    device_raw = os.environ.get("NEMOIR_DEVICE", "cpu").strip().lower()
    if device_raw not in ("cpu", "cuda"):
        raise ValueError(
            f"NEMOIR_DEVICE must be 'cpu' or 'cuda', got {device_raw!r}"
        )

    nthread_raw = os.environ.get("NEMOIR_N_JOBS", "1").strip()
    try:
        nthread = int(nthread_raw)
    except ValueError:
        raise ValueError(
            f"NEMOIR_N_JOBS must be a positive integer, got {nthread_raw!r}"
        )
    if nthread < 1:
        raise ValueError(
            f"NEMOIR_N_JOBS must be a positive integer, got {nthread}"
        )

    return device_raw, nthread


def _is_finite(val: Any) -> bool:
    """Return True if val is a finite number (not NaN or Inf)."""
    if isinstance(val, bool):
        return False
    if isinstance(val, (int, float)):
        return math.isfinite(float(val))
    return True  # non-numeric types pass this gate


# ── Public API ───────────────────────────────────────────────────────────────


def load_candidate(path: Path | None = None) -> dict[str, Any]:
    """Load candidate JSON from path or the default CANDIDATE_PATH."""
    p = path or CANDIDATE_PATH
    if not p.exists():
        raise FileNotFoundError(f"candidate file not found: {p}")
    with open(p) as f:
        return json.load(f)


def load_baseline() -> dict[str, Any]:
    """Load the frozen baseline config."""
    if not BASELINE_PATH.exists():
        raise FileNotFoundError(f"baseline config not found: {BASELINE_PATH}")
    with open(BASELINE_PATH) as f:
        return json.load(f)


def validate_candidate(candidate: dict[str, Any]) -> list[str]:
    """Validate candidate against schema. Returns list of error messages (empty = valid).

    Checks performed:
      - schema_version is present and valid
      - No unknown keys
      - Required keys are present and non-None
      - Types match (int, float, str, enum)
      - Numeric values are finite
      - Numeric values are within [min, max] bounds
      - Enum values are in the allowed set
    """
    errors: list[str] = []

    # ── schema_version gate ──────────────────────────────────────────────
    sv = candidate.get("schema_version")
    if sv is None:
        errors.append("schema_version: required but missing")
    elif not isinstance(sv, int) or isinstance(sv, bool):
        errors.append(f"schema_version: expected int, got {type(sv).__name__}")
    elif sv not in ALLOWED_SCHEMA_VERSIONS:
        errors.append(
            f"schema_version: {sv} not in allowed versions: "
            f"{sorted(ALLOWED_SCHEMA_VERSIONS)}"
        )

    # ── Unknown keys ─────────────────────────────────────────────────────
    for key in candidate:
        if key not in CANDIDATE_SCHEMA:
            errors.append(f"unknown key: {key!r}")

    # ── Per-field validation ─────────────────────────────────────────────
    for key, spec in CANDIDATE_SCHEMA.items():
        val = candidate.get(key)

        # Search-space fields are complete by default; only explicitly optional
        # metadata such as candidate_id may be omitted.
        if spec.get("required", True) and val is None:
            errors.append(f"{key}: required but missing")
            continue

        if val is None:
            continue  # optional, skipped

        spec_type = spec["type"]

        if spec_type == "enum":
            allowed = spec["allowed"]
            if val not in allowed:
                errors.append(
                    f"{key}: {val!r} not in allowed values: {allowed}"
                )
        elif spec_type == "int":
            if not isinstance(val, int) or isinstance(val, bool):
                errors.append(f"{key}: expected int, got {type(val).__name__}")
            else:
                if not _is_finite(val):
                    errors.append(f"{key}: value must be finite, got {val}")
                else:
                    if "allowed_values" in spec and val not in spec["allowed_values"]:
                        errors.append(
                            f"{key}: {val} not in allowed values: {spec['allowed_values']}"
                        )
                    lo, hi = spec.get("min"), spec.get("max")
                    if lo is not None and val < lo:
                        errors.append(f"{key}: {val} < min {lo}")
                    if hi is not None and val > hi:
                        errors.append(f"{key}: {val} > max {hi}")
        elif spec_type == "float":
            if not isinstance(val, (int, float)) or isinstance(val, bool):
                errors.append(f"{key}: expected float, got {type(val).__name__}")
            else:
                fval = float(val)
                if not _is_finite(fval):
                    errors.append(f"{key}: value must be finite, got {fval}")
                else:
                    lo, hi = spec.get("min"), spec.get("max")
                    if lo is not None and fval < lo:
                        errors.append(f"{key}: {fval} < min {lo}")
                    if hi is not None and fval > hi:
                        errors.append(f"{key}: {fval} > max {hi}")
        elif spec_type == "str":
            if not isinstance(val, str):
                errors.append(f"{key}: expected str, got {type(val).__name__}")

    return errors


def build_xgb_params(candidate: dict[str, Any]) -> dict[str, Any]:
    """Merge validated candidate params with fixed invariants into XGBoost kwargs.

    Device and nthread are resolved from NEMOIR_DEVICE / NEMOIR_N_JOBS
    environment variables and are never read from the candidate.
    """
    params: dict[str, Any] = dict(FIXED_INVARIANTS)

    # Inject environment-controlled settings
    device, nthread = _resolve_device_nthread()
    params["device"] = device
    params["nthread"] = nthread

    # Tree / training params the candidate controls
    for key in (
        "max_depth", "learning_rate", "subsample",
        "colsample_bytree", "colsample_bylevel", "colsample_bynode",
        "reg_alpha", "reg_lambda", "gamma", "min_child_weight",
        "max_bin", "grow_policy", "max_delta_step",
    ):
        if key in candidate:
            params[key] = candidate[key]

    return params


def build_xgb_fit_kwargs(candidate: dict[str, Any]) -> dict[str, Any]:
    """Extract fit-time kwargs from candidate (bounded to schema)."""
    return {
        "n_estimators": int(candidate.get("n_estimators", 500)),
        "early_stopping_rounds": int(candidate.get("early_stopping_rounds", 50)),
    }


def resolve_class_weight(
    candidate: dict[str, Any],
    y_train: Any,
) -> Any | None:
    """Resolve class_weight_mode to deterministic per-sample weights.

    Modes:
      "none"           → None (no per-sample weighting)
      "balanced"       → inverse-frequency weights:
                          w[i] = n_samples / (n_classes * count[ y[i] ])
      "balanced_sqrt"  → sqrt of inverse-frequency weights:
                          w[i] = sqrt(n_samples / (n_classes * count[ y[i] ]))

    Returns:
        None if mode is "none", otherwise a 1-D numpy float64 array of
        per-sample weights suitable for passing to xgb.DMatrix(..., weight=w).
    """
    mode = candidate.get("class_weight_mode", "none")
    if mode == "none":
        return None

    import numpy as np

    y = np.asarray(y_train, dtype=np.int64).ravel()
    n_samples = y.shape[0]

    # Count occurrences of each class
    classes, counts = np.unique(y, return_counts=True)
    n_classes = len(classes)

    # Inverse-frequency weight per class
    #   w_c = n_samples / (n_classes * count_c)
    # Guard against zero-count classes (shouldn't happen given unique).
    with np.errstate(divide="ignore", invalid="ignore"):
        class_weight = np.where(counts > 0, n_samples / (n_classes * counts), 1.0)

    if mode == "balanced_sqrt":
        class_weight = np.sqrt(class_weight)

    # Map class label → weight
    label_to_weight: dict[int, float] = {
        int(c): float(w) for c, w in zip(classes, class_weight)
    }

    # Build per-sample weight array
    sample_weights = np.array(
        [label_to_weight.get(int(label), 1.0) for label in y],
        dtype=np.float64,
    )

    return sample_weights


def config_hash(candidate: dict[str, Any]) -> str:
    """Deterministic semantic hash of candidate config for deduplication.

    candidate_id is excluded (it is a non-semantic label).  The hash covers
    all schema-defined parameter keys plus schema_version so that the same
    parameter values under different schema versions produce different hashes.
    """
    # Build canonical dict: only schema keys, sorted, exclude candidate_id
    canonical: dict[str, Any] = {}
    for key in sorted(CANDIDATE_SCHEMA.keys()):
        if key == "candidate_id":
            continue
        if key in candidate:
            canonical[key] = candidate[key]
        elif "default" in CANDIDATE_SCHEMA[key]:
            canonical[key] = CANDIDATE_SCHEMA[key]["default"]

    raw = json.dumps(canonical, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(raw.encode()).hexdigest()
