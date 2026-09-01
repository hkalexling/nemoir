"""Data loading, provenance, and deterministic stratified partitioning.

All heavy imports are deferred so the module is safe to import without
installed ML dependencies.  The public functions accept optional array-like
arguments for unit testing with synthetic data.

Key artifacts written under NEMOIR_DATA_DIR:
  - covtype_cache.npz          raw cached Covertype arrays
  - dataset_manifest.json      content fingerprint + metadata
  - split_manifest.json        split indices keyed to dataset fingerprint
"""

from __future__ import annotations

import hashlib
import json
import os
import struct
from pathlib import Path
from typing import Any

# ── Constants ────────────────────────────────────────────────────────────────

COVTYPE_N_SAMPLES = 581012
COVTYPE_N_FEATURES = 54
COVTYPE_N_CLASSES = 7

# Default partition fractions — must sum to exactly 1.0.
PARTITION_DEFAULTS: dict[str, float] = {
    "fit": 0.60,
    "early_stop": 0.10,
    "selection": 0.15,
    "confirmation": 0.05,
    "final": 0.10,
}

# Seed for all deterministic partitioning.
PARTITION_SEED = 20240101


# ── Path helpers ─────────────────────────────────────────────────────────────


def _data_dir() -> Path:
    raw = os.environ.get("NEMOIR_DATA_DIR", "")
    if raw:
        return Path(raw)
    return Path(__file__).resolve().parent.parent / "data"


def _run_dir() -> Path:
    raw = os.environ.get("NEMOIR_RUN_DIR", "")
    if raw:
        return Path(raw)
    return Path(__file__).resolve().parent.parent / "runs" / "current"


def _cache_path() -> Path:
    return _data_dir() / "covtype_cache.npz"


def _manifest_path() -> Path:
    return _data_dir() / "dataset_manifest.json"


def _splits_manifest_path() -> Path:
    return _data_dir() / "split_manifest.json"


# ── Provenance ───────────────────────────────────────────────────────────────


def _canonical_bytes(x: Any, y: Any) -> bytes:
    """Produce deterministic canonical bytes from (X, y) for fingerprinting.

    Serializes the full float64 feature matrix and int64 label vector in a
    reproducible order using struct so that the SHA-256 digest reflects the
    exact numeric contents.
    """
    import numpy as np

    x_arr = np.asarray(x, dtype=np.float64)
    y_arr = np.asarray(y, dtype=np.int64)

    # Force C-contiguous to guarantee predictable byte-order
    x_bytes = np.ascontiguousarray(x_arr).tobytes()
    y_bytes = np.ascontiguousarray(y_arr).tobytes()

    # Prefix with shape so the hash covers dimensions
    header = struct.pack("=qqq", x_arr.shape[0], x_arr.shape[1], y_arr.shape[0])
    return header + x_bytes + y_bytes


def compute_provenance(x: Any, y: Any) -> str:
    """Compute a deterministic SHA-256 content fingerprint from (X, y).

    Arrays are canonicalized to C-contiguous float64/int64 bytes and fed to
    the digest incrementally, avoiding a second full dataset-sized byte string
    in memory.
    """
    import numpy as np

    x_arr = np.ascontiguousarray(np.asarray(x, dtype=np.float64))
    y_arr = np.ascontiguousarray(np.asarray(y, dtype=np.int64))
    if x_arr.ndim != 2 or y_arr.ndim != 1 or x_arr.shape[0] != y_arr.shape[0]:
        raise ValueError(f"invalid Covertype arrays for provenance: X={x_arr.shape}, y={y_arr.shape}")
    digest = hashlib.sha256()
    digest.update(struct.pack("=qqq", x_arr.shape[0], x_arr.shape[1], y_arr.shape[0]))
    digest.update(memoryview(x_arr).cast("B"))
    digest.update(memoryview(y_arr).cast("B"))
    return digest.hexdigest()


def _compute_quick_fingerprint(x: Any, y: Any) -> str:
    """Fast fingerprint from shape + first/last row + label distribution.

    Used as a secondary check in the manifest (not the primary hash).
    """
    import numpy as np

    x_arr = np.asarray(x, dtype=np.float64)
    y_arr = np.asarray(y, dtype=np.int64)

    parts: list[bytes] = []
    parts.append(struct.pack("=qq", x_arr.shape[0], x_arr.shape[1]))
    # First and last row of features + label histogram
    parts.append(np.ascontiguousarray(x_arr[0]).tobytes())
    parts.append(np.ascontiguousarray(x_arr[-1]).tobytes())
    parts.append(np.bincount(y_arr).tobytes())
    return hashlib.sha256(b"".join(parts)).hexdigest()[:16]


# ── Dataset manifest ─────────────────────────────────────────────────────────


def build_dataset_manifest(x: Any, y: Any, fingerprint: str) -> dict[str, Any]:
    """Build a dataset manifest dict suitable for writing to dataset_manifest.json."""
    import numpy as np
    import sklearn

    x_arr = np.asarray(x, dtype=np.float64)
    y_arr = np.asarray(y, dtype=np.int64)
    label_counts = [int(c) for c in np.bincount(y_arr)]

    return {
        "schema_version": 1,
        "name": "Covertype",
        "source": "UCI Machine Learning Repository",
        "source_url": "https://archive.ics.uci.edu/dataset/31/covertype",
        "license": "CC BY 4.0",
        "attribution": (
            "Blackard, Jock A. and Dean, Denis J. 1999. "
            "'Comparative Accuracies of Artificial Neural Networks and "
            "Discriminant Analysis in Predicting Forest Cover Types from "
            "Cartographic Variables.' Computers and Electronics in Agriculture."
        ),
        "sklearn_version": sklearn.__version__,
        "shape": list(x_arr.shape),
        "dtype": str(x_arr.dtype),
        "n_classes": COVTYPE_N_CLASSES,
        "classes": [int(i) for i in range(COVTYPE_N_CLASSES)],
        "label_counts": label_counts,
        "content_fingerprint_sha256": fingerprint,
        "quick_fingerprint": _compute_quick_fingerprint(x, y),
    }


def write_dataset_manifest(x: Any, y: Any, fingerprint: str) -> Path:
    """Write dataset_manifest.json to NEMOIR_DATA_DIR. Returns the path."""
    data_dir = _data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)
    manifest = build_dataset_manifest(x, y, fingerprint)
    manifest_path = _manifest_path()
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return manifest_path


# ── Data loading ─────────────────────────────────────────────────────────────


def load_covtype(cache: bool = True) -> tuple[Any, Any]:
    """Load the Covertype dataset, caching to NEMOIR_DATA_DIR.

    Returns (X, y) where X shape is (581012, 54) float64 and y has labels
    0..6 (shifted from the original 1..7).

    Does NOT download at import time — sklearn is imported lazily.
    On first load the raw arrays are cached and a dataset_manifest.json
    is written.
    """
    import numpy as np
    from sklearn.datasets import fetch_covtype

    cache_path = _cache_path()

    if cache and cache_path.exists():
        loaded = np.load(cache_path)
        X, y = loaded["X"], loaded["y"]
        # Validate cached shapes and the frozen 0..6 label contract.
        if X.shape != (COVTYPE_N_SAMPLES, COVTYPE_N_FEATURES):
            raise RuntimeError(
                f"Cached X has unexpected shape {X.shape}, "
                f"expected ({COVTYPE_N_SAMPLES}, {COVTYPE_N_FEATURES})"
            )
        if y.shape != (COVTYPE_N_SAMPLES,):
            raise RuntimeError(
                f"Cached y has unexpected shape {y.shape}, "
                f"expected ({COVTYPE_N_SAMPLES},)"
            )
        if int(np.min(y)) != 0 or int(np.max(y)) != COVTYPE_N_CLASSES - 1:
            raise RuntimeError("Cached labels do not satisfy the required 0..6 contract")
        # A copied cache may lack provenance; create it once, not on every trial.
        if not _manifest_path().exists():
            write_dataset_manifest(X, y, compute_provenance(X, y))
        return X, y

    data_dir = _data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)

    bundle = fetch_covtype(data_home=str(data_dir))
    X: np.ndarray = bundle.data.astype(np.float64)  # type: ignore[union-attr]
    y_raw: np.ndarray = bundle.target.astype(np.int64)  # type: ignore[union-attr]

    # Shift labels from 1..7 to 0..6 per contract.
    y = y_raw - 1

    if cache:
        np.savez_compressed(cache_path, X=X, y=y)
        # Compute content fingerprint and write manifest
        fingerprint = compute_provenance(X, y)
        write_dataset_manifest(X, y, fingerprint)

    return X, y


# ── Partitioning ─────────────────────────────────────────────────────────────


def create_splits(
    y: Any,
    fractions: dict[str, float] | None = None,
    seed: int = PARTITION_SEED,
) -> dict[str, Any]:
    """Create deterministic stratified train/early_stop/selection/confirmation/final splits.

    Args:
        y: label array (for stratification).
        fractions: dict mapping split name -> fraction. Must sum exactly to 1.0.
                   Defaults to PARTITION_DEFAULTS.
        seed: random seed for determinism.

    Returns:
        dict of split_name -> boolean mask array of length len(y).
        All masks are disjoint and together cover every sample.
    """
    from sklearn.model_selection import train_test_split
    import numpy as np

    fracs = fractions or PARTITION_DEFAULTS
    frac_sum = sum(fracs.values())
    if abs(frac_sum - 1.0) > 1e-9:
        raise ValueError(
            f"Partition fractions must sum to 1.0, got {frac_sum:.6f}: {fracs}"
        )

    n = len(np.asarray(y))
    indices = np.arange(n, dtype=np.int64)
    y_arr = np.asarray(y)

    # ── fit vs rest ──────────────────────────────────────────────────────
    rest_frac = 1.0 - fracs["fit"]
    fit_idx, rest_idx, _, y_rest = train_test_split(
        indices, y_arr,
        test_size=rest_frac,
        stratify=y,
        random_state=seed,
    )

    # ── Rest fractions relative to rest ──────────────────────────────────
    rest_total = (
        fracs["early_stop"] + fracs["selection"]
        + fracs["confirmation"] + fracs["final"]
    )

    def _rel_frac(name: str) -> float:
        return fracs[name] / rest_total if rest_total > 0 else 0.0

    # early_stop from rest
    es_frac_rel = _rel_frac("early_stop")
    es_idx, tmp_idx, y_es, y_tmp = train_test_split(
        rest_idx, y_rest,
        test_size=(1.0 - es_frac_rel),
        stratify=y_rest,
        random_state=seed + 1,
    )

    # selection from tmp
    sel_frac_rel = _rel_frac("selection")
    conf_frac_rel = _rel_frac("confirmation")
    fin_frac_rel = _rel_frac("final")
    tmp_total = sel_frac_rel + conf_frac_rel + fin_frac_rel

    sel_frac_nested = sel_frac_rel / tmp_total if tmp_total > 0 else 0.0
    sel_idx, tmp2_idx, y_sel, y_tmp2 = train_test_split(
        tmp_idx, y_tmp,
        test_size=(1.0 - sel_frac_nested),
        stratify=y_tmp,
        random_state=seed + 2,
    )

    # confirmation and final from tmp2
    total_cf = conf_frac_rel + fin_frac_rel
    conf_frac_final = conf_frac_rel / total_cf if total_cf > 0 else 0.5
    conf_idx, fin_idx, _, _ = train_test_split(
        tmp2_idx, y_tmp2,
        test_size=(1.0 - conf_frac_final),
        stratify=y_tmp2,
        random_state=seed + 3,
    )

    # Build boolean masks — every sample assigned exactly once
    masks: dict[str, np.ndarray] = {}
    for name, idx in [
        ("fit", fit_idx),
        ("early_stop", es_idx),
        ("selection", sel_idx),
        ("confirmation", conf_idx),
        ("final", fin_idx),
    ]:
        mask = np.zeros(n, dtype=bool)
        mask[idx] = True
        masks[name] = mask

    # Sanity check: all masks sum to n, no overlap
    coverage = np.zeros(n, dtype=bool)
    for mask in masks.values():
        if np.any(coverage & mask):
            raise RuntimeError("Split masks overlap — this is a bug.")
        coverage = coverage | mask
    if int(coverage.sum()) != n:
        raise RuntimeError(
            f"Split masks do not cover all {n} samples "
            f"(covered {int(coverage.sum())})"
        )

    return masks


def load_or_create_splits(y: Any) -> dict[str, Any]:
    """Load cached splits if available and consistent, otherwise create and cache.

    Splits are keyed to the dataset content fingerprint so that data changes
    automatically invalidate old splits.  The split_manifest.json records
    the fingerprint, split sizes, and index lists.
    """
    import numpy as np

    y_arr = np.asarray(y)

    # We need the dataset fingerprint — compute from the cached data.
    # This is called after load_covtype so the cache is guaranteed to exist.
    fingerprint: str | None = None
    manifest_path = _manifest_path()
    if manifest_path.exists():
        try:
            manifest = json.loads(manifest_path.read_text())
            fingerprint = manifest.get("content_fingerprint_sha256")
        except (json.JSONDecodeError, OSError):
            pass

    splits_path = _splits_manifest_path()

    if fingerprint and splits_path.exists():
        try:
            split_manifest = json.loads(splits_path.read_text())
            # Validate fingerprint match
            if split_manifest.get("dataset_fingerprint") == fingerprint:
                # Validate same total length
                n = len(y_arr)
                idx_lists = split_manifest.get("splits", {})
                total_indices = sum(len(v) for v in idx_lists.values())
                if total_indices == n:  # each sample assigned exactly once
                    masks: dict[str, np.ndarray] = {}
                    for name, idx_list in idx_lists.items():
                        mask = np.zeros(n, dtype=bool)
                        mask[np.array(idx_list, dtype=np.int64)] = True
                        masks[name] = mask
                    return masks
        except Exception:
            pass

    # Create fresh splits
    masks = create_splits(y_arr)

    # Persist split manifest keyed to fingerprint
    if fingerprint is None:
        # We have y loaded; compute fingerprint from cached data
        cache_path = _cache_path()
        if cache_path.exists():
            loaded = np.load(cache_path)
            fingerprint = compute_provenance(loaded["X"], loaded["y"])
        else:
            fingerprint = "unknown"

    _write_split_manifest(masks, fingerprint)

    return masks


def _write_split_manifest(
    masks: dict[str, Any],
    fingerprint: str,
) -> Path:
    """Write split_manifest.json to NEMOIR_DATA_DIR."""
    import numpy as np

    idx_lists: dict[str, list[int]] = {}
    sizes: dict[str, int] = {}
    for name, mask in masks.items():
        idx_arr = np.where(np.asarray(mask))[0]
        idx_lists[name] = [int(i) for i in idx_arr]
        sizes[name] = int(len(idx_arr))

    digest = hashlib.sha256()
    for name in sorted(idx_lists):
        digest.update(name.encode("utf-8"))
        digest.update(np.asarray(idx_lists[name], dtype=np.int64).tobytes())
    manifest: dict[str, Any] = {
        "schema_version": 1,
        "dataset_fingerprint": fingerprint,
        "split_indices_sha256": digest.hexdigest(),
        "split_sizes": sizes,
        "total_samples": sum(sizes.values()),
        "partition_seed": PARTITION_SEED,
        "fractions": PARTITION_DEFAULTS,
        "splits": idx_lists,
    }

    path = _splits_manifest_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return path


def get_split_data(
    X: Any,
    y: Any,
    split: str,
    masks: dict[str, Any] | None = None,
) -> tuple[Any, Any]:
    """Return (X_split, y_split) for a named split.

    Args:
        X: feature array.
        y: label array.
        split: name of the split ("fit", "early_stop", "selection",
               "confirmation", "final").
        masks: optional pre-computed split masks. If None, masks are
               created from y via create_splits.

    Returns:
        (X_split, y_split) as numpy arrays.
    """
    import numpy as np

    if masks is None:
        masks = create_splits(np.asarray(y))

    if split not in masks:
        available = sorted(masks.keys())
        raise KeyError(
            f"Unknown split {split!r}. Available: {available}"
        )

    mask = masks[split]
    X_arr = np.asarray(X)
    y_arr = np.asarray(y)
    return X_arr[mask], y_arr[mask]


# ── Backward-compatible exported name ────────────────────────────────────────
# Kept for older callers; a runtime manifest now carries the real fingerprint.
EXPECTED_PROVENANCE: str | None = None


def _main() -> int:
    """Prepare/fingerprint the public dataset outside the agent workflow."""
    import argparse

    parser = argparse.ArgumentParser(description="Prepare the frozen Covertype dataset and split manifest")
    parser.add_argument("command", choices=("prepare",), nargs="?", default="prepare")
    parser.parse_args()
    X, y = load_covtype(cache=True)
    splits = load_or_create_splits(y)
    manifest = json.loads(_manifest_path().read_text(encoding="utf-8"))
    split_manifest = json.loads(_splits_manifest_path().read_text(encoding="utf-8"))
    print(json.dumps({
        "ok": True,
        "dataset_manifest": str(_manifest_path()),
        "split_manifest": str(_splits_manifest_path()),
        "dataset_fingerprint": manifest["content_fingerprint_sha256"],
        "split_indices_sha256": split_manifest["split_indices_sha256"],
        "shape": list(X.shape),
        "split_sizes": {name: int(mask.sum()) for name, mask in splits.items()},
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
