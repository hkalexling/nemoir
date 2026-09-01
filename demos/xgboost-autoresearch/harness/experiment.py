"""Frozen XGBoost experiment primitives shared by baseline/train/evaluation scripts."""

from __future__ import annotations

import hashlib
import os
import time
from pathlib import Path
from typing import Any


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_experiment_data(candidate: dict[str, Any]) -> tuple[Any, Any, dict[str, Any], str, int]:
    """Load frozen data/splits and apply a vetted, label-independent recipe."""
    import numpy as np

    from harness.data import load_covtype, load_or_create_splits
    from harness.features import feature_count, get_recipe

    recipe_name = str(candidate["feature_recipe"])
    X_raw, y = load_covtype(cache=True)
    X = get_recipe(recipe_name)(X_raw)
    X_arr = np.asarray(X, dtype=np.float32)
    y_arr = np.asarray(y, dtype=np.int64)
    expected_features = feature_count(recipe_name)
    if X_arr.ndim != 2 or X_arr.shape[0] != len(y_arr) or X_arr.shape[1] != expected_features:
        raise ValueError(
            f"invalid recipe output for {recipe_name}: X={X_arr.shape}, y={y_arr.shape}, "
            f"expected features={expected_features}"
        )
    return X_arr, y_arr, load_or_create_splits(y_arr), recipe_name, expected_features


def train_booster(
    candidate: dict[str, Any],
    *,
    model_path: Path,
) -> tuple[Any, dict[str, Any]]:
    """Train one frozen-family model and return its Booster plus evidence facts."""
    import xgboost as xgb

    from harness.config import build_xgb_fit_kwargs, build_xgb_params, resolve_class_weight

    X, y, masks, recipe_name, n_features = load_experiment_data(candidate)
    X_fit, y_fit = X[masks["fit"]], y[masks["fit"]]
    X_early, y_early = X[masks["early_stop"]], y[masks["early_stop"]]
    sample_weight = resolve_class_weight(candidate, y_fit)
    params = build_xgb_params(candidate)
    fit_kwargs = build_xgb_fit_kwargs(candidate)

    dtrain = xgb.DMatrix(X_fit, label=y_fit, weight=sample_weight)
    dearly = xgb.DMatrix(X_early, label=y_early)
    evals_result: dict[str, dict[str, list[float]]] = {}
    started = time.perf_counter()
    booster = xgb.train(
        params=params,
        dtrain=dtrain,
        num_boost_round=fit_kwargs["n_estimators"],
        evals=[(dtrain, "fit"), (dearly, "early_stop")],
        early_stopping_rounds=fit_kwargs["early_stopping_rounds"],
        evals_result=evals_result,
        verbose_eval=False,
    )
    elapsed = time.perf_counter() - started
    model_path.parent.mkdir(parents=True, exist_ok=True)
    booster.save_model(str(model_path))
    best_iteration = int(getattr(booster, "best_iteration", fit_kwargs["n_estimators"] - 1))
    best_score_raw = getattr(booster, "best_score", None)
    try:
        best_score = float(best_score_raw) if best_score_raw is not None else None
    except (TypeError, ValueError):
        best_score = None
    facts: dict[str, Any] = {
        "recipe": recipe_name,
        "n_features": n_features,
        "n_fit": int(len(y_fit)),
        "n_early_stop": int(len(y_early)),
        "best_iteration": best_iteration,
        "best_mlogloss": best_score,
        "elapsed_seconds": round(elapsed, 6),
        "xgboost_version": xgb.__version__,
        "device": params["device"],
        "nthread": params["nthread"],
        "class_weight_mode": candidate["class_weight_mode"],
        "sample_weighted": sample_weight is not None,
        "model_sha256": sha256_file(model_path),
    }
    return booster, facts


def load_booster(path: Path) -> Any:
    import xgboost as xgb

    if not path.exists():
        raise FileNotFoundError(f"model artifact not found: {path}")
    booster = xgb.Booster()
    booster.load_model(str(path))
    return booster


def evaluate_booster(
    booster: Any,
    candidate: dict[str, Any],
    *,
    split: str,
) -> dict[str, Any]:
    """Evaluate a Booster on one locked split using frozen metrics."""
    import numpy as np
    import xgboost as xgb

    from harness.metrics import compute_all_metrics

    X, y, masks, recipe_name, n_features = load_experiment_data(candidate)
    if split not in masks:
        raise KeyError(f"unknown split {split!r}")
    X_split, y_split = X[masks[split]], y[masks[split]]
    probabilities = booster.predict(xgb.DMatrix(X_split))
    predictions = np.argmax(probabilities, axis=1).astype(np.int64)
    metrics = compute_all_metrics(y_split, predictions, probabilities)
    metrics.update(
        {
            "split": split,
            "recipe": recipe_name,
            "n_features": n_features,
        }
    )
    return metrics


def model_runtime_summary() -> dict[str, Any]:
    """Return non-secret environment facts for provenance evidence."""
    import platform
    import xgboost as xgb

    build_info: Any
    try:
        build_info = xgb.build_info()
    except Exception:
        build_info = {}
    return {
        "python": platform.python_version(),
        "platform": platform.platform(),
        "xgboost_version": xgb.__version__,
        "xgboost_build_info": build_info,
        "device": os.environ.get("NEMOIR_DEVICE", "cpu"),
        "n_jobs": os.environ.get("NEMOIR_N_JOBS", "1"),
    }
