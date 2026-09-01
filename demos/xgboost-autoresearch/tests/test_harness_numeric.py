"""Tests for metrics, features, and data split logic.

Uses synthetic labels so no real data download is required.  All
heavy-dependency imports are gated with ``pytest.importorskip`` so
the suite stays runnable in a runtime-only environment.
"""

from __future__ import annotations

from typing import Any

import pytest

np = pytest.importorskip("numpy")
sklearn = pytest.importorskip("sklearn")


# ---------------------------------------------------------------------------
# Metrics tests
# ---------------------------------------------------------------------------


class TestMetrics:
    """Tests for harness/metrics.py using synthetic 7-class data."""

    @staticmethod
    def _perfect_7class(n: int = 100) -> tuple[Any, Any]:
        """Return y_true, y_pred that are identical across all 7 classes."""
        rng = np.random.default_rng(42)
        labels = rng.integers(0, 7, size=n)
        return labels, labels.copy()

    @staticmethod
    def _random_7class(n: int = 200) -> tuple[Any, Any]:
        """Return independent random y_true, y_pred."""
        rng = np.random.default_rng(99)
        y_true = rng.integers(0, 7, size=n)
        y_pred = rng.integers(0, 7, size=n)
        return y_true, y_pred

    def test_import_metrics(self) -> None:
        from harness.metrics import N_COVTYPE_CLASSES, compute_all_metrics, macro_f1

        assert N_COVTYPE_CLASSES == 7
        assert callable(compute_all_metrics)
        assert callable(macro_f1)

    def test_macro_f1_perfect(self) -> None:
        from harness.metrics import macro_f1

        y_true, y_pred = self._perfect_7class()
        score = macro_f1(y_true, y_pred)
        assert score == pytest.approx(1.0)

    def test_macro_f1_range(self) -> None:
        from harness.metrics import macro_f1

        y_true, y_pred = self._random_7class()
        score = macro_f1(y_true, y_pred)
        assert 0.0 <= score <= 1.0

    def test_accuracy_perfect(self) -> None:
        from harness.metrics import accuracy

        y_true, y_pred = self._perfect_7class()
        assert accuracy(y_true, y_pred) == pytest.approx(1.0)

    def test_accuracy_zero_on_all_wrong(self) -> None:
        from harness.metrics import accuracy

        y_true = np.array([0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6])
        y_pred = np.array([1, 2, 3, 4, 5, 6, 0, 2, 3, 4, 5, 6, 0, 1])
        # Every prediction is intentionally wrong
        assert accuracy(y_true, y_pred) == 0.0

    def test_multiclass_log_loss_perfect(self) -> None:
        from harness.metrics import multiclass_log_loss

        n = 21
        y_true = np.array([i % 7 for i in range(n)])
        # perfect one-hot
        y_proba = np.zeros((n, 7), dtype=np.float64)
        y_proba[np.arange(n), y_true] = 1.0
        loss = multiclass_log_loss(y_true, y_proba)
        # clipping at 1e-15 means loss is very small but not exactly 0
        assert loss < 1e-10

    def test_multiclass_log_loss_uniform(self) -> None:
        from harness.metrics import multiclass_log_loss

        n = 7
        y_true = np.arange(n)
        y_proba = np.full((n, 7), 1.0 / 7.0, dtype=np.float64)
        loss = multiclass_log_loss(y_true, y_proba)
        expected = -np.log(1.0 / 7.0)
        assert loss == pytest.approx(expected, rel=1e-6)

    def test_multiclass_log_loss_requires_7_columns(self) -> None:
        from harness.metrics import multiclass_log_loss

        with pytest.raises(ValueError, match="7 columns"):
            multiclass_log_loss(np.array([0, 1]), np.eye(2))

    def test_multiclass_log_loss_requires_probability_rows_sum_to_one(self) -> None:
        from harness.metrics import multiclass_log_loss

        y_true = np.array([0, 1])
        y_proba_bad = np.full((2, 7), 0.5)
        with pytest.raises(ValueError, match="sum to 1"):
            multiclass_log_loss(y_true, y_proba_bad)

    def test_per_class_metrics_always_7(self) -> None:
        from harness.metrics import per_class_metrics

        y_true, y_pred = self._perfect_7class(70)
        result = per_class_metrics(y_true, y_pred)
        assert len(result) == 7
        classes = {r["class"] for r in result}
        assert classes == {0, 1, 2, 3, 4, 5, 6}

    def test_per_class_metrics_zero_support_class(self) -> None:
        """Class absent from y_true gets zero support and NaN-safe zero scores."""
        from harness.metrics import per_class_metrics

        y_true = np.array([0, 1, 2, 3, 4, 5])  # class 6 absent
        y_pred = np.array([0, 1, 2, 3, 4, 5])
        result = per_class_metrics(y_true, y_pred)
        assert len(result) == 7
        cls6 = [r for r in result if r["class"] == 6][0]
        assert cls6["support"] == 0
        assert cls6["f1"] == 0.0

    def test_compute_all_metrics_shape(self) -> None:
        from harness.metrics import compute_all_metrics

        y_true, y_pred = self._perfect_7class(14)
        result = compute_all_metrics(y_true, y_pred)
        assert result["ok"] is True
        assert result["n_samples"] == 14
        assert result["n_classes"] == 7
        assert "macro_f1" in result
        assert "accuracy" in result
        assert "per_class" in result

    def test_compute_all_metrics_with_proba(self) -> None:
        from harness.metrics import compute_all_metrics

        n = 7
        y_true = np.arange(n)
        y_pred = np.arange(n)
        y_proba = np.zeros((n, 7))
        y_proba[np.arange(n), y_true] = 1.0
        result = compute_all_metrics(y_true, y_pred, y_proba)
        assert "log_loss" in result
        assert result["log_loss"] < 1e-10

    def test_y_true_label_out_of_bounds_raises(self) -> None:
        from harness.metrics import macro_f1

        y_true = np.array([0, 7])  # 7 is out of 0..6
        y_pred = np.array([0, 1])
        with pytest.raises(ValueError, match="outside.*Covertype"):
            macro_f1(y_true, y_pred)

    def test_y_pred_label_out_of_bounds_raises(self) -> None:
        from harness.metrics import macro_f1

        y_true = np.array([0, 1])
        y_pred = np.array([0, 7])
        with pytest.raises(ValueError, match="outside.*Covertype"):
            macro_f1(y_true, y_pred)

    def test_length_mismatch_raises(self) -> None:
        from harness.metrics import macro_f1

        with pytest.raises(ValueError, match="length"):
            macro_f1(np.array([0, 1]), np.array([0, 1, 2]))

    def test_y_proba_wrong_samples_raises(self) -> None:
        from harness.metrics import multiclass_log_loss

        with pytest.raises(ValueError, match="n_samples"):
            multiclass_log_loss(np.array([0, 1]), np.zeros((3, 7)))


# ---------------------------------------------------------------------------
# Feature recipe tests
# ---------------------------------------------------------------------------


class TestFeatures:
    def test_import_features(self) -> None:
        from harness.features import list_recipes

        recipes = list_recipes()
        assert "raw_v1" in recipes
        assert "terrain_v1" in recipes

    def test_raw_v1_pass_through(self) -> None:
        from harness.features import get_recipe

        X = np.random.default_rng(1).normal(size=(10, 54))
        recipe = get_recipe("raw_v1")
        result = recipe(X)
        assert result.shape == (10, 54)
        np.testing.assert_array_almost_equal(result, X)

    def test_raw_v1_wrong_features_raises(self) -> None:
        from harness.features import get_recipe

        recipe = get_recipe("raw_v1")
        with pytest.raises(ValueError, match="expects.*54"):
            recipe(np.zeros((5, 10)))

    def test_terrain_v1_output_count(self) -> None:
        """terrain_v1 produces 69 features (54 original + 15 derived)."""
        from harness.features import get_recipe, feature_count

        assert feature_count("terrain_v1") == 69
        X = np.random.default_rng(2).normal(size=(5, 54))
        recipe = get_recipe("terrain_v1")
        result = recipe(X)
        assert result.shape == (5, 69)
        assert result.dtype == np.float64

    def test_terrain_v1_wrong_features_raises(self) -> None:
        from harness.features import get_recipe

        recipe = get_recipe("terrain_v1")
        with pytest.raises(ValueError, match="expects.*54"):
            recipe(np.zeros((3, 20)))

    def test_raw_v1_feature_count(self) -> None:
        from harness.features import feature_count

        assert feature_count("raw_v1") == 54

    def test_minimal_v1_output_count(self) -> None:
        """minimal_v1 produces 10 features (first 10 continuous columns only)."""
        from harness.features import get_recipe, feature_count

        assert feature_count("minimal_v1") == 10
        X = np.random.default_rng(5).normal(size=(10, 54))
        result = get_recipe("minimal_v1")(X)
        assert result.shape == (10, 10)
        # Should equal first 10 columns of input
        np.testing.assert_array_almost_equal(result, X[:, :10])

    def test_minimal_v1_wrong_features_raises(self) -> None:
        from harness.features import get_recipe

        with pytest.raises(ValueError, match="expects.*54"):
            get_recipe("minimal_v1")(np.zeros((3, 10)))

    def test_minimal_v1_drops_categorical(self) -> None:
        """minimal_v1 drops all 44 binary one-hot columns (wilderness + soil)."""
        from harness.features import get_recipe

        X = np.zeros((5, 54))
        # Set some binary columns to 1
        X[0, 10] = 1.0  # wilderness area
        X[1, 20] = 1.0  # soil type
        result = get_recipe("minimal_v1")(X)
        assert result.shape == (5, 10)
        # All binary columns (10-53) should be gone
        assert result[:, :10].sum() == 0  # continuous cols are 0

    def test_unknown_recipe_raises(self) -> None:
        from harness.features import get_recipe

        with pytest.raises(KeyError, match="unknown feature recipe"):
            get_recipe("nonexistent_v99")

    def test_terrain_v1_preserves_original_features(self) -> None:
        """First 54 columns of terrain_v1 output == input X (float64)."""
        from harness.features import get_recipe

        X = np.random.default_rng(3).normal(size=(8, 54))
        recipe = get_recipe("terrain_v1")
        result = recipe(X)
        np.testing.assert_array_almost_equal(result[:, :54], X)

    def test_terrain_v1_deterministic(self) -> None:
        from harness.features import get_recipe

        X = np.random.default_rng(4).normal(size=(20, 54))
        recipe = get_recipe("terrain_v1")
        r1 = recipe(X)
        r2 = recipe(X)
        np.testing.assert_array_equal(r1, r2)


# ---------------------------------------------------------------------------
# Split / data tests
# ---------------------------------------------------------------------------


class TestSplits:
    def test_create_splits_shape(self) -> None:
        from harness.data import create_splits

        y = np.random.default_rng(10).integers(0, 7, size=1000)
        masks = create_splits(y)
        assert set(masks.keys()) == {"fit", "early_stop", "selection", "confirmation", "final"}

        n = len(y)
        for name, mask in masks.items():
            assert len(mask) == n
            assert mask.dtype == bool

    def test_create_splits_disjoint_and_complete(self) -> None:
        from harness.data import create_splits

        y = np.random.default_rng(11).integers(0, 7, size=500)
        masks = create_splits(y)
        coverage = np.zeros(len(y), dtype=bool)
        for mask in masks.values():
            assert not np.any(coverage & mask), "splits overlap"
            coverage |= mask
        assert coverage.all(), "splits do not cover all samples"

    def test_create_splits_deterministic(self) -> None:
        from harness.data import create_splits

        y = np.random.default_rng(12).integers(0, 7, size=300)
        m1 = create_splits(y, seed=42)
        m2 = create_splits(y, seed=42)
        for key in m1:
            np.testing.assert_array_equal(m1[key], m2[key])

    def test_create_splits_different_seed_different(self) -> None:
        from harness.data import create_splits

        y = np.random.default_rng(13).integers(0, 7, size=300)
        m1 = create_splits(y, seed=42)
        m2 = create_splits(y, seed=99)

        # At least one split should differ
        any_differ = False
        for key in m1:
            if not np.array_equal(m1[key], m2[key]):
                any_differ = True
                break
        assert any_differ, "different seeds should produce different splits"

    def test_create_splits_stratified_approximate(self) -> None:
        """Each split should roughly preserve label distribution."""
        from harness.data import create_splits

        # Ensure every class has enough samples for stratified splitting
        y = np.repeat(np.arange(7), 500)  # 3500 samples, 500 per class
        rng = np.random.default_rng(14)
        rng.shuffle(y)
        overall_dist = np.bincount(y) / len(y)
        masks = create_splits(y)

        for name, mask in masks.items():
            split_y = y[mask]
            split_dist = np.bincount(split_y, minlength=7) / len(split_y)
            # Within 5% tolerance
            np.testing.assert_allclose(split_dist, overall_dist, atol=0.05)

    def test_create_splits_custom_fractions(self) -> None:
        from harness.data import create_splits

        y = np.random.default_rng(15).integers(0, 7, size=1000)
        custom = {
            "fit": 0.5,
            "early_stop": 0.1,
            "selection": 0.2,
            "confirmation": 0.1,
            "final": 0.1,
        }
        masks = create_splits(y, fractions=custom)
        for name, frac in custom.items():
            actual = float(masks[name].sum()) / len(y)
            assert actual == pytest.approx(frac, abs=0.02)

    def test_create_splits_bad_fractions_raises(self) -> None:
        from harness.data import create_splits

        y = np.array([0, 1, 2, 3, 4, 5, 6] * 10)
        with pytest.raises(ValueError, match="sum to 1"):
            create_splits(y, fractions={
                "fit": 0.3, "early_stop": 0.1,
                "selection": 0.1, "confirmation": 0.1,
                "final": 0.1,
            })

    def test_get_split_data(self) -> None:
        from harness.data import create_splits, get_split_data

        rng = np.random.default_rng(16)
        # Ensure enough samples per class for stratified splits
        y = np.repeat(np.arange(7), 30)  # 210 samples, 30 per class
        rng.shuffle(y)
        X = rng.normal(size=(len(y), 54))
        masks = create_splits(y)
        X_sel, y_sel = get_split_data(X, y, "selection", masks)
        assert X_sel.shape[0] == y_sel.shape[0]
        assert X_sel.shape[1] == 54
        np.testing.assert_array_equal(X_sel, X[masks["selection"]])

    def test_get_split_data_unknown_split_raises(self) -> None:
        from harness.data import get_split_data

        X = np.zeros((100, 54))
        y = np.zeros(100, dtype=np.int64)
        # Create valid masks first, then query a bogus name
        with pytest.raises(KeyError, match="Unknown split"):
            # Pass valid masks so the unknown-split path is exercised
            from harness.data import create_splits
            masks = create_splits(np.repeat(np.arange(7), 30))
            get_split_data(X[:21], y[:21], "bogus_split", masks)

    def test_partition_constants(self) -> None:
        from harness.data import (
            COVTYPE_N_CLASSES,
            COVTYPE_N_FEATURES,
            COVTYPE_N_SAMPLES,
            PARTITION_DEFAULTS,
            PARTITION_SEED,
        )

        assert COVTYPE_N_CLASSES == 7
        assert COVTYPE_N_FEATURES == 54
        assert COVTYPE_N_SAMPLES == 581012
        assert PARTITION_SEED == 20240101
        total = sum(PARTITION_DEFAULTS.values())
        assert total == pytest.approx(1.0)

    def test_compute_provenance(self) -> None:
        from harness.data import compute_provenance

        X = np.random.default_rng(18).normal(size=(100, 54))
        y = np.random.default_rng(19).integers(0, 7, size=100)
        fp1 = compute_provenance(X, y)
        fp2 = compute_provenance(X, y)
        assert fp1 == fp2
        assert len(fp1) == 64  # SHA-256 hex

    def test_compute_provenance_shape_mismatch_raises(self) -> None:
        from harness.data import compute_provenance

        with pytest.raises(ValueError, match="invalid Covertype arrays"):
            compute_provenance(
                np.zeros((10, 54)),
                np.zeros(20, dtype=np.int64),  # wrong length
            )

    def test_build_dataset_manifest(self) -> None:
        from harness.data import build_dataset_manifest

        X = np.random.default_rng(20).normal(size=(50, 54))
        y = np.random.default_rng(21).integers(0, 7, size=50)
        from harness.data import compute_provenance

        fp = compute_provenance(X, y)
        manifest = build_dataset_manifest(X, y, fp)
        assert manifest["schema_version"] == 1
        assert manifest["name"] == "Covertype"
        assert manifest["shape"] == [50, 54]
        assert manifest["n_classes"] == 7
        assert manifest["content_fingerprint_sha256"] == fp

    def test_resolve_class_weight_none(self) -> None:
        from harness.config import resolve_class_weight

        candidate = {"class_weight_mode": "none"}
        y = np.array([0, 1, 2, 3, 4, 5, 6])
        assert resolve_class_weight(candidate, y) is None

    def test_resolve_class_weight_balanced(self) -> None:
        from harness.config import resolve_class_weight

        candidate = {"class_weight_mode": "balanced"}
        y = np.array([0, 0, 1, 1, 2])  # 5 samples, 3 classes (0,1,2)
        weights = resolve_class_weight(candidate, y)
        assert weights is not None
        assert weights.shape == (5,)
        assert weights.dtype == np.float64
        # class 0: 2 samples, class 1: 2 samples, class 2: 1 sample
        # n_samples=5, n_classes=3
        # w_0 = 5/(3*2) = 5/6, w_1 = 5/6, w_2 = 5/(3*1) = 5/3
        np.testing.assert_array_almost_equal(
            weights, np.array([5 / 6, 5 / 6, 5 / 6, 5 / 6, 5 / 3])
        )

    def test_resolve_class_weight_balanced_sqrt(self) -> None:
        from harness.config import resolve_class_weight

        candidate = {"class_weight_mode": "balanced_sqrt"}
        y = np.array([0, 0, 1, 1, 2])
        weights = resolve_class_weight(candidate, y)
        assert weights is not None
        # sqrt of balanced weights
        expected = np.sqrt(np.array([5 / 6, 5 / 6, 5 / 6, 5 / 6, 5 / 3]))
        np.testing.assert_array_almost_equal(weights, expected)
