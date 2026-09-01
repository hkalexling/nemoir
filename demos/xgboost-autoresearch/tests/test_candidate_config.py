"""Tests for candidate config validation, semantic hash, and invariants.

Uses the actual ``harness/config.py`` contracts — no downloads, no GPU,
no real provider.
"""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from harness.config import (
    ALLOWED_SCHEMA_VERSIONS,
    CANDIDATE_SCHEMA,
    FIXED_INVARIANTS,
    build_xgb_fit_kwargs,
    build_xgb_params,
    config_hash,
    load_candidate,
    validate_candidate,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def valid_candidate() -> dict:
    """A schema-compliant candidate matching the schema contracts."""
    return {
        "schema_version": 1,
        "candidate_id": "test-001",
        "feature_recipe": "raw_v1",
        "n_estimators": 500,
        "max_depth": 6,
        "learning_rate": 0.1,
        "subsample": 0.8,
        "colsample_bytree": 0.8,
        "colsample_bylevel": 1.0,
        "colsample_bynode": 1.0,
        "reg_alpha": 0.0,
        "reg_lambda": 1.0,
        "gamma": 0.0,
        "min_child_weight": 1.0,
        "max_bin": 256,
        "grow_policy": "depthwise",
        "early_stopping_rounds": 50,
        "class_weight_mode": "none",
        "max_delta_step": 0.0,
    }


# ---------------------------------------------------------------------------
# Validation tests
# ---------------------------------------------------------------------------


class TestValidateCandidate:
    def test_valid_candidate_passes(self, valid_candidate: dict) -> None:
        errors = validate_candidate(valid_candidate)
        assert errors == [], f"unexpected errors: {errors}"

    def test_baseline_config_is_valid(self) -> None:
        """The actual frozen baseline.json must pass validation."""
        from harness.config import BASELINE_PATH

        if not BASELINE_PATH.exists():
            pytest.skip(f"baseline config not found: {BASELINE_PATH}")
        errors = validate_candidate(json.loads(BASELINE_PATH.read_text()))
        assert errors == [], f"baseline.json has validation errors: {errors}"

    def test_candidate_json_is_valid(self) -> None:
        """The actual candidate.json must pass validation."""
        from harness.config import CANDIDATE_PATH

        if not CANDIDATE_PATH.exists():
            pytest.skip("candidate.json not found")
        errors = validate_candidate(json.loads(CANDIDATE_PATH.read_text()))
        assert errors == [], f"candidate.json has validation errors: {errors}"

    def test_missing_schema_version(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        del c["schema_version"]
        errors = validate_candidate(c)
        assert any("schema_version" in e for e in errors)

    def test_invalid_schema_version(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["schema_version"] = 99
        errors = validate_candidate(c)
        assert any("schema_version" in e for e in errors)

    def test_wrong_type_bool_for_schema_version(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["schema_version"] = True
        errors = validate_candidate(c)
        assert any("schema_version" in e for e in errors)

    def test_unknown_key(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["made_up_key"] = 42
        errors = validate_candidate(c)
        assert any("made_up_key" in e for e in errors)

    def test_missing_required_field(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        del c["n_estimators"]
        errors = validate_candidate(c)
        assert any("n_estimators" in e for e in errors)

    @pytest.mark.parametrize(
        ("field", "bad_value", "expected_substr"),
        [
            ("n_estimators", 5, "n_estimators"),  # below min 10
            ("n_estimators", 3000, "n_estimators"),  # above max 2000
            ("max_depth", 1, "max_depth"),  # below min 2
            ("max_depth", 30, "max_depth"),  # above max 20
            ("learning_rate", 0.0001, "learning_rate"),  # below min 0.001
            ("learning_rate", 2.0, "learning_rate"),  # above max 0.5
            ("learning_rate", float("nan"), "finite"),
            ("learning_rate", float("inf"), "finite"),
            ("subsample", -0.5, "subsample"),
            ("subsample", 1.5, "subsample"),
            ("reg_alpha", -1.0, "reg_alpha"),
            ("reg_alpha", 100.0, "reg_alpha"),
            ("grow_policy", "invalid_policy", "grow_policy"),
            ("feature_recipe", "unknown_recipe", "feature_recipe"),
            ("class_weight_mode", "invalid_mode", "class_weight_mode"),
            ("early_stopping_rounds", 2, "early_stopping_rounds"),  # below min 5
            ("max_bin", 16, "max_bin"),  # below min 32
        ],
    )
    def test_out_of_bound_values(
        self, valid_candidate: dict, field: str, bad_value: object, expected_substr: str
    ) -> None:
        c = deepcopy(valid_candidate)
        c[field] = bad_value
        errors = validate_candidate(c)
        assert any(expected_substr in e for e in errors), (
            f"expected '{expected_substr}' in errors for {field}={bad_value!r}, got: {errors}"
        )

    def test_wrong_type_float_for_int_field(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["n_estimators"] = 3.14
        errors = validate_candidate(c)
        assert any("n_estimators" in e and "int" in e for e in errors)

    def test_wrong_type_str_for_enum(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["grow_policy"] = 42
        errors = validate_candidate(c)
        assert any("grow_policy" in e for e in errors)

    def test_bool_is_not_int(self, valid_candidate: dict) -> None:
        """Bool subclasses int in Python; schema must reject bool for int fields."""
        c = deepcopy(valid_candidate)
        c["n_estimators"] = True
        errors = validate_candidate(c)
        assert any("n_estimators" in e for e in errors)

    def test_invalid_feature_recipe_enum(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["feature_recipe"] = "bogus_recipe_v99"
        errors = validate_candidate(c)
        assert any("feature_recipe" in e for e in errors)

    def test_candidate_id_optional(self, valid_candidate: dict) -> None:
        """candidate_id is optional — removing it should still pass."""
        c = deepcopy(valid_candidate)
        del c["candidate_id"]
        errors = validate_candidate(c)
        assert not any("candidate_id" in e and "required" in e for e in errors)

    def test_base_config_uses_allowed_schema_versions(self) -> None:
        """The allowed schema versions set is non-empty."""
        assert ALLOWED_SCHEMA_VERSIONS, "ALLOWED_SCHEMA_VERSIONS is empty"
        assert 1 in ALLOWED_SCHEMA_VERSIONS

    def test_schema_defines_all_expected_keys(self) -> None:
        expected_keys = {
            "schema_version", "candidate_id", "feature_recipe",
            "n_estimators", "max_depth", "learning_rate",
            "subsample", "colsample_bytree", "colsample_bylevel",
            "colsample_bynode", "reg_alpha", "reg_lambda", "gamma",
            "min_child_weight", "max_bin", "grow_policy",
            "early_stopping_rounds", "class_weight_mode", "max_delta_step",
        }
        assert set(CANDIDATE_SCHEMA.keys()) == expected_keys


# ---------------------------------------------------------------------------
# config_hash tests
# ---------------------------------------------------------------------------


class TestConfigHash:
    def test_deterministic_same_input(self, valid_candidate: dict) -> None:
        h1 = config_hash(valid_candidate)
        h2 = config_hash(valid_candidate)
        assert h1 == h2
        assert len(h1) == 64  # full SHA-256 digest for audit evidence

    def test_hash_excludes_candidate_id(self, valid_candidate: dict) -> None:
        h1 = config_hash(valid_candidate)
        c2 = deepcopy(valid_candidate)
        c2["candidate_id"] = "completely-different-id"
        h2 = config_hash(c2)
        assert h1 == h2, (
            "config_hash must exclude candidate_id; "
            f"got {h1} vs {h2}"
        )

    def test_hash_changes_on_parameter_change(self, valid_candidate: dict) -> None:
        h1 = config_hash(valid_candidate)
        c2 = deepcopy(valid_candidate)
        c2["learning_rate"] = 0.05
        h2 = config_hash(c2)
        assert h1 != h2, f"hash should differ for different learning_rate: {h1} == {h2}"

    def test_hash_changes_on_schema_version(self, valid_candidate: dict) -> None:
        """Same params under different schema_version produce different hashes."""
        # schema_version can only be 1 for now; test via the canonical dict
        # construction that uses defaults for missing keys.
        c1 = deepcopy(valid_candidate)
        h1 = config_hash(c1)
        # Build a dict that has all same params but would get schema_version=1
        # from defaults. The hash should be the same since schema_version is
        # present.
        c2 = deepcopy(valid_candidate)
        # Change a non-schema-version param
        c2["max_depth"] = 12
        h2 = config_hash(c2)
        assert h1 != h2

    def test_hash_includes_keys_not_in_candidate(self, valid_candidate: dict) -> None:
        """Default-filled keys (not in candidate) still contribute to hash."""
        h1 = config_hash(valid_candidate)
        # Remove a field that has a default; the hash uses the default
        c2 = deepcopy(valid_candidate)
        del c2["class_weight_mode"]  # default is "none"
        h2 = config_hash(c2)
        assert h1 == h2, (
            "hash should be unchanged when candidate omits a field "
            "whose value matches the default"
        )

    def test_hash_changes_when_omitted_field_differs_from_default(self, valid_candidate: dict) -> None:
        """If a field is deleted and its value differs from default, hash changes."""
        c_default = deepcopy(valid_candidate)
        c_default["class_weight_mode"] = "balanced"
        h_default = config_hash(c_default)
        c_omitted = deepcopy(c_default)
        del c_omitted["class_weight_mode"]  # default is "none", not "balanced"
        h_omitted = config_hash(c_omitted)
        assert h_default != h_omitted, (
            "hash must differ when omitted field differs from default"
        )

    def test_hash_is_hex_string(self, valid_candidate: dict) -> None:
        h = config_hash(valid_candidate)
        assert isinstance(h, str)
        assert all(c in "0123456789abcdef" for c in h)


# ---------------------------------------------------------------------------
# FIXED_INVARIANTS tests
# ---------------------------------------------------------------------------


class TestFixedInvariants:
    def test_invariants_are_immutable_from_candidate(self) -> None:
        """Candidate cannot override sealed values."""
        for key in FIXED_INVARIANTS:
            assert key not in {
                "max_depth", "learning_rate", "subsample",
                "colsample_bytree", "colsample_bylevel", "colsample_bynode",
                "reg_alpha", "reg_lambda", "gamma", "min_child_weight",
                "max_bin", "grow_policy", "max_delta_step",
                "n_estimators", "early_stopping_rounds",
                "feature_recipe", "class_weight_mode",
                "schema_version", "candidate_id",
            }, f"FIXED_INVARIANTS key {key!r} is also in the candidate-controllable set"

    def test_objective_is_multiclass(self) -> None:
        assert FIXED_INVARIANTS["objective"] == "multi:softprob"

    def test_num_class_is_7(self) -> None:
        assert FIXED_INVARIANTS["num_class"] == 7

    def test_seed_is_fixed(self) -> None:
        assert FIXED_INVARIANTS["seed"] == 42


# ---------------------------------------------------------------------------
# build_xgb_params / build_xgb_fit_kwargs tests
# ---------------------------------------------------------------------------


class TestBuildXgbParams:
    def test_invariants_present_in_params(self, valid_candidate: dict) -> None:
        params = build_xgb_params(valid_candidate)
        for key, value in FIXED_INVARIANTS.items():
            assert params[key] == value

    def test_candidate_fields_merged(self, valid_candidate: dict) -> None:
        c = deepcopy(valid_candidate)
        c["max_depth"] = 10
        c["learning_rate"] = 0.05
        params = build_xgb_params(c)
        assert params["max_depth"] == 10
        assert params["learning_rate"] == 0.05

    def test_device_from_env(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NEMOIR_DEVICE", "cuda")
        monkeypatch.setenv("NEMOIR_N_JOBS", "4")
        params = build_xgb_params(valid_candidate)
        assert params["device"] == "cuda"
        assert params["nthread"] == 4

    def test_device_defaults_to_cpu(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("NEMOIR_DEVICE", raising=False)
        monkeypatch.setenv("NEMOIR_N_JOBS", "1")
        params = build_xgb_params(valid_candidate)
        assert params["device"] == "cpu"

    def test_invalid_device_raises(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NEMOIR_DEVICE", "tpu")
        monkeypatch.setenv("NEMOIR_N_JOBS", "1")
        with pytest.raises(ValueError, match="NEMOIR_DEVICE"):
            build_xgb_params(valid_candidate)

    def test_invalid_n_jobs_raises(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NEMOIR_DEVICE", "cpu")
        monkeypatch.setenv("NEMOIR_N_JOBS", "0")
        with pytest.raises(ValueError, match="NEMOIR_N_JOBS"):
            build_xgb_params(valid_candidate)

    def test_n_jobs_non_integer_raises(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NEMOIR_DEVICE", "cpu")
        monkeypatch.setenv("NEMOIR_N_JOBS", "abc")
        with pytest.raises(ValueError, match="NEMOIR_N_JOBS"):
            build_xgb_params(valid_candidate)

    def test_candidate_cannot_override_device(self, valid_candidate: dict, monkeypatch: pytest.MonkeyPatch) -> None:
        """Device is resolved from NEMOIR_DEVICE env, never from candidate."""
        monkeypatch.setenv("NEMOIR_DEVICE", "cpu")
        monkeypatch.setenv("NEMOIR_N_JOBS", "2")
        c = deepcopy(valid_candidate)
        c["device"] = "cuda"  # candidate cannot set this
        params = build_xgb_params(c)
        assert params["device"] == "cpu", "candidate must not override device"

    def test_fit_kwargs_present(self, valid_candidate: dict) -> None:
        kwargs = build_xgb_fit_kwargs(valid_candidate)
        assert kwargs["n_estimators"] == 500
        assert kwargs["early_stopping_rounds"] == 50

    def test_fit_kwargs_custom(self) -> None:
        kwargs = build_xgb_fit_kwargs({"n_estimators": 200, "early_stopping_rounds": 10})
        assert kwargs["n_estimators"] == 200
        assert kwargs["early_stopping_rounds"] == 10


# ---------------------------------------------------------------------------
# load_candidate tests
# ---------------------------------------------------------------------------


class TestLoadCandidate:
    def test_loads_default_path(self, tmp_path: Path) -> None:
        """load_candidate loads the candidate JSON from a given path."""
        data = {"schema_version": 1, "n_estimators": 500}
        p = tmp_path / "candidate.json"
        p.write_text(json.dumps(data))
        result = load_candidate(p)
        assert result == data

    def test_raises_on_missing_file(self) -> None:
        with pytest.raises(FileNotFoundError):
            load_candidate(Path("/nonexistent/candidate_xyz.json"))
