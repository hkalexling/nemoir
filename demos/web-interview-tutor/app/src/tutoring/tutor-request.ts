/**
 * NemoIR Interview Tutor -- tutor-request builder.
 *
 * Phase 3 -- tutoring domain.
 *
 * Builds a bounded, immutable `TutorRequest` snapshot from a Problem
 * definition, an AttemptSnapshot, and a completed RunReport.  The
 * resulting object is the single `tutor_request` JSON value required by
 * the `InterviewTutor` workflow input.
 *
 * Every field is capped and validated before freezing.  The builder
 * never reaches into a live editor -- it only reads the snapshot.
 */

import type { Problem, RunReport, RunReportStatus } from "../catalog/types.js";
import type { AttemptSnapshot } from "../execution/attempt.js";

// ---------------------------------------------------------------------------
// Hint level
// ---------------------------------------------------------------------------

/**
 * Explicit hint levels supported by the tutoring workflow.
 *
 * - `"nudge"`: conceptual direction and a question only
 * - `"targeted"`: identify a relevant assumption, edge case, or region
 * - `"plan"`: short algorithmic plan or high-level pseudocode
 * - `"review"`: for passed reports; discuss complexity, robustness, or a
 *   next challenge
 */
export type HintLevel = "nudge" | "targeted" | "plan" | "review";

/** The set of valid hint levels (for runtime guards). */
export const VALID_HINT_LEVELS: ReadonlySet<string> = new Set([
  "nudge",
  "targeted",
  "plan",
  "review",
]);

/**
 * Return `true` when `value` is a recognised `HintLevel`.
 */
export function isHintLevel(value: unknown): value is HintLevel {
  return typeof value === "string" && VALID_HINT_LEVELS.has(value);
}

// ---------------------------------------------------------------------------
// Problem metadata (subset sent to the workflow)
// ---------------------------------------------------------------------------

/**
 * Compact, JSON-safe problem metadata carried in the tutor request.
 *
 * This is intentionally smaller than the full `Problem` type -- the
 * workflow only needs identity, difficulty, topics, and the entry
 * function name.
 */
export interface ProblemMetadata {
  readonly problemId: string;
  readonly title: string;
  readonly difficulty: string;
  readonly topics: readonly string[];
  readonly entryFunctionName: string;
}

/** Maximum length for `problemContext` (statement + constraints). */
const MAX_PROBLEM_CONTEXT = 12000;

/** Maximum length for `learnerCode` (snapshot source). */
const MAX_LEARNER_CODE = 24000;

/** Maximum length for `priorSummary`. */
const MAX_PRIOR_SUMMARY = 8000;

// ---------------------------------------------------------------------------
// Tutor request
// ---------------------------------------------------------------------------

/**
 * The bounded, immutable snapshot consumed by the `InterviewTutor`
 * workflow as its `tutor_request` input.
 *
 * Every string field is capped to the workflow's declared limits.
 * `runReport` is the completed, validated RunReport from a prior test
 * run.  `hintLevel` is explicitly chosen by the caller.
 */
export interface TutorRequest {
  /** Problem statement + constraints (capped). */
  readonly problemContext: string;

  /** Frozen learner source from the attempt snapshot (capped). */
  readonly learnerCode: string;

  /** The completed, validated run report. */
  readonly runReport: RunReport;

  /** Explicit hint level.  Must be `"review"` when the report passed. */
  readonly hintLevel: HintLevel;

  /**
   * Prior guidance summary (empty string on first request).  Callers
   * should append each validated `hint` to build a running summary, but
   * must keep it under the cap.
   */
  readonly priorSummary: string;

  /** Compact problem metadata for the workflow. */
  readonly problemMetadata: ProblemMetadata;
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

/**
 * Build compact `ProblemMetadata` from a `Problem`.
 */
export function buildProblemMetadata(problem: Problem): ProblemMetadata {
  return deepFreeze({
    problemId: problem.id,
    title: problem.title,
    difficulty: problem.difficulty,
    topics: [...problem.topics],
    entryFunctionName: problem.entryFunctionName,
  }) as ProblemMetadata;
}

/**
 * Assemble a `problemContext` string from a problem's statement and
 * constraints.  The result is capped to `MAX_PROBLEM_CONTEXT`.
 */
export function buildProblemContext(problem: Problem): string {
  const parts: string[] = [
    `Problem: ${problem.title}`,
    `Function: ${problem.entryFunctionName}(/* JSON-compatible arguments */)`,
  ];

  if (problem.statement.length > 0) {
    parts.push("", problem.statement);
  }

  if (problem.constraints.length > 0) {
    parts.push("", "Constraints:");
    for (const constraint of problem.constraints) {
      parts.push(`- ${constraint}`);
    }
  }

  // Keep only a small pedagogical sample rather than sending every catalog
  // example to the local model.
  if (problem.examples.length > 0) {
    parts.push("", "Examples:");
    for (const example of problem.examples.slice(0, 3)) {
      parts.push(`- Input: ${example.input}`);
      parts.push(`  Output: ${example.output}`);
      if (example.explanation) parts.push(`  Note: ${example.explanation}`);
    }
  }

  return capString(parts.join("\n"), MAX_PROBLEM_CONTEXT);
}

/**
 * Safely truncate a string to `max` characters, appending an ellipsis
 * marker when truncation occurred.
 */
function capString(value: string, max: number): string {
  if (value.length <= max) return value;
  return value.slice(0, max - 3) + "...";
}

// ---------------------------------------------------------------------------
// Tutor request builder
// ---------------------------------------------------------------------------

/**
 * Options for `createTutorRequest`.
 */
export interface CreateTutorRequestOptions {
  /**
   * Explicit hint level.  When omitted and the report is non-passing,
   * defaults to `"targeted"`.  When the report is `"passed"`, this is
   * always forced to `"review"` regardless of the caller's choice.
   */
  readonly hintLevel?: HintLevel;

  /**
   * Prior guidance summary from earlier requests.  Defaults to `""`.
   */
  readonly priorSummary?: string;
}

/**
 * Build a bounded, immutable `TutorRequest`.
 *
 * Rules:
 * - The `learnerCode` is taken exclusively from the snapshot -- never
 *   from the live editor.
 * - `hintLevel` is forced to `"review"` for passed reports.
 * - All string fields are capped to workflow limits.
 * - The returned object is deeply frozen.
 *
 * Throws `TutorRequestError` when the report's `status` is not a
 * recognised `RunReportStatus`.
 */
export function createTutorRequest(
  problem: Problem,
  snapshot: AttemptSnapshot,
  report: RunReport,
  opts: CreateTutorRequestOptions = {},
): TutorRequest {
  // ---- guard: report must come from this snapshot ----
  if (
    snapshot.problemId !== problem.id ||
    snapshot.evaluatorVersion !== problem.evaluatorVersion ||
    snapshot.evaluatorVersion !== report.evaluatorVersion
  ) {
    throw new TutorRequestError(
      "Run report does not match the problem or attempt snapshot.",
    );
  }

  if (!isRunReportStatus(report.status)) {
    throw new TutorRequestError(`Unsupported run-report status: ${String(report.status)}.`);
  }

  if (snapshot.source.length === 0 || snapshot.source.length > MAX_LEARNER_CODE) {
    throw new TutorRequestError(
      `The immutable submission snapshot must be 1-${MAX_LEARNER_CODE} characters for local tutoring.`,
    );
  }

  // ---- resolve hint level ----
  const isPassed = report.status === "passed";
  let hintLevel: HintLevel;

  if (isPassed) {
    hintLevel = "review";
  } else if (opts.hintLevel !== undefined && isHintLevel(opts.hintLevel)) {
    if (opts.hintLevel === "review") {
      throw new TutorRequestError("The review level is available only after all tests pass.");
    }
    hintLevel = opts.hintLevel;
  } else {
    hintLevel = "targeted";
  }

  // ---- build fields ----
  const problemContext = buildProblemContext(problem);
  const learnerCode = snapshot.source;
  const priorSummary = capString(opts.priorSummary ?? "", MAX_PRIOR_SUMMARY);
  const problemMetadata = buildProblemMetadata(problem);
  const runReport = deepFreeze(cloneJson(report)) as RunReport;

  return deepFreeze({
    problemContext,
    learnerCode,
    runReport,
    hintLevel,
    priorSummary,
    problemMetadata,
  }) as TutorRequest;
}

// ---------------------------------------------------------------------------
// Serialise for workflow input
// ---------------------------------------------------------------------------

/**
 * Serialise a `TutorRequest` into the `{ tutor_request }` input expected
 * by `Agent.stream()` / `Agent.run()`.
 */
export function tutorRequestToAgentInput(
  request: TutorRequest,
): { tutor_request: TutorRequest } {
  return { tutor_request: request };
}

// ---------------------------------------------------------------------------
// JSON snapshot helpers
// ---------------------------------------------------------------------------

const RUN_REPORT_STATUSES: ReadonlySet<RunReportStatus> = new Set([
  "passed",
  "failed",
  "syntax_error",
  "runtime_error",
  "timeout",
]);

function isRunReportStatus(value: unknown): value is RunReportStatus {
  return typeof value === "string" && RUN_REPORT_STATUSES.has(value as RunReportStatus);
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function deepFreeze<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value as Record<string, unknown>)) {
      deepFreeze(child);
    }
  }
  return value;
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/** Validation error raised by the request builder. */
export class TutorRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TutorRequestError";
  }
}
