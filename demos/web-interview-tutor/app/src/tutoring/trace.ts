/**
 * Small adapter around the shared JSONL helpers for tutor-specific exports.
 *
 * The download is intentionally user-triggered by the feedback pane. The
 * trace is lossless and may include the learner source snapshot, public test
 * report, and raw local-model deltas, so the UI must disclose that before a
 * learner shares it.
 */

import { downloadJsonl, eventsToJsonl } from "@nemoir/web-ui";
import type { WorkflowEvent } from "@nemoir/web-runtime";

/** Build a stable, filesystem-friendly filename for a tutor trace. */
export function tutorTraceFilename(
  problemId: string,
  at: Date = new Date(),
): string {
  const safeProblemId = problemId.replace(/[^a-zA-Z0-9_-]/g, "-") || "problem";
  const timestamp = at.toISOString().replace(/[:.]/g, "-");
  return `nemoir-interview-tutor-${safeProblemId}-${timestamp}.jsonl`;
}

/** Serialize raw tutor events without dropping model/tool/stage details. */
export function tutorTraceJsonl(events: readonly WorkflowEvent[]): string {
  return eventsToJsonl(events);
}

/** Trigger the browser download and return its filename for UI/test callers. */
export function exportTutorTrace(
  events: readonly WorkflowEvent[],
  problemId: string,
  at: Date = new Date(),
): string {
  const filename = tutorTraceFilename(problemId, at);
  downloadJsonl(events, filename);
  return filename;
}
