/**
 * Phase 3 workflow contract checks.
 *
 * Generated artifacts are intentionally read-only. These tests assert the
 * important safety/architecture shape after `npm run compile:workflows` has
 * lowered the authored .nemo workflow.
 */

import { describe, expect, it } from "vitest";
import {
  HAS_JS_RUN,
  HAS_JS_SANDBOX,
  HAS_MODEL_STAGES,
  REQUIRED_CAPABILITIES,
} from "../../generated/interview-tutor/src/agent";
import manifest from "../../generated/interview-tutor/src/workflow.json";

type ManifestNode = (typeof manifest.nodes)[number];

function node(id: string): ManifestNode {
  const found = manifest.nodes.find((candidate) => candidate.id === id);
  if (!found) throw new Error(`Missing generated workflow stage: ${id}`);
  return found;
}

function requiredCapabilities(stage: ManifestNode): readonly string[] {
  return stage.requires.map((requirement) => requirement.capability);
}

function executionKind(stage: ManifestNode): string {
  if (!stage.execution) throw new Error(`Missing execution descriptor for ${stage.id}`);
  return stage.execution.kind;
}

describe("InterviewTutor generated workflow contract", () => {
  it("keeps a typed model workflow without a learner-code sandbox", () => {
    expect(HAS_MODEL_STAGES).toBe(true);
    expect(HAS_JS_RUN).toBe(true);
    expect(HAS_JS_SANDBOX).toBe(false);
    expect(REQUIRED_CAPABILITIES).toContain("browser.js.run");
    expect(REQUIRED_CAPABILITIES).toContain("browser.storage.read");
    expect(REQUIRED_CAPABILITIES).toContain("browser.storage.write");
    expect(REQUIRED_CAPABILITIES).toContain("user.elicit");
    expect(REQUIRED_CAPABILITIES).not.toContain("browser.js.sandbox");
    expect(manifest.inputs).toEqual([{ id: "tutor_request", type: "json" }]);
  });

  it("keeps storage/profile tools deterministic and user.elicit stage-scoped", () => {
    expect(executionKind(node("CaptureRequest"))).toBe("tool");
    expect(requiredCapabilities(node("CaptureRequest"))).toEqual(["browser.js.run"]);
    expect(requiredCapabilities(node("LoadProfile"))).toEqual(["browser.storage.read"]);
    expect(requiredCapabilities(node("NormalizeProfile"))).toEqual(["browser.js.run"]);
    expect(requiredCapabilities(node("SaveProfile"))).toEqual(["browser.storage.write"]);
    expect(requiredCapabilities(node("DiagnoseAttempt"))).toEqual([]);
    expect(requiredCapabilities(node("AskClarify"))).toEqual(["user.elicit"]);
    expect(requiredCapabilities(node("ProduceGuidance"))).toEqual([]);
  });

  it("routes optional clarification before deterministic profile persistence and typed guidance", () => {
    const capture = node("CaptureRequest");
    const diagnosis = node("DiagnoseAttempt");
    expect(capture.writes.map((write) => write.name)).toEqual([
      "problemContext",
      "learnerCode",
      "runReport",
      "hintLevel",
      "priorSummary",
      "problemMetadata",
      "profile_seed",
    ]);
    expect(diagnosis.writes.map((write) => write.name)).toEqual([
      "needs_clarify",
      "diagnosis",
    ]);
    // DiagnoseAttempt receives flat readable context fields, not a nested
    // JSON blob, mirroring the web-hints demo pattern.
    const diagnoseReads = diagnosis.reads.filter(
      (r) => r.origin === "dsl_stage_input",
    );
    expect(diagnoseReads.map((r) => r.ref)).toEqual([
      { kind: "node_output", node: "CaptureRequest", field: "problemContext" },
      { kind: "node_output", node: "CaptureRequest", field: "learnerCode" },
      { kind: "node_output", node: "CaptureRequest", field: "runReport" },
      { kind: "node_output", node: "CaptureRequest", field: "hintLevel" },
      { kind: "node_output", node: "CaptureRequest", field: "priorSummary" },
      { kind: "node_output", node: "LoadProfile", field: "found" },
      { kind: "node_output", node: "LoadProfile", field: "value" },
    ]);
    expect(node("NormalizeProfile").reads).toContainEqual({
      ref: { kind: "node_output", node: "CaptureRequest", field: "profile_seed" },
      optional: false,
      origin: "exec_arg",
    });
    expect(diagnosis.reads).toContainEqual({
      ref: { kind: "node_output", node: "LoadProfile", field: "value" },
      optional: true,
      origin: "dsl_stage_input",
    });
    expect(diagnosis.transitions.map((transition) => transition.to)).toEqual([
      "AskClarify",
      "NormalizeProfile",
    ]);
    expect(node("SaveProfile").transitions.map((transition) => transition.to)).toEqual([
      "ProduceGuidance",
    ]);
    expect(manifest.workflow.entry).toBe("CaptureRequest");
    expect(manifest.workflow.exits).toEqual(["ProduceGuidance"]);
    const guidanceWrites = node("ProduceGuidance").writes;
    expect(guidanceWrites.map((write) => write.name)).toEqual([
      "mode",
      "hint",
      "concept",
      "next_steps",
    ]);
    expect(guidanceWrites.find((write) => write.name === "mode")?.optional).toBe(true);
    expect(guidanceWrites.find((write) => write.name === "next_steps")?.optional).toBe(true);
  });
});
