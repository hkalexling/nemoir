/**
 * NemoIR Interview Tutor -- test-runner factory / adapter around the
 * generated `InterviewTestRunner` Agent.
 *
 * Phase 2 -- execution domain.
 *
 * Goals:
 * - Wrap the generated `Agent.stream()` with uiHost + opaque sandbox
 * - Classify terminal outcomes from the event stream
 * - Provide robust type-guards for untrusted sandbox output
 * - Never manufacture a normal harness report for an outer timeout
 *
 * This module is framework-neutral (no React).  It exports a runner
 * function compatible with `WorkflowRunner` from `@nemoir/web-ui`, plus
 * standalone helpers for outcome classification.
 */

import { Agent } from "../generated/interview-test-runner/src/agent.js";
import {
  createOpaqueOriginJsSandbox,
  type WorkflowEvent,
  type WebUiHost,
  type SandboxedJsRunner,
} from "@nemoir/web-runtime";
import type { AttemptBundle, RunReport } from "../catalog/types.js";
import { assertBundleValid, isRunReport } from "./attempt.js";

// ---------------------------------------------------------------------------
// Outcome types
// ---------------------------------------------------------------------------

/**
 * Discriminated union of every terminal outcome the runner can produce.
 *
 * The design intentionally separates a *completed harness report* from
 * an *outer sandbox timeout*: the runtime terminates the iframe/worker
 * tree on timeout, so the harness cannot return a normal report.
 */
export type RunOutcome =
  | { readonly kind: "completed"; readonly report: RunReport }
  | { readonly kind: "sandbox_timeout" }
  | { readonly kind: "cancelled" }
  | { readonly kind: "policy_denied" }
  | { readonly kind: "infrastructure_error"; readonly error: string };

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

/**
 * Classify a terminal outcome from the collected workflow events.
 *
 * Inspects the final event(s) and any earlier policy-denial markers to
 * determine what happened.
 *
 * The function is pure and synchronous: it examines already-collected
 * `WorkflowEvent[]` and delegates to guards.  It does **not** fabricate a
 * report for timeouts that prevented the harness from returning one.
 */
export function classifyOutcome(
  events: readonly WorkflowEvent[],
): RunOutcome | null {
  if (events.length === 0) return null;

  // Check for policy denial -- can appear before run_failed
  for (const e of events) {
    if (e.kind === "policy_denied") {
      return { kind: "policy_denied" };
    }
  }

  // Check for sandbox timeout (tool_call_failed with timeout message)
  for (const e of events) {
    if (e.kind === "tool_call_failed") {
      const err = e.error ?? "";
      if (/timed?\s*out/i.test(err) || /aborted/i.test(err)) {
        // Distinguish sandbox timeout from user cancellation by checking
        // for a preceding abort event or explicit cancellation.
        const isCancel = events.some(
          (ev) =>
            ev.kind === "run_failed" &&
            ev.error !== null &&
            /abort|cancel/i.test(ev.error ?? ""),
        );
        if (isCancel) return { kind: "cancelled" };

        // The runtime error message for sandbox timeout is "browser.js.sandbox
        // timed out after Nms". Treat that as sandbox_timeout.
        if (/timed?\s*out/i.test(err)) {
          return { kind: "sandbox_timeout" };
        }

        // An abort that is not tied to the learner's cancellation remains an
        // operational failure, but preserve the useful tool message instead
        // of falling through to a misleading stream-end error.
        return {
          kind: "infrastructure_error",
          error: `Sandbox execution aborted: ${err || "unknown error"}`,
        };
      }
    }
  }

  // Check the final event
  const last = events[events.length - 1];

  if (last.kind === "run_completed") {
    const output = extractReportFromResult(last.result);
    if (output && isRunReport(output)) {
      return { kind: "completed", report: output };
    }
    // The run completed but we couldn't extract a valid report -- treat
    // as infrastructure error.
    return {
      kind: "infrastructure_error",
      error: "Workflow completed but did not produce a valid run report.",
    };
  }

  if (last.kind === "run_failed") {
    const err = last.error ?? "";

    // User cancellation
    if (/abort|cancel/i.test(err)) {
      return { kind: "cancelled" };
    }

    // Sandbox timeout (harness couldn't return a report)
    if (/timed?\s*out/i.test(err)) {
      return { kind: "sandbox_timeout" };
    }

    // Any other failure is infrastructure
    return { kind: "infrastructure_error", error: err || "Unknown workflow error" };
  }

  // Stream ended without a terminal event -- unexpected
  return {
    kind: "infrastructure_error",
    error: "Workflow stream ended without a terminal event.",
  };
}

/**
 * Extract the run report from a workflow result value.
 *
 * The result may be `WorkflowResult` (nested `.output.report`) or a
 * bare record (direct `.report`).  This is intentionally defensive
 * because the shape depends on the generated agent version.
 */
function extractReportFromResult(
  result: unknown,
): unknown {
  if (result === null || typeof result !== "object") return null;

  const r = result as Record<string, unknown>;

  // Direct `.report` field (AgentResult-style or bare stage output)
  if ("report" in r && r.report !== undefined) {
    return r.report;
  }

  // Nested `.output.report` (WorkflowResult-style)
  if (
    "output" in r &&
    r.output !== null &&
    typeof r.output === "object"
  ) {
    const out = r.output as Record<string, unknown>;
    if ("report" in out) return out.report;
  }

  return null;
}

// ---------------------------------------------------------------------------
// Event-based outcome detection (live-stream helper)
// ---------------------------------------------------------------------------

/**
 * Inspect a single `WorkflowEvent` and return an outcome if this event is
 * terminal.
 *
 * Useful for live-stream consumers that want to short-circuit on the
 * terminal event rather than buffering all events.
 */
export function terminalOutcomeFromEvent(
  event: WorkflowEvent,
  priorEvents?: readonly WorkflowEvent[],
): RunOutcome | null {
  // Policy denial
  if (event.kind === "policy_denied") {
    return { kind: "policy_denied" };
  }

  // Tool failure -- may be timeout
  if (event.kind === "tool_call_failed") {
    const err = event.error ?? "";
    if (/timed?\s*out/i.test(err) || /aborted/i.test(err)) {
      // Check prior events for cancellation.
      const wasCancelled = priorEvents?.some(
        (e) => e.kind === "run_failed" && /abort|cancel/i.test(e.error ?? ""),
      );
      if (wasCancelled) return { kind: "cancelled" };
      if (/timed?\s*out/i.test(err)) return { kind: "sandbox_timeout" };
      return {
        kind: "infrastructure_error",
        error: `Sandbox execution aborted: ${err || "unknown error"}`,
      };
    }
  }

  // Completed
  if (event.kind === "run_completed") {
    const output = extractReportFromResult(event.result);
    if (output && isRunReport(output)) {
      return { kind: "completed", report: output };
    }
    return {
      kind: "infrastructure_error",
      error: "Workflow completed but did not produce a valid run report.",
    };
  }

  // Failed
  if (event.kind === "run_failed") {
    const err = event.error ?? "";
    if (/abort|cancel/i.test(err)) return { kind: "cancelled" };
    if (/timed?\s*out/i.test(err)) return { kind: "sandbox_timeout" };
    return { kind: "infrastructure_error", error: err || "Unknown workflow error" };
  }

  return null;
}

// ---------------------------------------------------------------------------
// Test-runner factory
// ---------------------------------------------------------------------------

/**
 * Options for `createTestRunner`.
 *
 * All fields are optional so callers that don't need customisation get
 * sensible defaults.
 */
export interface TestRunnerOptions {
  /** UI host for the `user.confirm` policy prompt. */
  readonly uiHost?: WebUiHost;

  /**
   * Custom sandbox runner.  When omitted, a default `OpaqueOriginJsSandbox`
   * is created with the standard runtime limits.
   */
  readonly sandboxRunner?: SandboxedJsRunner;

  /** Override the default sandbox timeout (ms). */
  readonly sandboxTimeoutMs?: number;

  /**
   * An `AbortController` that the caller can signal for cooperative
   * cancellation across runs.  When omitted, each call to `run()` manages
   * its own controller internally.
   */
  readonly externalAbortController?: AbortController;
}

/**
 * The return type of `createTestRunner`.
 *
 * Exposes:
 * - `run(bundle, signal?)` -- an async generator yielding `WorkflowEvent`s
 * - `classifyOutcome` -- the pure classification helper (bound to no state)
 * - `agent` -- the underlying generated Agent instance (for introspection)
 */
export interface TestRunner {
  /**
   * Stream workflow events for an attempt bundle.
   *
   * Accepts an optional `AbortSignal` for per-run cancellation.
   * The `signal` is forwarded to the runtime's `RunOptions`.
   */
  run(
    bundle: AttemptBundle,
    signal?: AbortSignal,
  ): AsyncIterable<WorkflowEvent>;

  /**
   * Classify the terminal outcome from a complete event array.
   *
   * This is the same `classifyOutcome` exported at module scope,
   * re-exposed for convenience.
   */
  classifyOutcome(events: readonly WorkflowEvent[]): RunOutcome | null;

  /** The generated Agent instance. */
  readonly agent: Agent;
}

/**
 * Create a test-runner adapter around the generated
 * `InterviewTestRunner` Agent.
 *
 * Validates the bundle before streaming (rejects invalid or oversized
 * bundles early), wires the UI host into the Agent for the sandbox
 * confirmation policy, and configures the opaque-origin sandbox runner.
 *
 * The returned `TestRunner.run()` matches the `WorkflowRunner` structural
 * contract consumed by `useWorkflowRun` from `@nemoir/web-ui`, but does
 * NOT import or depend on React.
 */
export function createTestRunner(
  opts: TestRunnerOptions = {},
): TestRunner {
  const sandbox = opts.sandboxRunner ?? createOpaqueOriginJsSandbox({
    timeoutMs: opts.sandboxTimeoutMs,
  });

  const agent = new Agent({
    uiHost: opts.uiHost,
    browserTools: { jsSandboxRunner: sandbox },
  });

  async function* run(
    bundle: AttemptBundle,
    signal?: AbortSignal,
  ): AsyncIterable<WorkflowEvent> {
    // ---- pre-flight validation ----
    assertBundleValid(bundle);

    // ---- check signal before starting ----
    if (signal?.aborted) {
      throw new DOMException("Cancelled before run", "AbortError");
    }

    // ---- stream ----
    // The generated Agent.stream accepts AgentInput = { attempt_bundle: unknown }
    // The workflow JSON binds the `input` sandbox arg to `attempt_bundle` ref,
    // so the whole bundle is passed as one JSON value.
    yield* agent.stream(
      { attempt_bundle: bundle as unknown },
      { signal },
    );
  }

  return {
    run,
    classifyOutcome: classifyOutcome,
    agent,
  };
}

// ---------------------------------------------------------------------------
// Convenience: run-to-completion helper
// ---------------------------------------------------------------------------

/**
 * Run a bundle to completion, collecting all events and returning a
 * terminal outcome.
 *
 * This is a convenience wrapper; prefer `createTestRunner().run()` for
 * streaming use-cases.
 */
export async function runToOutcome(
  runner: TestRunner,
  bundle: AttemptBundle,
  signal?: AbortSignal,
): Promise<{ outcome: RunOutcome; events: WorkflowEvent[] }> {
  const events: WorkflowEvent[] = [];
  let outcome: RunOutcome | null = null;

  try {
    for await (const event of runner.run(bundle, signal)) {
      events.push(event);
      const terminal = terminalOutcomeFromEvent(event, events);
      if (terminal) {
        outcome = terminal;
        // Don't break -- the stream may still yield cleanup events.
        // The outcome from the first terminal event is definitive.
      }
    }
  } catch (err) {
    // If the stream itself throws (e.g. the AbortError from cancellation
    // propagating through the agent), treat as the corresponding outcome.
    if (
      (err instanceof DOMException && err.name === "AbortError") ||
      (err instanceof Error && err.name === "AbortError")
    ) {
      outcome = { kind: "cancelled" };
    } else {
      const message = err instanceof Error ? err.message : String(err);
      outcome = { kind: "infrastructure_error", error: message };
    }
  }

  // If no terminal event was emitted but we have events, classify from
  // the full array.
  if (!outcome && events.length > 0) {
    outcome = classifyOutcome(events);
  }

  if (!outcome) {
    outcome = {
      kind: "infrastructure_error",
      error: "No events produced by the workflow.",
    };
  }

  return { outcome, events };
}
