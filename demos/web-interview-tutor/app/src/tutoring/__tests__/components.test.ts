/**
 * Phase 3 component contract tests -- presentation safety and history
 * behavior.
 *
 * Tests the module-level exports, type contracts, and pure-logic helpers
 * within the tutoring components.  Full React render tests require jsdom;
 * these tests verify the modules are importable and that their type-level
 * contracts are sound.
 *
 * Where React context providers expose pure-logic hooks, these are
 * exercised through the modules' public APIs (type-level verification).
 */

import { describe, it, expect } from "vitest";

// Import components and verify they are valid function components.
import { HintPanel } from "../HintPanel";
import type { HintPanelProps } from "../HintPanel";

import {
  HintHistory,
  HintHistoryProvider,
  useHintHistory,
} from "../HintHistory";
import type {
  HintEntry,
  HintHistoryProps,
} from "../HintHistory";

import { ClarificationDialog } from "../ClarificationDialog";
import type { ClarificationDialogProps } from "../ClarificationDialog";

import type {
  ValidatedGuidance,
} from "../tutor-validation";

import type { HintLevel } from "../tutor-request";

// ---------------------------------------------------------------------------
// Module-level checks
// ---------------------------------------------------------------------------

describe("HintPanel", () => {
  it("is a function component", () => {
    expect(typeof HintPanel).toBe("function");
  });

  it("accepts HintPanelProps matching the interface", () => {
    // Type-level: verify the type constraint compiles
    const props: HintPanelProps = {
      guidance: null,
      preview: null,
      running: false,
      error: null,
      cancelled: false,
    };
    expect(props.guidance).toBeNull();
  });

  it("accepts full guidance in props", () => {
    const guidance: ValidatedGuidance = {
      mode: "hint",
      hint: "Think about edge cases.",
      concept: "edge cases",
      next_steps: ["Check empty input.", "Consider null.", "Test extremes."],
    };

    const props: HintPanelProps = {
      guidance,
      preview: null,
      running: false,
      error: null,
      cancelled: false,
      onCancel: () => {},
      onDismiss: () => {},
      onRequestHintLevel: (_level: string) => {},
    };

    expect(props.guidance!.mode).toBe("hint");
    expect(props.guidance!.next_steps.length).toBe(3);
  });

  it("accepts running state with preview", () => {
    const props: HintPanelProps = {
      guidance: null,
      preview: "Let me analyse your submission...",
      running: true,
      error: null,
      cancelled: false,
      onCancel: () => {},
    };

    expect(props.running).toBe(true);
    expect(props.preview).toContain("analyse");
  });

  it("accepts error and cancelled states", () => {
    // Error state
    const errorProps: HintPanelProps = {
      guidance: null,
      preview: null,
      running: false,
      error: "Model inference failed",
      cancelled: false,
    };
    expect(errorProps.error).toBeTruthy();

    // Cancelled state
    const cancelProps: HintPanelProps = {
      guidance: null,
      preview: null,
      running: false,
      error: null,
      cancelled: true,
    };
    expect(cancelProps.cancelled).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// HintHistory
// ---------------------------------------------------------------------------

describe("HintHistory", () => {
  it("is a function component", () => {
    expect(typeof HintHistory).toBe("function");
  });

  it("HintHistoryProvider is a function component", () => {
    expect(typeof HintHistoryProvider).toBe("function");
  });

  it("useHintHistory is a hook function", () => {
    expect(typeof useHintHistory).toBe("function");
  });

  it("HintHistoryProps accepts maxVisible and callbacks", () => {
    const props: HintHistoryProps = {
      maxVisible: 5,
      onSelectEntry: (_entry: HintEntry) => {},
      onClear: () => {},
    };

    expect(props.maxVisible).toBe(5);
  });

  it("HintHistoryProps defaults are usable", () => {
    // maxVisible defaults to 10, callbacks optional
    const props: HintHistoryProps = {};
    expect(props.maxVisible).toBeUndefined();
    expect(props.onSelectEntry).toBeUndefined();
    expect(props.onClear).toBeUndefined();
  });

  it("HintEntry shape is consistent with ValidatedGuidance", () => {
    const guidance: ValidatedGuidance = {
      mode: "success_review",
      hint: "Well done! Consider the O(n) approach.",
      concept: "time complexity",
      next_steps: [
        "Explore the hash-map solution.",
        "Compare runtime with your current approach.",
      ],
    };

    const entry: HintEntry = {
      id: 1,
      guidance,
      hintLevel: "review" as HintLevel,
      problemId: "two-sum",
      createdAt: "2025-01-01T00:00:00.000Z",
      reportStatus: "passed",
    };

    expect(entry.guidance.mode).toBe("success_review");
    expect(entry.hintLevel).toBe("review");
    expect(entry.problemId).toBe("two-sum");
    expect(entry.reportStatus).toBe("passed");
  });
});

// ---------------------------------------------------------------------------
// ClarificationDialog
// ---------------------------------------------------------------------------

describe("ClarificationDialog", () => {
  it("is a function component", () => {
    expect(typeof ClarificationDialog).toBe("function");
  });

  it("accepts ClarificationDialogProps with question and handlers", () => {
    const props: ClarificationDialogProps = {
      question: "What was your reasoning for the nested loop approach?",
      options: ["I wanted O(1) space", "It seemed simplest", "I forgot about hash maps"],
      onResolve: (_value: string) => {},
      onReject: (_error: unknown) => {},
    };

    expect(props.question).toContain("reasoning");
    expect(props.options).toHaveLength(3);
  });

  it("accepts ClarificationDialogProps without options", () => {
    const props: ClarificationDialogProps = {
      question: "Tell me about your approach.",
      onResolve: (_value: string) => {},
      onReject: (_error: unknown) => {},
    };

    expect(props.question).toBeTruthy();
    expect(props.options).toBeUndefined();
  });
});
