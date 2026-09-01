/**
 * NemoIR Interview Tutor -- attempt snapshot, construction, validation,
 * byte sizing, and stale detection.
 *
 * An `AttemptSnapshot` freezes the exact learner submission, problem
 * identity, visible tests, and evaluator version for one test run.
 *
 * Validation gates run *before* a sandbox is created: blank source,
 * oversized bundles, and non-JSON-safe values are rejected early.
 *
 * Phase 2 -- execution domain.  Imports catalog types but does not edit
 * catalog files.
 */

import type {
  JsonValue,
  Problem,
  AttemptBundle,
  VisibleTest,
  RunReport,
  FirstFailure,
} from "../catalog/types.js";
import { utf8ByteLength } from "@nemoir/web-runtime";

// ---------------------------------------------------------------------------
// Constants (mirror runtime defaults from the plan §7 + sandbox module)
// ---------------------------------------------------------------------------

/** Maximum learner-source bytes before we refuse to bundle. */
export const MAX_SOURCE_BYTES = 64 * 1024; // 64 KiB

/** Maximum serialised attempt-bundle bytes (sandbox input limit). */
export const MAX_BUNDLE_BYTES = 256 * 1024; // 256 KiB

// ---------------------------------------------------------------------------
// Attempt snapshot
// ---------------------------------------------------------------------------

/**
 * Immutable record of exactly what was submitted for a test run.
 *
 * The snapshot is intentionally independent of the runtime event stream:
 * callers can compare `originalSource` against the current editor contents
 * to decide whether results are stale.
 */
export interface AttemptSnapshot {
  /** Learner source captured at construction time. */
  readonly source: string;

  /**
   * The source text at the moment the snapshot was created.  Always
   * identical to `source` at construction time, but retained as a separate
   * field so callers can reliably test equality without worrying about
   * mutation (strings are immutable in JS, but the distinction is
   * semantically useful for stale detection across re-renders).
   */
  readonly originalSource: string;

  /** Problem id from the catalog. */
  readonly problemId: string;

  /** Entry function name declared by the problem. */
  readonly entryFunctionName: string;

  /** Visible tests bundled into the attempt. */
  readonly tests: readonly VisibleTest[];

  /** Evaluator version from the problem at construction time. */
  readonly evaluatorVersion: string;

  /** Monotonic timestamp (Date.now()) when the snapshot was created. */
  readonly capturedAt: number;
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

/**
 * Create a frozen attempt snapshot from a problem definition and the
 * learner's current source text.
 *
 * Validation is eager: a blank or over-sized source is rejected before
 * the snapshot is constructed.
 */
export function createAttemptSnapshot(
  problem: Problem,
  source: string,
): AttemptSnapshot {
  // ---- blank-source gate ----
  if (source.trim().length === 0) {
    throw new AttemptValidationError("Learner source must not be blank.");
  }

  // ---- byte-size gate (source) ----
  const sourceBytes = utf8ByteLength(source);
  if (sourceBytes > MAX_SOURCE_BYTES) {
    throw new AttemptValidationError(
      `Learner source is ${sourceBytes} bytes; maximum is ${MAX_SOURCE_BYTES} bytes.`,
    );
  }

  // ---- test-presence gate ----
  if (!problem.visibleTests || problem.visibleTests.length === 0) {
    throw new AttemptValidationError(
      "Problem has no visible tests defined.",
    );
  }

  return Object.freeze({
    source,
    originalSource: source,
    problemId: problem.id,
    entryFunctionName: problem.entryFunctionName,
    tests: problem.visibleTests,
    evaluatorVersion: problem.evaluatorVersion,
    capturedAt: Date.now(),
  }) as AttemptSnapshot;
}

// ---------------------------------------------------------------------------
// Stale detection
// ---------------------------------------------------------------------------

/**
 * Return `true` when the snapshot no longer matches the current editor
 * source.
 *
 * Staleness is purely a string-identity check.  Version mismatches
 * (evaluatorVersion, test changes) are not surfaced here because a new
 * snapshot must be built for a new problem or a modified test set.
 */
export function isSnapshotStale(
  snapshot: AttemptSnapshot,
  currentSource: string,
): boolean {
  return snapshot.originalSource !== currentSource;
}

// ---------------------------------------------------------------------------
// Bundle I/O
// ---------------------------------------------------------------------------

/**
 * Build the JSON-safe attempt bundle that becomes the workflow input.
 *
 * This is the `{ source, entryFunctionName, tests, evaluatorVersion }`
 * payload expected by `InterviewTestRunner`.
 */
export function buildAttemptBundle(
  snapshot: AttemptSnapshot,
): AttemptBundle {
  return {
    source: snapshot.source,
    entryFunctionName: snapshot.entryFunctionName,
    tests: [...snapshot.tests],
    evaluatorVersion: snapshot.evaluatorVersion,
  };
}

/**
 * Return the UTF-8 byte length of the serialised bundle.
 *
 * Used to reject oversized bundles *before* submitting to the sandbox.
 */
export function bundleByteLength(bundle: AttemptBundle): number {
  const json = JSON.stringify(bundle);
  return utf8ByteLength(json);
}

// ---------------------------------------------------------------------------
// Validation (pre-sandbox)
// ---------------------------------------------------------------------------

/**
 * Validate that a value is structurally an `AttemptBundle` and that every
 * field is JSON-safe.
 *
 * This is a type-guard, not a full schema validator: it checks the shape
 * and JSON safety but does *not* check that `source` or test arguments
 * are sensible for a particular problem.
 */
export function validateAttemptBundleShape(
  value: unknown,
): value is AttemptBundle {
  if (value === null || typeof value !== "object") return false;

  const b = value as Record<string, unknown>;

  // Required string fields
  if (typeof b.source !== "string" || b.source.trim().length === 0) {
    return false;
  }
  if (typeof b.entryFunctionName !== "string") return false;
  if (typeof b.evaluatorVersion !== "string") return false;

  // tests must be an array
  if (!Array.isArray(b.tests)) return false;
  for (const t of b.tests) {
    if (!isVisibleTest(t)) return false;
  }

  return true;
}

/** Type-guard for a single visible test entry. */
function isVisibleTest(value: unknown): value is VisibleTest {
  if (value === null || typeof value !== "object") return false;
  const t = value as Record<string, unknown>;
  if (typeof t.id !== "string" || t.id.length === 0) return false;
  if (!Array.isArray(t.args) || !isJsonSafe(t.args)) return false;
  // expected is required -- must be present (even if null)
  if (!("expected" in t) || !isJsonSafe(t.expected)) return false;
  return true;
}

// ---------------------------------------------------------------------------
// Run-report validation (untrusted sandbox output)
// ---------------------------------------------------------------------------

const VALID_RUN_REPORT_STATUSES: ReadonlySet<string> = new Set([
  "passed",
  "failed",
  "syntax_error",
  "runtime_error",
  "timeout",
]);

/**
 * Strict type-guard for an untrusted run report returned from the sandbox.
 *
 * Does **not** manufacture a normal harness report for a sandbox timeout;
 * that classification happens at the event/outcome layer.
 */
export function isRunReport(value: unknown): value is RunReport {
  if (value === null || typeof value !== "object") return false;
  const r = value as Record<string, unknown>;

  if (typeof r.status !== "string" || !VALID_RUN_REPORT_STATUSES.has(r.status)) {
    return false;
  }
  if (typeof r.total !== "number" || !Number.isSafeInteger(r.total) || r.total < 0) {
    return false;
  }
  if (typeof r.passed !== "number" || !Number.isSafeInteger(r.passed) || r.passed < 0 || r.passed > (r.total as number)) {
    return false;
  }
  if (typeof r.elapsedMs !== "number" || !Number.isFinite(r.elapsedMs) || (r.elapsedMs as number) < 0) {
    return false;
  }
  if (typeof r.evaluatorVersion !== "string") return false;

  // firstFailure is optional
  if (r.firstFailure !== undefined && r.firstFailure !== null) {
    if (!isFirstFailure(r.firstFailure)) return false;
  }

  return true;
}

/** Type-guard for the optional `FirstFailure` in a run report. */
export function isFirstFailure(value: unknown): value is FirstFailure {
  if (value === null || typeof value !== "object") return false;
  const f = value as Record<string, unknown>;

  if (typeof f.testId !== "string") return false;
  if (!Array.isArray(f.args) || !isJsonSafe(f.args)) return false;

  // expected is required
  if (!("expected" in f) || !isJsonSafe(f.expected)) return false;

  // actual and error are optional; if present must be JSON-safe / string.
  if ("actual" in f && !isJsonSafe(f.actual)) return false;
  if ("error" in f && typeof f.error !== "string") return false;

  return true;
}

// ---------------------------------------------------------------------------
// JSON-safe check (defensive)
// ---------------------------------------------------------------------------

/**
 * Return `true` when `value` is a JSON-safe value.
 *
 * Mirrors the runtime's `isJsonSafeValue` check so we can reject
 * non-serialisable inputs before passing them to the sandbox.
 */
export function isJsonSafe(value: unknown): value is JsonValue {
  const seen = new WeakSet<object>();

  const visit = (candidate: unknown): boolean => {
    if (candidate === null) return true;
    switch (typeof candidate) {
      case "string":
      case "boolean":
        return true;
      case "number":
        return Number.isFinite(candidate);
      case "object": {
        if (seen.has(candidate)) return false;
        seen.add(candidate);

        if (Array.isArray(candidate)) return candidate.every(visit);

        try {
          const prototype = Object.getPrototypeOf(candidate);
          if (prototype !== Object.prototype && prototype !== null) return false;
          return Object.keys(candidate).every((key) =>
            visit((candidate as Record<string, unknown>)[key]),
          );
        } catch {
          return false;
        }
      }
      default:
        return false;
    }
  };

  return visit(value);
}

/**
 * Verify that an `AttemptBundle` is fully JSON-safe and within byte
 * limits.
 *
 * Throws `AttemptValidationError` on failure.
 */
export function assertBundleValid(bundle: AttemptBundle): void {
  if (!isJsonSafe(bundle)) {
    throw new AttemptValidationError(
      "Attempt bundle contains non-JSON-safe values.",
    );
  }

  const bytes = bundleByteLength(bundle);
  if (bytes > MAX_BUNDLE_BYTES) {
    throw new AttemptValidationError(
      `Serialised attempt bundle is ${bytes} bytes; maximum is ${MAX_BUNDLE_BYTES} bytes.`,
    );
  }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/** Validation error raised before any sandbox is created. */
export class AttemptValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AttemptValidationError";
  }
}
