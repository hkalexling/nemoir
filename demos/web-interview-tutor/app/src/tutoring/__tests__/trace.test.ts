import { describe, expect, it } from "vitest";
import type { WorkflowEvent } from "@nemoir/web-runtime";
import {
  tutorTraceFilename,
  tutorTraceJsonl,
} from "../trace";

function event(overrides: Partial<WorkflowEvent> = {}): WorkflowEvent {
  return {
    kind: "model_delta",
    runId: "run-1",
    sequence: 3,
    timestamp: "2026-01-02T03:04:05.000Z",
    stageId: "ProduceGuidance",
    channel: "assistant",
    text: "{\"kind\":\"final\"}",
    ...overrides,
  } as WorkflowEvent;
}

describe("tutor trace export helpers", () => {
  it("creates a predictable JSONL filename", () => {
    expect(
      tutorTraceFilename("two sum/unsafe", new Date("2026-01-02T03:04:05.678Z")),
    ).toBe("nemoir-interview-tutor-two-sum-unsafe-2026-01-02T03-04-05-678Z.jsonl");
  });

  it("serializes raw events losslessly as JSONL", () => {
    const events = [event(), event({ sequence: 4, text: "more" })];
    const lines = tutorTraceJsonl(events).trimEnd().split("\n");

    expect(lines).toHaveLength(2);
    expect(JSON.parse(lines[0] ?? "{}")).toEqual(events[0]);
    expect(JSON.parse(lines[1] ?? "{}")).toEqual(events[1]);
  });
});
