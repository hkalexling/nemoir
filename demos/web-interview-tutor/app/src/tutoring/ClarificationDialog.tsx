/**
 * NemoIR Interview Tutor -- `ClarificationDialog` component.
 *
 * Phase 3 -- tutoring domain.
 *
 * A custom elicit renderer designed to slot into
 * `WebUiHostProvider.renderElicit`.  It satisfies the renderer contract
 * (`FC<{ question, options?, onResolve, onReject }>`) and provides a
 * focused, accessible prompt for the `AskClarify` stage of the tutoring
 * workflow.
 *
 * The dialog presents the model's question, an optional set of
 * constrained choices, and a free-text input with Cancel.
 *
 * Accessibility: the dialog is a `role="dialog"` with `aria-modal`,
 * clear headings, and labelled controls.
 */

import { useState, type FC } from "react";

// ---------------------------------------------------------------------------
// Props -- satisfies the elicit renderer contract from WebUiHostProvider
// ---------------------------------------------------------------------------

export interface ClarificationDialogProps {
  /** The question from the `AskClarify` model stage. */
  readonly question: string;

  /** Optional constrained choices. */
  readonly options?: string[];

  /** Resolve with the learner's answer. */
  readonly onResolve: (value: string) => void;

  /** Reject (cancel) the elicitation. */
  readonly onReject: (error: unknown) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ClarificationDialog: FC<ClarificationDialogProps> = ({
  question,
  options,
  onResolve,
  onReject,
}) => {
  const [text, setText] = useState("");

  const handleSubmit = (value?: string) => {
    const answer = (value ?? text).trim();
    if (answer.length > 0) {
      onResolve(answer);
    }
  };

  const handleCancel = () => {
    onReject(new DOMException("Cancelled by user", "AbortError"));
  };

  return (
    <div
      className="nemoir-modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="clarification-dialog-title"
    >
      <div className="nemoir-modal clarification-dialog">
        <h2 id="clarification-dialog-title">Quick Question</h2>

        <p className="clarification-question">{question}</p>

        {options && options.length > 0 ? (
          <>
            <p className="clarification-options-label">Choose one:</p>
            <div className="nemoir-modal-actions clarification-options">
              {options.map((opt) => (
                <button
                  key={opt}
                  type="button"
                  className="nemoir-primary"
                  onClick={() => handleSubmit(opt)}
                  autoFocus={options.indexOf(opt) === 0}
                >
                  {opt}
                </button>
              ))}
            </div>
            {/* Always provide a free-text fallback even when options exist */}
            <div className="clarification-free-text">
              <label htmlFor="clarification-free-input">
                Or type your own answer:
              </label>
              <input
                id="clarification-free-input"
                type="text"
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSubmit();
                }}
                placeholder="Describe your reasoning…"
                aria-label="Your answer"
              />
              <button
                type="button"
                className="nemoir-primary"
                onClick={() => handleSubmit()}
                disabled={text.trim().length === 0}
              >
                Send
              </button>
              <button type="button" onClick={handleCancel}>
                Cancel
              </button>
            </div>
          </>
        ) : (
          <>
            <label htmlFor="clarification-input">
              Your answer:
            </label>
            <input
              id="clarification-input"
              type="text"
              autoFocus
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSubmit();
              }}
              placeholder="Describe your reasoning…"
              aria-label="Your answer"
            />
            <div className="nemoir-modal-actions">
              <button
                type="button"
                className="nemoir-primary"
                onClick={() => handleSubmit()}
                disabled={text.trim().length === 0}
              >
                Send
              </button>
              <button type="button" onClick={handleCancel}>
                Cancel
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
};
