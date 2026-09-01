/**
 * Phase 3 tutor-request unit tests -- request builder, hint levels,
 * field capping, and error conditions.
 *
 * Tests ONLY pure exported functions; never touches workflows or models.
 * Uses synthetic Problem fixtures to avoid coupling to the real catalog.
 */

import { describe, it, expect } from "vitest";
import type { Problem, RunReport, RunReportStatus } from "../../catalog/types";
import { createAttemptSnapshot, type AttemptSnapshot } from "../../execution/attempt";
import {
  createTutorRequest,
  buildProblemMetadata,
  buildProblemContext,
  isHintLevel,
  VALID_HINT_LEVELS,
  tutorRequestToAgentInput,
  TutorRequestError,
  type HintLevel,
  type TutorRequest,
} from "../tutor-request";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeProblem(overrides: Partial<Problem> = {}): Problem {
  return {
    id: "two-sum",
    title: "Two Sum",
    difficulty: "beginner",
    topics: ["arrays", "hash-map"],
    statement: "Given an array of integers nums and an integer target, return indices of the two numbers that add up to target.",
    constraints: ["2 <= nums.length <= 10^4", "-10^9 <= nums[i] <= 10^9"],
    examples: [{ input: "nums = [2,7,11,15], target = 9", output: "[0,1]" }],
    entryFunctionName: "twoSum",
    starterCode: "function twoSum(nums, target) {\n  // TODO\n}",
    visibleTests: [
      { id: "t1", args: [[2, 7, 11, 15], 9], expected: [0, 1] },
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

function makeReport(
  overrides: Partial<RunReport> = {},
): RunReport {
  return {
    status: "failed" as RunReportStatus,
    total: 3,
    passed: 1,
    elapsedMs: 120,
    evaluatorVersion: "1.0.0",
    ...overrides,
  };
}

function makeSnapshot(
  problem: Problem,
  source?: string,
): AttemptSnapshot {
  return createAttemptSnapshot(
    problem,
    source ?? "function twoSum(nums, target) { for (let i = 0; i < nums.length; i++) { for (let j = i + 1; j < nums.length; j++) { if (nums[i] + nums[j] === target) return [i, j]; } } return []; }",
  );
}

// ---------------------------------------------------------------------------
// isHintLevel
// ---------------------------------------------------------------------------

describe("isHintLevel", () => {
  it("accepts all valid hint levels", () => {
    for (const level of VALID_HINT_LEVELS) {
      expect(isHintLevel(level)).toBe(true);
    }
  });

  it("rejects invalid strings", () => {
    expect(isHintLevel("full_solution")).toBe(false);
    expect(isHintLevel("")).toBe(false);
    expect(isHintLevel("NUDGE")).toBe(false);
    expect(isHintLevel("hint")).toBe(false);
  });

  it("rejects non-strings", () => {
    expect(isHintLevel(null)).toBe(false);
    expect(isHintLevel(undefined)).toBe(false);
    expect(isHintLevel(42)).toBe(false);
    expect(isHintLevel({})).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// buildProblemMetadata
// ---------------------------------------------------------------------------

describe("buildProblemMetadata", () => {
  it("extracts correct metadata fields", () => {
    const problem = makeProblem();
    const meta = buildProblemMetadata(problem);

    expect(meta.problemId).toBe("two-sum");
    expect(meta.title).toBe("Two Sum");
    expect(meta.difficulty).toBe("beginner");
    expect(meta.topics).toEqual(["arrays", "hash-map"]);
    expect(meta.entryFunctionName).toBe("twoSum");
  });

  it("returns a frozen object", () => {
    const meta = buildProblemMetadata(makeProblem());
    expect(Object.isFrozen(meta)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// buildProblemContext
// ---------------------------------------------------------------------------

describe("buildProblemContext", () => {
  it("includes the statement, signature, constraints, and examples", () => {
    const problem = makeProblem();
    const ctx = buildProblemContext(problem);

    expect(ctx).toContain("Given an array of integers");
    expect(ctx).toContain("Function: twoSum");
    expect(ctx).toContain("Constraints:");
    expect(ctx).toContain("2 <= nums.length");
    expect(ctx).toContain("Examples:");
    expect(ctx).toContain("nums = [2,7,11,15]");
  });

  it("handles a problem with no constraints", () => {
    const problem = makeProblem({ constraints: [] });
    const ctx = buildProblemContext(problem);

    expect(ctx).toContain("Given an array");
    expect(ctx).not.toContain("Constraints:");
  });
});

// ---------------------------------------------------------------------------
// createTutorRequest
// ---------------------------------------------------------------------------

describe("createTutorRequest", () => {
  it("builds a request with all fields populated", () => {
    const problem = makeProblem();
    const snapshot = makeSnapshot(problem);
    const report = makeReport();
    const opts = { hintLevel: "targeted" as HintLevel, priorSummary: "" };

    const req = createTutorRequest(problem, snapshot, report, opts);

    expect(req.problemContext).toContain("Given an array");
    expect(req.learnerCode).toBe(snapshot.source);
    expect(req.runReport).toEqual(report);
    expect(req.hintLevel).toBe("targeted");
    expect(req.priorSummary).toBe("");
    expect(req.problemMetadata.problemId).toBe("two-sum");
  });

  it("returns a frozen object", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport(),
    );
    expect(Object.isFrozen(req)).toBe(true);
  });

  // ---- hint level resolution ----

  it("defaults to targeted for non-passed reports", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "failed" }),
    );
    expect(req.hintLevel).toBe("targeted");
  });

  it("defaults to targeted for syntax_error reports", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "syntax_error", total: 0, passed: 0 }),
    );
    expect(req.hintLevel).toBe("targeted");
  });

  it("forces review for passed reports regardless of caller choice", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "passed", total: 3, passed: 3 }),
      { hintLevel: "plan" },
    );
    expect(req.hintLevel).toBe("review");
  });

  it("forces review for passed reports even with no explicit hint level", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "passed", total: 3, passed: 3 }),
    );
    expect(req.hintLevel).toBe("review");
  });

  it("uses explicit hint level for non-passed reports", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "runtime_error", total: 3, passed: 0 }),
      { hintLevel: "plan" },
    );
    expect(req.hintLevel).toBe("plan");
  });

  it("uses nudge hint level when explicitly set", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport({ status: "timeout", total: 3, passed: 1 }),
      { hintLevel: "nudge" },
    );
    expect(req.hintLevel).toBe("nudge");
  });

  it("rejects review level for a non-passing report", () => {
    expect(() =>
      createTutorRequest(
        makeProblem(),
        makeSnapshot(makeProblem()),
        makeReport(),
        { hintLevel: "review" },
      ),
    ).toThrow(/review level/i);
  });

  // ---- priorSummary ----

  it("passes through priorSummary", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport(),
      { priorSummary: "Previously: consider edge case n=0." },
    );
    expect(req.priorSummary).toBe("Previously: consider edge case n=0.");
  });

  it("caps overlong priorSummary", () => {
    const long = "x".repeat(9000);
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport(),
      { priorSummary: long },
    );
    expect(req.priorSummary.length).toBeLessThan(long.length);
    expect(req.priorSummary).toContain("...");
  });

  // ---- error conditions ----

  it("rejects mismatched problem id", () => {
    const problem = makeProblem({ id: "other" });
    const snapshot = makeSnapshot(makeProblem({ id: "two-sum" }));
    expect(() =>
      createTutorRequest(problem, snapshot, makeReport()),
    ).toThrow(TutorRequestError);
    expect(() =>
      createTutorRequest(problem, snapshot, makeReport()),
    ).toThrow(/does not match/i);
  });

  it("rejects mismatched evaluator version", () => {
    const problem = makeProblem();
    const snapshot = makeSnapshot(problem);
    const report = makeReport({ evaluatorVersion: "2.0.0" });
    expect(() =>
      createTutorRequest(problem, snapshot, report),
    ).toThrow(TutorRequestError);
  });

  it("rejects a snapshot from an older evaluator even when its report matches it", () => {
    const oldProblem = makeProblem({ evaluatorVersion: "1.0.0" });
    const snapshot = makeSnapshot(oldProblem);
    const report = makeReport({ evaluatorVersion: "1.0.0" });
    const currentProblem = makeProblem({ evaluatorVersion: "2.0.0" });

    expect(() => createTutorRequest(currentProblem, snapshot, report)).toThrow(
      TutorRequestError,
    );
  });

  it("uses snapshot source, not live editor source", () => {
    const problem = makeProblem();
    const snapshot = makeSnapshot(problem, "function twoSum(nums, target) { /* snapshot */ }");
    const report = makeReport();

    const req = createTutorRequest(problem, snapshot, report);

    // `learnerCode` must come from the snapshot, never from a "live" source
    expect(req.learnerCode).toContain("/* snapshot */");
  });

  it("rejects rather than truncating an oversized immutable source snapshot", () => {
    const problem = makeProblem();
    const snapshot = makeSnapshot(problem, "x".repeat(24001));

    expect(() => createTutorRequest(problem, snapshot, makeReport())).toThrow(
      /immutable submission snapshot/i,
    );
  });

  // ---- immutability of nested objects ----

  it("deep-freezes an independent runReport copy", () => {
    const report = makeReport({
      firstFailure: {
        testId: "t1",
        args: [[1]],
        expected: 2,
        actual: 1,
      },
    });
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      report,
    );
    expect(Object.isFrozen(req.runReport)).toBe(true);
    expect(Object.isFrozen(req.runReport.firstFailure)).toBe(true);
    expect(Object.isFrozen(req.runReport.firstFailure?.args)).toBe(true);
    expect(req.runReport).not.toBe(report);
  });

  it("deep-freezes problemMetadata", () => {
    const req = createTutorRequest(
      makeProblem(),
      makeSnapshot(makeProblem()),
      makeReport(),
    );
    expect(Object.isFrozen(req.problemMetadata)).toBe(true);
    expect(Object.isFrozen(req.problemMetadata.topics)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// tutorRequestToAgentInput
// ---------------------------------------------------------------------------

describe("tutorRequestToAgentInput", () => {
  it("wraps the request under the tutor_request key", () => {
    const req: TutorRequest = {
      problemContext: "context",
      learnerCode: "code",
      runReport: makeReport({ status: "passed", total: 1, passed: 1 }),
      hintLevel: "review",
      priorSummary: "",
      problemMetadata: {
        problemId: "x",
        title: "X",
        difficulty: "beginner",
        topics: [],
        entryFunctionName: "f",
      },
    };

    const input = tutorRequestToAgentInput(req);
    expect(input).toEqual({ tutor_request: req });
    expect(input.tutor_request).toBe(req);
  });
});

// ---------------------------------------------------------------------------
// TutorRequestError
// ---------------------------------------------------------------------------

describe("TutorRequestError", () => {
  it("is an instance of Error", () => {
    const err = new TutorRequestError("test");
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("TutorRequestError");
    expect(err.message).toBe("test");
  });
});
