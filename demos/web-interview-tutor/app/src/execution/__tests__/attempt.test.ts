// ---------------------------------------------------------------------------
// Phase 2 attempt unit tests — snapshots, stale detection, bundle validation,
// and run-report type guards.
//
// Tests ONLY pure exported functions from attempt.ts; never executes learner
// source or touches the sandbox runtime.  Uses a synthetic Problem fixture
// to avoid coupling to the real catalog.
// ---------------------------------------------------------------------------

import { describe, it, expect } from "vitest";
import type {
  Problem,
  AttemptBundle,
  RunReport,
  FirstFailure,
  RunReportStatus,
} from "../../catalog/types";
import {
  createAttemptSnapshot,
  isSnapshotStale,
  buildAttemptBundle,
  bundleByteLength,
  validateAttemptBundleShape,
  assertBundleValid,
  isRunReport,
  isFirstFailure,
  isJsonSafe,
  MAX_SOURCE_BYTES,
  MAX_BUNDLE_BYTES,
  AttemptValidationError,
} from "../attempt";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeProblem(
  overrides: Partial<Problem> = {},
): Problem {
  return {
    id: "test-problem",
    title: "Test Problem",
    difficulty: "beginner",
    topics: ["arrays"],
    statement: "Solve it.",
    constraints: ["n > 0"],
    examples: [{ input: "n=1", output: "1" }],
    entryFunctionName: "solve",
    starterCode: "function solve(n) { return n; }",
    visibleTests: [
      { id: "t1", args: [1], expected: 1 },
      { id: "t2", args: [2], expected: 2 },
    ],
    evaluatorVersion: "1.0.0",
    executionLimits: {
      timeoutMs: 3000,
      maxSourceBytes: 65536,
      maxInputBytes: 262144,
      maxOutputBytes: 262144,
    },
    ...overrides,
  };
}

function makeRunReport(
  overrides: Partial<RunReport> = {},
): RunReport {
  return {
    status: "passed" as RunReportStatus,
    total: 3,
    passed: 3,
    elapsedMs: 42,
    evaluatorVersion: "1.0.0",
    ...overrides,
  };
}

function makeFirstFailure(
  overrides: Partial<FirstFailure> = {},
): FirstFailure {
  return {
    testId: "t1",
    args: [1],
    expected: 1,
    actual: 2,
    error: "Expected 1 but got 2",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// AttemptSnapshotError
// ---------------------------------------------------------------------------

describe("AttemptValidationError", () => {
  it("is an instance of Error", () => {
    const err = new AttemptValidationError("test");
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("AttemptValidationError");
    expect(err.message).toBe("test");
  });
});

// ---------------------------------------------------------------------------
// createAttemptSnapshot
// ---------------------------------------------------------------------------

describe("createAttemptSnapshot", () => {
  it("creates a snapshot with correct fields", () => {
    const problem = makeProblem();
    const source = "function solve(n) { return n * 2; }";
    const now = Date.now();

    const snap = createAttemptSnapshot(problem, source);

    expect(snap.source).toBe(source);
    expect(snap.originalSource).toBe(source);
    expect(snap.problemId).toBe(problem.id);
    expect(snap.entryFunctionName).toBe(problem.entryFunctionName);
    expect(snap.tests).toEqual(problem.visibleTests);
    expect(snap.evaluatorVersion).toBe(problem.evaluatorVersion);
    expect(snap.capturedAt).toBeGreaterThanOrEqual(now);
  });

  it("returns a frozen object", () => {
    const problem = makeProblem();
    const snap = createAttemptSnapshot(problem, "function solve(n) { return n; }");
    expect(Object.isFrozen(snap)).toBe(true);
  });

  it("rejects blank source (empty)", () => {
    const problem = makeProblem();
    expect(() => createAttemptSnapshot(problem, "")).toThrow(
      AttemptValidationError,
    );
    expect(() => createAttemptSnapshot(problem, "")).toThrow(
      "must not be blank",
    );
  });

  it("rejects blank source (whitespace only)", () => {
    const problem = makeProblem();
    expect(() => createAttemptSnapshot(problem, "   \n  \t  ")).toThrow(
      AttemptValidationError,
    );
  });

  it("rejects oversize source", () => {
    const problem = makeProblem();
    // Build a source string just over the limit
    const bigSource = "x".repeat(MAX_SOURCE_BYTES + 1);
    expect(() => createAttemptSnapshot(problem, bigSource)).toThrow(
      AttemptValidationError,
    );
    expect(() => createAttemptSnapshot(problem, bigSource)).toThrow(
      /bytes/i,
    );
  });

  it("accepts source at the byte limit", () => {
    const problem = makeProblem();
    // Build a source that is exactly MAX_SOURCE_BYTES
    // We need a single-byte character repeated.
    const exactSource = "a".repeat(MAX_SOURCE_BYTES);
    const snap = createAttemptSnapshot(problem, exactSource);
    expect(snap.source).toHaveLength(MAX_SOURCE_BYTES);
  });

  it("rejects source with multi-byte chars exceeding byte limit", () => {
    const problem = makeProblem();
    // Emoji are 4 bytes each in UTF-8: 16384 * 4 = 65536, + 1 more = 65540
    const emojiSource = "🌟".repeat(16385);
    expect(() => createAttemptSnapshot(problem, emojiSource)).toThrow(
      AttemptValidationError,
    );
  });

  it("rejects problem with no visible tests", () => {
    const problem = makeProblem({ visibleTests: [] });
    expect(() =>
      createAttemptSnapshot(problem, "function solve(n) { return n; }"),
    ).toThrow(AttemptValidationError);
    expect(() =>
      createAttemptSnapshot(problem, "function solve(n) { return n; }"),
    ).toThrow("no visible tests");
  });
});

// ---------------------------------------------------------------------------
// isSnapshotStale
// ---------------------------------------------------------------------------

describe("isSnapshotStale", () => {
  const problem = makeProblem();
  const src = "function solve(n) { return n; }";

  it("returns false when source matches", () => {
    const snap = createAttemptSnapshot(problem, src);
    expect(isSnapshotStale(snap, src)).toBe(false);
  });

  it("returns true when source differs", () => {
    const snap = createAttemptSnapshot(problem, src);
    expect(isSnapshotStale(snap, "function solve(n) { return n + 1; }")).toBe(
      true,
    );
  });

  it("returns true when source is fully replaced", () => {
    const snap = createAttemptSnapshot(problem, src);
    expect(isSnapshotStale(snap, "")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// buildAttemptBundle
// ---------------------------------------------------------------------------

describe("buildAttemptBundle", () => {
  it("builds a bundle matching the snapshot", () => {
    const problem = makeProblem();
    const src = "function solve(n) { return n * 2; }";
    const snap = createAttemptSnapshot(problem, src);

    const bundle = buildAttemptBundle(snap);

    expect(bundle.source).toBe(src);
    expect(bundle.entryFunctionName).toBe(problem.entryFunctionName);
    expect(bundle.tests).toEqual(problem.visibleTests);
    expect(bundle.evaluatorVersion).toBe(problem.evaluatorVersion);
  });

  it("tests array is a shallow copy, not the snapshot's array", () => {
    const problem = makeProblem();
    const snap = createAttemptSnapshot(problem, "function solve(n) { return n; }");
    const bundle = buildAttemptBundle(snap);

    // Same content but different reference (shallow copy via spread)
    expect(bundle.tests).toEqual(snap.tests);
    expect(bundle.tests).not.toBe(snap.tests as unknown as typeof bundle.tests);
  });
});

// ---------------------------------------------------------------------------
// bundleByteLength
// ---------------------------------------------------------------------------

describe("bundleByteLength", () => {
  it("returns a positive number", () => {
    const problem = makeProblem();
    const snap = createAttemptSnapshot(problem, "function solve(n) { return n; }");
    const bundle = buildAttemptBundle(snap);

    const bytes = bundleByteLength(bundle);
    expect(bytes).toBeGreaterThan(0);
  });

  it("is larger for larger sources", () => {
    const problem = makeProblem();
    const short = buildAttemptBundle(
      createAttemptSnapshot(problem, "function s(){}"),
    );
    const long = buildAttemptBundle(
      createAttemptSnapshot(problem, "function solve(n) { return n * 2 + 1; }"),
    );
    expect(bundleByteLength(long)).toBeGreaterThan(bundleByteLength(short));
  });
});

// ---------------------------------------------------------------------------
// validateAttemptBundleShape
// ---------------------------------------------------------------------------

describe("validateAttemptBundleShape", () => {
  const validBundle: AttemptBundle = {
    source: "function solve(n) { return n; }",
    entryFunctionName: "solve",
    tests: [{ id: "t1", args: [1], expected: 1 }],
    evaluatorVersion: "1.0.0",
  };

  it("accepts a valid bundle", () => {
    expect(validateAttemptBundleShape(validBundle)).toBe(true);
  });

  it("rejects null", () => {
    expect(validateAttemptBundleShape(null)).toBe(false);
  });

  it("rejects non-objects", () => {
    expect(validateAttemptBundleShape("string")).toBe(false);
    expect(validateAttemptBundleShape(42)).toBe(false);
    expect(validateAttemptBundleShape(undefined)).toBe(false);
  });

  it("rejects blank source", () => {
    expect(
      validateAttemptBundleShape({ ...validBundle, source: "" }),
    ).toBe(false);
    expect(
      validateAttemptBundleShape({ ...validBundle, source: "   " }),
    ).toBe(false);
  });

  it("rejects missing source field", () => {
    const { source: _s, ...rest } = validBundle;
    expect(validateAttemptBundleShape(rest)).toBe(false);
  });

  it("rejects missing entryFunctionName", () => {
    const { entryFunctionName: _e, ...rest } = validBundle;
    expect(validateAttemptBundleShape(rest)).toBe(false);
  });

  it("rejects missing evaluatorVersion", () => {
    const { evaluatorVersion: _v, ...rest } = validBundle;
    expect(validateAttemptBundleShape(rest)).toBe(false);
  });

  it("rejects missing tests array", () => {
    const { tests: _t, ...rest } = validBundle;
    expect(validateAttemptBundleShape(rest)).toBe(false);
  });

  it("rejects tests that is not an array", () => {
    expect(
      validateAttemptBundleShape({ ...validBundle, tests: "not-array" }),
    ).toBe(false);
  });

  it("rejects tests with missing id", () => {
    expect(
      validateAttemptBundleShape({
        ...validBundle,
        tests: [{ args: [1], expected: 1 }],
      }),
    ).toBe(false);
  });

  it("rejects tests with missing expected", () => {
    expect(
      validateAttemptBundleShape({
        ...validBundle,
        tests: [{ id: "t1", args: [1] }],
      }),
    ).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// assertBundleValid
// ---------------------------------------------------------------------------

describe("assertBundleValid", () => {
  const validBundle: AttemptBundle = {
    source: "function solve(n) { return n; }",
    entryFunctionName: "solve",
    tests: [{ id: "t1", args: [1], expected: 1 }],
    evaluatorVersion: "1.0.0",
  };

  it("does not throw for a valid bundle", () => {
    expect(() => assertBundleValid(validBundle)).not.toThrow();
  });

  it("rejects non-JSON-safe bundles", () => {
    // undefined values are not JSON-safe
    const badBundle = {
      ...validBundle,
      source: undefined,
    };
    expect(() => assertBundleValid(badBundle as unknown as AttemptBundle)).toThrow(
      /non-JSON-safe/i,
    );
  });

  it("rejects oversized bundles", () => {
    // Build a bundle that exceeds MAX_BUNDLE_BYTES by stuffing the
    // source field with many characters.
    const longSource = "a".repeat(MAX_BUNDLE_BYTES);
    const bigBundle: AttemptBundle = {
      ...validBundle,
      source: longSource,
    };
    expect(() => assertBundleValid(bigBundle)).toThrow(
      AttemptValidationError,
    );
    expect(() => assertBundleValid(bigBundle)).toThrow(/bytes/);
  });
});

// ---------------------------------------------------------------------------
// isRunReport
// ---------------------------------------------------------------------------

describe("isRunReport", () => {
  it("accepts a valid passed report", () => {
    const report = makeRunReport({ status: "passed", total: 3, passed: 3 });
    expect(isRunReport(report)).toBe(true);
  });

  it("accepts a valid failed report with firstFailure", () => {
    const report = makeRunReport({
      status: "failed",
      total: 3,
      passed: 2,
      firstFailure: makeFirstFailure(),
    });
    expect(isRunReport(report)).toBe(true);
  });

  it("accepts a syntax_error report", () => {
    const report = makeRunReport({ status: "syntax_error", total: 0, passed: 0 });
    expect(isRunReport(report)).toBe(true);
  });

  it("accepts a runtime_error report", () => {
    const report = makeRunReport({
      status: "runtime_error",
      total: 1,
      passed: 0,
      firstFailure: { testId: "source", args: [], expected: null, error: "TypeError" },
    });
    expect(isRunReport(report)).toBe(true);
  });

  it("accepts a timeout report", () => {
    const report = makeRunReport({ status: "timeout", total: 3, passed: 1 });
    expect(isRunReport(report)).toBe(true);
  });

  it("rejects non-object values", () => {
    expect(isRunReport(null)).toBe(false);
    expect(isRunReport("string")).toBe(false);
    expect(isRunReport(42)).toBe(false);
    expect(isRunReport(undefined)).toBe(false);
  });

  it("reports invalid status", () => {
    const report = makeRunReport();
    expect(isRunReport({ ...report, status: "bogus" })).toBe(false);
  });

  it("rejects non-integer total", () => {
    const report = makeRunReport({ total: 3.5, passed: 0, status: "failed" });
    expect(isRunReport(report)).toBe(false);
  });

  it("rejects negative total", () => {
    const report = makeRunReport({ total: -1, passed: 0, status: "failed" });
    expect(isRunReport(report)).toBe(false);
  });

  it("rejects passed > total", () => {
    const report = makeRunReport({ status: "failed", total: 2, passed: 5 });
    expect(isRunReport(report)).toBe(false);
  });

  it("rejects negative elapsedMs", () => {
    const report = makeRunReport({ elapsedMs: -1 });
    expect(isRunReport(report)).toBe(false);
  });

  it("rejects non-finite elapsedMs", () => {
    expect(isRunReport(makeRunReport({ elapsedMs: Infinity }))).toBe(false);
    expect(isRunReport(makeRunReport({ elapsedMs: NaN }))).toBe(false);
  });

  it("rejects missing evaluatorVersion", () => {
    const { evaluatorVersion: _v, ...rest } = makeRunReport();
    expect(isRunReport(rest)).toBe(false);
  });

  it("rejects malformed firstFailure", () => {
    const report = makeRunReport({
      status: "failed",
      total: 2,
      passed: 1,
      firstFailure: { testId: "t1" } as unknown as FirstFailure,
    });
    expect(isRunReport(report)).toBe(false);
  });

  it("accepts firstFailure with all optional fields absent", () => {
    const report = makeRunReport({
      status: "failed",
      total: 2,
      passed: 1,
      firstFailure: { testId: "t1", args: [1], expected: 1 },
    });
    expect(isRunReport(report)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// isFirstFailure
// ---------------------------------------------------------------------------

describe("isFirstFailure", () => {
  it("accepts a complete firstFailure", () => {
    const ff = makeFirstFailure();
    expect(isFirstFailure(ff)).toBe(true);
  });

  it("accepts a minimal firstFailure (only required fields)", () => {
    expect(isFirstFailure({ testId: "t1", args: [1], expected: 1 })).toBe(
      true,
    );
  });

  it("rejects null", () => {
    expect(isFirstFailure(null)).toBe(false);
  });

  it("rejects missing testId", () => {
    expect(isFirstFailure({ args: [1], expected: 1 })).toBe(false);
  });

  it("rejects missing args", () => {
    expect(isFirstFailure({ testId: "t1", expected: 1 })).toBe(false);
  });

  it("rejects missing expected", () => {
    expect(isFirstFailure({ testId: "t1", args: [1] })).toBe(false);
  });

  it("rejects non-array args", () => {
    expect(
      isFirstFailure({ testId: "t1", args: "not-array", expected: 1 }),
    ).toBe(false);
  });

  it("rejects non-string error field", () => {
    expect(
      isFirstFailure({
        testId: "t1",
        args: [1],
        expected: 1,
        error: 42,
      }),
    ).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// isJsonSafe (attempt module's defensive version)
// ---------------------------------------------------------------------------

describe("isJsonSafe", () => {
  it("accepts JSON primitives", () => {
    expect(isJsonSafe(null)).toBe(true);
    expect(isJsonSafe("hello")).toBe(true);
    expect(isJsonSafe(42)).toBe(true);
    expect(isJsonSafe(true)).toBe(true);
  });

  it("rejects NaN and Infinity", () => {
    expect(isJsonSafe(NaN)).toBe(false);
    expect(isJsonSafe(Infinity)).toBe(false);
    expect(isJsonSafe(-Infinity)).toBe(false);
  });

  it("rejects undefined", () => {
    expect(isJsonSafe(undefined)).toBe(false);
  });

  it("rejects non-plain objects", () => {
    expect(isJsonSafe(new Date())).toBe(false);
  });

  it("accepts deep nested structures", () => {
    expect(isJsonSafe({ a: [{ b: [1, 2] }] })).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

describe("constants", () => {
  it("MAX_SOURCE_BYTES is positive", () => {
    expect(MAX_SOURCE_BYTES).toBeGreaterThan(0);
  });

  it("MAX_BUNDLE_BYTES is larger than MAX_SOURCE_BYTES", () => {
    expect(MAX_BUNDLE_BYTES).toBeGreaterThan(MAX_SOURCE_BYTES);
  });
});
