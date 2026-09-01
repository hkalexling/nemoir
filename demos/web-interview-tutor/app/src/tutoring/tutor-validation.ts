/**
 * NemoIR Interview Tutor -- output validation and learner-facing safety
 * guards.
 *
 * Phase 3 -- tutoring domain.
 *
 * The `InterviewTutor` workflow returns `AgentOutput`:
 * `{ mode?: string; hint: string; concept: string; next_steps: string[] }`.
 *
 * This module validates that output before it reaches the learner. The
 * deterministic test report, rather than the model, determines whether the
 * UI presents a hint or a success review. The rules are:
 * - `mode` is optional model metadata and is ignored for presentation;
 *   non-passing reports always become `"hint"`, passed reports always become
 *   `"success_review"`.
 * - `hint` and `concept` must be non-empty strings.
 * - Model-provided `next_steps` are validated when present; deterministic,
 *   learner-safe actions fill any missing entries so small local models still
 *   produce a useful 2-4-step result.
 * - No field may contain code fences (```) or text that looks like a
 *   complete function definition.
 *
 * These guards are runtime-only: the workflow's JSON schema constraints
 * enforce the structural shape, but the semantic safety checks (no
 * full-function code and deterministic presentation mode) live here.
 */

import type { RunReportStatus } from "../catalog/types.js";

// ---------------------------------------------------------------------------
// Validated guidance type
// ---------------------------------------------------------------------------

/**
 * Validated, learner-safe guidance produced by the tutor workflow.
 *
 * This is `AgentOutput` after all safety checks have passed.
 */
export interface ValidatedGuidance {
  /** Either `"hint"` or `"success_review"`. */
  readonly mode: "hint" | "success_review";

  /** Concise, learner-facing Socratic hint (never a full solution). */
  readonly hint: string;

  /** One short central concept. */
  readonly concept: string;

  /** 2-4 concrete learner actions. */
  readonly next_steps: readonly string[];
}

// ---------------------------------------------------------------------------
// Validation result
// ---------------------------------------------------------------------------

/**
 * Result of validating a workflow output.
 *
 * On success, `valid` is `true` and `guidance` is populated.
 * On failure, `valid` is `false` and `errors` lists every violation.
 */
export type TutorValidationResult =
  | { readonly valid: true; readonly guidance: ValidatedGuidance }
  | { readonly valid: false; readonly errors: readonly string[] };

// ---------------------------------------------------------------------------
// Safety patterns
// ---------------------------------------------------------------------------

/** Detects fenced code blocks (```) in any field. */
const RE_CODE_FENCE = /```/;

/**
 * Detects text that looks like a complete function definition.
 *
 * Matches when the text contains a `function` keyword followed by a
 * parameter list and body braces -- this is the typical shape of a
 * complete implementation rather than pseudocode or a code fragment.
 */
const RE_FULL_FUNCTION =
  /\bfunction\s+\w+\s*\([^)]*\)\s*\{[^}]*\}/;

/** Detects `async function` patterns. */
const RE_ASYNC_FUNCTION =
  /async\s+function\s+\w+\s*\([^)]*\)\s*\{[^}]*\}/;

/** Detects arrow functions with body braces (parametric). */
const RE_ARROW_BODY = /=>\s*\{/;

// Streaming previews can arrive before a closing brace. Be stricter than the
// final-output guard so a partial code solution never flashes on screen.
const RE_FUNCTION_PREFIX = /\b(?:async\s+)?function\s+\w*\s*\(/;
const RE_ANY_ARROW = /=>/;

const MAX_HINT_LENGTH = 2400;
const MAX_CONCEPT_LENGTH = 180;
const MAX_NEXT_STEP_LENGTH = 480;
const MIN_NEXT_STEPS = 2;
const MAX_NEXT_STEPS = 4;

const FALLBACK_FAILED_NEXT_STEPS = [
  "Use the deterministic failure report to check the assumption behind this hint.",
  "Update your solution, then run the public tests again.",
] as const;

const FALLBACK_PASSED_NEXT_STEPS = [
  "Estimate the time and space complexity of your approach.",
  "Try one boundary case before moving to a harder variation.",
] as const;

function appendTextSafetyErrors(
  value: string,
  label: string,
  maxLength: number,
  errors: string[],
): void {
  if (value.length > maxLength) {
    errors.push(`${label} must be no longer than ${maxLength} characters.`);
  }
  if (RE_CODE_FENCE.test(value)) {
    errors.push(`${label} contains one or more code fences (\`\`\`).`);
  }
  if (RE_FULL_FUNCTION.test(value) || RE_ASYNC_FUNCTION.test(value)) {
    errors.push(`${label} contains text that looks like a complete function.`);
  }
  if (RE_ARROW_BODY.test(value)) {
    errors.push(`${label} contains an arrow-function body (=> {).`);
  }
}

/**
 * Return whether a partial streamed hint is safe enough to display as a
 * non-authoritative preview. The final output still goes through full schema
 * and mode validation before becoming learner guidance.
 */
export function isSafeTutorPreview(value: string): boolean {
  if (value.trim().length === 0) return false;
  const errors: string[] = [];
  appendTextSafetyErrors(value, "preview", MAX_HINT_LENGTH, errors);
  if (RE_FUNCTION_PREFIX.test(value) || RE_ANY_ARROW.test(value)) return false;
  return errors.length === 0;
}

/**
 * Normalize the weakest structured field from small local models. A model may
 * return one useful next action—or omit it entirely—despite the prompt. Keep
 * every model-provided item subject to the usual safety checks, then add only
 * deterministic, generic actions until the learner gets a useful 2-4 item
 * list. A wrong type or unsafe supplied action remains a hard validation
 * error; we never silently display it.
 */
function normalizeNextSteps(
  value: unknown,
  reportStatus: RunReportStatus,
  errors: string[],
): readonly string[] | null {
  let supplied: readonly unknown[];
  if (value === undefined || value === null) {
    supplied = [];
  } else if (Array.isArray(value)) {
    supplied = value;
  } else {
    errors.push("next_steps must be an array when supplied.");
    return null;
  }

  const normalized: string[] = [];
  for (let index = 0; index < supplied.length; index++) {
    const step = supplied[index];
    if (typeof step !== "string" || step.trim().length === 0) {
      errors.push(`next_steps[${index}] must be a non-empty string.`);
      continue;
    }

    const cleanStep = step.trim();
    appendTextSafetyErrors(
      cleanStep,
      `next_steps[${index}]`,
      MAX_NEXT_STEP_LENGTH,
      errors,
    );
    if (!normalized.includes(cleanStep)) normalized.push(cleanStep);
  }

  if (errors.length > 0) return null;

  const fallback = reportStatus === "passed"
    ? FALLBACK_PASSED_NEXT_STEPS
    : FALLBACK_FAILED_NEXT_STEPS;
  for (const step of fallback) {
    if (normalized.length >= MIN_NEXT_STEPS) break;
    if (!normalized.includes(step)) normalized.push(step);
  }

  return normalized.slice(0, MAX_NEXT_STEPS);
}

// ---------------------------------------------------------------------------
// Core validation
// ---------------------------------------------------------------------------

/**
 * Validate a raw workflow output against learner-facing safety rules.
 *
 * The caller must supply `reportStatus` because deterministic test evidence,
 * not model-provided metadata, selects the learner-facing presentation mode.
 */
export function validateTutorOutput(
  output: unknown,
  reportStatus: RunReportStatus,
): TutorValidationResult {
  const errors: string[] = [];

  // ---- structural: must be a plain object ----
  if (output === null || typeof output !== "object" || Array.isArray(output)) {
    return { valid: false, errors: ["Tutor output must be a plain object."] };
  }

  const o = output as Record<string, unknown>;

  // ---- deterministic presentation mode ----
  // The model may include `mode` for compatibility, but it never determines
  // correctness or whether the learner receives a review. That is derived
  // exclusively from the deterministic report supplied by the test workflow.
  const mode: ValidatedGuidance["mode"] =
    reportStatus === "passed" ? "success_review" : "hint";

  // ---- hint ----
  const hint = o.hint;
  if (typeof hint !== "string" || hint.trim().length === 0) {
    errors.push("hint must be a non-empty string.");
  } else {
    appendTextSafetyErrors(hint, "hint", MAX_HINT_LENGTH, errors);
  }

  // ---- concept ----
  const concept = o.concept;
  if (typeof concept !== "string" || concept.trim().length === 0) {
    errors.push("concept must be a non-empty string.");
  } else {
    appendTextSafetyErrors(concept, "concept", MAX_CONCEPT_LENGTH, errors);
  }

  // ---- next_steps ----
  const nextSteps = normalizeNextSteps(o.next_steps, reportStatus, errors);

  // ---- reject additional unexpected keys (defence-in-depth) ----
  const knownKeys = new Set(["mode", "hint", "concept", "next_steps"]);
  for (const key of Object.keys(o)) {
    if (!knownKeys.has(key)) {
      errors.push(`Unexpected key "${key}" in tutor output.`);
    }
  }

  // ---- result ----
  if (errors.length > 0) {
    return { valid: false, errors };
  }

  return {
    valid: true,
    guidance: {
      mode,
      hint: hint as string,
      concept: concept as string,
      next_steps: nextSteps as readonly string[],
    },
  };
}

/**
 * Return semantic guidance errors in the shape expected by the runtime's
 * per-stage model-output retry hook. The same check remains at the display
 * boundary as defence in depth, but installing this inside ProduceGuidance
 * lets local models correct overlong or unsafe prose before the workflow
 * completes.
 */
export function tutorOutputValidationErrors(
  output: unknown,
  reportStatus: RunReportStatus,
): readonly string[] | null {
  const result = validateTutorOutput(output, reportStatus);
  return result.valid ? null : result.errors;
}

// ---------------------------------------------------------------------------
// Convenience: extract from AgentResult
// ---------------------------------------------------------------------------

/**
 * Extract and validate guidance from a workflow `AgentResult`-shaped
 * value.
 *
 * Many consumers receive `{ output: { mode, hint, concept, next_steps } }`
 * rather than the raw stage output.  This helper tries both shapes.
 */
export function validateAgentResult(
  result: unknown,
  reportStatus: RunReportStatus,
): TutorValidationResult {
  if (result === null || typeof result !== "object") {
    return { valid: false, errors: ["Agent result must be an object."] };
  }

  const r = result as Record<string, unknown>;

  // Direct `AgentOutput` shape (output at top level -- plain result). `mode`
  // is optional workflow metadata; the deterministic report derives it.
  if ("hint" in r && "concept" in r) {
    return validateTutorOutput(r, reportStatus);
  }

  // Nested `{ output: {...} }` shape (`AgentResult`)
  if ("output" in r && r.output !== null && typeof r.output === "object") {
    return validateTutorOutput(r.output, reportStatus);
  }

  return { valid: false, errors: ["Result does not contain a recognised output shape."] };
}
