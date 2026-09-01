/**
 * NemoIR Interview Tutor -- `HintHistory` component and context.
 *
 * Phase 3 -- tutoring domain.
 *
 * Maintains an in-memory list of `HintEntry` records.  Each entry
 * captures the guidance, snapshot metadata, and a monotonic timestamp.
 * The component renders the history as an accessible list.
 *
 * The module exports:
 * - `HintHistoryProvider` -- React context provider
 * - `useHintHistory` -- hook to read / append entries
 * - `HintHistory` -- display component
 */

import {
  createContext,
  useContext,
  useState,
  useCallback,
  type FC,
  type ReactNode,
} from "react";
import type { ValidatedGuidance } from "./tutor-validation.js";
import type { HintLevel } from "./tutor-request.js";

// ---------------------------------------------------------------------------
// Entry type
// ---------------------------------------------------------------------------

/** A single entry in the hint history. */
export interface HintEntry {
  /** Monotonic creation timestamp. */
  readonly id: number;

  /** The validated guidance that was shown to the learner. */
  readonly guidance: ValidatedGuidance;

  /** The hint level that was requested. */
  readonly hintLevel: HintLevel;

  /** The problem ID at the time of the request. */
  readonly problemId: string;

  /** ISO 8601 timestamp when the entry was created. */
  readonly createdAt: string;

  /** The run-report status that triggered this guidance. */
  readonly reportStatus: string;
}

// ---------------------------------------------------------------------------
// Context shape
// ---------------------------------------------------------------------------

export interface HintHistoryContextValue {
  /** All entries, newest first. */
  readonly entries: readonly HintEntry[];

  /**
   * Append a new entry to the history.
   *
   * The entry is stamped with a monotonic id and the current time.
   */
  readonly append: (
    guidance: ValidatedGuidance,
    hintLevel: HintLevel,
    problemId: string,
    reportStatus: string,
  ) => void;

  /** Clear all history entries. */
  readonly clear: () => void;
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

const HintHistoryContext = createContext<HintHistoryContextValue | null>(null);

/**
 * Retrieve the hint history context.
 *
 * Throws when used outside a `<HintHistoryProvider>`.
 */
export function useHintHistory(): HintHistoryContextValue {
  const ctx = useContext(HintHistoryContext);
  if (!ctx) {
    throw new Error(
      "useHintHistory() must be used within a <HintHistoryProvider>.",
    );
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

let nextId = 1;

export const HintHistoryProvider: FC<{ readonly children: ReactNode }> = ({
  children,
}) => {
  const [entries, setEntries] = useState<readonly HintEntry[]>([]);

  const append = useCallback(
    (
      guidance: ValidatedGuidance,
      hintLevel: HintLevel,
      problemId: string,
      reportStatus: string,
    ) => {
      const entry: HintEntry = Object.freeze({
        id: nextId++,
        guidance,
        hintLevel,
        problemId,
        createdAt: new Date().toISOString(),
        reportStatus,
      }) as HintEntry;

      setEntries((prev) => [entry, ...prev]);
    },
    [],
  );

  const clear = useCallback(() => setEntries([]), []);

  return (
    <HintHistoryContext.Provider value={{ entries, append, clear }}>
      {children}
    </HintHistoryContext.Provider>
  );
};

// ---------------------------------------------------------------------------
// Display component
// ---------------------------------------------------------------------------

export interface HintHistoryProps {
  /**
   * Maximum number of entries to display.  Defaults to 10.
   * Older entries beyond this limit are still available via the context
   * but are not rendered.
   */
  readonly maxVisible?: number;

  /** Fired when the learner clicks an entry to view it in detail. */
  readonly onSelectEntry?: (entry: HintEntry) => void;

  /** Fired when the learner clears the history. */
  readonly onClear?: () => void;
}

export const HintHistory: FC<HintHistoryProps> = ({
  maxVisible = 10,
  onSelectEntry,
  onClear,
}) => {
  const { entries, clear } = useHintHistory();
  const visible = entries.slice(0, maxVisible);

  const handleClear = () => {
    clear();
    onClear?.();
  };

  if (entries.length === 0) {
    return (
      <section
        className="hint-history"
        aria-labelledby="hint-history-title"
      >
        <h3 id="hint-history-title">Hint History</h3>
        <p className="hint-history-empty">No hints requested yet.</p>
      </section>
    );
  }

  return (
    <section
      className="hint-history"
      aria-labelledby="hint-history-title"
    >
      <div className="hint-history-header">
        <h3 id="hint-history-title">
          Hint History ({entries.length})
        </h3>
        {onClear && entries.length > 0 && (
          <button
            type="button"
            className="btn-clear-history"
            onClick={handleClear}
            aria-label="Clear all hint history"
          >
            Clear
          </button>
        )}
      </div>

      <ol className="hint-history-list" aria-label="Previous hints">
        {visible.map((entry) => (
          <li key={entry.id} className="hint-history-item">
            <button
              type="button"
              className="hint-history-entry"
              onClick={() => onSelectEntry?.(entry)}
              aria-label={`Hint from ${formatTimestamp(entry.createdAt)}`}
            >
              <span className="hint-entry-timestamp">
                {formatTimestamp(entry.createdAt)}
              </span>
              <span className="hint-entry-level">
                {entry.hintLevel === "review" ? "Review" : entry.hintLevel}
              </span>
              <span className="hint-entry-concept">
                {entry.guidance.concept}
              </span>
              <span className="hint-entry-report-status">
                {entry.reportStatus === "passed" ? "passed" : "needs work"}
              </span>
            </button>
          </li>
        ))}
      </ol>

      {entries.length > maxVisible && (
        <p className="hint-history-more">
          {entries.length - maxVisible} older entries not shown.
        </p>
      )}
    </section>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}
