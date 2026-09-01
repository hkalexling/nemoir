/**
 * NemoIR Interview Tutor -- run-tutor-workflow factory.
 *
 * Phase 3 -- tutoring domain.
 *
 * Framework-neutral factory around the generated `InterviewTutor` Agent,
 * analogous to `run-test-workflow.ts` for the test runner.
 *
 * Goals:
 * - Wire `Agent.stream()` with a `modelAdapter`, `WebUiHost`, a
 *   `jsWorkerFactory` (for `browser.js.run` stages), and a per-run
 *   `AbortSignal`.
 * - Classify terminal outcomes from the event stream (completed,
 *   cancelled, infrastructure_error).
 * - Provide a helper to extract validated `ValidatedGuidance` from
 *   terminal-stage events or the final result.
 *
 * This module is React-free.  It exports a runner function compatible
 * with `WorkflowRunner` from `@nemoir/web-ui`.
 */

import { Agent } from "../generated/interview-tutor/src/agent.js";
import type {
  WorkflowEvent,
  WebUiHost,
  ModelAdapter,
  ModelRouter,
  ModelGenerationParams,
} from "@nemoir/web-runtime";
import type { RunReportStatus } from "../catalog/types.js";
import {
  validateTutorOutput,
  validateAgentResult,
  tutorOutputValidationErrors,
  isSafeTutorPreview,
  type ValidatedGuidance,
} from "./tutor-validation.js";

// ---------------------------------------------------------------------------
// Outcome types
// ---------------------------------------------------------------------------

/**
 * Discriminated union of every terminal outcome the tutor runner can
 * produce.
 */
export type TutorRunOutcome =
  | { readonly kind: "completed"; readonly guidance: ValidatedGuidance }
  | { readonly kind: "cancelled" }
  | { readonly kind: "infrastructure_error"; readonly error: string };

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

/**
 * Extract the raw workflow result value from a `run_completed` event.
 *
 * Tries both `event.result.output` (WorkflowResult shape) and
 * `event.result` (bare AgentResult shape).
 */
function extractResultFromEvent(event: WorkflowEvent): unknown {
  const result = event.result;
  if (result === null || result === undefined || typeof result !== "object") {
    return null;
  }

  const r = result as Record<string, unknown>;

  // WorkflowResult shape: { output: { ... } }
  if ("output" in r && r.output !== null && typeof r.output === "object") {
    return r.output;
  }

  // Bare AgentResult shape: { mode, hint, concept, next_steps }
  if ("mode" in r || "hint" in r) {
    return r;
  }

  return null;
}

/**
 * Classify the terminal outcome from a collected array of workflow
 * events and the `reportStatus` that produced the tutor request.
 *
 * Inspects the final event(s) and delegates to validation helpers.
 * Pure and synchronous.
 */
export function classifyTutorOutcome(
  events: readonly WorkflowEvent[],
  reportStatus: RunReportStatus,
): TutorRunOutcome | null {
  if (events.length === 0) return null;

  // Check final event first
  const last = events[events.length - 1];

  if (last.kind === "run_completed") {
    const output = extractResultFromEvent(last);
    if (output !== null) {
      const validation = validateAgentResult(output, reportStatus);
      if (validation.valid) {
        return { kind: "completed", guidance: validation.guidance };
      }
      // Completed but invalid: infrastructure error with validation detail
      const detail =
        "errors" in validation && validation.errors.length > 0
          ? validation.errors.join("; ")
          : "unknown validation failure";
      return {
        kind: "infrastructure_error",
        error: `Workflow completed but output failed safety validation: ${detail}`,
      };
    }
    return {
      kind: "infrastructure_error",
      error: "Workflow completed but did not produce recognisable guidance output.",
    };
  }

  if (last.kind === "run_failed") {
    const err = last.error ?? "";

    // User cancellation
    if (/abort|cancel/i.test(err)) {
      return { kind: "cancelled" };
    }

    return { kind: "infrastructure_error", error: err || "Unknown workflow error" };
  }

  // Policy denial -- treat as infrastructure for the tutor
  for (const e of events) {
    if (e.kind === "policy_denied") {
      return {
        kind: "infrastructure_error",
        error: "Workflow policy was denied.",
      };
    }
  }

  // Stream ended without a terminal event
  return {
    kind: "infrastructure_error",
    error: "Workflow stream ended without a terminal event.",
  };
}

/**
 * Inspect a single `WorkflowEvent` and return an outcome if it is
 * terminal.
 *
 * Useful for live-stream consumers that want to short-circuit.
 */
export function terminalTutorOutcomeFromEvent(
  event: WorkflowEvent,
  reportStatus: RunReportStatus,
  priorEvents?: readonly WorkflowEvent[],
): TutorRunOutcome | null {
  if (event.kind === "run_completed") {
    const output = extractResultFromEvent(event);
    if (output !== null) {
      const validation = validateAgentResult(output, reportStatus);
      if (validation.valid) {
        return { kind: "completed", guidance: validation.guidance };
      }
      const detail =
        "errors" in validation && validation.errors.length > 0
          ? validation.errors.join("; ")
          : "unknown validation failure";
      return {
        kind: "infrastructure_error",
        error: `Workflow completed but output failed safety validation: ${detail}`,
      };
    }
    return {
      kind: "infrastructure_error",
      error: "Workflow completed but did not produce recognisable guidance output.",
    };
  }

  if (event.kind === "policy_denied") {
    return {
      kind: "infrastructure_error",
      error: "Workflow policy was denied.",
    };
  }

  if (event.kind === "run_failed") {
    const err = event.error ?? "";
    if (/abort|cancel/i.test(err)) return { kind: "cancelled" };
    return { kind: "infrastructure_error", error: err || "Unknown workflow error" };
  }

  // Tool-call failures in the tutor workflow are infrastructure issues
  // (there's no sandbox to distinguish timeouts from).
  if (event.kind === "tool_call_failed") {
    const wasCancelled = priorEvents?.some(
      (e) =>
        e.kind === "run_failed" &&
        /abort|cancel/i.test(e.error ?? ""),
    );
    if (wasCancelled) return { kind: "cancelled" };
    return {
      kind: "infrastructure_error",
      error: `Tool call failed: ${event.error ?? "unknown error"}`,
    };
  }

  return null;
}

// ---------------------------------------------------------------------------
// Guidance extraction helpers
// ---------------------------------------------------------------------------

/**
 * Extract validated guidance from a fully-collected event stream.
 *
 * Returns `null` when no guidance-producing stage was found.  This is
 * intentionally robust: even when the stream ended with a terminal error,
 * an earlier stage may have produced useful output (partial guidance).
 *
 * The caller should prefer `classifyTutorOutcome` for the authoritative
 * terminal outcome; this helper is for live-progress display.
 */
export function extractGuidanceFromEvents(
  events: readonly WorkflowEvent[],
  reportStatus: RunReportStatus,
): ValidatedGuidance | null {
  // Walk events backwards to find the last stage_completed for the exit
  // stage or the run_completed event.
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i];

    // stage_completed with a "ProduceGuidance" stage may carry output
    if (
      event.kind === "stage_completed" &&
      event.stageId === "ProduceGuidance" &&
      event.output
    ) {
      const validation = validateTutorOutput(event.output, reportStatus);
      if (validation.valid) return validation.guidance;
    }

    // run_completed carries the final result
    if (event.kind === "run_completed") {
      const output = extractResultFromEvent(event);
      if (output !== null) {
        const validation = validateAgentResult(output, reportStatus);
        if (validation.valid) return validation.guidance;
      }
    }
  }

  return null;
}

/**
 * Safely derive a non-authoritative streamed preview from tagged-envelope
 * deltas for the learner-facing `ProduceGuidance` stage. The model protocol
 * streams JSON such as `{"kind":"final","output":{"hint":"…"}}`; raw
 * tokens must never be rendered directly. This helper decodes only the
 * partial `hint` string, clears it after a retry, and suppresses text that
 * violates the same no-full-solution guard used for final guidance.
 */
export function extractGuidancePreview(
  events: readonly WorkflowEvent[],
): string | null {
  let accumulated = "";

  for (const event of events) {
    if (event.stageId !== "ProduceGuidance") continue;
    if (event.kind === "model_retry") {
      accumulated = "";
      continue;
    }
    if (event.kind === "model_delta") {
      accumulated += event.text ?? "";
    }
  }

  const preview = extractPartialJsonStringField(accumulated, "hint");
  if (!preview || !isSafeTutorPreview(preview)) return null;
  return preview.slice(0, 1200);
}

function extractPartialJsonStringField(
  source: string,
  field: string,
): string | null {
  const marker = `"${field}"`;
  const markerIndex = source.indexOf(marker);
  if (markerIndex < 0) return null;

  let index = markerIndex + marker.length;
  while (index < source.length && /\s/.test(source[index] ?? "")) index++;
  if (source[index] !== ":") return null;
  index++;
  while (index < source.length && /\s/.test(source[index] ?? "")) index++;
  if (source[index] !== "\"") return null;
  index++;

  let value = "";
  while (index < source.length) {
    const character = source[index] ?? "";
    if (character === "\"") return value;
    if (character !== "\\") {
      value += character;
      index++;
      continue;
    }

    const escape = source[index + 1];
    if (escape === undefined) break;
    switch (escape) {
      case "\"": value += "\""; index += 2; break;
      case "\\": value += "\\"; index += 2; break;
      case "/": value += "/"; index += 2; break;
      case "b": value += "\b"; index += 2; break;
      case "f": value += "\f"; index += 2; break;
      case "n": value += "\n"; index += 2; break;
      case "r": value += "\r"; index += 2; break;
      case "t": value += "\t"; index += 2; break;
      case "u": {
        const hex = source.slice(index + 2, index + 6);
        if (hex.length < 4) return value || null;
        if (!/^[0-9a-fA-F]{4}$/.test(hex)) return value || null;
        value += String.fromCharCode(Number.parseInt(hex, 16));
        index += 6;
        break;
      }
      default:
        // The model may transiently emit a non-JSON escape before the runtime
        // retries it. Preserve the already decoded safe prefix rather than
        // flashing raw envelope text or clearing the panel unnecessarily.
        return value || null;
    }
  }

  // The string has not closed yet, which is normal while a model is
  // streaming. Return the decoded prefix as a preview.
  return value || null;
}

const RUN_REPORT_STATUSES: ReadonlySet<RunReportStatus> = new Set([
  "passed",
  "failed",
  "syntax_error",
  "runtime_error",
  "timeout",
]);

function reportStatusFromTutorRequest(request: unknown): RunReportStatus {
  if (request === null || typeof request !== "object" || Array.isArray(request)) {
    throw new Error("Tutor request must be an object with a deterministic run report.");
  }
  const runReport = (request as Record<string, unknown>).runReport;
  if (runReport === null || typeof runReport !== "object" || Array.isArray(runReport)) {
    throw new Error("Tutor request must include a deterministic run report.");
  }
  const status = (runReport as Record<string, unknown>).status;
  if (typeof status !== "string" || !RUN_REPORT_STATUSES.has(status as RunReportStatus)) {
    throw new Error("Tutor request has an unsupported run-report status.");
  }
  return status as RunReportStatus;
}

// ---------------------------------------------------------------------------
// Tutor-runner factory
// ---------------------------------------------------------------------------

/**
 * Options for `createTutorRunner`.
 */
export interface TutorRunnerOptions {
  /**
   * Model adapter.  Required because `InterviewTutor` has model stages.
   */
  readonly modelAdapter: ModelAdapter | ModelRouter;

  /** UI host for the `user.elicit` capability. */
  readonly uiHost: WebUiHost;

  /**
   * Pre-built JS worker factory for the `browser.js.run` stages
   * (`CaptureRequest` and `NormalizeProfile`).
   *
   * Must produce fresh Workers from a module URL (e.g. via
   * `new Worker(new URL("../generated/interview-tutor/src/js.worker.ts", import.meta.url), { type: "module" })`).
   */
  readonly jsWorkerFactory: () => Worker;

  /**
   * Timeout for each `browser.js.run` invocation in milliseconds.
   * Default 30_000.
   */
  readonly jsRunTimeoutMs?: number;

  /**
   * An `AbortController` that the caller can signal for cooperative
   * cancellation across runs.  When omitted, each call to `run()` manages
   * its own controller internally.
   */
  readonly externalAbortController?: AbortController;

  /**
   * Sampling temperature for model stages. Default 0.7 — low enough for
   * coherent structured output, high enough to break the near-deterministic
   * retry loop that 0.2 caused (identical output on each retry). With
   * grammar-constrained decoding now guaranteeing valid JSON, the old reason
   * to keep temperature low is gone. Raise toward 1.0 for more variation.
   * Other generation params (maxTokens, penalties) keep the runtime defaults.
   */
  readonly temperature?: number;
}

/**
 * The return type of `createTutorRunner`.
 */
export interface TutorRunner {
  /**
   * Stream workflow events for a tutor request.
   *
   * The input must be `{ tutor_request: TutorRequest }` -- use
   * `tutorRequestToAgentInput()` from `tutor-request.ts` to build it.
   *
   * Accepts an optional `AbortSignal` for per-run cancellation.
   */
  run(
    input: Record<string, unknown>,
    signal?: AbortSignal,
  ): AsyncIterable<WorkflowEvent>;

  /**
   * Classify the terminal outcome from a complete event array.
   *
   * Re-exposes `classifyTutorOutcome` for convenience.
   */
  classifyOutcome(
    events: readonly WorkflowEvent[],
    reportStatus: RunReportStatus,
  ): TutorRunOutcome | null;

  /** The generated Agent instance (for introspection). */
  readonly agent: Agent;
}

/**
 * Create a tutor-runner adapter around the generated `InterviewTutor`
 * Agent.
 *
 * The returned `TutorRunner.run()` matches the `WorkflowRunner`
 * structural contract consumed by `useWorkflowRun` from `@nemoir/web-ui`,
 * but does NOT import or depend on React.
 */
export function createTutorRunner(
  opts: TutorRunnerOptions,
): TutorRunner {
  const temperature = opts.temperature ?? 0.7;
  const generationParams: ModelGenerationParams = { temperature };
  const agent = new Agent({
    modelAdapter: opts.modelAdapter,
    uiHost: opts.uiHost,
    browserTools: {
      jsWorkerFactory: opts.jsWorkerFactory,
      jsRunTimeoutMs: opts.jsRunTimeoutMs,
    },
    actionProtocol: "tagged_envelope",
  });

  async function* run(
    input: Record<string, unknown>,
    signal?: AbortSignal,
  ): AsyncIterable<WorkflowEvent> {
    // ---- check signal before starting ----
    if (signal?.aborted) {
      throw new DOMException("Cancelled before run", "AbortError");
    }

    const tutorRequest = input.tutor_request ?? input;
    const reportStatus = reportStatusFromTutorRequest(tutorRequest);

    // The semantic validator runs inside ModelStageExecutor after structural
    // output validation. Returning errors causes the existing retry loop to
    // feed concise corrective feedback back to ProduceGuidance rather than
    // completing a workflow that the learner-facing UI must reject.
    yield* agent.stream(
      { tutor_request: tutorRequest },
      {
        signal,
        generationParams,
        modelOutputValidators: {
          ProduceGuidance: (output) =>
            tutorOutputValidationErrors(output, reportStatus),
        },
      },
    );
  }

  return {
    run,
    classifyOutcome: classifyTutorOutcome,
    agent,
  };
}

// ---------------------------------------------------------------------------
// Convenience: run-to-completion helper
// ---------------------------------------------------------------------------

/**
 * Run a tutor request to completion, collecting all events and returning
 * a terminal outcome.
 *
 * Prefer `createTutorRunner().run()` for streaming use-cases.
 */
export async function runTutorToOutcome(
  runner: TutorRunner,
  input: Record<string, unknown>,
  reportStatus: RunReportStatus,
  signal?: AbortSignal,
): Promise<{ outcome: TutorRunOutcome; events: WorkflowEvent[] }> {
  const events: WorkflowEvent[] = [];
  let outcome: TutorRunOutcome | null = null;

  try {
    for await (const event of runner.run(input, signal)) {
      events.push(event);
      const terminal = terminalTutorOutcomeFromEvent(event, reportStatus, events);
      if (terminal && !outcome) {
        // Keep the first terminal outcome definitive while still consuming any
        // cleanup events that follow it.
        outcome = terminal;
      }
    }
  } catch (err) {
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

  if (!outcome && events.length > 0) {
    outcome = classifyTutorOutcome(events, reportStatus);
  }

  if (!outcome) {
    outcome = {
      kind: "infrastructure_error",
      error: "No events produced by the workflow.",
    };
  }

  return { outcome, events };
}
