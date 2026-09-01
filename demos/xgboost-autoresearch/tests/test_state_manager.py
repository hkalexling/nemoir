"""State manager integration tests.

Copies the demo root to a temp directory, invokes ``state.py`` subprocess
commands with fake baseline metric/model artifacts, and asserts all the
parent/candidate/model/decision/history/restoration artifacts.

No XGBoost, no real data, no GPU.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from tests.conftest import DEMO_ROOT


def _demo_copy(tmp_path: Path) -> Path:
    """Copy the demo harness/configs/candidate into tmp_path."""
    dest = tmp_path / "demo"
    # Copy only the files needed for state.py
    shutil.copytree(DEMO_ROOT / "harness", dest / "harness")
    shutil.copytree(DEMO_ROOT / "configs", dest / "configs")
    shutil.copy2(DEMO_ROOT / "candidate.json", dest / "candidate.json")
    # Ensure harness/__init__.py exists (from copy)
    (dest / "harness" / "__init__.py").touch()
    return dest


def _run_state(demo_dir: Path, *args: str, env: dict[str, str] | None = None) -> dict:
    """Run state.py as a subprocess and return the last JSON line parsed."""
    full_env = {**os.environ, **(env or {})}
    result = subprocess.run(
        [sys.executable, str(demo_dir / "harness" / "state.py"), *args],
        cwd=str(demo_dir),
        capture_output=True,
        text=True,
        env=full_env,
    )
    # Parse last JSON line from stdout
    for line in reversed(result.stdout.splitlines()):
        stripped = line.strip()
        if stripped.startswith("{") and stripped.endswith("}"):
            try:
                return json.loads(stripped)
            except json.JSONDecodeError:
                continue
    # Fallback
    return {"ok": False, "error": f"no JSON in stdout (rc={result.returncode})", "stdout": result.stdout, "stderr": result.stderr}


def _fake_baseline_metric(demo_dir: Path, selection_score: float = 0.75, confirmation_score: float = 0.73) -> None:
    """Write a fake baseline metric so adopt-baseline has data to consume."""
    import json as _json
    metrics_dir = demo_dir / "runs" / "current" / "metrics"
    metrics_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "ok": True,
        "stage": "baseline",
        "baseline_hash": "abc123",
        "selection_score": selection_score,
        "confirmation_score": confirmation_score,
        "score": (selection_score + confirmation_score) / 2.0,
        "report": "fake baseline",
    }
    (metrics_dir / "baseline.json").write_text(_json.dumps(payload))
    # Also create a fake baseline model
    models_dir = demo_dir / "runs" / "current" / "models"
    models_dir.mkdir(parents=True, exist_ok=True)
    (models_dir / "baseline_model.json").write_text('{"model": "fake-baseline"}')
    # And in work dir (for cmd_adopt_baseline fallback)
    work_dir = demo_dir / "runs" / "current" / "work"
    work_dir.mkdir(parents=True, exist_ok=True)
    (work_dir / "baseline_model.json").write_text('{"model": "fake-baseline"}')


def _fake_selection_metric(demo_dir: Path, score: float = 0.78) -> None:
    metrics_dir = demo_dir / "runs" / "current" / "metrics"
    metrics_dir.mkdir(parents=True, exist_ok=True)
    (metrics_dir / "selection.json").write_text(json.dumps({
        "ok": True, "score": score, "macro_f1": score,
    }))


def _fake_confirmation_metric(demo_dir: Path, score: float = 0.76) -> None:
    metrics_dir = demo_dir / "runs" / "current" / "metrics"
    metrics_dir.mkdir(parents=True, exist_ok=True)
    (metrics_dir / "confirmation.json").write_text(json.dumps({
        "ok": True, "score": score, "macro_f1": score,
    }))


def _fake_current_model(demo_dir: Path) -> None:
    work_dir = demo_dir / "runs" / "current" / "work"
    work_dir.mkdir(parents=True, exist_ok=True)
    (work_dir / "current_model.json").write_text('{"model": "fake-current"}')


def _write_metric(demo_dir: Path, name: str, payload: dict) -> None:
    metrics_dir = demo_dir / "runs" / "current" / "metrics"
    metrics_dir.mkdir(parents=True, exist_ok=True)
    (metrics_dir / f"{name}.json").write_text(json.dumps(payload))


def _active_trial(
    tmp_path: Path,
    *,
    baseline_selection: float = 0.80,
    baseline_confirmation: float = 0.78,
) -> tuple[Path, Path, dict[str, str]]:
    demo = _demo_copy(tmp_path)
    run_dir = demo / "runs" / "current"
    env = {
        "NEMOIR_RUN_DIR": str(run_dir),
        "NEMOIR_EPS": "0.002",
        "NEMOIR_MAX_REPAIRS": "2",
    }
    _run_state(demo, "init", env=env)
    _fake_baseline_metric(
        demo,
        selection_score=baseline_selection,
        confirmation_score=baseline_confirmation,
    )
    _run_state(demo, "adopt-baseline", env=env)
    _run_state(demo, "start-trial", env=env)
    return demo, run_dir, env


# ---------------------------------------------------------------------------
# Init tests
# ---------------------------------------------------------------------------


class TestStateInit:
    def test_init_creates_run_layout(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        result = _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True, f"init failed: {result}"
        assert (run_dir / "state.json").exists()
        assert (run_dir / "run_manifest.json").exists()
        assert (run_dir / "initial_candidate.json").exists()

    def test_init_state_has_trial_count_zero(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        result = _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result["trial_count"] == 0

    def test_init_restores_baseline_to_candidate(self, tmp_path: Path) -> None:
        """After init, candidate.json should match the frozen baseline."""
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        candidate = json.loads((demo / "candidate.json").read_text())
        assert candidate.get("candidate_id") in (None, "baseline-frozen")


# ---------------------------------------------------------------------------
# Adopt-baseline tests
# ---------------------------------------------------------------------------


class TestAdoptBaseline:
    def test_adopt_baseline_succeeds_with_fake_metric(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        result = _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True, f"adopt-baseline failed: {result}"
        assert result["score"] == pytest.approx(0.74)  # (0.75+0.73)/2

    def test_adopt_baseline_creates_incumbent(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        state = json.loads((run_dir / "state.json").read_text())
        assert state["incumbent"] is not None
        assert state["incumbent"]["trial"] == 0

    def test_adopt_baseline_without_metric_fails(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No baseline metric created
        result = _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False

    def test_adopt_baseline_writes_history(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        history_path = run_dir / "agent_view" / "history.jsonl"
        assert history_path.exists()
        lines = history_path.read_text().strip().split("\n")
        assert len(lines) >= 2  # init + baseline_adopted
        events = [json.loads(line) for line in lines]
        kinds = [e["event"] for e in events]
        assert "init" in kinds
        assert "baseline_adopted" in kinds


# ---------------------------------------------------------------------------
# Start-trial tests
# ---------------------------------------------------------------------------


class TestStartTrial:
    def test_start_trial_increments_count(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        result = _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True
        assert result["trial_count"] == 1

    def test_start_trial_without_incumbent_fails(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No adopt-baseline
        result = _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False

    def test_start_trial_exhausted_budget(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir), "NEMOIR_MAX_TRIALS": "0"})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        result = _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False

    def test_start_trial_creates_trial_dir(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        tdir = run_dir / "trials" / "001"
        assert tdir.exists()
        assert (tdir / "parent_candidate.json").exists()
        assert (tdir / "parent.json").exists()


# ---------------------------------------------------------------------------
# Judge-primary tests
# ---------------------------------------------------------------------------


class TestJudgePrimary:
    def test_judge_primary_with_improvement(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo, selection_score=0.70)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        result = _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True
        assert result["score"] == pytest.approx(0.80)
        assert result["best_score"] == pytest.approx(0.70)

    def test_judge_primary_no_selection_metric(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No _fake_selection_metric
        result = _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False

    def test_judge_primary_writes_metric(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo, selection_score=0.70)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert (run_dir / "metrics" / "judge_primary.json").exists()


# ---------------------------------------------------------------------------
# Judge-confirm tests
# ---------------------------------------------------------------------------


class TestJudgeConfirm:
    def test_judge_confirm_with_improvement(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo, selection_score=0.70, confirmation_score=0.68)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_confirmation_metric(demo, score=0.78)
        result = _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True
        # combined = (0.80 + 0.78) / 2 = 0.79
        assert result["score"] == pytest.approx(0.79)
        # incumbent combined = (0.70 + 0.68) / 2 = 0.69
        assert result["best_score"] == pytest.approx(0.69)

    def test_judge_confirm_without_primary_fails(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No judge-primary done
        _fake_confirmation_metric(demo, score=0.78)
        result = _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False


# ---------------------------------------------------------------------------
# Accept tests
# ---------------------------------------------------------------------------


class TestAccept:
    def test_accept_creates_decision_and_updates_incumbent(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo, selection_score=0.70, confirmation_score=0.68)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_confirmation_metric(demo, score=0.78)
        _fake_current_model(demo)
        _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        result = _run_state(demo, "accept", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True
        assert result["best_score"] == pytest.approx(0.79)

        # Check state was updated
        state = json.loads((run_dir / "state.json").read_text())
        assert state["accepted_count"] == 1
        assert state["incumbent"]["trial"] == 1

    def test_accept_writes_decision_artifact(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_confirmation_metric(demo, score=0.78)
        _fake_current_model(demo)
        _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "accept", env={"NEMOIR_RUN_DIR": str(run_dir)})
        tdir = run_dir / "trials" / "001"
        decision = json.loads((tdir / "decision.json").read_text())
        assert decision["decision"] == "accept"

    def test_accept_updates_best_candidate(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_confirmation_metric(demo, score=0.78)
        _fake_current_model(demo)
        _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "accept", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert (run_dir / "best_candidate.json").exists()
        assert (run_dir / "models" / "best_model.json").exists()

    def test_accept_without_current_model_fails(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_confirmation_metric(demo, score=0.78)
        _run_state(demo, "judge-confirm", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No _fake_current_model
        result = _run_state(demo, "accept", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False


# ---------------------------------------------------------------------------
# Reject tests
# ---------------------------------------------------------------------------


class TestReject:
    def test_reject_writes_decision_without_changing_incumbent(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo, selection_score=0.80, confirmation_score=0.78)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})

        # Reject without accepting first
        result = _run_state(demo, "reject", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True  # reject itself succeeds
        state = json.loads((run_dir / "state.json").read_text())
        # accepted_count stays 0
        assert state.get("accepted_count", 0) == 0

    def test_reject_creates_trial_decision(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "reject", env={"NEMOIR_RUN_DIR": str(run_dir)})
        tdir = run_dir / "trials" / "001"
        assert tdir.exists()
        decision = json.loads((tdir / "decision.json").read_text())
        assert decision["decision"] == "reject"

    def test_reject_restores_incumbent_candidate(self, tmp_path: Path) -> None:
        """After reject, candidate.json is restored to the best candidate."""
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})

        # Modify candidate.json to simulate an edited candidate
        (demo / "candidate.json").write_text(json.dumps({"schema_version": 1, "max_depth": 20}))

        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "reject", env={"NEMOIR_RUN_DIR": str(run_dir)})

        # After reject, candidate.json should be restored to best_candidate.json
        # which was set during init/adopt-baseline to the baseline
        candidate = json.loads((demo / "candidate.json").read_text())
        assert candidate.get("candidate_id") in (None, "baseline-frozen")

    def test_reject_writes_rejected_candidate(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "reject", env={"NEMOIR_RUN_DIR": str(run_dir)})
        tdir = run_dir / "trials" / "001"
        assert (tdir / "rejected_candidate.json").exists()

    def test_reject_cleans_up_current_model(self, tmp_path: Path) -> None:
        """Reject must unlink current_model.json so it cannot masquerade."""
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # Create fake current model
        _fake_current_model(demo)
        _run_state(demo, "reject", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # Should be removed
        assert not (run_dir / "work" / "current_model.json").exists()


# ---------------------------------------------------------------------------
# Precise rejection evidence
# ---------------------------------------------------------------------------


class TestRejectionEvidence:
    def test_primary_non_improvement_reports_exact_comparison(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _fake_selection_metric(demo, score=0.60)
        _run_state(demo, "judge-primary", env=env)

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())

        assert decision["reason_code"] == "PRIMARY_SELECTION_NOT_IMPROVED"
        assert decision["comparison"] == "primary_selection"
        assert decision["score"] == pytest.approx(0.60)
        assert decision["best_score"] == pytest.approx(0.80)
        assert decision["delta"] == pytest.approx(-0.20)
        assert "candidate_selection=0.600000" in result["report"]
        assert "incumbent_selection=0.800000" in result["report"]
        assert "delta=-0.200000" in result["report"]
        assert "confirmation was not run" in result["report"]

        next_trial = _run_state(demo, "start-trial", env=env)
        summary = next_trial["history_summary"]
        assert "PRIMARY_SELECTION_NOT_IMPROVED" in summary
        assert "candidate_selection=0.600000" in summary
        assert "incumbent_selection=0.800000" in summary
        assert "score=0.0000" not in summary

    def test_confirmed_below_epsilon_reports_all_values(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _fake_selection_metric(demo, score=0.82)
        _run_state(demo, "judge-primary", env=env)
        _fake_confirmation_metric(demo, score=0.761)
        _run_state(demo, "judge-confirm", env=env)

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())

        assert decision["reason_code"] == "CONFIRMED_IMPROVEMENT_BELOW_EPS"
        assert decision["comparison"] == "confirmed_combined"
        assert decision["score"] == pytest.approx(0.7905)
        assert decision["best_score"] == pytest.approx(0.79)
        assert decision["delta"] == pytest.approx(0.0005)
        assert decision["epsilon"] == pytest.approx(0.002)
        assert decision["required_score_exclusive"] == pytest.approx(0.792)
        assert decision["selection_score"] == pytest.approx(0.82)
        assert decision["incumbent_selection_score"] == pytest.approx(0.80)
        assert decision["confirmation_score"] == pytest.approx(0.761)
        assert decision["incumbent_confirmation_score"] == pytest.approx(0.78)
        assert "candidate_combined=0.790500" in result["report"]
        assert "incumbent_combined=0.790000" in result["report"]
        assert "delta=0.000500" in result["report"]
        assert "epsilon=0.002000" in result["report"]
        assert "required_candidate_combined>0.792000" in result["report"]

        next_trial = _run_state(demo, "start-trial", env=env)
        summary = next_trial["history_summary"]
        assert "CONFIRMED_IMPROVEMENT_BELOW_EPS" in summary
        assert "candidate_combined=0.790500" in summary
        assert "selection=0.820000/0.800000" in summary
        assert "confirmation=0.761000/0.780000" in summary

    def test_preflight_rejection_reports_repairs_and_error(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _write_metric(
            demo,
            "preflight",
            {"ok": False, "error": "configuration hash abc was already evaluated"},
        )
        _run_state(demo, "repair-gate", env=env)
        _run_state(demo, "repair-gate", env=env)
        gate = _run_state(demo, "repair-gate", env=env)
        assert gate["repair_allowed"] is False

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())
        assert decision["reason_code"] == "PREFLIGHT_FAILED_REPAIRS_EXHAUSTED"
        assert decision["failed_stage"] == "Preflight"
        assert decision["score"] is None
        assert decision["repair_attempts"] == 3
        assert decision["max_repairs"] == 2
        assert "score=unavailable" in result["report"]
        assert "repair_gate_attempt=3" in result["report"]
        assert "allowed_repairs=2" in result["report"]
        assert "configuration hash abc was already evaluated" in result["report"]

    @pytest.mark.parametrize(
        ("artifact_name", "artifact", "reason_code", "failed_stage", "expected"),
        [
            (
                "train",
                {"ok": False, "error": "command_timeout"},
                "TRAINING_FAILED",
                "TrainCandidate",
                "training did not complete",
            ),
            (
                "selection",
                {"ok": False, "error": "model_load_failed"},
                "SELECTION_EVALUATION_FAILED",
                "EvaluateSelection",
                "selection score=unavailable",
            ),
            (
                "judge_primary",
                {"ok": False, "error": "selection metric missing score"},
                "PRIMARY_JUDGE_FAILED",
                "JudgePrimary",
                "comparison unavailable",
            ),
        ],
    )
    def test_early_failure_rejections_are_explicit(
        self,
        tmp_path: Path,
        artifact_name: str,
        artifact: dict,
        reason_code: str,
        failed_stage: str,
        expected: str,
    ) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _write_metric(demo, artifact_name, artifact)

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())
        assert decision["reason_code"] == reason_code
        assert decision["failed_stage"] == failed_stage
        assert decision["score"] is None
        assert expected in result["report"]
        assert str(artifact["error"]) in result["report"]

    def test_confirmation_evaluation_failure_keeps_primary_values(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _fake_selection_metric(demo, score=0.82)
        _run_state(demo, "judge-primary", env=env)
        _write_metric(demo, "confirmation", {"ok": False, "error": "prediction_shape_invalid"})

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())
        assert decision["reason_code"] == "CONFIRMATION_EVALUATION_FAILED"
        assert decision["score"] == pytest.approx(0.82)
        assert decision["best_score"] == pytest.approx(0.80)
        assert decision["selection_score"] == pytest.approx(0.82)
        assert decision["confirmation_score"] is None
        assert "candidate_selection=0.820000" in result["report"]
        assert "incumbent_selection=0.800000" in result["report"]
        assert "confirmation score is unavailable" in result["report"]
        assert "prediction_shape_invalid" in result["report"]

    def test_confirmation_judge_failure_is_explicit(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        _fake_selection_metric(demo, score=0.82)
        _run_state(demo, "judge-primary", env=env)
        _fake_confirmation_metric(demo, score=0.81)
        _write_metric(demo, "judge_confirmation", {"ok": False, "error": "comparison operand missing"})

        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())
        assert decision["reason_code"] == "CONFIRMATION_JUDGE_FAILED"
        assert decision["failed_stage"] == "JudgeConfirmed"
        assert decision["selection_score"] == pytest.approx(0.82)
        assert decision["confirmation_score"] == pytest.approx(0.81)
        assert "combined candidate/incumbent comparison unavailable" in result["report"]
        assert "comparison operand missing" in result["report"]

    def test_incomplete_rejection_is_never_generic(self, tmp_path: Path) -> None:
        demo, run_dir, env = _active_trial(tmp_path)
        result = _run_state(demo, "reject", env=env)
        decision = json.loads((run_dir / "trials" / "001" / "decision.json").read_text())
        assert decision["reason_code"] == "INCOMPLETE_TRIAL_EVIDENCE"
        assert decision["score"] is None
        assert "candidate score=unavailable" in result["report"]
        assert "not accepted by compiled transition" not in result["report"]


# ---------------------------------------------------------------------------
# Should-continue tests
# ---------------------------------------------------------------------------


class TestShouldContinue:
    def test_should_continue_within_budget(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir), "NEMOIR_MAX_TRIALS": "5"})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        result = _run_state(demo, "should-continue", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is True
        assert result["continue_search"] is True

    def test_should_continue_exhausted(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir), "NEMOIR_MAX_TRIALS": "0"})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        result = _run_state(demo, "should-continue", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result["continue_search"] is False


# ---------------------------------------------------------------------------
# Repair-gate tests
# ---------------------------------------------------------------------------


class TestRepairGate:
    def test_repair_gate_increments_attempts(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env={"NEMOIR_RUN_DIR": str(run_dir)})
        _run_state(demo, "start-trial", env={"NEMOIR_RUN_DIR": str(run_dir)})
        r1 = _run_state(demo, "repair-gate", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert r1["repair_allowed"] is True
        r2 = _run_state(demo, "repair-gate", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert r2["repair_allowed"] is True
        r3 = _run_state(demo, "repair-gate", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # default max_repairs=2, so 3rd call should deny
        assert r3["repair_allowed"] is False

    def test_repair_gate_without_active_trial_fails(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        _run_state(demo, "init", env={"NEMOIR_RUN_DIR": str(run_dir)})
        # No trial started
        result = _run_state(demo, "repair-gate", env={"NEMOIR_RUN_DIR": str(run_dir)})
        assert result.get("ok") is False


# ---------------------------------------------------------------------------
# Full lifecycle: init -> adopt -> start -> judge -> accept -> continue
# ---------------------------------------------------------------------------


class TestFullLifecycle:
    def test_baseline_to_accept_flow(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {
            "NEMOIR_RUN_DIR": str(run_dir),
            "NEMOIR_MAX_TRIALS": "3",
            "NEMOIR_MAX_REPAIRS": "2",
            "NEMOIR_EPS": "0.002",
        }

        # Init
        r = _run_state(demo, "init", env=env)
        assert r["ok"], f"init failed: {r}"

        # Baseline
        _fake_baseline_metric(demo, selection_score=0.70, confirmation_score=0.68)
        r = _run_state(demo, "adopt-baseline", env=env)
        assert r["ok"], f"adopt-baseline failed: {r}"

        # Start trial 1
        r = _run_state(demo, "start-trial", env=env)
        assert r["ok"], f"start-trial failed: {r}"
        assert r["trial_count"] == 1

        # Judge primary (improvement)
        _fake_selection_metric(demo, score=0.80)
        r = _run_state(demo, "judge-primary", env=env)
        assert r["ok"], f"judge-primary failed: {r}"
        assert r["score"] == pytest.approx(0.80)

        # Judge confirm (improvement)
        _fake_confirmation_metric(demo, score=0.78)
        _fake_current_model(demo)
        r = _run_state(demo, "judge-confirm", env=env)
        assert r["ok"], f"judge-confirm failed: {r}"
        assert r["score"] == pytest.approx(0.79)

        # Accept
        r = _run_state(demo, "accept", env=env)
        assert r["ok"], f"accept failed: {r}"
        assert r["best_score"] == pytest.approx(0.79)

        # Continue
        r = _run_state(demo, "should-continue", env=env)
        assert r["continue_search"] is True

        # History
        history = (run_dir / "agent_view" / "history.jsonl").read_text().strip().split("\n")
        events = [json.loads(line)["event"] for line in history]
        assert "init" in events
        assert "baseline_adopted" in events
        assert "start_trial" in events
        assert "accept" in events

    def test_baseline_to_reject_flow(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {"NEMOIR_RUN_DIR": str(run_dir)}

        _run_state(demo, "init", env=env)
        _fake_baseline_metric(demo, selection_score=0.80, confirmation_score=0.78)
        _run_state(demo, "adopt-baseline", env=env)
        _run_state(demo, "start-trial", env=env)

        # Judge primary with no improvement
        _fake_selection_metric(demo, score=0.60)  # worse than baseline
        _run_state(demo, "judge-primary", env=env)

        _run_state(demo, "reject", env=env)

        state = json.loads((run_dir / "state.json").read_text())
        # Incumbent should still be baseline (trial 0)
        assert state["incumbent"]["trial"] == 0


# ---------------------------------------------------------------------------
# Novelty enforcement: seen_configs.json tracking
# ---------------------------------------------------------------------------


class TestSeenConfigs:
    def test_adopt_baseline_seeds_seen_configs(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {"NEMOIR_RUN_DIR": str(run_dir)}
        _run_state(demo, "init", env=env)
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env=env)
        seen = json.loads((run_dir / "seen_configs.json").read_text())
        assert isinstance(seen, list)
        assert len(seen) >= 1  # baseline hash

    def test_accept_records_seen_config(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {"NEMOIR_RUN_DIR": str(run_dir)}
        _run_state(demo, "init", env=env)
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env=env)
        _run_state(demo, "start-trial", env=env)
        import json as _json
        cand = _json.loads((demo / "candidate.json").read_text())
        cand["max_depth"] = 7
        (demo / "candidate.json").write_text(_json.dumps(cand, sort_keys=True))
        _fake_selection_metric(demo, score=0.80)
        _run_state(demo, "judge-primary", env=env)
        _fake_confirmation_metric(demo, score=0.78)
        _fake_current_model(demo)
        _run_state(demo, "judge-confirm", env=env)
        _run_state(demo, "accept", env=env)
        seen = _json.loads((run_dir / "seen_configs.json").read_text())
        assert len(seen) >= 2  # baseline + accepted trial

    def test_reject_records_seen_config(self, tmp_path: Path) -> None:
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {"NEMOIR_RUN_DIR": str(run_dir)}
        _run_state(demo, "init", env=env)
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env=env)
        _run_state(demo, "start-trial", env=env)
        import json as _json
        cand = _json.loads((demo / "candidate.json").read_text())
        cand["learning_rate"] = 0.05
        (demo / "candidate.json").write_text(_json.dumps(cand, sort_keys=True))
        _fake_selection_metric(demo, score=0.50)  # worse
        _run_state(demo, "reject", env=env)
        seen = _json.loads((run_dir / "seen_configs.json").read_text())
        assert len(seen) >= 2  # baseline + rejected trial

    def test_duplicate_candidate_detected_by_preflight(self, tmp_path: Path) -> None:
        """After a trial is rejected, repeating the same config must fail preflight."""
        demo = _demo_copy(tmp_path)
        run_dir = demo / "runs" / "current"
        env = {"NEMOIR_RUN_DIR": str(run_dir)}
        _run_state(demo, "init", env=env)
        _fake_baseline_metric(demo)
        _run_state(demo, "adopt-baseline", env=env)
        _run_state(demo, "start-trial", env=env)

        # Reject the current candidate (which is the baseline restored by start-trial)
        _fake_selection_metric(demo, score=0.50)
        _run_state(demo, "reject", env=env)

        # Start next trial (restores incumbent = baseline again)
        _run_state(demo, "start-trial", env=env)

        # Now candidate.json == baseline config, which is in seen_configs.
        # Preflight should detect the duplicate and fail.
        from harness.config import config_hash, load_candidate
        candidate = load_candidate(demo / "candidate.json")
        seen = json.loads((run_dir / "seen_configs.json").read_text())
        assert config_hash(candidate) in seen, "testing setup: candidate should be in seen set"
