"""Multiclass metrics for Covertype XGBoost evaluation.

Primary metric: macro-averaged F1 (macro-F1) across all 7 Covertype
classes.  Safeguard metrics: multiclass log loss, accuracy, and per-class
precision/recall/F1.

All functions accept array-like inputs and return plain Python types
(JSON-serializable).  Probability arrays are validated to have the
correct shape (n_samples, 7).  Per-class metrics always report all 7
classes, even if a class has zero predictions or zero ground-truth
occurrences in the split, so that rare class performance is never hidden.
"""

from __future__ import annotations

from typing import Any

N_COVTYPE_CLASSES: int = 7
LABELS: tuple[int, ...] = tuple(range(N_COVTYPE_CLASSES))


def _ensure_arrays(
    y_true: Any,
    y_pred: Any,
    y_proba: Any = None,
):
    """Convert inputs to numpy arrays, validating shapes.

    y_proba must be (n_samples, 7) float64.
    y_true and y_pred must be 1-D int64 of equal length.
    """
    import numpy as np

    yt = np.asarray(y_true, dtype=np.int64).ravel()

    if np.any((yt < 0) | (yt >= N_COVTYPE_CLASSES)):
        raise ValueError("y_true contains labels outside the fixed 0..6 Covertype contract")

    yp = None
    if y_pred is not None:
        yp = np.asarray(y_pred, dtype=np.int64).ravel()
        if yt.shape[0] != yp.shape[0]:
            raise ValueError(
                f"y_true length {yt.shape[0]} != y_pred length {yp.shape[0]}"
            )
        if np.any((yp < 0) | (yp >= N_COVTYPE_CLASSES)):
            raise ValueError("y_pred contains labels outside the fixed 0..6 Covertype contract")

    y_proba_arr = None
    if y_proba is not None:
        y_proba_arr = np.asarray(y_proba, dtype=np.float64)
        if y_proba_arr.ndim != 2:
            raise ValueError(
                f"y_proba must be 2-D, got shape {y_proba_arr.shape}"
            )
        if y_proba_arr.shape[0] != yt.shape[0]:
            raise ValueError(
                f"y_proba n_samples {y_proba_arr.shape[0]} != "
                f"y_true length {yt.shape[0]}"
            )
        if y_proba_arr.shape[1] != N_COVTYPE_CLASSES:
            raise ValueError(
                f"y_proba must have {N_COVTYPE_CLASSES} columns "
                f"(got {y_proba_arr.shape[1]}) for Covertype 7-class task"
            )
        # Check that probabilities sum to ~1 per row (soft tolerance)
        row_sums = y_proba_arr.sum(axis=1)
        if not np.allclose(row_sums, 1.0, atol=1e-4):
            max_deviation = float(np.max(np.abs(row_sums - 1.0)))
            raise ValueError(
                f"y_proba rows do not sum to 1 "
                f"(max deviation: {max_deviation:.6f})"
            )

    return yt, yp, y_proba_arr


def _all_labels() -> list[int]:
    """Return the full label set [0..6] for Covertype."""
    return list(LABELS)


def accuracy(y_true: Any, y_pred: Any) -> float:
    """Compute multiclass accuracy."""
    yt, yp, _ = _ensure_arrays(y_true, y_pred)
    correct = int((yt == yp).sum())
    total = int(yt.shape[0])
    return correct / total if total > 0 else 0.0


def macro_f1(y_true: Any, y_pred: Any) -> float:
    """Compute macro-averaged F1 score across all 7 Covertype classes.

    All seven fixed task classes contribute. Classes with zero support receive
    F1=0.0, making the behavior explicit even in small synthetic test slices.
    """
    yt, yp, _ = _ensure_arrays(y_true, y_pred)
    import numpy as np

    scores: list[float] = []
    for label in _all_labels():
        tp = int(np.sum((yt == label) & (yp == label)))
        fp = int(np.sum((yt != label) & (yp == label)))
        fn = int(np.sum((yt == label) & (yp != label)))
        precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        f1 = (
            2.0 * precision * recall / (precision + recall)
            if (precision + recall) > 0
            else 0.0
        )
        scores.append(f1)

    return float(np.mean(scores)) if scores else 0.0


def per_class_metrics(y_true: Any, y_pred: Any) -> list[dict[str, Any]]:
    """Compute per-class precision, recall, F1, and support.

    Always reports all 7 Covertype classes (0..6).  Classes absent from
    both y_true and y_pred get zero support and NaN-safe zero scores.
    """
    yt, yp, _ = _ensure_arrays(y_true, y_pred)
    import numpy as np

    result: list[dict[str, Any]] = []
    for label in _all_labels():
        tp = int(np.sum((yt == label) & (yp == label)))
        fp = int(np.sum((yt != label) & (yp == label)))
        fn = int(np.sum((yt == label) & (yp != label)))
        support = int(np.sum(yt == label))

        precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        f1 = (
            2.0 * precision * recall / (precision + recall)
            if (precision + recall) > 0
            else 0.0
        )

        result.append({
            "class": label,
            "precision": round(float(precision), 6),
            "recall": round(float(recall), 6),
            "f1": round(float(f1), 6),
            "support": support,
        })

    return result


def multiclass_log_loss(y_true: Any, y_proba: Any) -> float:
    """Compute multiclass log loss (cross-entropy).

    Clips probabilities to [1e-15, 1 - 1e-15] for numerical stability.
    Requires y_proba with shape (n_samples, 7).
    """
    import numpy as np

    yt, _, yp = _ensure_arrays(y_true, None, y_proba)
    if yp is None:
        raise ValueError("y_proba is required for log loss")

    n, n_classes = yp.shape
    if n_classes != N_COVTYPE_CLASSES:
        raise ValueError(
            f"y_proba must have {N_COVTYPE_CLASSES} columns, got {n_classes}"
        )

    # Clip for numerical stability
    yp_clipped = np.clip(yp, 1e-15, 1.0 - 1e-15)

    # One-hot encode y_true
    y_onehot = np.zeros((n, n_classes), dtype=np.float64)
    y_onehot[np.arange(n), yt] = 1.0

    # Log loss per sample
    log_losses = -np.sum(y_onehot * np.log(yp_clipped), axis=1)
    return float(np.mean(log_losses))


def compute_all_metrics(
    y_true: Any,
    y_pred: Any,
    y_proba: Any = None,
) -> dict[str, Any]:
    """Compute all metrics and return a JSON-serializable dict.

    Args:
        y_true: ground-truth labels (int, shape (n,)).
        y_pred: predicted labels (int, shape (n,)).
        y_proba: predicted probabilities (float, shape (n, 7)).
                 Required for log_loss; optional otherwise.

    Returns:
        dict with keys: ok, n_samples, n_classes, macro_f1, accuracy,
        per_class (list of 7 dicts), and optionally log_loss.
    """
    yt, yp, yprob = _ensure_arrays(y_true, y_pred, y_proba)

    result: dict[str, Any] = {
        "ok": True,
        "n_samples": int(yt.shape[0]),
        "n_classes": N_COVTYPE_CLASSES,
        "macro_f1": round(macro_f1(yt, yp), 6),
        "accuracy": round(accuracy(yt, yp), 6),
        "per_class": per_class_metrics(yt, yp),
    }

    if yprob is not None:
        result["log_loss"] = round(multiclass_log_loss(yt, yprob), 6)

    return result
