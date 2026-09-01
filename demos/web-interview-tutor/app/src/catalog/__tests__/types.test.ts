// ---------------------------------------------------------------------------
// Phase 2 catalog unit tests — types, validators, and catalog integrity.
//
// Tests ONLY pure exported functions and the compiled PROBLEMS catalog;
// never executes learner source or touches the sandbox runtime.
// ---------------------------------------------------------------------------

import { describe, it, expect } from "vitest";
import {
  isJsonSafe,
  isValidProblemId,
  isValidJsIdentifier,
  validateProblem,
  validateCatalog,
  isCatalogValid,
} from "../types";
import type { Problem, JsonValue } from "../types";
import { PROBLEMS, getProblemById, ALL_TOPICS } from "../problems";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal valid problem for targeted validation tests. */
function makeMinimalProblem(
  overrides: Partial<Problem> = {},
): Problem {
  return {
    id: "test-problem",
    title: "Test Problem",
    difficulty: "beginner",
    topics: ["arrays"],
    statement: "Solve the test.",
    constraints: ["0 < n < 100"],
    examples: [{ input: "n = 1", output: "1" }],
    entryFunctionName: "testFn",
    starterCode: "function testFn(n) { return n; }",
    visibleTests: [
      { id: "t1", args: [1], expected: 1 },
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

// ---------------------------------------------------------------------------
// isJsonSafe
// ---------------------------------------------------------------------------

describe("isJsonSafe", () => {
  it("accepts primitives", () => {
    expect(isJsonSafe(null)).toBe(true);
    expect(isJsonSafe("hello")).toBe(true);
    expect(isJsonSafe(42)).toBe(true);
    expect(isJsonSafe(true)).toBe(true);
    expect(isJsonSafe(false)).toBe(true);
  });

  it("accepts bare arrays and objects", () => {
    expect(isJsonSafe([])).toBe(true);
    expect(isJsonSafe([1, "two", null])).toBe(true);
    expect(isJsonSafe({})).toBe(true);
    expect(isJsonSafe({ a: 1, b: [true, null] })).toBe(true);
  });

  it("accepts deeply nested structures", () => {
    expect(
      isJsonSafe({ a: [{ b: { c: [1, [2, [3]]] } }] }),
    ).toBe(true);
  });

  it("rejects non-finite numbers", () => {
    expect(isJsonSafe(NaN)).toBe(false);
    expect(isJsonSafe(Infinity)).toBe(false);
    expect(isJsonSafe(-Infinity)).toBe(false);
  });

  it("rejects undefined", () => {
    expect(isJsonSafe(undefined)).toBe(false);
  });

  it("rejects functions", () => {
    expect(isJsonSafe(() => {})).toBe(false);
  });

  it("rejects symbols", () => {
    expect(isJsonSafe(Symbol("test"))).toBe(false);
  });

  it("rejects objects with non-plain prototype", () => {
    expect(isJsonSafe(new Date())).toBe(false);
    expect(isJsonSafe(new Map())).toBe(false);
    expect(isJsonSafe(new Set())).toBe(false);
  });

  it("rejects objects containing undefined", () => {
    // typeof undefined !== 'object' && !== 'string'/'number'/'boolean'
    // so it falls to the default branch → false.
    expect(isJsonSafe({ a: undefined })).toBe(false);
    expect(isJsonSafe([1, undefined, 2])).toBe(false);
  });

  it("rejects non-finite numbers nested in objects", () => {
    expect(isJsonSafe({ a: NaN })).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// isValidProblemId
// ---------------------------------------------------------------------------

describe("isValidProblemId", () => {
  it("accepts valid kebab-case slugs", () => {
    expect(isValidProblemId("two-sum")).toBe(true);
    expect(isValidProblemId("a")).toBe(true);
    expect(isValidProblemId("abc123")).toBe(true);
    expect(isValidProblemId("a-b-c")).toBe(true);
    expect(isValidProblemId("valid-palindrome-2")).toBe(true);
  });

  it("rejects empty strings", () => {
    expect(isValidProblemId("")).toBe(false);
  });

  it("rejects uppercase characters", () => {
    expect(isValidProblemId("Two-Sum")).toBe(false);
  });

  it("rejects leading digits", () => {
    expect(isValidProblemId("2sum")).toBe(false);
  });

  it("rejects leading hyphens", () => {
    expect(isValidProblemId("-test")).toBe(false);
  });

  it("rejects trailing hyphens", () => {
    expect(isValidProblemId("test-")).toBe(false);
  });

  it("rejects underscores", () => {
    expect(isValidProblemId("two_sum")).toBe(false);
  });

  it("rejects spaces", () => {
    expect(isValidProblemId("two sum")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// isValidJsIdentifier
// ---------------------------------------------------------------------------

describe("isValidJsIdentifier", () => {
  it("accepts valid identifiers", () => {
    expect(isValidJsIdentifier("foo")).toBe(true);
    expect(isValidJsIdentifier("_bar")).toBe(true);
    expect(isValidJsIdentifier("$baz")).toBe(true);
    expect(isValidJsIdentifier("camelCase")).toBe(true);
    expect(isValidJsIdentifier("PascalCase")).toBe(true);
    expect(isValidJsIdentifier("a1")).toBe(true);
    expect(isValidJsIdentifier("_")).toBe(true);
  });

  it("rejects empty string", () => {
    expect(isValidJsIdentifier("")).toBe(false);
  });

  it("rejects leading digits", () => {
    expect(isValidJsIdentifier("1foo")).toBe(false);
  });

  it("rejects hyphens", () => {
    expect(isValidJsIdentifier("two-sum")).toBe(false);
  });

  it("rejects spaces", () => {
    expect(isValidJsIdentifier("foo bar")).toBe(false);
  });

  it("rejects reserved characters", () => {
    expect(isValidJsIdentifier("foo.bar")).toBe(false);
    expect(isValidJsIdentifier("foo!")).toBe(false);
    expect(isValidJsIdentifier("foo@")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// validateProblem
// ---------------------------------------------------------------------------

describe("validateProblem", () => {
  it("validates a well-formed problem", () => {
    const problem = makeMinimalProblem();
    const result = validateProblem(problem);
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it("rejects invalid problem id", () => {
    const problem = makeMinimalProblem({ id: "Invalid ID!" });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("problem id"))).toBe(true);
  });

  it("rejects invalid entry function name", () => {
    const problem = makeMinimalProblem({ entryFunctionName: "123bad" });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("entryFunctionName")),
    ).toBe(true);
  });

  it("rejects empty visible tests", () => {
    const problem = makeMinimalProblem({ visibleTests: [] });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("visibleTests")),
    ).toBe(true);
  });

  it("rejects missing test id", () => {
    const problem = makeMinimalProblem({
      // @ts-expect-error — deliberate invalid test shape
      visibleTests: [{ args: [1], expected: 1 }],
    });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("empty or missing id")),
    ).toBe(true);
  });

  it("rejects non-JSON-safe test args (undefined)", () => {
    // undefined is not JSON-safe — validateProblem flags it.
    const problem = makeMinimalProblem({
      visibleTests: [
        { id: "t1", args: [undefined], expected: 1 },
      ] as unknown as Problem["visibleTests"],
    });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("not JSON-safe")),
    ).toBe(true);
  });

  it("rejects non-JSON-safe test expected (undefined)", () => {
    const problem = makeMinimalProblem({
      visibleTests: [
        { id: "t1", args: [1], expected: undefined },
      ] as unknown as Problem["visibleTests"],
    });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("not JSON-safe")),
    ).toBe(true);
  });

  it("rejects empty evaluator version", () => {
    const problem = makeMinimalProblem({ evaluatorVersion: "" });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("evaluatorVersion")),
    ).toBe(true);
  });

  it("rejects non-positive timeoutMs", () => {
    const problem = makeMinimalProblem({
      executionLimits: {
        timeoutMs: 0,
        maxSourceBytes: 65536,
        maxInputBytes: 262144,
        maxOutputBytes: 262144,
      },
    });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("timeoutMs")),
    ).toBe(true);
  });

  it("rejects blank starter code", () => {
    const problem = makeMinimalProblem({ starterCode: "   " });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("starterCode")),
    ).toBe(true);
  });

  it("rejects dangerous patterns in starter code", () => {
    const problem = makeMinimalProblem({
      starterCode: "function foo() { eval('alert(1)'); }",
    });
    const result = validateProblem(problem);
    expect(result.valid).toBe(false);
    expect(
      result.errors.some((e) => e.includes("dangerous pattern")),
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// validateCatalog
// ---------------------------------------------------------------------------

describe("validateCatalog", () => {
  it("validates a clean catalog", () => {
    const problems = [
      makeMinimalProblem({ id: "p1" }),
      makeMinimalProblem({ id: "p2" }),
    ];
    const { results, catalogErrors } = validateCatalog(problems);
    expect(catalogErrors).toHaveLength(0);
    expect(results).toHaveLength(2);
    expect(results.every((r) => r.valid)).toBe(true);
  });

  it("detects duplicate IDs", () => {
    const problems = [
      makeMinimalProblem({ id: "dup" }),
      makeMinimalProblem({ id: "dup" }),
    ];
    const { catalogErrors } = validateCatalog(problems);
    expect(catalogErrors).toHaveLength(1);
    expect(catalogErrors[0]).toContain("Duplicate problem id");
  });

  it("includes per-problem errors alongside catalog errors", () => {
    const problems = [
      makeMinimalProblem({ id: "dup", entryFunctionName: "1bad" }),
      makeMinimalProblem({ id: "dup", entryFunctionName: "2bad" }),
    ];
    const { results, catalogErrors } = validateCatalog(problems);
    expect(catalogErrors).toHaveLength(1);
    expect(results).toHaveLength(2);
    expect(results[0].valid).toBe(false);
    expect(results[1].valid).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// isCatalogValid
// ---------------------------------------------------------------------------

describe("isCatalogValid", () => {
  it("returns true for a valid catalog", () => {
    expect(isCatalogValid([makeMinimalProblem({ id: "p1" })])).toBe(true);
  });

  it("returns false for a catalog with per-problem errors", () => {
    expect(
      isCatalogValid([makeMinimalProblem({ id: "p1", visibleTests: [] })]),
    ).toBe(false);
  });

  it("returns false for a catalog with duplicate IDs", () => {
    expect(
      isCatalogValid([
        makeMinimalProblem({ id: "same" }),
        makeMinimalProblem({ id: "same" }),
      ]),
    ).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// PROBLEMS catalog integrity
// ---------------------------------------------------------------------------

describe("PROBLEMS catalog", () => {
  it("has the expected number of problems", () => {
    expect(PROBLEMS).toHaveLength(8);
  });

  it("all problem IDs are unique", () => {
    const ids = PROBLEMS.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("every problem passes validation", () => {
    for (const problem of PROBLEMS) {
      const result = validateProblem(problem);
      if (!result.valid) {
        // Fail with details so the developer knows which problem is broken.
        expect(result.errors).toStrictEqual([]);
      }
    }
  });

  it("validateCatalog reports no errors", () => {
    const { results, catalogErrors } = validateCatalog(PROBLEMS);
    expect(catalogErrors).toHaveLength(0);
    const failures = results.filter((r) => !r.valid);
    expect(failures).toHaveLength(0);
  });

  it("every visible test in every problem is JSON-safe", () => {
    for (const problem of PROBLEMS) {
      for (const test of problem.visibleTests) {
        expect(
          isJsonSafe(test.args),
          `${problem.id} / ${test.id}: args not JSON-safe`,
        ).toBe(true);
        expect(
          isJsonSafe(test.expected),
          `${problem.id} / ${test.id}: expected not JSON-safe`,
        ).toBe(true);
      }
    }
  });

  it("getProblemById returns the correct problem", () => {
    const first = PROBLEMS[0];
    expect(getProblemById(first.id)).toBe(first);
  });

  it("getProblemById returns undefined for unknown id", () => {
    expect(getProblemById("nonexistent-problem-id")).toBeUndefined();
  });

  it("ALL_TOPICS is non-empty and sorted", () => {
    expect(ALL_TOPICS.length).toBeGreaterThan(0);
    for (let i = 1; i < ALL_TOPICS.length; i++) {
      expect(ALL_TOPICS[i] >= ALL_TOPICS[i - 1]).toBe(true);
    }
  });

  it("ALL_TOPICS has no duplicates", () => {
    expect(new Set(ALL_TOPICS).size).toBe(ALL_TOPICS.length);
  });

  it("every problem topic appears in ALL_TOPICS", () => {
    const topics = new Set(ALL_TOPICS);
    for (const problem of PROBLEMS) {
      for (const t of problem.topics) {
        expect(topics.has(t)).toBe(true);
      }
    }
  });
});

// ---------------------------------------------------------------------------
// Edge cases — JsonValue type safety
// ---------------------------------------------------------------------------

describe("JsonValue type safety", () => {
  it("isJsonSafe accepts the null object prototype case", () => {
    const obj = Object.create(null);
    obj.key = "value";
    expect(isJsonSafe(obj)).toBe(true);
  });

  it("isJsonSafe rejects objects with inherited non-JSON properties", () => {
    const obj = Object.create({ inherited: true });
    obj.own = "value";
    expect(isJsonSafe(obj)).toBe(false);
  });

  it("isJsonSafe accepts empty objects and arrays", () => {
    expect(isJsonSafe({})).toBe(true);
    expect(isJsonSafe([])).toBe(true);
  });

  it("isJsonSafe rejects BigInt", () => {
    // BigInt is not a JSON value
    expect(isJsonSafe(BigInt(1) as unknown as JsonValue)).toBe(false);
  });
});
