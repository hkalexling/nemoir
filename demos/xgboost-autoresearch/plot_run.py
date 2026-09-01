#!/usr/bin/env python3
"""Render an inline diagnostic dashboard for a completed autoresearch run.

This module is deliberately read-only: it consumes the evidence bundle written
under ``runs/<run-id>/`` and never invokes the model, evaluator, or data
loader.  In a Colab notebook, use it after the workflow exits::

    from plot_run import plot_run
    dashboard = plot_run("runs/20260719T075416Z-b08eef4e")
    dashboard.summary

The trial-level charts use only fit, selection, and confirmation artifacts.
Held-out final-test metrics are kept in a separate final-comparison chart.
"""

from __future__ import annotations

import argparse
import json
import math
import warnings
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

JsonDict = dict[str, Any]

# Stable, colorblind-friendly meanings shared by all charts.
OUTCOME_COLORS: dict[str, str] = {
    "accepted": "#2ca02c",
    "primary_rejected": "#ff7f0e",
    "confirmation_rejected": "#bcbd22",
    "preflight_failed": "#8c564b",
    "training_failed": "#d62728",
    "selection_evaluation_failed": "#9467bd",
    "confirmation_evaluation_failed": "#e377c2",
    "confirmation_judge_failed": "#17becf",
    "primary_judge_failed": "#1f77b4",
    "unknown": "#7f7f7f",
}

OUTCOME_LABELS: dict[str, str] = {
    "accepted": "Accepted",
    "primary_rejected": "Selection rejected",
    "confirmation_rejected": "Confirmation rejected",
    "preflight_failed": "Preflight failed",
    "training_failed": "Training failed",
    "selection_evaluation_failed": "Selection evaluation failed",
    "confirmation_evaluation_failed": "Confirmation evaluation failed",
    "confirmation_judge_failed": "Confirmation judge failed",
    "primary_judge_failed": "Primary judge failed",
    "unknown": "Unknown / incomplete",
}

PARAMETERS: tuple[tuple[str, str], ...] = (
    ("max_depth", "Max depth"),
    ("learning_rate", "Learning rate"),
    ("n_estimators", "Boost rounds"),
    ("subsample", "Row subsample"),
    ("colsample_bytree", "Column subsample"),
    ("reg_lambda", "L2 regularization"),
    ("reg_alpha", "L1 regularization"),
    ("min_child_weight", "Min child weight"),
    ("early_stopping_rounds", "Early stopping"),
    ("feature_recipe", "Feature recipe"),
    ("class_weight_mode", "Class weighting"),
    ("grow_policy", "Grow policy"),
)


@dataclass(frozen=True)
class TrialRecord:
    """Normalized evidence for one non-baseline trial."""

    trial_id: int
    path: Path
    decision: JsonDict
    candidate: JsonDict | None
    candidate_source: str | None
    preflight: JsonDict | None
    train: JsonDict | None
    selection: JsonDict | None
    confirmation: JsonDict | None
    judge_primary: JsonDict | None
    judge_confirmation: JsonDict | None
    outcome: str
    reason_code: str | None

    @property
    def accepted(self) -> bool:
        return self.outcome == "accepted"

    @property
    def selection_score(self) -> float | None:
        return _metric_score(self.selection)

    @property
    def confirmation_score(self) -> float | None:
        return _metric_score(self.confirmation)

    @property
    def combined_score(self) -> float | None:
        """Return the deterministic combined score when confirmation ran."""
        from_judge = _number(self.judge_confirmation, "score")
        if from_judge is not None:
            return from_judge
        selection = self.selection_score
        confirmation = self.confirmation_score
        if selection is not None and confirmation is not None:
            return (selection + confirmation) / 2.0
        return None

    @property
    def elapsed_seconds(self) -> float | None:
        return _number(self.train, "elapsed_seconds")

    @property
    def best_iteration(self) -> float | None:
        return _number(self.train, "best_iteration")


@dataclass(frozen=True)
class RunData:
    """Read-only normalized representation of a run evidence bundle."""

    path: Path
    run_id: str
    manifest: JsonDict
    state: JsonDict
    baseline: JsonDict
    final_metrics: JsonDict | None
    trials: tuple[TrialRecord, ...]


@dataclass(frozen=True)
class PlotDashboard:
    """Figures and a compact machine-readable summary returned to notebooks."""

    figures: dict[str, Any]
    summary: JsonDict
    run: RunData


class RunArtifactError(ValueError):
    """Raised when a requested directory is not a usable run evidence bundle."""


def _require_pyplot() -> Any:
    try:
        import matplotlib.pyplot as plt
    except ImportError as exc:  # pragma: no cover - exercised in user environments
        raise RuntimeError(
            "Inline plotting requires matplotlib. Install it with "
            "`pip install -r requirements/plots.txt`."
        ) from exc
    return plt


def _read_json(path: Path, *, required: bool = False, strict: bool = False) -> JsonDict | None:
    """Read a JSON object, warning for optional malformed/missing artifacts."""
    if not path.exists():
        # Missing stage artifacts are normal: a primary-rejected candidate has
        # no confirmation artifact, and a preflight failure has no train file.
        # ``strict`` applies to malformed present files and consistency checks,
        # not to stages the workflow never reached.
        if required:
            raise RunArtifactError(f"Missing required run artifact: {path}")
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        if required or strict:
            raise RunArtifactError(f"Cannot parse JSON artifact {path}: {exc}") from exc
        warnings.warn(f"Skipping unreadable artifact {path}: {exc}", stacklevel=2)
        return None
    if not isinstance(value, dict):
        message = f"Expected a JSON object in {path}, got {type(value).__name__}"
        if required or strict:
            raise RunArtifactError(message)
        warnings.warn(message, stacklevel=2)
        return None
    return value


def _number(value: JsonDict | None, key: str) -> float | None:
    """Extract a finite numeric field without treating booleans as numbers."""
    if not value:
        return None
    raw = value.get(key)
    if isinstance(raw, bool):
        return None
    try:
        number = float(raw)
    except (TypeError, ValueError):
        return None
    return number if math.isfinite(number) else None


def _metric_score(metric: JsonDict | None) -> float | None:
    """Return a successful metric's macro-F1/score, if present."""
    if not metric or metric.get("ok") is False:
        return None
    macro_f1 = _number(metric, "macro_f1")
    return macro_f1 if macro_f1 is not None else _number(metric, "score")


def _trial_id(path: Path) -> int | None:
    try:
        return int(path.name)
    except ValueError:
        return None


def _candidate_snapshot(path: Path, *, strict: bool) -> tuple[JsonDict | None, str | None]:
    """Find the candidate actually associated with a completed trial."""
    for filename in ("candidate.json", "rejected_candidate.json", "parent_candidate.json"):
        candidate = _read_json(path / filename, strict=strict)
        if candidate is not None:
            return candidate, filename
    return None, None


def _reason_to_outcome(reason_code: str | None) -> str | None:
    if not reason_code:
        return None
    mapping = {
        "PREFLIGHT_FAILED_REPAIRS_EXHAUSTED": "preflight_failed",
        "TRAINING_FAILED": "training_failed",
        "SELECTION_EVALUATION_FAILED": "selection_evaluation_failed",
        "PRIMARY_JUDGE_FAILED": "primary_judge_failed",
        "CONFIRMATION_EVALUATION_FAILED": "confirmation_evaluation_failed",
        "CONFIRMATION_JUDGE_FAILED": "confirmation_judge_failed",
        "CONFIRMED_IMPROVEMENT_BELOW_EPS": "confirmation_rejected",
        "PRIMARY_SELECTION_NOT_IMPROVED": "primary_rejected",
        "INCOMPLETE_TRIAL_EVIDENCE": "unknown",
    }
    return mapping.get(reason_code)


def _classify_trial(
    decision: JsonDict,
    *,
    preflight: JsonDict | None,
    train: JsonDict | None,
    selection: JsonDict | None,
    confirmation: JsonDict | None,
    judge_primary: JsonDict | None,
    judge_confirmation: JsonDict | None,
) -> tuple[str, str | None]:
    """Classify current and older artifact schemas into a visual outcome."""
    if decision.get("decision") == "accept":
        return "accepted", None

    reason_raw = decision.get("reason_code")
    reason_code = str(reason_raw) if reason_raw else None
    mapped = _reason_to_outcome(reason_code)
    if mapped:
        return mapped, reason_code

    # Older evidence bundles did not have a stable reason_code. Infer only from
    # deterministic artifacts, in execution order.
    if preflight is not None and preflight.get("ok") is False:
        return "preflight_failed", reason_code
    if train is not None and train.get("ok") is False:
        return "training_failed", reason_code
    if selection is not None and selection.get("ok") is False:
        return "selection_evaluation_failed", reason_code
    if judge_primary is not None and judge_primary.get("ok") is False:
        return "primary_judge_failed", reason_code
    if confirmation is not None and confirmation.get("ok") is False:
        return "confirmation_evaluation_failed", reason_code
    if judge_confirmation is not None and judge_confirmation.get("ok") is False:
        return "confirmation_judge_failed", reason_code
    if _metric_score(confirmation) is not None:
        return "confirmation_rejected", reason_code
    if _metric_score(selection) is not None:
        return "primary_rejected", reason_code
    return "unknown", reason_code


def _load_trial(path: Path, *, strict: bool) -> TrialRecord | None:
    trial_id = _trial_id(path)
    if trial_id is None or trial_id == 0:
        return None
    decision = _read_json(path / "decision.json", strict=strict)
    if decision is None:
        warnings.warn(f"Skipping incomplete trial without decision.json: {path}", stacklevel=2)
        return None
    preflight = _read_json(path / "preflight.json", strict=strict)
    train = _read_json(path / "train.json", strict=strict)
    selection = _read_json(path / "selection.json", strict=strict)
    confirmation = _read_json(path / "confirmation.json", strict=strict)
    judge_primary = _read_json(path / "judge_primary.json", strict=strict)
    judge_confirmation = _read_json(path / "judge_confirmation.json", strict=strict)
    candidate, candidate_source = _candidate_snapshot(path, strict=strict)
    outcome, reason_code = _classify_trial(
        decision,
        preflight=preflight,
        train=train,
        selection=selection,
        confirmation=confirmation,
        judge_primary=judge_primary,
        judge_confirmation=judge_confirmation,
    )
    return TrialRecord(
        trial_id=trial_id,
        path=path,
        decision=decision,
        candidate=candidate,
        candidate_source=candidate_source,
        preflight=preflight,
        train=train,
        selection=selection,
        confirmation=confirmation,
        judge_primary=judge_primary,
        judge_confirmation=judge_confirmation,
        outcome=outcome,
        reason_code=reason_code,
    )


def load_run(run_dir: str | Path, *, strict: bool = False) -> RunData:
    """Load a completed or partially completed run without changing it.

    ``metrics/final.json`` is optional so an in-progress run can still be
    inspected. It remains separate from ``TrialRecord`` by design.
    """
    path = Path(run_dir).expanduser().resolve()
    if not path.is_dir():
        raise RunArtifactError(f"Run directory does not exist: {path}")

    manifest = _read_json(path / "run_manifest.json", required=True, strict=True)
    state = _read_json(path / "state.json", required=True, strict=True)
    baseline = _read_json(path / "metrics" / "baseline.json", required=True, strict=True)
    assert manifest is not None and state is not None and baseline is not None
    if manifest.get("schema_version") != 1:
        raise RunArtifactError(
            f"Unsupported run manifest schema_version={manifest.get('schema_version')!r}; expected 1"
        )

    trials_dir = path / "trials"
    if not trials_dir.is_dir():
        raise RunArtifactError(f"Run directory has no trials/ directory: {path}")
    trial_paths = sorted(
        (item for item in trials_dir.iterdir() if item.is_dir() and _trial_id(item) is not None),
        key=lambda item: int(item.name),
    )
    trials = tuple(
        record
        for trial_path in trial_paths
        if (record := _load_trial(trial_path, strict=strict)) is not None
    )
    if not trials:
        warnings.warn(f"No completed non-baseline trials found under {trials_dir}", stacklevel=2)

    expected_trials = _number(state, "trial_count")
    if expected_trials is not None and int(expected_trials) != len(trials):
        message = (
            f"state.json records {int(expected_trials)} trials but {len(trials)} completed "
            "trial decision artifacts were loaded"
        )
        if strict:
            raise RunArtifactError(message)
        warnings.warn(message, stacklevel=2)

    final_metrics = _read_json(path / "metrics" / "final.json", strict=strict)
    run_id = str(manifest.get("run_id") or path.name)
    return RunData(
        path=path,
        run_id=run_id,
        manifest=manifest,
        state=state,
        baseline=baseline,
        final_metrics=final_metrics,
        trials=trials,
    )


def _baseline_scores(run: RunData) -> tuple[float | None, float | None, float | None]:
    selection = _number(run.baseline, "selection_score")
    confirmation = _number(run.baseline, "confirmation_score")
    combined = _number(run.baseline, "score")
    if combined is None and selection is not None and confirmation is not None:
        combined = (selection + confirmation) / 2.0
    return selection, confirmation, combined


def _accepted_score(trial: TrialRecord) -> float | None:
    incumbent = trial.decision.get("incumbent")
    if isinstance(incumbent, dict):
        incumbent_score = _number(incumbent, "score")
        if incumbent_score is not None:
            return incumbent_score
    return trial.combined_score


def _incumbent_series(run: RunData) -> tuple[list[int], list[float | None]]:
    """Return the incumbent after each trial, including baseline at trial 0."""
    _, _, baseline = _baseline_scores(run)
    current = baseline
    xs = [0]
    ys: list[float | None] = [current]
    for trial in run.trials:
        if trial.accepted:
            accepted_score = _accepted_score(trial)
            if accepted_score is not None:
                current = accepted_score
        xs.append(trial.trial_id)
        ys.append(current)
    return xs, ys


def _legend_handles(plt: Any, outcomes: Iterable[str]) -> list[Any]:
    from matplotlib.patches import Patch

    return [
        Patch(
            color=OUTCOME_COLORS[outcome],
            label=OUTCOME_LABELS[outcome],
        )
        for outcome in sorted(set(outcomes))
        if outcome in OUTCOME_COLORS
    ]


def _style_axis(ax: Any) -> None:
    ax.grid(True, color="#d9d9d9", alpha=0.45, linewidth=0.7)
    ax.set_axisbelow(True)


def _placeholder_figure(title: str, message: str) -> Any:
    plt = _require_pyplot()
    figure, axis = plt.subplots(figsize=(9, 4))
    axis.set_axis_off()
    axis.text(0.5, 0.5, message, ha="center", va="center", wrap=True, fontsize=11)
    axis.set_title(title, fontsize=13, weight="bold")
    figure.tight_layout()
    return figure


def plot_score_trajectory(run: RunData) -> Any:
    """Plot selection, confirmation, and incumbent macro-F1 over all trials."""
    plt = _require_pyplot()
    figure, axis = plt.subplots(figsize=(11, 5.5))
    baseline_selection, baseline_confirmation, baseline_combined = _baseline_scores(run)

    if baseline_selection is not None:
        axis.axhline(
            baseline_selection,
            color="#4c4c4c",
            linestyle=":",
            linewidth=1.2,
            label="Baseline selection",
        )
    if baseline_confirmation is not None:
        axis.axhline(
            baseline_confirmation,
            color="#7f7f7f",
            linestyle="--",
            linewidth=1.0,
            label="Baseline confirmation",
        )

    selection_trials = [trial for trial in run.trials if trial.selection_score is not None]
    for outcome in sorted({trial.outcome for trial in selection_trials}):
        subset = [trial for trial in selection_trials if trial.outcome == outcome]
        axis.scatter(
            [trial.trial_id for trial in subset],
            [trial.selection_score for trial in subset],
            color=OUTCOME_COLORS.get(outcome, OUTCOME_COLORS["unknown"]),
            edgecolor="white",
            linewidth=0.7,
            s=52,
            label=f"Selection · {OUTCOME_LABELS.get(outcome, outcome)}",
            zorder=3,
        )

    confirmation_trials = [trial for trial in run.trials if trial.confirmation_score is not None]
    if confirmation_trials:
        axis.scatter(
            [trial.trial_id for trial in confirmation_trials],
            [trial.confirmation_score for trial in confirmation_trials],
            marker="s",
            color="#1f77b4",
            edgecolor="white",
            linewidth=0.7,
            s=52,
            label="Confirmation macro-F1",
            zorder=4,
        )
        combined_trials = [trial for trial in confirmation_trials if trial.combined_score is not None]
        axis.scatter(
            [trial.trial_id for trial in combined_trials],
            [trial.combined_score for trial in combined_trials],
            marker="D",
            color="#9467bd",
            edgecolor="white",
            linewidth=0.7,
            s=48,
            label="Combined validation score",
            zorder=5,
        )

    incumbent_x, incumbent_y = _incumbent_series(run)
    numeric_incumbent = [(x, y) for x, y in zip(incumbent_x, incumbent_y) if y is not None]
    if numeric_incumbent:
        axis.step(
            [x for x, _ in numeric_incumbent],
            [y for _, y in numeric_incumbent],
            where="post",
            color="#2ca02c",
            linewidth=2.2,
            label="Incumbent combined score",
            zorder=2,
        )

    best_accepts = [trial for trial in run.trials if trial.accepted and _accepted_score(trial) is not None]
    if best_accepts:
        best = max(best_accepts, key=lambda trial: _accepted_score(trial) or float("-inf"))
        best_score = _accepted_score(best)
        assert best_score is not None
        axis.annotate(
            f"Best incumbent\nT{best.trial_id}: {best_score:.6f}",
            xy=(best.trial_id, best_score),
            xytext=(8, 14),
            textcoords="offset points",
            arrowprops={"arrowstyle": "->", "color": "#2ca02c"},
            fontsize=9,
            color="#166b25",
        )

    # Failed preflight/training trials have no score. Place a rug marker below
    # the data region rather than drawing a false zero-valued result.
    scoreless = [trial for trial in run.trials if trial.selection_score is None]
    if scoreless:
        axis.scatter(
            [trial.trial_id for trial in scoreless],
            [0.035] * len(scoreless),
            transform=axis.get_xaxis_transform(),
            marker="v",
            s=38,
            color=[OUTCOME_COLORS.get(trial.outcome, OUTCOME_COLORS["unknown"]) for trial in scoreless],
            label="No selection score",
            clip_on=False,
            zorder=6,
        )

    if baseline_combined is not None:
        axis.scatter([0], [baseline_combined], marker="*", s=120, color="#4c4c4c", label="Baseline combined", zorder=6)
    axis.set_title(f"{run.run_id} — validation score trajectory", fontsize=13, weight="bold")
    axis.set_xlabel("Trial")
    axis.set_ylabel("Macro-F1")
    if run.trials:
        axis.set_xlim(-0.7, max(trial.trial_id for trial in run.trials) + 0.7)
    _style_axis(axis)
    axis.legend(loc="best", fontsize=8, ncol=2)
    figure.tight_layout()
    return figure


def plot_trial_outcomes_and_cost(run: RunData) -> Any:
    """Plot outcome taxonomy alongside training runtime and boosting rounds."""
    plt = _require_pyplot()
    figure, (outcome_axis, cost_axis) = plt.subplots(
        2,
        1,
        figsize=(11, 6.5),
        sharex=True,
        gridspec_kw={"height_ratios": (1, 2.1)},
    )

    if not run.trials:
        return _placeholder_figure(f"{run.run_id} — trial outcomes", "No completed trials are available.")

    for trial in run.trials:
        outcome_axis.bar(
            trial.trial_id,
            1,
            width=0.82,
            color=OUTCOME_COLORS.get(trial.outcome, OUTCOME_COLORS["unknown"]),
            edgecolor="white",
            linewidth=0.4,
        )
    outcome_axis.set_ylim(0, 1)
    outcome_axis.set_yticks([])
    outcome_axis.set_title("Trial outcome strip", loc="left", fontsize=11, weight="bold")
    outcome_axis.grid(False)
    outcome_axis.legend(
        handles=_legend_handles(plt, (trial.outcome for trial in run.trials)),
        loc="upper center",
        bbox_to_anchor=(0.5, -0.28),
        ncol=3,
        fontsize=8,
        frameon=False,
    )

    timed_trials = [trial for trial in run.trials if trial.elapsed_seconds is not None]
    if timed_trials:
        cost_axis.bar(
            [trial.trial_id for trial in timed_trials],
            [trial.elapsed_seconds for trial in timed_trials],
            color=[OUTCOME_COLORS.get(trial.outcome, OUTCOME_COLORS["unknown"]) for trial in timed_trials],
            width=0.75,
            alpha=0.86,
            label="Training elapsed seconds",
        )
        iteration_trials = [trial for trial in timed_trials if trial.best_iteration is not None]
        if iteration_trials:
            iteration_axis = cost_axis.twinx()
            iteration_axis.plot(
                [trial.trial_id for trial in iteration_trials],
                [trial.best_iteration for trial in iteration_trials],
                color="#1f77b4",
                marker="o",
                markersize=3.5,
                linewidth=1.2,
                label="Best iteration",
            )
            iteration_axis.set_ylabel("Best boosting iteration", color="#1f77b4")
            iteration_axis.tick_params(axis="y", labelcolor="#1f77b4")
    else:
        cost_axis.text(0.5, 0.5, "No completed training artifacts", ha="center", va="center", transform=cost_axis.transAxes)
    cost_axis.set_title("Training cost", loc="left", fontsize=11, weight="bold")
    cost_axis.set_ylabel("Elapsed seconds")
    cost_axis.set_xlabel("Trial")
    cost_axis.set_xlim(0.25, max(trial.trial_id for trial in run.trials) + 0.75)
    _style_axis(cost_axis)
    figure.suptitle(f"{run.run_id} — outcomes and training cost", fontsize=13, weight="bold")
    figure.tight_layout(rect=(0, 0.02, 1, 0.96))
    return figure


def _categorical_positions(trials: Iterable[TrialRecord], key: str) -> tuple[dict[str, int], list[str]]:
    values = sorted(
        {
            str(trial.candidate[key])
            for trial in trials
            if trial.candidate is not None and trial.candidate.get(key) is not None
        }
    )
    return {value: index for index, value in enumerate(values)}, values


def plot_hyperparameter_trajectory(run: RunData) -> Any:
    """Plot candidate values for all trials as compact parameter small multiples."""
    plt = _require_pyplot()
    if not run.trials:
        return _placeholder_figure(f"{run.run_id} — hyperparameter search", "No completed trials are available.")

    figure, axes = plt.subplots(4, 3, figsize=(15, 12), sharex=True)
    axes_flat = list(axes.flat)
    categorical = {"feature_recipe", "class_weight_mode", "grow_policy"}

    for axis, (key, label) in zip(axes_flat, PARAMETERS):
        points: list[tuple[TrialRecord, float]] = []
        labels: list[str] = []
        mapping: dict[str, int] = {}
        if key in categorical:
            mapping, labels = _categorical_positions(run.trials, key)
        for trial in run.trials:
            if trial.candidate is None or key not in trial.candidate:
                continue
            raw = trial.candidate[key]
            if key in categorical:
                value = mapping.get(str(raw))
                if value is None:
                    continue
            else:
                try:
                    value = float(raw)
                except (TypeError, ValueError):
                    continue
                if not math.isfinite(value):
                    continue
            points.append((trial, value))

        for outcome in sorted({trial.outcome for trial, _ in points}):
            subset = [(trial, value) for trial, value in points if trial.outcome == outcome]
            axis.scatter(
                [trial.trial_id for trial, _ in subset],
                [value for _, value in subset],
                color=OUTCOME_COLORS.get(outcome, OUTCOME_COLORS["unknown"]),
                edgecolor="white",
                linewidth=0.5,
                s=34,
                zorder=3,
            )
        if key in categorical:
            axis.set_yticks(range(len(labels)), labels=labels, fontsize=8)
        axis.set_title(label, fontsize=10, weight="bold")
        _style_axis(axis)

    for axis in axes_flat[-3:]:
        axis.set_xlabel("Trial")
    for axis in axes_flat:
        axis.set_xlim(0.25, max(trial.trial_id for trial in run.trials) + 0.75)
    figure.legend(
        handles=_legend_handles(plt, (trial.outcome for trial in run.trials)),
        loc="lower center",
        bbox_to_anchor=(0.5, -0.005),
        ncol=4,
        fontsize=8,
        frameon=False,
    )
    figure.suptitle(f"{run.run_id} — hyperparameter search trajectory", fontsize=13, weight="bold")
    figure.tight_layout(rect=(0, 0.045, 1, 0.96))
    return figure


def plot_selection_confirmation_gap(run: RunData) -> Any:
    """Compare selection and confirmation scores for candidates that reached both."""
    plt = _require_pyplot()
    figure, axis = plt.subplots(figsize=(7.5, 6.2))
    comparable = [
        trial
        for trial in run.trials
        if trial.selection_score is not None and trial.confirmation_score is not None
    ]
    if not comparable:
        axis.set_axis_off()
        axis.text(0.5, 0.5, "No candidates reached confirmation.", ha="center", va="center")
        axis.set_title(f"{run.run_id} — selection vs confirmation", fontsize=13, weight="bold")
        figure.tight_layout()
        return figure

    for outcome in sorted({trial.outcome for trial in comparable}):
        subset = [trial for trial in comparable if trial.outcome == outcome]
        axis.scatter(
            [trial.selection_score for trial in subset],
            [trial.confirmation_score for trial in subset],
            color=OUTCOME_COLORS.get(outcome, OUTCOME_COLORS["unknown"]),
            edgecolor="white",
            linewidth=0.7,
            s=64,
            label=OUTCOME_LABELS.get(outcome, outcome),
            zorder=3,
        )

    # Label only the decision-critical exceptions plus the strongest accepted
    # point. Labeling every confirmation candidate becomes unreadable on a
    # long Colab run while adding little explanatory value.
    notable = [trial for trial in comparable if trial.outcome == "confirmation_rejected"]
    accepted = [trial for trial in comparable if trial.accepted]
    if accepted:
        notable.append(max(accepted, key=lambda trial: trial.combined_score or float("-inf")))
    for trial in {trial.trial_id: trial for trial in notable}.values():
        axis.annotate(
            f"T{trial.trial_id}",
            (trial.selection_score, trial.confirmation_score),
            xytext=(4, 4),
            textcoords="offset points",
            fontsize=8,
        )

    baseline_selection, baseline_confirmation, _ = _baseline_scores(run)
    if baseline_selection is not None and baseline_confirmation is not None:
        axis.scatter(
            [baseline_selection],
            [baseline_confirmation],
            marker="*",
            color="#4c4c4c",
            s=150,
            label="Baseline",
            zorder=4,
        )

    values = [
        score
        for trial in comparable
        for score in (trial.selection_score, trial.confirmation_score)
        if score is not None
    ]
    if baseline_selection is not None:
        values.append(baseline_selection)
    if baseline_confirmation is not None:
        values.append(baseline_confirmation)
    low = max(0.0, min(values) - 0.02)
    high = min(1.0, max(values) + 0.02)
    axis.plot([low, high], [low, high], color="#4c4c4c", linewidth=1, linestyle="--", label="Equal scores")
    axis.set_xlim(low, high)
    axis.set_ylim(low, high)
    axis.set_aspect("equal", adjustable="box")
    axis.set_xlabel("Selection macro-F1")
    axis.set_ylabel("Confirmation macro-F1")
    axis.set_title(f"{run.run_id} — selection vs confirmation", fontsize=13, weight="bold")
    _style_axis(axis)
    axis.legend(loc="best", fontsize=8)
    figure.tight_layout()
    return figure


def _per_class_f1(metric: JsonDict | None) -> dict[int, float]:
    if not metric:
        return {}
    raw = metric.get("per_class")
    if not isinstance(raw, list):
        return {}
    result: dict[int, float] = {}
    for row in raw:
        if not isinstance(row, dict):
            continue
        class_id = _number(row, "class")
        f1 = _number(row, "f1")
        if class_id is not None and f1 is not None:
            result[int(class_id)] = f1
    return result


def plot_final_test_comparison(run: RunData) -> Any:
    """Plot only baseline vs incumbent metrics from the held-out final split."""
    plt = _require_pyplot()
    final = run.final_metrics
    if not final or final.get("ok") is False:
        return _placeholder_figure(
            f"{run.run_id} — held-out final test",
            "Final-test metrics are unavailable. Run this after FinalEval completes.",
        )
    baseline = final.get("baseline")
    best = final.get("best")
    if not isinstance(baseline, dict) or not isinstance(best, dict):
        return _placeholder_figure(
            f"{run.run_id} — held-out final test",
            "Final-test artifact does not contain baseline and incumbent metrics.",
        )

    figure, axes = plt.subplots(2, 2, figsize=(12, 8))
    aggregate_axis, loss_axis, class_axis, summary_axis = axes.flat
    labels = ["Macro-F1", "Accuracy"]
    baseline_values = [_number(baseline, "macro_f1") or 0.0, _number(baseline, "accuracy") or 0.0]
    best_values = [_number(best, "macro_f1") or 0.0, _number(best, "accuracy") or 0.0]
    positions = list(range(len(labels)))
    width = 0.36
    aggregate_axis.bar([position - width / 2 for position in positions], baseline_values, width, label="Baseline", color="#7f7f7f")
    aggregate_axis.bar([position + width / 2 for position in positions], best_values, width, label="Final incumbent", color="#2ca02c")
    aggregate_axis.set_xticks(positions, labels=labels)
    aggregate_axis.set_ylim(0, 1.0)
    aggregate_axis.set_ylabel("Score")
    aggregate_axis.set_title("Aggregate final metrics", fontsize=10, weight="bold")
    aggregate_axis.legend(fontsize=8)
    _style_axis(aggregate_axis)

    baseline_loss = _number(baseline, "log_loss")
    best_loss = _number(best, "log_loss")
    if baseline_loss is not None and best_loss is not None:
        loss_axis.bar(["Baseline", "Final incumbent"], [baseline_loss, best_loss], color=["#7f7f7f", "#2ca02c"])
        loss_axis.set_ylabel("Multiclass log loss (lower is better)")
        _style_axis(loss_axis)
    else:
        loss_axis.set_axis_off()
        loss_axis.text(0.5, 0.5, "Log-loss data unavailable", ha="center", va="center")
    loss_axis.set_title("Held-out final log loss", fontsize=10, weight="bold")

    baseline_classes = _per_class_f1(baseline)
    best_classes = _per_class_f1(best)
    class_ids = sorted(set(baseline_classes) | set(best_classes))
    if class_ids:
        class_positions = list(range(len(class_ids)))
        class_axis.bar(
            [position - width / 2 for position in class_positions],
            [baseline_classes.get(class_id, 0.0) for class_id in class_ids],
            width,
            label="Baseline",
            color="#7f7f7f",
        )
        class_axis.bar(
            [position + width / 2 for position in class_positions],
            [best_classes.get(class_id, 0.0) for class_id in class_ids],
            width,
            label="Final incumbent",
            color="#2ca02c",
        )
        class_axis.set_xticks(class_positions, labels=[f"Class {class_id}" for class_id in class_ids])
        class_axis.set_ylim(0, 1.0)
        class_axis.set_ylabel("F1")
        class_axis.legend(fontsize=8)
        _style_axis(class_axis)
    else:
        class_axis.set_axis_off()
        class_axis.text(0.5, 0.5, "Per-class final metrics unavailable", ha="center", va="center")
    class_axis.set_title("Per-class final F1", fontsize=10, weight="bold")

    baseline_validation = _number(baseline, "validation_combined_score")
    best_validation = _number(best, "validation_combined_score")
    final_best = _number(best, "macro_f1")
    delta = _number(final.get("delta") if isinstance(final.get("delta"), dict) else None, "macro_f1")
    lines = [
        "Held-out final split",
        "", 
        f"Baseline final macro-F1: {(_number(baseline, 'macro_f1') or 0.0):.6f}",
        f"Incumbent final macro-F1: {(final_best or 0.0):.6f}",
    ]
    if delta is not None:
        lines.append(f"Final macro-F1 delta: {delta:+.6f}")
    if best_validation is not None and final_best is not None:
        lines.append(f"Incumbent validation-to-test gap: {final_best - best_validation:+.6f}")
    if baseline_validation is not None:
        lines.append(f"Baseline validation score: {baseline_validation:.6f}")
    summary_axis.set_axis_off()
    summary_axis.text(0.04, 0.94, "\n".join(lines), va="top", fontsize=10)
    summary_axis.set_title("Final-test summary", fontsize=10, weight="bold")

    figure.suptitle(f"{run.run_id} — held-out final test (evaluated after search)", fontsize=13, weight="bold")
    figure.tight_layout(rect=(0, 0, 1, 0.95))
    return figure


def build_summary(run: RunData) -> JsonDict:
    """Build a JSON-serializable overview without writing any artifact."""
    baseline_selection, baseline_confirmation, baseline_combined = _baseline_scores(run)
    outcome_counts = Counter(trial.outcome for trial in run.trials)
    accepted = [trial for trial in run.trials if trial.accepted]
    best_trial: TrialRecord | None = None
    if accepted:
        best_trial = max(accepted, key=lambda trial: _accepted_score(trial) or float("-inf"))
    final_best: JsonDict | None = None
    if run.final_metrics and isinstance(run.final_metrics.get("best"), dict):
        final_best = run.final_metrics["best"]
    return {
        "run_id": run.run_id,
        "run_dir": str(run.path),
        "trial_count": len(run.trials),
        "evaluated_trials": sum(trial.selection_score is not None for trial in run.trials),
        "accepted_trials": len(accepted),
        "outcome_counts": dict(sorted(outcome_counts.items())),
        "baseline": {
            "selection_score": baseline_selection,
            "confirmation_score": baseline_confirmation,
            "combined_score": baseline_combined,
        },
        "best_incumbent": {
            "trial": best_trial.trial_id if best_trial else None,
            "combined_score": _accepted_score(best_trial) if best_trial else baseline_combined,
        },
        "total_training_seconds": round(
            sum(trial.elapsed_seconds or 0.0 for trial in run.trials),
            6,
        ),
        "final_test": {
            "available": final_best is not None,
            "macro_f1": _number(final_best, "macro_f1") if final_best else None,
            "accuracy": _number(final_best, "accuracy") if final_best else None,
            "validation_combined_score": _number(final_best, "validation_combined_score") if final_best else None,
        },
    }


def plot_run(
    run_dir: str | Path,
    *,
    show: bool = True,
    strict: bool = False,
) -> PlotDashboard:
    """Render the five-chart inline dashboard and return its figures/summary.

    Args:
        run_dir: Path such as ``runs/20260719T075416Z-b08eef4e``.
        show: Call ``matplotlib.pyplot.show()`` after building the figures.
            Keep this true in Colab; tests and batch callers can set false.
        strict: Treat missing optional trial artifacts and consistency warnings
            as errors.
    """
    run = load_run(run_dir, strict=strict)
    figures = {
        "score_trajectory": plot_score_trajectory(run),
        "trial_outcomes_and_cost": plot_trial_outcomes_and_cost(run),
        "hyperparameter_trajectory": plot_hyperparameter_trajectory(run),
        "selection_confirmation_gap": plot_selection_confirmation_gap(run),
        "final_test_comparison": plot_final_test_comparison(run),
    }
    dashboard = PlotDashboard(figures=figures, summary=build_summary(run), run=run)
    if show:
        _require_pyplot().show()
    return dashboard


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run_dir", type=Path, help="Completed run directory, for example runs/<run-id>")
    parser.add_argument("--no-show", action="store_true", help="Build plots but do not call matplotlib.pyplot.show()")
    parser.add_argument("--strict", action="store_true", help="Treat incomplete optional artifacts as errors")
    parser.add_argument("--print-summary", action="store_true", help="Print the returned JSON summary")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        dashboard = plot_run(args.run_dir, show=not args.no_show, strict=args.strict)
    except (RunArtifactError, RuntimeError) as exc:
        print(f"error: {exc}")
        return 2
    if args.print_summary:
        print(json.dumps(dashboard.summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
