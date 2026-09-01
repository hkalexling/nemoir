"""Tests for the notebook-facing post-run visualization helper.

The fixture contains only synthetic JSON evidence. It does not import XGBoost,
download Covertype, or execute an agent workflow.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.conftest import DEMO_ROOT

matplotlib = pytest.importorskip("matplotlib")
matplotlib.use("Agg")

if str(DEMO_ROOT) not in sys.path:
    sys.path.insert(0, str(DEMO_ROOT))

import plot_run as plots  # noqa: E402


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _candidate(*, depth: int, recipe: str = "raw_v1", weighted: str = "none") -> dict[str, Any]:
    return {
        "schema_version": 1,
        "candidate_id": "synthetic",
        "feature_recipe": recipe,
        "n_estimators": 200,
        "max_depth": depth,
        "learning_rate": 0.1,
        "subsample": 0.8,
        "colsample_bytree": 0.8,
        "colsample_bylevel": 1.0,
        "colsample_bynode": 1.0,
        "reg_alpha": 0.0,
        "reg_lambda": 1.0,
        "gamma": 0.0,
        "min_child_weight": 2.0,
        "max_bin": 256,
        "grow_policy": "depthwise",
        "early_stopping_rounds": 20,
        "class_weight_mode": weighted,
        "max_delta_step": 0.0,
    }


def _per_class(a: float, b: float) -> list[dict[str, float | int]]:
    return [
        {"class": 0, "f1": a, "precision": a, "recall": a, "support": 10},
        {"class": 1, "f1": b, "precision": b, "recall": b, "support": 10},
    ]


def _metric(score: float, *, elapsed: float | None = None, iteration: int | None = None) -> dict[str, Any]:
    metric: dict[str, Any] = {
        "ok": True,
        "macro_f1": score,
        "score": score,
        "accuracy": min(1.0, score + 0.04),
        "log_loss": 1.0 - score,
        "per_class": _per_class(score - 0.02, score + 0.02),
    }
    if elapsed is not None:
        metric["elapsed_seconds"] = elapsed
    if iteration is not None:
        metric["best_iteration"] = iteration
    return metric


@pytest.fixture
def synthetic_run(tmp_path: Path) -> Path:
    run_dir = tmp_path / "runs" / "synthetic-run"
    _write_json(run_dir / "run_manifest.json", {"schema_version": 1, "run_id": "synthetic-run"})
    _write_json(
        run_dir / "state.json",
        {"schema_version": 1, "trial_count": 4, "accepted_count": 1},
    )
    _write_json(
        run_dir / "metrics" / "baseline.json",
        {
            "ok": True,
            "selection_score": 0.60,
            "confirmation_score": 0.62,
            "score": 0.61,
            "selection_metrics": _metric(0.60),
            "confirmation_metrics": _metric(0.62),
        },
    )
    _write_json(
        run_dir / "metrics" / "final.json",
        {
            "ok": True,
            "baseline": {
                **_metric(0.61),
                "validation_combined_score": 0.61,
            },
            "best": {
                **_metric(0.74),
                "validation_combined_score": 0.75,
                "same_as_baseline": False,
            },
            "delta": {"macro_f1": 0.13, "accuracy": 0.13},
        },
    )

    # Accepted candidate with selection and confirmation evidence.
    trial = run_dir / "trials" / "001"
    _write_json(
        trial / "decision.json",
        {"decision": "accept", "trial": 1, "incumbent": {"trial": 1, "score": 0.70}},
    )
    _write_json(trial / "candidate.json", _candidate(depth=6))
    _write_json(trial / "preflight.json", {"ok": True})
    _write_json(trial / "train.json", _metric(0.75, elapsed=4.2, iteration=143))
    _write_json(trial / "selection.json", _metric(0.71))
    _write_json(trial / "confirmation.json", _metric(0.69))
    _write_json(trial / "judge_primary.json", {"ok": True, "selection_score": 0.71})
    _write_json(trial / "judge_confirmation.json", {"ok": True, "score": 0.70})

    # Selection-stage rejection: confirmation was never run.
    trial = run_dir / "trials" / "002"
    _write_json(
        trial / "decision.json",
        {"decision": "reject", "trial": 2, "reason_code": "PRIMARY_SELECTION_NOT_IMPROVED"},
    )
    _write_json(trial / "rejected_candidate.json", _candidate(depth=5))
    _write_json(trial / "preflight.json", {"ok": True})
    _write_json(trial / "train.json", _metric(0.68, elapsed=3.7, iteration=121))
    _write_json(trial / "selection.json", _metric(0.65))
    _write_json(trial / "judge_primary.json", {"ok": True, "selection_score": 0.65})

    # Candidate that passed selection but failed the confirmation guard.
    trial = run_dir / "trials" / "003"
    _write_json(
        trial / "decision.json",
        {
            "decision": "reject",
            "trial": 3,
            "reason_code": "CONFIRMED_IMPROVEMENT_BELOW_EPS",
            "score": 0.695,
            "best_score": 0.70,
        },
    )
    _write_json(trial / "rejected_candidate.json", _candidate(depth=8, recipe="terrain_v1"))
    _write_json(trial / "preflight.json", {"ok": True})
    _write_json(trial / "train.json", _metric(0.76, elapsed=7.1, iteration=198))
    _write_json(trial / "selection.json", _metric(0.73))
    _write_json(trial / "confirmation.json", _metric(0.66))
    _write_json(trial / "judge_primary.json", {"ok": True, "selection_score": 0.73})
    _write_json(trial / "judge_confirmation.json", {"ok": True, "score": 0.695})

    # Preflight failure has no training or selection score and must not become
    # a zero-valued point in score charts.
    trial = run_dir / "trials" / "004"
    _write_json(
        trial / "decision.json",
        {
            "decision": "reject",
            "trial": 4,
            "reason_code": "PREFLIGHT_FAILED_REPAIRS_EXHAUSTED",
        },
    )
    _write_json(trial / "rejected_candidate.json", _candidate(depth=9, weighted="balanced"))
    _write_json(trial / "preflight.json", {"ok": False, "error": "duplicate configuration"})
    return run_dir


def test_load_run_classifies_all_trial_outcomes(synthetic_run: Path) -> None:
    run = plots.load_run(synthetic_run, strict=True)

    assert [trial.outcome for trial in run.trials] == [
        "accepted",
        "primary_rejected",
        "confirmation_rejected",
        "preflight_failed",
    ]
    assert run.trials[3].selection_score is None
    assert run.trials[3].combined_score is None
    assert run.trials[2].combined_score == pytest.approx(0.695)
    assert run.final_metrics is not None
    assert all(not hasattr(trial, "final_metrics") for trial in run.trials)


def test_plot_run_renders_the_five_inline_figures(synthetic_run: Path) -> None:
    dashboard = plots.plot_run(synthetic_run, show=False, strict=True)

    assert set(dashboard.figures) == {
        "score_trajectory",
        "trial_outcomes_and_cost",
        "hyperparameter_trajectory",
        "selection_confirmation_gap",
        "final_test_comparison",
    }
    for figure in dashboard.figures.values():
        figure.canvas.draw()

    assert dashboard.summary["accepted_trials"] == 1
    assert dashboard.summary["evaluated_trials"] == 3
    assert dashboard.summary["outcome_counts"]["preflight_failed"] == 1
    assert dashboard.summary["final_test"]["macro_f1"] == pytest.approx(0.74)


def test_missing_required_baseline_artifact_fails_clearly(tmp_path: Path) -> None:
    run_dir = tmp_path / "bad-run"
    _write_json(run_dir / "run_manifest.json", {"schema_version": 1})
    _write_json(run_dir / "state.json", {"trial_count": 0})
    (run_dir / "trials").mkdir(parents=True)

    with pytest.raises(plots.RunArtifactError, match="baseline.json"):
        plots.load_run(run_dir)


def test_cli_no_show_prints_summary(synthetic_run: Path, capsys: pytest.CaptureFixture[str]) -> None:
    assert plots.main([str(synthetic_run), "--no-show", "--print-summary", "--strict"]) == 0
    output = capsys.readouterr().out
    assert '"run_id": "synthetic-run"' in output
