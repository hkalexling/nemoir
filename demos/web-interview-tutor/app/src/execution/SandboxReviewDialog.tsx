/**
 * NemoIR Interview Tutor -- `SandboxReviewDialog` component.
 *
 * Phase 2 -- execution domain.
 *
 * A custom confirmation renderer designed to slot into
 * `WebUiHostProvider.renderConfirm`.  It satisfies the `ConfirmRenderer`
 * contract (`FC<ConfirmRendererProps>`) and goes beyond the default
 * Yes/No dialog by also displaying:
 *
 * - The runtime-provided `message` (the sandbox harness source code)
 * - The current learner submission snapshot
 * - A public test summary
 * - Explicit **Approve**, **Deny**, and **Cancel** buttons
 *
 * Review context (submission source, tests, entry-function name) arrives
 * via React context, *not* by parsing the runtime message, so the dialog
 * remains decoupled from the workflow's prompt format.
 *
 * Accessibility: the dialog is a `role="dialog"` with `aria-modal`, clear
 * headings, and labelled controls.
 */

import {
  createContext,
  useContext,
  useState,
  type FC,
} from "react";
import type { ConfirmRenderer, ConfirmRendererProps } from "@nemoir/web-ui";
import type { ExecutionLimits, VisibleTest } from "../catalog/types.js";
import { formatJsonForDisplay } from "./TestResultsPanel.js";

// ---------------------------------------------------------------------------
// Review context
// ---------------------------------------------------------------------------

/**
 * Extra review information provided by the app when it configures
 * the `SandboxReviewDialog` as the `renderConfirm` renderer.
 *
 * The dialog reads this data from context; it never attempts to extract
 * submission metadata from the runtime message string.
 */
export interface SandboxReview {
  /** The learner's current source code (the submission being reviewed). */
  readonly submissionSource: string;

  /** The entry function name for the current problem. */
  readonly entryFunctionName: string;

  /** The visible test cases bundled with this run. */
  readonly tests: readonly VisibleTest[];

  /** Product limits passed to the sandbox runner for this problem. */
  readonly executionLimits: ExecutionLimits;
}

const SandboxReviewContext = createContext<SandboxReview | null>(null);

/**
 * Provide sandbox review data to any `SandboxReviewDialog` rendered
 * inside this provider.
 *
 * Usage:
 * ```tsx
 * <SandboxReviewProvider review={{ submissionSource, entryFunctionName, tests }}>
 *   <WebUiHostProvider renderConfirm={SandboxReviewDialog}>
 *     {children}
 *   </WebUiHostProvider>
 * </SandboxReviewProvider>
 * ```
 */
export const SandboxReviewProvider: FC<{
  readonly review: SandboxReview | null;
  readonly children: React.ReactNode;
}> = ({ review, children }) => (
  <SandboxReviewContext.Provider value={review}>
    {children}
  </SandboxReviewContext.Provider>
);

/** Retrieve the current review context.  Returns `null` when unavailable. */
function useSandboxReview(): SandboxReview | null {
  return useContext(SandboxReviewContext);
}

// ---------------------------------------------------------------------------
// Dialog component
// ---------------------------------------------------------------------------

/**
 * A rich sandbox-review confirmation dialog.
 *
 * Satisfies `ConfirmRenderer` so it can be passed directly as
 * `WebUiHostProvider`'s `renderConfirm` prop.
 */
export const SandboxReviewDialog: ConfirmRenderer = ({
  message,
  onResolve,
  onReject,
}: ConfirmRendererProps) => {
  const review = useSandboxReview();
  // The runtime-provided source is intentionally visible on first render:
  // the confirmation must show exactly what the sandbox will execute.
  const [harnessExpanded, setHarnessExpanded] = useState(true);

  const handleApprove = () => onResolve(true);
  const handleDeny = () => onResolve(false);
  const handleCancel = () =>
    onReject(new DOMException("Cancelled by user", "AbortError"));

  return (
    <div
      className="nemoir-modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sandbox-review-title"
    >
      <div className="nemoir-modal sandbox-review-dialog">
        <h2 id="sandbox-review-title">Review Sandbox Execution</h2>

        <p className="sandbox-review-lede">
          The deterministic test-runner workflow is about to execute your
          code in an isolated sandbox.  Please review the details below
          before approving.
        </p>

        {/* ---- Learner submission ---- */}
        {review && (
          <section
            className="sandbox-review-section"
            aria-labelledby="sandbox-review-submission-title"
          >
            <h3 id="sandbox-review-submission-title">Your Submission</h3>
            <div className="sandbox-review-meta">
              <span>
                Entry function: <code>{review.entryFunctionName}</code>
              </span>
            </div>
            <pre className="sandbox-review-code">
              <code>{review.submissionSource}</code>
            </pre>
          </section>
        )}

        {/* ---- Public test summary ---- */}
        {review && review.tests.length > 0 && (
          <section
            className="sandbox-review-section"
            aria-labelledby="sandbox-review-tests-title"
          >
            <h3 id="sandbox-review-tests-title">
              Public Tests ({review.tests.length})
            </h3>
            <ul className="sandbox-review-test-list">
              {review.tests.map((t) => (
                <li key={t.id} className="sandbox-review-test-item">
                  <span className="test-id">
                    <code>{t.id}</code>
                  </span>
                  <span className="test-args">
                    args:{" "}
                    <code>{formatJsonForDisplay(t.args)}</code>
                  </span>
                  <span className="test-expected">
                    expected:{" "}
                    <code>{formatJsonForDisplay(t.expected)}</code>
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* ---- Runtime message (harness source) ---- */}
        <section
          className="sandbox-review-section"
          aria-labelledby="sandbox-review-harness-title"
        >
          <h3 id="sandbox-review-harness-title">
            <button
              type="button"
              className="sandbox-review-toggle"
              onClick={() => setHarnessExpanded((v) => !v)}
              aria-expanded={harnessExpanded}
              aria-controls="sandbox-review-harness-content"
            >
              {harnessExpanded ? "▾" : "▸"} Evaluator Harness Source
            </button>
          </h3>
          {harnessExpanded && (
            <pre id="sandbox-review-harness-content" className="sandbox-review-code">
              <code>{message}</code>
            </pre>
          )}
        </section>

        {/* ---- Disclaimer ---- */}
        {review && (
          <p className="sandbox-review-disclaimer">
            Your code will run in an isolated sandbox with no network, file
            system, or host-page access. It is subject to a{" "}
            <strong>{(review.executionLimits.timeoutMs / 1000).toLocaleString()}-second timeout</strong>, a{" "}
            <strong>{Math.round(review.executionLimits.maxSourceBytes / 1024)} KiB submission limit</strong>, and JSON-only input/output.
          </p>
        )}

        {/* ---- Actions ---- */}
        <div className="nemoir-modal-actions">
          <button
            type="button"
            className="nemoir-primary"
            onClick={handleApprove}
            autoFocus
          >
            Approve &amp; Run
          </button>
          <button type="button" onClick={handleDeny}>
            Deny
          </button>
          <button type="button" onClick={handleCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Re-export the ConfirmRendererProps type for convenience
// ---------------------------------------------------------------------------

export type { ConfirmRendererProps };
