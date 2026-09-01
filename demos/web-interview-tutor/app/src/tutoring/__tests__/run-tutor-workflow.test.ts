/**
 * Phase 3 run-tutor-workflow unit tests -- outcome classification from
 * synthetic WorkflowEvent streams.
 *
 * Tests ONLY pure exported functions; never creates real agents, real
 * models, or any async I/O.  All events are plain-object stubs that
 * satisfy the WorkflowEvent structural contract.
 */

import { describe, it, expect } from "vitest";
import type { WorkflowEvent } from "@nemoir/web-runtime";
import type { RunReportStatus } from "../../catalog/types";
import {
  classifyTutorOutcome,
  terminalTutorOutcomeFromEvent,
  extractGuidanceFromEvents,
  extractGuidancePreview,
  runTutorToOutcome,
  type TutorRunner,
} from "../run-tutor-workflow";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type EventKind = WorkflowEvent["kind"];

function ev(
  kind: EventKind,
  overrides: Partial<Omit<WorkflowEvent, "kind">> = {},
): WorkflowEvent {
  return {
    kind,
    runId: "test-run-1",
    sequence: 0,
    timestamp: "2025-01-01T00:00:00.000Z",
    stageId: "s1",
    ...overrides,
  } as WorkflowEvent;
}

function validGuidanceOutput(mode: "hint" | "success_review" = "hint"): Record<string, unknown> {
  return {
    mode,
    hint: "Think about edge cases.",
    concept: "edge cases",
    next_steps: ["Check empty input.", "Consider null.", "Test extremes."],
  };
}

// ---------------------------------------------------------------------------
// classifyTutorOutcome
// ---------------------------------------------------------------------------

describe("classifyTutorOutcome", () => {
  const reportStatus: RunReportStatus = "failed";

  // ---- empty ----

  it("returns null for empty event array", () => {
    expect(classifyTutorOutcome([], reportStatus)).toBeNull();
  });

  // ---- run_completed with valid output ----

  it("returns completed when run_completed has valid output in result.output", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("run_completed", { result: { output } }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.guidance.mode).toBe("hint");
    }
  });

  it("returns completed when result is bare AgentOutput shape", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("run_completed", { result: output }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
  });

  it("returns completed for success_review with passed report", () => {
    const output = validGuidanceOutput("success_review");
    const events = [
      ev("run_completed", { result: { output } }),
    ];
    const outcome = classifyTutorOutcome(events, "passed");
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.guidance.mode).toBe("success_review");
    }
  });

  // ---- run_completed with invalid output ----

  it("returns infrastructure_error when output fails safety validation", () => {
    const output = {
      mode: "hint",
      hint: "```function f() {}```", // code fence violation
      concept: "test",
      next_steps: ["a", "b"],
    };
    const events = [
      ev("run_completed", { result: { output } }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
    if (outcome?.kind === "infrastructure_error") {
      expect(outcome.error).toContain("safety validation");
    }
  });

  it("returns infrastructure_error when run_completed has no output", () => {
    const events = [
      ev("run_completed", { result: {} }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
    if (outcome?.kind === "infrastructure_error") {
      expect(outcome.error).toContain("recognisable guidance");
    }
  });

  it("returns infrastructure_error when run_completed result is null", () => {
    const events = [
      ev("run_completed", { result: null }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  // ---- deterministic presentation mode ----

  it("derives success_review for a passed report despite model hint metadata", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("run_completed", { result: { output } }),
    ];
    const outcome = classifyTutorOutcome(events, "passed");
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.guidance.mode).toBe("success_review");
    }
  });

  // ---- run_failed ----

  it("returns cancelled for run_failed with abort error", () => {
    const events = [
      ev("run_failed", { error: "User cancelled the operation" }),
    ];
    expect(classifyTutorOutcome(events, reportStatus)).toEqual({
      kind: "cancelled",
    });
  });

  it("returns cancelled for run_failed with AbortError", () => {
    const events = [
      ev("run_failed", { error: "AbortError: operation was aborted" }),
    ];
    expect(classifyTutorOutcome(events, reportStatus)).toEqual({
      kind: "cancelled",
    });
  });

  it("returns infrastructure_error for run_failed with unknown error", () => {
    const events = [
      ev("run_failed", { error: "Model inference failed" }),
    ];
    expect(classifyTutorOutcome(events, reportStatus)).toEqual({
      kind: "infrastructure_error",
      error: "Model inference failed",
    });
  });

  it("returns infrastructure_error for run_failed with null error", () => {
    const events = [
      ev("run_failed", { error: null }),
    ];
    expect(classifyTutorOutcome(events, reportStatus)).toEqual({
      kind: "infrastructure_error",
      error: "Unknown workflow error",
    });
  });

  // ---- policy_denied ----

  it("returns infrastructure_error for policy_denied", () => {
    const events = [
      ev("policy_denied"),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
    if (outcome?.kind === "infrastructure_error") {
      expect(outcome.error).toContain("policy");
    }
  });

  // ---- non-terminal stream end ----

  it("returns infrastructure_error when stream ends without terminal event", () => {
    const events = [
      ev("stage_started"),
      ev("model_delta", { text: "thinking..." }),
    ];
    const outcome = classifyTutorOutcome(events, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });
});

// ---------------------------------------------------------------------------
// terminalTutorOutcomeFromEvent
// ---------------------------------------------------------------------------

describe("terminalTutorOutcomeFromEvent", () => {
  const reportStatus: RunReportStatus = "failed";

  // ---- run_completed ----

  it("returns completed for run_completed with valid output", () => {
    const output = validGuidanceOutput("hint");
    const event = ev("run_completed", { result: { output } });
    const outcome = terminalTutorOutcomeFromEvent(event, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
  });

  it("returns infrastructure_error for run_completed with invalid output", () => {
    const event = ev("run_completed", { result: {} });
    const outcome = terminalTutorOutcomeFromEvent(event, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  // ---- policy_denied ----

  it("returns infrastructure_error for policy_denied", () => {
    const event = ev("policy_denied");
    const outcome = terminalTutorOutcomeFromEvent(event, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  // ---- run_failed ----

  it("returns cancelled for run_failed with abort error", () => {
    const event = ev("run_failed", { error: "User cancelled" });
    expect(terminalTutorOutcomeFromEvent(event, reportStatus)).toEqual({
      kind: "cancelled",
    });
  });

  it("returns infrastructure_error for run_failed with unknown error", () => {
    const event = ev("run_failed", { error: "Model error" });
    expect(terminalTutorOutcomeFromEvent(event, reportStatus)).toEqual({
      kind: "infrastructure_error",
      error: "Model error",
    });
  });

  // ---- tool_call_failed ----

  it("returns infrastructure_error for tool_call_failed", () => {
    const event = ev("tool_call_failed", { error: "browser.js.run timed out" });
    const outcome = terminalTutorOutcomeFromEvent(event, reportStatus);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  it("returns cancelled for tool_call_failed with prior cancellation context", () => {
    const prior: WorkflowEvent[] = [
      ev("run_failed", { error: "AbortError: cancelled" }),
    ];
    const event = ev("tool_call_failed", { error: "timeout" });
    const outcome = terminalTutorOutcomeFromEvent(event, reportStatus, prior);
    expect(outcome).toEqual({ kind: "cancelled" });
  });

  // ---- non-terminal events ----

  it("returns null for non-terminal events", () => {
    expect(terminalTutorOutcomeFromEvent(ev("run_started"), reportStatus)).toBeNull();
    expect(terminalTutorOutcomeFromEvent(ev("stage_started"), reportStatus)).toBeNull();
    expect(terminalTutorOutcomeFromEvent(ev("model_delta"), reportStatus)).toBeNull();
    expect(terminalTutorOutcomeFromEvent(ev("model_completed"), reportStatus)).toBeNull();
    expect(terminalTutorOutcomeFromEvent(ev("stage_completed"), reportStatus)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// extractGuidancePreview
// ---------------------------------------------------------------------------

describe("extractGuidancePreview", () => {
  it("extracts only the partial hint string from ProduceGuidance deltas", () => {
    const events = [
      ev("model_delta", { stageId: "DiagnoseAttempt", text: "{\"kind\":\"final\"}" }),
      ev("model_delta", { stageId: "ProduceGuidance", text: "{\"kind\":\"final\",\"output\":{\"hint\":\"Check the " }),
      ev("model_delta", { stageId: "ProduceGuidance", text: "empty case" }),
    ];

    expect(extractGuidancePreview(events)).toBe("Check the empty case");
  });

  it("decodes escaped characters without rendering the tagged envelope", () => {
    const events = [
      ev("model_delta", {
        stageId: "ProduceGuidance",
        text: "{\"kind\":\"final\",\"output\":{\"hint\":\"Ask why\\nthis branch is skipped\"}}}",
      }),
    ];

    expect(extractGuidancePreview(events)).toBe("Ask why\nthis branch is skipped");
  });

  it("clears an invalid attempt after a model retry", () => {
    const events = [
      ev("model_delta", { stageId: "ProduceGuidance", text: "{\"hint\":\"```bad" }),
      ev("model_retry", { stageId: "ProduceGuidance" }),
      ev("model_delta", { stageId: "ProduceGuidance", text: "{\"hint\":\"Try a boundary case" }),
    ];

    expect(extractGuidancePreview(events)).toBe("Try a boundary case");
  });

  it("keeps a safe decoded prefix when a model emits an invalid escape", () => {
    const events = [
      ev("model_delta", {
        stageId: "ProduceGuidance",
        text: "{\"hint\":\"Check the boundary\\x",
      }),
    ];

    expect(extractGuidancePreview(events)).toBe("Check the boundary");
  });

  it("suppresses code-like previews and unrelated model deltas", () => {
    const unsafe = [
      ev("model_delta", { stageId: "ProduceGuidance", text: "{\"hint\":\"function solve(x) { return x; }" }),
    ];
    const unrelated = [
      ev("model_delta", { stageId: "DiagnoseAttempt", text: "{\"hint\":\"Internal diagnosis\"" }),
    ];

    expect(extractGuidancePreview(unsafe)).toBeNull();
    expect(extractGuidancePreview(unrelated)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// extractGuidanceFromEvents
// ---------------------------------------------------------------------------

describe("extractGuidanceFromEvents", () => {
  const reportStatus: RunReportStatus = "failed";

  it("extracts guidance from stage_completed with ProduceGuidance stage", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("stage_started", { stageId: "CaptureRequest" }),
      ev("stage_completed", { stageId: "CaptureRequest" }),
      ev("stage_started", { stageId: "ProduceGuidance" }),
      ev("stage_completed", { stageId: "ProduceGuidance", output }),
    ];
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeTruthy();
    expect(guidance!.mode).toBe("hint");
    expect(guidance!.concept).toBe("edge cases");
    expect(guidance!.next_steps.length).toBe(3);
  });

  it("extracts guidance from run_completed event", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("stage_started", { stageId: "ProduceGuidance" }),
      ev("run_completed", { result: { output } }),
    ];
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeTruthy();
    expect(guidance!.mode).toBe("hint");
  });

  it("extracts guidance from bare run_completed result", () => {
    const output = validGuidanceOutput("hint");
    const events = [
      ev("run_completed", { result: output }),
    ];
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeTruthy();
  });

  it("returns null when no guidance is found", () => {
    const events = [
      ev("stage_started", { stageId: "CaptureRequest" }),
      ev("stage_completed", { stageId: "CaptureRequest" }),
    ];
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeNull();
  });

  it("returns null for empty events", () => {
    expect(extractGuidanceFromEvents([], reportStatus)).toBeNull();
  });

  it("prefers the last guidance-producing event", () => {
    const first = validGuidanceOutput("hint");
    const second = {
      ...validGuidanceOutput("hint"),
      hint: "Second, more refined hint.",
    };
    const events = [
      ev("stage_completed", {
        stageId: "ProduceGuidance",
        output: first,
        sequence: 1,
      }),
      ev("stage_completed", {
        stageId: "ProduceGuidance",
        output: second,
        sequence: 2,
      }),
    ];
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeTruthy();
    expect(guidance!.hint).toBe("Second, more refined hint.");
  });

  it("skips invalid stage outputs", () => {
    const invalid = {
      mode: "hint",
      hint: "```code```",
      concept: "x",
      next_steps: ["a"],
    };
    const valid = validGuidanceOutput("hint");

    const events = [
      ev("stage_completed", {
        stageId: "ProduceGuidance",
        output: invalid,
        sequence: 1,
      }),
      ev("stage_completed", {
        stageId: "ProduceGuidance",
        output: valid,
        sequence: 2,
      }),
    ];

    // The first (invalid) output is skipped; the second is returned
    const guidance = extractGuidanceFromEvents(events, reportStatus);
    expect(guidance).toBeTruthy();
    expect(guidance!.hint).toBe("Think about edge cases.");
  });
});

// ---------------------------------------------------------------------------
// runTutorToOutcome
// ---------------------------------------------------------------------------

describe("runTutorToOutcome", () => {
  it("keeps the first terminal outcome when cleanup emits a later event", async () => {
    const runner = {
      agent: {},
      classifyOutcome: classifyTutorOutcome,
      async *run() {
        yield ev("run_failed", { error: "Cancelled by user" });
        yield ev("run_completed", { result: { output: validGuidanceOutput("hint") } });
      },
    } as unknown as TutorRunner;

    const result = await runTutorToOutcome(
      runner,
      { tutor_request: {} },
      "failed",
    );

    expect(result.outcome).toEqual({ kind: "cancelled" });
  });
});
