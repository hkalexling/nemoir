/**
 * NemoIR Interview Tutor -- `TestResultsPanel` React component.
 *
 * Phase 2 -- execution domain.
 *
 * Renders all learner-facing test outcomes accessibly:
 * - Status (passed / failed / syntax_error / runtime_error / timeout)
 * - Summary counts (passed / total)
 * - Elapsed wall-clock ms
 * - Optional first-failure detail with JSON expected / actual / error
 * - Fresh / stale indicator
 * - Retry and cancel action buttons
 *
 * The component owns **no** app state.  All data arrives via props;
 * callbacks (`onRetry`, `onCancel`) fire in response to user interaction
 * but do not manage the run lifecycle themselves.
 */

import { type FC } from "react";
import type { RunReport } from "../catalog/types.js";
import type { RunOutcome } from "./run-test-workflow.js";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface TestResultsPanelProps {
  /** The harness report (null when no run has completed). */
  readonly report: RunReport | null;

  /**
   * Terminal state for a run which could not produce a normal harness report.
   * A timeout, cancellation, and policy denial are meaningful learner-facing
   * outcomes rather than generic runner errors.
   */
  readonly outcome?: RunOutcome | null;

  /**
   * When `true`, the editor contents have changed since the report was
   * produced and the displayed results are out of date.
   */
  readonly stale: boolean;

  /** `true` while a run is in progress. */
  readonly running: boolean;

  /**
   * Error message from a failed run (infrastructure error, not learner
   * error).  Rendered separately from the harness report.
   */
  readonly error: string | null;

  /** Fired when the learner clicks Retry / Run Tests. */
  readonly onRetry?: () => void;

  /** Fired when the learner clicks Cancel while a run is in flight. */
  readonly onCancel?: () => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const TestResultsPanel: FC<TestResultsPanelProps> = ({
  report,
  outcome = null,
  stale,
  running,
  error,
  onRetry,
  onCancel,
}) => {
  return (
    <section
      className="test-results-panel"
      aria-labelledby="test-results-title"
      aria-live="polite"
      aria-atomic="true"
    >
      <div className="test-results-header">
        <h2 id="test-results-title">Test Results</h2>
        <StatusBadge
          report={report}
          outcome={outcome}
          stale={stale}
          running={running}
          error={error}
        />
      </div>

      {/* Running indicator */}
      {running && (
        <div className="test-results-running" role="status">
          <span className="spinner" aria-hidden="true" />
          Running tests&hellip;
        </div>
      )}

      {/* Stale warning */}
      {stale && !running && (
        <div className="test-results-stale" role="alert">
          Results are out of date.  Your code has changed since the last
          run.  Run tests again to see current results.
        </div>
      )}

      {/* Terminal workflow outcomes that do not have a harness report. */}
      {outcome && outcome.kind !== "completed" && !running && (
        <OutcomeNotice outcome={outcome} />
      )}

      {/* Infrastructure error (not a learner error) */}
      {error && !running && outcome?.kind !== "infrastructure_error" && (
        <div className="test-results-error" role="alert">
          <strong>Runner error:</strong> {error}
        </div>
      )}

      {/* Harness report */}
      {report && !running && (
        <div className="test-results-report">
          <ReportSummary report={report} />
          <ReportFirstFailure report={report} />
        </div>
      )}

      {/* Empty state */}
      {!report && !outcome && !running && !error && (
        <p className="test-results-empty">
          Run your code to see test results.
        </p>
      )}

      {/* Actions */}
      <div className="test-results-actions">
        {running && onCancel && (
          <button
            type="button"
            className="btn-cancel"
            onClick={onCancel}
            aria-label="Cancel test run"
          >
            Cancel
          </button>
        )}
        {!running && onRetry && (report || outcome || error) && (
          <button
            type="button"
            className="btn-retry"
            onClick={onRetry}
            aria-label={stale ? "Re-run tests with current code" : "Run tests again"}
          >
            {report || outcome ? "Run Again" : "Run Tests"}
          </button>
        )}
      </div>
    </section>
  );
};

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Small pill showing the current status. */
const StatusBadge: FC<{
  report: RunReport | null;
  outcome: RunOutcome | null;
  stale: boolean;
  running: boolean;
  error: string | null;
}> = ({ report, outcome, stale, running, error }) => {
  if (running) {
    return (
      <span className="status-badge status-running" aria-label="Tests are running">
        Running
      </span>
    );
  }

  if (outcome && outcome.kind !== "completed") {
    switch (outcome.kind) {
      case "sandbox_timeout":
        return <span className="status-badge status-timeout">Timeout</span>;
      case "cancelled":
        return <span className="status-badge status-cancelled">Cancelled</span>;
      case "policy_denied":
        return <span className="status-badge status-denied">Not run</span>;
      case "infrastructure_error":
        return <span className="status-badge status-error">Error</span>;
    }
  }

  if (error) {
    return (
      <span className="status-badge status-error" aria-label="Runner error">
        Error
      </span>
    );
  }

  if (!report) {
    return (
      <span className="status-badge status-idle" aria-label="No test results yet">
        &mdash;
      </span>
    );
  }

  const label = statusLabel(report.status);
  const className = stale
    ? "status-badge status-stale"
    : `status-badge status-${report.status}`;

  return (
    <span className={className} aria-label={`${label}${stale ? " (stale)" : ""}`}>
      {label}
      {stale && <span className="stale-marker"> (stale)</span>}
    </span>
  );
};

/** Pass / total count + elapsed time. */
const ReportSummary: FC<{ report: RunReport }> = ({ report }) => (
  <div className="report-summary" aria-label="Test summary">
    <span className="report-count">
      <strong>{report.passed}</strong> / {report.total} passed
    </span>
    <span className="report-elapsed" aria-label={`${report.elapsedMs} milliseconds elapsed`}>
      in {report.elapsedMs} ms
    </span>
  </div>
);

/** First-failure detail block with JSON values. */
const ReportFirstFailure: FC<{ report: RunReport }> = ({ report }) => {
  if (!report.firstFailure) return null;

  const ff = report.firstFailure;

  return (
    <details className="first-failure" open>
      <summary>
        First failure: <code>{ff.testId}</code>
      </summary>
      <dl className="failure-detail" aria-label="Failure details">
        <div>
          <dt>Arguments</dt>
          <dd>
            <JsonValueDisplay value={ff.args} />
          </dd>
        </div>
        <div>
          <dt>Expected</dt>
          <dd>
            <JsonValueDisplay value={ff.expected} />
          </dd>
        </div>
        {ff.actual !== undefined && (
          <div>
            <dt>Actual</dt>
            <dd>
              <JsonValueDisplay value={ff.actual} />
            </dd>
          </div>
        )}
        {ff.error && (
          <div>
            <dt>Error</dt>
            <dd>
              <pre className="failure-error">{ff.error}</pre>
            </dd>
          </div>
        )}
      </dl>
    </details>
  );
};

// ---------------------------------------------------------------------------
// Outcome and display helpers
// ---------------------------------------------------------------------------

const OutcomeNotice: FC<{ outcome: Exclude<RunOutcome, { kind: "completed" }> }> = ({ outcome }) => {
  switch (outcome.kind) {
    case "sandbox_timeout":
      return (
        <div className="test-results-timeout" role="alert">
          Your code did not finish before the sandbox time limit. Check for an
          infinite loop or unexpectedly expensive work, then try again.
        </div>
      );
    case "cancelled":
      return <div className="test-results-cancelled" role="status">Test run cancelled.</div>;
    case "policy_denied":
      return (
        <div className="test-results-denied" role="status">
          Test run was not started because sandbox execution was not approved.
        </div>
      );
    case "infrastructure_error":
      return (
        <div className="test-results-error" role="alert">
          <strong>Runner error:</strong> {outcome.error}
        </div>
      );
  }
};

/** Compact, accessibly-labelled JSON display. */
const JsonValueDisplay: FC<{ value: unknown }> = ({ value }) => (
  <code
    className="json-value"
    aria-label={formatJsonForScreenReader(value)}
  >
    {formatJsonForDisplay(value)}
  </code>
);

/** Format a JSON value for inline display.  Exported for reuse by other execution-domain components. */
export function formatJsonForDisplay(value: unknown): string {
  try {
    if (typeof value === "string") {
      return JSON.stringify(value);
    }
    return JSON.stringify(value, null, 0);
  } catch {
    return String(value);
  }
}

/** Format a JSON value for screen-reader announcement. */
function formatJsonForScreenReader(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string") return `string "${value}"`;
  if (typeof value === "number") return `number ${value}`;
  if (typeof value === "boolean") return `boolean ${value}`;
  if (Array.isArray(value)) {
    const items = value.map(formatJsonForScreenReader).join(", ");
    return `array with ${value.length} items: ${items}`;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .map(([k, v]) => `${k}: ${formatJsonForScreenReader(v)}`)
      .join("; ");
    return `object with ${Object.keys(value as object).length} keys: ${entries}`;
  }
  return String(value);
}

/** Human-readable label for each report status. */
function statusLabel(status: string): string {
  switch (status) {
    case "passed":
      return "All Passed";
    case "failed":
      return "Failed";
    case "syntax_error":
      return "Syntax Error";
    case "runtime_error":
      return "Runtime Error";
    case "timeout":
      return "Timeout";
    default:
      return status;
  }
}
