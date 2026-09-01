// ---------------------------------------------------------------------------
// Phase 2 run-outcome unit tests — outcome classification from synthetic
// WorkflowEvent streams.
//
// Tests ONLY pure exported functions from run-test-workflow.ts; never
// creates real agents, real sandboxes, or any async I/O.  All events are
// plain-object stubs that satisfy the WorkflowEvent structural contract.
// ---------------------------------------------------------------------------

import { describe, it, expect } from "vitest";
import type { WorkflowEvent } from "@nemoir/web-runtime";
import type { RunReport } from "../../catalog/types";
import {
  classifyOutcome,
  terminalOutcomeFromEvent,
} from "../run-test-workflow";

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

function makeValidRunReport(overrides: Partial<RunReport> = {}): RunReport {
  return {
    status: "passed",
    total: 3,
    passed: 3,
    elapsedMs: 42,
    evaluatorVersion: "1.0.0",
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// classifyOutcome
// ---------------------------------------------------------------------------

describe("classifyOutcome", () => {
  // ---- empty / non-terminal -----------------------------------------------

  it("returns null for empty event array", () => {
    expect(classifyOutcome([])).toBeNull();
  });

  it("returns infrastructure_error when no terminal event is present and no policy denial", () => {
    // Non-terminal events only — classifyOutcome falls through to the
    // final catch-all and returns infrastructure_error.
    const events = [
      ev("run_started"),
      ev("stage_started"),
      ev("model_delta", { text: "thinking..." }),
    ];
    expect(classifyOutcome(events)).toEqual({
      kind: "infrastructure_error",
      error: "Workflow stream ended without a terminal event.",
    });
  });

  // ---- run_completed -------------------------------------------------------

  it("returns completed when run_completed has a valid report in result", () => {
    const report = makeValidRunReport();
    const events = [
      ev("run_started"),
      ev("stage_started"),
      ev("run_completed", { result: { report } }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.report).toEqual(report);
    }
  });

  it("returns completed when report is nested under output.report", () => {
    const report = makeValidRunReport({ status: "failed", total: 2, passed: 1 });
    const events = [
      ev("run_completed", { result: { output: { report } } }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.report).toEqual(report);
    }
  });

  it("returns infrastructure_error when run_completed result has no report", () => {
    const events = [
      ev("run_completed", { result: { something: "else" } }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
    if (outcome?.kind === "infrastructure_error") {
      expect(outcome.error).toContain("did not produce a valid run report");
    }
  });

  it("returns infrastructure_error when run_completed result is null", () => {
    const events = [
      ev("run_completed", { result: null }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  it("returns infrastructure_error when run_completed report fails isRunReport", () => {
    // A report missing total / elapsedMs etc.
    const events = [
      ev("run_completed", {
        result: { report: { status: "passed", evaluatorVersion: "1.0" } },
      }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("infrastructure_error");
  });

  // ---- policy_denied -------------------------------------------------------

  it("returns policy_denied when any event has kind policy_denied", () => {
    const events = [
      ev("run_started"),
      ev("policy_checked", { text: "sandbox check" }),
      ev("policy_denied"),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "policy_denied" });
  });

  it("policy_denied takes precedence over run_failed", () => {
    const events = [
      ev("policy_denied"),
      ev("run_failed", { error: "some failure" }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "policy_denied" });
  });

  // ---- sandbox_timeout -----------------------------------------------------

  it("returns sandbox_timeout for tool_call_failed with 'timed out' error", () => {
    const events = [
      ev("tool_call_started"),
      ev("tool_call_failed", {
        error: "browser.js.sandbox timed out after 5000ms",
      }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "sandbox_timeout" });
  });

  it("returns sandbox_timeout for tool_call_failed with 'time out' error (two words)", () => {
    const events = [
      ev("tool_call_failed", {
        error: "execution time out: exceeded limit",
      }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "sandbox_timeout" });
  });

  it("returns sandbox_timeout for tool_call_failed with 'Timeout' error (mixed case)", () => {
    const events = [
      ev("tool_call_failed", {
        error: "Execution timed out",
      }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "sandbox_timeout" });
  });

  // ---- cancellation --------------------------------------------------------

  it("returns cancelled when run_failed has abort/cancel error", () => {
    const events = [
      ev("run_started"),
      ev("run_failed", { error: "User cancelled the run" }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "cancelled" });
  });

  it("returns cancelled when run_failed has AbortError", () => {
    const events = [
      ev("run_failed", { error: "AbortError: operation was aborted" }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "cancelled" });
  });

  it("returns cancelled when tool_call_failed timed out preceded by cancellation", () => {
    const events = [
      ev("run_failed", { error: "User cancelled the operation" }),
      ev("tool_call_failed", { error: "browser.js.sandbox timed out after 1000ms" }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({ kind: "cancelled" });
  });

  // ---- infrastructure_error ------------------------------------------------

  it("returns infrastructure_error for run_failed with unknown message", () => {
    const events = [
      ev("run_started"),
      ev("run_failed", { error: "Some unexpected runtime crash" }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({
      kind: "infrastructure_error",
      error: "Some unexpected runtime crash",
    });
  });

  it("returns infrastructure_error for run_failed without error text", () => {
    // error is null — classify falls back to "Unknown workflow error"
    const events = [
      ev("run_started"),
      ev("run_failed", { error: null }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({
      kind: "infrastructure_error",
      error: "Unknown workflow error",
    });
  });

  // ---- non-terminal stream end --------------------------------------------

  it("preserves an unexpected sandbox-abort message as infrastructure_error", () => {
    const events = [
      ev("tool_call_started"),
      ev("tool_call_failed", {
        error: "Connection aborted",
      }),
    ];
    const outcome = classifyOutcome(events);
    expect(outcome).toEqual({
      kind: "infrastructure_error",
      error: "Sandbox execution aborted: Connection aborted",
    });
  });
});

// ---------------------------------------------------------------------------
// terminalOutcomeFromEvent
// ---------------------------------------------------------------------------

describe("terminalOutcomeFromEvent", () => {
  // ---- run_completed -------------------------------------------------------

  it("returns completed for run_completed with valid report", () => {
    const report = makeValidRunReport();
    const event = ev("run_completed", { result: { report } });
    const outcome = terminalOutcomeFromEvent(event);
    expect(outcome).toBeTruthy();
    expect(outcome!.kind).toBe("completed");
    if (outcome?.kind === "completed") {
      expect(outcome.report).toEqual(report);
    }
  });

  it("returns infrastructure_error for run_completed without valid report", () => {
    const event = ev("run_completed", { result: {} });
    const outcome = terminalOutcomeFromEvent(event);
    expect(outcome).toEqual({
      kind: "infrastructure_error",
      error: "Workflow completed but did not produce a valid run report.",
    });
  });

  // ---- policy_denied -------------------------------------------------------

  it("returns policy_denied immediately", () => {
    const event = ev("policy_denied");
    expect(terminalOutcomeFromEvent(event)).toEqual({ kind: "policy_denied" });
  });

  // ---- sandbox_timeout -----------------------------------------------------

  it("returns sandbox_timeout for tool_call_failed with timed out (no prior events)", () => {
    const event = ev("tool_call_failed", {
      error: "browser.js.sandbox timed out after 5000ms",
    });
    const outcome = terminalOutcomeFromEvent(event);
    expect(outcome).toEqual({ kind: "sandbox_timeout" });
  });

  // ---- cancellation with prior events --------------------------------------

  it("returns cancelled for tool_call_failed timed out with prior cancel context", () => {
    const prior: WorkflowEvent[] = [
      ev("run_failed", { error: "AbortError: operation was aborted" }),
    ];
    const event = ev("tool_call_failed", {
      error: "browser.js.sandbox timed out after 1000ms",
    });
    const outcome = terminalOutcomeFromEvent(event, prior);
    expect(outcome).toEqual({ kind: "cancelled" });
  });

  // ---- run_failed variants ------------------------------------------------

  it("returns cancelled for run_failed with abort error", () => {
    const event = ev("run_failed", { error: "User cancelled the run" });
    expect(terminalOutcomeFromEvent(event)).toEqual({ kind: "cancelled" });
  });

  it("returns sandbox_timeout for run_failed with timeout error", () => {
    const event = ev("run_failed", { error: "execution timed out" });
    expect(terminalOutcomeFromEvent(event)).toEqual({
      kind: "sandbox_timeout",
    });
  });

  it("returns infrastructure_error for run_failed with unknown error", () => {
    const event = ev("run_failed", { error: "Unexpected failure" });
    expect(terminalOutcomeFromEvent(event)).toEqual({
      kind: "infrastructure_error",
      error: "Unexpected failure",
    });
  });

  it("returns infrastructure_error for run_failed with null error", () => {
    const event = ev("run_failed", { error: null });
    expect(terminalOutcomeFromEvent(event)).toEqual({
      kind: "infrastructure_error",
      error: "Unknown workflow error",
    });
  });

  // ---- non-terminal events ------------------------------------------------

  it("returns null for non-terminal events", () => {
    expect(terminalOutcomeFromEvent(ev("run_started"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("stage_started"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("model_delta"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("model_completed"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("tool_call_started"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("tool_call_completed"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("policy_checked"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("transition_selected"))).toBeNull();
    expect(terminalOutcomeFromEvent(ev("stage_completed"))).toBeNull();
  });

  // ---- tool_call_failed with aborted, no timeout ---------------------------

  it("returns infrastructure_error for an aborted sandbox without cancellation", () => {
    const event = ev("tool_call_failed", { error: "Connection aborted" });
    expect(terminalOutcomeFromEvent(event)).toEqual({
      kind: "infrastructure_error",
      error: "Sandbox execution aborted: Connection aborted",
    });
  });

  it("returns cancelled for an aborted sandbox with prior cancellation context", () => {
    const prior: WorkflowEvent[] = [
      ev("run_failed", { error: "Cancelled by user" }),
    ];
    const event = ev("tool_call_failed", { error: "Connection aborted" });
    expect(terminalOutcomeFromEvent(event, prior)).toEqual({ kind: "cancelled" });
  });
});
