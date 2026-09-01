/**
 * NemoIR Interview Tutor -- `HintPanel` component.
 *
 * Phase 3 -- tutoring domain.
 *
 * Renders learner-facing tutoring output accessibly.  The panel never
 * renders raw tagged JSON deltas or model intermediate text -- it only
 * displays validated guidance, partial progress (e.g. a running state),
 * and error / cancellation states.
 *
 * The component owns **no** app state.  All data arrives via props;
 * callbacks (`onCancel`, `onDismiss`) fire in response to user
 * interaction but do not manage the run lifecycle themselves.
 */

import { type FC } from "react";
import type { ValidatedGuidance } from "./tutor-validation.js";
import type { HintLevel } from "./tutor-request.js";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface HintPanelProps {
  /** Validated guidance to display (null when no guidance is available). */
  readonly guidance: ValidatedGuidance | null;

  /**
   * A partial preview string for in-progress model output.  When set,
   * the panel shows a "generating…" indicator alongside this text.
   *
   * This text is never treated as authoritative guidance -- it is a
   * preview only and the panel does not attempt to parse or structure
   * it.
   */
  readonly preview: string | null;

  /** `true` while a tutoring run is in progress. */
  readonly running: boolean;

  /**
   * Error message from a failed run (infrastructure error, not learner
   * error).
   */
  readonly error: string | null;

  /** `true` when the run was cancelled by the learner. */
  readonly cancelled: boolean;

  /** Fired when the learner clicks Cancel while a run is in flight. */
  readonly onCancel?: () => void;

  /** Fired when the learner dismisses the current guidance/error. */
  readonly onDismiss?: () => void;

  /**
   * Callback for requesting a new hint at a different level.
   * The panel renders hint-level adjustment controls when this is
   * provided and guidance is visible.
   */
  readonly onRequestHintLevel?: (level: Exclude<HintLevel, "review">) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const HintPanel: FC<HintPanelProps> = ({
  guidance,
  preview,
  running,
  error,
  cancelled,
  onCancel,
  onDismiss,
  onRequestHintLevel,
}) => {
  return (
    <section
      className="hint-panel"
      aria-labelledby="hint-panel-title"
      aria-live="polite"
      aria-atomic="true"
    >
      <div className="hint-panel-header">
        <h2 id="hint-panel-title">Tutor Guidance</h2>
        <GuidanceBadge
          guidance={guidance}
          running={running}
          error={error}
          cancelled={cancelled}
        />
      </div>

      {/* Running indicator */}
      {running && (
        <div className="hint-panel-running" role="status">
          <span className="spinner" aria-hidden="true" />
          Analysing your submission&hellip;
        </div>
      )}

      {/* Preview (in-progress model output) */}
      {preview && running && (
        <div className="hint-panel-preview" aria-label="In-progress model output">
          <em>Preview:</em>
          <p className="hint-preview-text">{preview}</p>
        </div>
      )}

      {/* Cancelled notice */}
      {cancelled && !running && (
        <div className="hint-panel-cancelled" role="status">
          Tutoring request was cancelled.
        </div>
      )}

      {/* Error notice */}
      {error && !running && (
        <div className="hint-panel-error" role="alert">
          <strong>Tutor error:</strong> {error}
        </div>
      )}

      {/* Validated guidance */}
      {guidance && !running && (
        <GuidanceDisplay
          guidance={guidance}
          onRequestHintLevel={onRequestHintLevel}
        />
      )}

      {/* Empty state */}
      {!guidance && !preview && !running && !error && !cancelled && (
        <p className="hint-panel-empty">
          Run your tests first, then request a hint.  The tutor uses your test
          results to give you targeted guidance.
        </p>
      )}

      {/* Actions */}
      <div className="hint-panel-actions">
        {running && onCancel && (
          <button
            type="button"
            className="btn-cancel"
            onClick={onCancel}
            aria-label="Cancel tutoring request"
          >
            Cancel
          </button>
        )}
        {!running && (guidance || error || cancelled) && onDismiss && (
          <button
            type="button"
            className="btn-dismiss"
            onClick={onDismiss}
            aria-label="Dismiss tutor guidance"
          >
            Dismiss
          </button>
        )}
      </div>
    </section>
  );
};

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Small pill showing the current tutoring status. */
const GuidanceBadge: FC<{
  guidance: ValidatedGuidance | null;
  running: boolean;
  error: string | null;
  cancelled: boolean;
}> = ({ guidance, running, error, cancelled }) => {
  if (running) {
    return (
      <span className="status-badge status-running" aria-label="Tutor is analysing">
        Analysing
      </span>
    );
  }

  if (cancelled) {
    return (
      <span className="status-badge status-cancelled">Cancelled</span>
    );
  }

  if (error) {
    return (
      <span className="status-badge status-error" aria-label="Tutor error">
        Error
      </span>
    );
  }

  if (!guidance) {
    return (
      <span className="status-badge status-idle" aria-label="No guidance available">
        &mdash;
      </span>
    );
  }

  const isReview = guidance.mode === "success_review";
  return (
    <span
      className={`status-badge ${isReview ? "status-review" : "status-hint"}`}
      aria-label={isReview ? "Success review" : "Hint"}
    >
      {isReview ? "Review" : "Hint"}
    </span>
  );
};

// ---------------------------------------------------------------------------
// Guidance display
// ---------------------------------------------------------------------------

interface GuidanceDisplayProps {
  readonly guidance: ValidatedGuidance;
  readonly onRequestHintLevel?: (level: Exclude<HintLevel, "review">) => void;
}

const GuidanceDisplay: FC<GuidanceDisplayProps> = ({
  guidance,
  onRequestHintLevel,
}) => {
  const isReview = guidance.mode === "success_review";

  return (
    <div className="guidance-display" aria-label="Tutor guidance">
      {/* Hint / review text */}
      <section aria-labelledby="guidance-hint-title">
        <h3 id="guidance-hint-title">
          {isReview ? "Review" : "Hint"}
        </h3>
        <p className="guidance-hint-text">{guidance.hint}</p>
      </section>

      {/* Central concept */}
      <section aria-labelledby="guidance-concept-title">
        <h3 id="guidance-concept-title">Concept</h3>
        <p className="guidance-concept-text">
          <strong>{guidance.concept}</strong>
        </p>
      </section>

      {/* Next steps */}
      <section aria-labelledby="guidance-steps-title">
        <h3 id="guidance-steps-title">Next Steps</h3>
        <ol className="guidance-steps-list">
          {guidance.next_steps.map((step, idx) => (
            <li key={idx}>{step}</li>
          ))}
        </ol>
      </section>

      {/* Hint-level adjustment (only for non-review mode) */}
      {!isReview && onRequestHintLevel && (
        <div className="guidance-hint-levels" aria-label="Request a different hint level">
          <span className="hint-level-label">Need more detail?</span>
          <div className="hint-level-buttons">
            <button
              type="button"
              className="btn-hint-level"
              onClick={() => onRequestHintLevel("nudge")}
            >
              Nudge
            </button>
            <button
              type="button"
              className="btn-hint-level"
              onClick={() => onRequestHintLevel("targeted")}
            >
              Targeted
            </button>
            <button
              type="button"
              className="btn-hint-level"
              onClick={() => onRequestHintLevel("plan")}
            >
              Plan
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
