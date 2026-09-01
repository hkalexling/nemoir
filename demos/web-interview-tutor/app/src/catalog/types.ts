// ---------------------------------------------------------------------------
// Phase 2 catalog types — shared API for problem definitions, attempt
// bundles, and structured run reports.
//
// All types are JSON-safe and align with the deterministic sandbox contract
// described in compiler/docs/targets/web.md and this demo's README.
// ---------------------------------------------------------------------------

// ---- recursive JSON-safe primitives ---------------------------------------

/**
 * A JSON-compatible value.  Recursive so that deeply-nested test arguments
 * and expected values (e.g. tree nodes, 2-D grids) can be represented
 * without losing type-safety.
 */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

/**
 * A JSON-compatible object whose keys are strings and whose values are
 * themselves JSON-compatible.
 */
export type JsonObject = { readonly [key: string]: JsonValue };

// ---- test & execution definitions -----------------------------------------

/** A single public, deterministic, JSON-compatible test case. */
export interface VisibleTest {
  readonly id: string;
  readonly args: readonly JsonValue[];
  readonly expected: JsonValue;
}

/** Resource limits enforced by the sandbox evaluator harness. */
export interface ExecutionLimits {
  readonly timeoutMs: number;
  readonly maxSourceBytes: number;
  readonly maxInputBytes: number;
  readonly maxOutputBytes: number;
}

// ---- problem catalog ------------------------------------------------------

/** A single worked example shown in the problem statement. */
export interface ProblemExample {
  readonly input: string;
  readonly output: string;
  readonly explanation?: string;
}

/**
 * A declarative, versioned coding-interview problem.
 *
 * Every field is `readonly` so catalog entries are treated as immutable
 * compile-time constants.  Starter code must be valid JavaScript, initially
 * incomplete/wrong, and must never be dangerous (no eval, DOM access, or
 * infinite loops).
 */
export interface Problem {
  readonly id: string;
  readonly title: string;
  readonly difficulty: "beginner" | "intermediate" | "advanced";
  readonly topics: readonly string[];
  readonly statement: string;
  readonly constraints: readonly string[];
  readonly examples: readonly ProblemExample[];
  readonly entryFunctionName: string;
  readonly starterCode: string;
  readonly visibleTests: readonly VisibleTest[];
  readonly evaluatorVersion: string;
  readonly executionLimits: ExecutionLimits;
}

// ---- attempt bundle -------------------------------------------------------

/**
 * The exact JSON-safe bundle passed as `attempt_bundle` to the deterministic
 * test-runner workflow input.
 */
export interface AttemptBundle {
  readonly source: string;
  readonly entryFunctionName: string;
  readonly tests: readonly VisibleTest[];
  readonly evaluatorVersion: string;
}

// ---- structured run report ------------------------------------------------

export type RunReportStatus =
  | "passed"
  | "failed"
  | "syntax_error"
  | "runtime_error"
  | "timeout";

/** Details about the first failing test, when available. */
export interface FirstFailure {
  readonly testId: string;
  readonly args: readonly JsonValue[];
  readonly expected: JsonValue;
  readonly actual?: JsonValue;
  readonly error?: string;
}

/**
 * Structured report returned by the deterministic sandbox evaluator harness.
 *
 * When `status` is `"passed"` the optional `firstFailure` MUST be absent.
 * When `status` is `"timeout"` the outer sandbox infrastructure terminates
 * the iframe/worker tree and the app maps the tool-failure event to this
 * shape; the harness may not be able to produce the report itself.
 */
export interface RunReport {
  readonly status: RunReportStatus;
  readonly total: number;
  readonly passed: number;
  readonly elapsedMs: number;
  readonly evaluatorVersion: string;
  readonly firstFailure?: FirstFailure;
}

// ---- validation helpers ---------------------------------------------------

/** Characters that are valid inside a JavaScript identifier. */
const RE_VALID_JS_IDENTIFIER = /^[a-zA-Z_$][a-zA-Z0-9_$]*$/;

/** Characters that are valid in a problem ID (kebab-case slug). */
const RE_VALID_PROBLEM_ID = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;

/**
 * Returns `true` when `name` is a non-empty, syntactically-valid JavaScript
 * identifier suitable for use as a function name.
 */
export function isValidJsIdentifier(name: string): boolean {
  return RE_VALID_JS_IDENTIFIER.test(name);
}

/**
 * Returns `true` when `id` is a non-empty, kebab-case problem slug.
 */
export function isValidProblemId(id: string): boolean {
  return RE_VALID_PROBLEM_ID.test(id);
}

/**
 * Returns `true` when `value` is deeply JSON-safe.  Because `JsonValue` is
 * a recursive type, the TypeScript compiler already enforces this at the
 * type-level for any value typed as `JsonValue`.  This runtime check is
 * available for defensive validation of data crossing an untyped boundary
 * (e.g. parsed JSON).
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

        if (Array.isArray(candidate)) {
          return candidate.every(visit);
        }

        try {
          const prototype = Object.getPrototypeOf(candidate);
          if (prototype !== Object.prototype && prototype !== null) return false;
          return Object.values(candidate as Record<string, unknown>).every(visit);
        } catch {
          // Proxy/getter failures are not safe JSON transport values.
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
 * Validation result for a single problem.  An empty `errors` array means
 * the problem passed all checks.
 */
export interface ProblemValidation {
  readonly id: string;
  readonly valid: boolean;
  readonly errors: readonly string[];
}

/**
 * Run all structural checks on a single problem entry.
 *
 * Checks:
 *  - `id` is a valid kebab-case slug
 *  - `entryFunctionName` is a valid JS identifier
 *  - `visibleTests` is non-empty
 *  - every test `id`, `args`, and `expected` value is JSON-safe
 *  - `evaluatorVersion` is non-empty
 *  - `executionLimits` fields are positive
 *  - `starterCode` is not empty and does not contain dangerous patterns
 */
export function validateProblem(problem: Problem): ProblemValidation {
  const errors: string[] = [];

  if (!isValidProblemId(problem.id)) {
    errors.push(`Invalid problem id: "${problem.id}". Must be a kebab-case slug.`);
  }

  if (!isValidJsIdentifier(problem.entryFunctionName)) {
    errors.push(
      `Invalid entryFunctionName: "${problem.entryFunctionName}". Must be a valid JS identifier.`,
    );
  }

  if (problem.visibleTests.length === 0) {
    errors.push("visibleTests must contain at least one test case.");
  }

  for (const test of problem.visibleTests) {
    if (typeof test.id !== "string" || test.id.length === 0) {
      errors.push(`VisibleTest has empty or missing id.`);
    }
    if (!isJsonSafe(test.args)) {
      errors.push(`VisibleTest "${test.id}" args are not JSON-safe.`);
    }
    if (!isJsonSafe(test.expected)) {
      errors.push(`VisibleTest "${test.id}" expected is not JSON-safe.`);
    }
  }

  if (typeof problem.evaluatorVersion !== "string" || problem.evaluatorVersion.length === 0) {
    errors.push("evaluatorVersion must be a non-empty string.");
  }

  const limits = problem.executionLimits;
  if (limits.timeoutMs <= 0) errors.push("executionLimits.timeoutMs must be > 0.");
  if (limits.maxSourceBytes <= 0) errors.push("executionLimits.maxSourceBytes must be > 0.");
  if (limits.maxInputBytes <= 0) errors.push("executionLimits.maxInputBytes must be > 0.");
  if (limits.maxOutputBytes <= 0) errors.push("executionLimits.maxOutputBytes must be > 0.");

  if (typeof problem.starterCode !== "string" || problem.starterCode.trim().length === 0) {
    errors.push("starterCode must be a non-empty string.");
  } else {
    // Lightweight dangerous-pattern scan.
    const DANGEROUS = ["eval(", "Function(", "document.", "window.", "fetch(", "XMLHttpRequest"];
    for (const pattern of DANGEROUS) {
      if (problem.starterCode.includes(pattern)) {
        errors.push(`starterCode contains dangerous pattern "${pattern}".`);
      }
    }
  }

  return {
    id: problem.id,
    valid: errors.length === 0,
    errors,
  };
}

/**
 * Validate a full catalog and return per-problem results plus any
 * cross-problem errors (e.g. duplicate IDs).
 */
export function validateCatalog(
  problems: readonly Problem[],
): { readonly results: readonly ProblemValidation[]; readonly catalogErrors: readonly string[] } {
  const results = problems.map(validateProblem);
  const catalogErrors: string[] = [];

  const seen = new Map<string, number>();
  for (let i = 0; i < problems.length; i++) {
    const id = problems[i].id;
    if (seen.has(id)) {
      catalogErrors.push(
        `Duplicate problem id "${id}" at indices ${seen.get(id)} and ${i}.`,
      );
    } else {
      seen.set(id, i);
    }
  }

  return { results, catalogErrors };
}

/**
 * True when the entire catalog is valid (no per-problem errors and no
 * cross-problem errors).
 */
export function isCatalogValid(
  problems: readonly Problem[],
): boolean {
  const { results, catalogErrors } = validateCatalog(problems);
  if (catalogErrors.length > 0) return false;
  return results.every((r) => r.valid);
}
