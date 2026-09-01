/**
 * Phase 3 tutor-validation unit tests -- output safety guards.
 *
 * Tests input/output validation without depending on real model output
 * or asynchronous workflows.
 */

import { describe, it, expect } from "vitest";
import {
  validateTutorOutput,
  validateAgentResult,
  tutorOutputValidationErrors,
  isSafeTutorPreview,
} from "../tutor-validation";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function validOutput(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    mode: "hint",
    hint: "Think about what happens when the array is empty.",
    concept: "edge cases",
    next_steps: [
      "Consider the empty-array scenario.",
      "Test your code with nums = [].",
      "Check the problem constraints again.",
    ],
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// validateTutorOutput
// ---------------------------------------------------------------------------

describe("validateTutorOutput", () => {
  // ---- valid outputs ----

  it("accepts a valid hint output for a failed report", () => {
    const result = validateTutorOutput(validOutput(), "failed");
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.guidance.mode).toBe("hint");
      expect(result.guidance.hint).toContain("empty");
      expect(result.guidance.concept).toBe("edge cases");
      expect(result.guidance.next_steps.length).toBe(3);
    }
  });

  it("accepts a valid success_review output for a passed report", () => {
    const output = validOutput({
      mode: "success_review",
      hint: "Good solution. Consider the O(n) hash-map approach for better performance.",
    });
    const result = validateTutorOutput(output, "passed");
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.guidance.mode).toBe("success_review");
    }
  });

  it("accepts output with exactly 2 next_steps", () => {
    const output = validOutput({
      next_steps: ["Fix the loop bounds.", "Re-run the tests."],
    });
    const result = validateTutorOutput(output, "runtime_error");
    expect(result.valid).toBe(true);
  });

  it("accepts output with exactly 4 next_steps", () => {
    const output = validOutput({
      next_steps: [
        "Step 1",
        "Step 2",
        "Step 3",
        "Step 4",
      ],
    });
    const result = validateTutorOutput(output, "timeout");
    expect(result.valid).toBe(true);
  });

  // ---- structural errors ----

  it("rejects non-object input", () => {
    expect(validateTutorOutput(null, "failed").valid).toBe(false);
    expect(validateTutorOutput("string", "failed").valid).toBe(false);
    expect(validateTutorOutput(42, "failed").valid).toBe(false);
    expect(validateTutorOutput([], "failed").valid).toBe(false);
  });

  it("error result includes errors array", () => {
    const result = validateTutorOutput(null, "failed");
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0]).toContain("plain object");
    }
  });

  // ---- deterministic presentation mode ----

  it("derives hint mode for a failed report despite incorrect model metadata", () => {
    const output = validOutput({ mode: "success_review" });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(true);
    if (result.valid) expect(result.guidance.mode).toBe("hint");
  });

  it("derives success-review mode for a passed report despite incorrect model metadata", () => {
    const output = validOutput({ mode: "hint" });
    const result = validateTutorOutput(output, "passed");
    expect(result.valid).toBe(true);
    if (result.valid) expect(result.guidance.mode).toBe("success_review");
  });

  it("accepts missing or arbitrary optional mode metadata", () => {
    const { mode: _m, ...withoutMode } = validOutput();
    const missing = validateTutorOutput(withoutMode, "failed");
    const arbitrary = validateTutorOutput(validOutput({ mode: "anything" }), "failed");
    expect(missing.valid).toBe(true);
    expect(arbitrary.valid).toBe(true);
  });

  // ---- hint validation ----

  it("rejects empty hint", () => {
    const output = validOutput({ hint: "" });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("rejects whitespace-only hint", () => {
    const output = validOutput({ hint: "   " });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("rejects missing hint", () => {
    const { hint: _h, ...rest } = validOutput();
    const result = validateTutorOutput(rest, "failed");
    expect(result.valid).toBe(false);
  });

  // ---- concept validation ----

  it("rejects empty concept", () => {
    const output = validOutput({ concept: "" });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("reports an oversized concept as a retryable semantic validation error", () => {
    const output = validOutput({ concept: "c".repeat(181) });
    const errors = tutorOutputValidationErrors(output, "failed");

    expect(errors).toEqual([
      "concept must be no longer than 180 characters.",
    ]);
  });

  // ---- next_steps validation ----

  it("fills deterministic actions when a small model omits next_steps", () => {
    const { next_steps: _ns, ...rest } = validOutput();
    const result = validateTutorOutput(rest, "failed");
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.guidance.next_steps).toEqual([
        "Use the deterministic failure report to check the assumption behind this hint.",
        "Update your solution, then run the public tests again.",
      ]);
    }
  });

  it("uses review-specific fallback actions for a passed report", () => {
    const { next_steps: _ns, ...rest } = validOutput({ mode: "success_review" });
    const result = validateTutorOutput(rest, "passed");
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.guidance.next_steps).toEqual([
        "Estimate the time and space complexity of your approach.",
        "Try one boundary case before moving to a harder variation.",
      ]);
    }
  });

  it("preserves one valid model action and adds a deterministic second action", () => {
    // Mirrors the one-item ProduceGuidance output captured in the supplied
    // TinyLlama browser trace.
    const output = validOutput({
      hint: "The two numbers must be in ascending order.",
      concept: "The indices of the two numbers must be valid.",
      next_steps: [
        "Verify that the indices are valid by checking array bounds and distinct elements.",
      ],
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.guidance.next_steps).toEqual([
        "Verify that the indices are valid by checking array bounds and distinct elements.",
        "Use the deterministic failure report to check the assumption behind this hint.",
      ]);
    }
  });

  it("bounds overly long model action lists to four safe items", () => {
    const output = validOutput({
      next_steps: ["a", "b", "c", "d", "e"],
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(true);
    if (result.valid) expect(result.guidance.next_steps).toEqual(["a", "b", "c", "d"]);
  });

  it("rejects non-array next_steps", () => {
    const output = validOutput({ next_steps: "not-an-array" });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("rejects empty string in next_steps", () => {
    const output = validOutput({
      next_steps: ["Good step", "", "Another step"],
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  // ---- code fence rejection ----

  it("rejects hint containing code fences", () => {
    const output = validOutput({ hint: "Try this: ```function f() {}```" });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.errors.some((e) => e.includes("code fence"))).toBe(true);
    }
  });

  it("rejects concept containing code fences or a full function", () => {
    const fenced = validateTutorOutput(validOutput({ concept: "```recursion```" }), "failed");
    const functionLike = validateTutorOutput(
      validOutput({ concept: "function twoSum(values) { return values; }" }),
      "failed",
    );
    expect(fenced.valid).toBe(false);
    expect(functionLike.valid).toBe(false);
  });

  it("rejects next_steps item containing code fences or function bodies", () => {
    const fenced = validateTutorOutput(validOutput({
      next_steps: ["Good step", "```dangerous```", "Another step"],
    }), "failed");
    const arrow = validateTutorOutput(validOutput({
      next_steps: ["Good step", "const answer = () => { return 1; }", "Another step"],
    }), "failed");
    expect(fenced.valid).toBe(false);
    expect(arrow.valid).toBe(false);
  });

  // ---- full-function rejection ----

  it("rejects hint that looks like a complete function", () => {
    const output = validOutput({
      hint: "You can write: function twoSum(nums, target) { const map = {}; for (let i = 0; i < nums.length; i++) { const complement = target - nums[i]; if (complement in map) return [map[complement], i]; map[nums[i]] = i; } return []; }",
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("rejects hint with async function pattern", () => {
    const output = validOutput({
      hint: "Use: async function fetchData(url) { const res = await fetch(url); return res.json(); }",
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("rejects hint with arrow-function body braces", () => {
    const output = validOutput({
      hint: "Consider using an arrow: const fn = (x) => { return x * 2; }",
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
  });

  it("accepts hint that mentions arrow without body braces", () => {
    // Arrow expression without braces is fine
    const output = validOutput({
      hint: "Try using .map(x => x * 2) instead of a loop.",
    });
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(true);
  });

  // ---- unexpected keys ----

  it("rejects unexpected keys in output", () => {
    const output = { ...validOutput(), extraField: "should not be here" };
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.errors.some((e) => e.includes("extraField"))).toBe(true);
    }
  });

  // ---- multiple errors collected at once ----

  it("collects all errors when multiple violations exist", () => {
    const output = {
      mode: "bogus",
      hint: "```function f() {}```",
      concept: "",
      next_steps: ["only one"],
    };
    const result = validateTutorOutput(output, "failed");
    expect(result.valid).toBe(false);
    if (!result.valid) {
      // mode is ignored, while hint has code fence + function and concept is empty.
      expect(result.errors.length).toBeGreaterThanOrEqual(3);
    }
  });
});

// ---------------------------------------------------------------------------
// streamed-preview safety
// ---------------------------------------------------------------------------

describe("isSafeTutorPreview", () => {
  it("accepts short prose and rejects empty or code-like text", () => {
    expect(isSafeTutorPreview("What happens when the array is empty?")).toBe(true);
    expect(isSafeTutorPreview("   ")).toBe(false);
    expect(isSafeTutorPreview("```const answer = 1```")).toBe(false);
    expect(isSafeTutorPreview("function solve(items) { return items; }")).toBe(false);
    expect(isSafeTutorPreview("function solve(items) { return")).toBe(false);
    expect(isSafeTutorPreview("const solve = (items) =>")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// validateAgentResult
// ---------------------------------------------------------------------------

describe("validateAgentResult", () => {
  it("accepts a direct AgentOutput shape", () => {
    const output = validOutput();
    const result = validateAgentResult(output, "failed");
    expect(result.valid).toBe(true);
  });

  it("accepts a nested { output: {...} } shape", () => {
    const result = validateAgentResult(
      { output: validOutput() },
      "failed",
    );
    expect(result.valid).toBe(true);
  });

  it("rejects non-object results", () => {
    const result = validateAgentResult(null, "failed");
    expect(result.valid).toBe(false);
    expect(result.valid === false && result.errors[0]).toContain("object");
  });

  it("rejects results without recognisable shape", () => {
    const result = validateAgentResult(
      { something: "else" },
      "failed",
    );
    expect(result.valid).toBe(false);
    expect(result.valid === false && result.errors[0]).toContain("recognis");
  });

  it("validates nested output against same safety rules", () => {
    // nested output with code fence in hint
    const result = validateAgentResult(
      { output: validOutput({ hint: "```code```" }) },
      "failed",
    );
    expect(result.valid).toBe(false);
  });
});
