/**
 * NemoIR Interview Tutor -- tutoring domain public API.
 *
 * Phase 3 -- tutoring domain.
 *
 * Exports:
 * - Request builders and types (`tutor-request.ts`)
 * - Output validation (`tutor-validation.ts`)
 * - Runner factory (`run-tutor-workflow.ts`)
 * - Components (`HintPanel.tsx`, `HintHistory.tsx`, `ClarificationDialog.tsx`)
 */

// Request builder
export {
  createTutorRequest,
  buildProblemContext,
  buildProblemMetadata,
  tutorRequestToAgentInput,
  isHintLevel,
  VALID_HINT_LEVELS,
  TutorRequestError,
} from "./tutor-request.js";
export type {
  HintLevel,
  ProblemMetadata,
  TutorRequest,
  CreateTutorRequestOptions,
} from "./tutor-request.js";

// Validation
export {
  validateTutorOutput,
  validateAgentResult,
  tutorOutputValidationErrors,
  isSafeTutorPreview,
} from "./tutor-validation.js";
export type {
  ValidatedGuidance,
  TutorValidationResult,
} from "./tutor-validation.js";

// Runner
export {
  createTutorRunner,
  classifyTutorOutcome,
  terminalTutorOutcomeFromEvent,
  extractGuidanceFromEvents,
  extractGuidancePreview,
  runTutorToOutcome,
} from "./run-tutor-workflow.js";
export type {
  TutorRunOutcome,
  TutorRunnerOptions,
  TutorRunner,
} from "./run-tutor-workflow.js";

// Components
export { HintPanel } from "./HintPanel.js";
export type { HintPanelProps } from "./HintPanel.js";

export {
  HintHistory,
  HintHistoryProvider,
  useHintHistory,
} from "./HintHistory.js";
export type {
  HintEntry,
  HintHistoryContextValue,
  HintHistoryProps,
} from "./HintHistory.js";

export { ClarificationDialog } from "./ClarificationDialog.js";
export type { ClarificationDialogProps } from "./ClarificationDialog.js";

// Trace export
export {
  exportTutorTrace,
  tutorTraceFilename,
  tutorTraceJsonl,
} from "./trace.js";
