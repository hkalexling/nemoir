import {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  ModelLoader,
  WebUiHostProvider,
  useWebUiHost,
  useWorkflowRun,
  type WorkflowRunner,
} from "@nemoir/web-ui";
import type { WorkflowEvent } from "@nemoir/web-runtime";
import {
  ALL_TOPICS,
  PROBLEMS,
  getProblemById,
} from "./catalog/problems.js";
import type { AttemptBundle, Problem, RunReport } from "./catalog/types.js";
import { SolutionEditor } from "./editor/SolutionEditor.js";
import {
  AttemptValidationError,
  assertBundleValid,
  buildAttemptBundle,
  createAttemptSnapshot,
  isSnapshotStale,
  type AttemptSnapshot,
} from "./execution/attempt.js";
import {
  SandboxReviewDialog,
  SandboxReviewProvider,
  type SandboxReview,
} from "./execution/SandboxReviewDialog.js";
import { TestResultsPanel } from "./execution/TestResultsPanel.js";
import {
  createTestRunner,
  type RunOutcome,
} from "./execution/run-test-workflow.js";
import { ModelStatus } from "./nemoir/ModelStatus.js";
import { useWebLlmSession } from "./nemoir/useWebLlmSession.js";
import {
  ClarificationDialog,
  HintHistory,
  HintHistoryProvider,
  HintPanel,
  classifyTutorOutcome,
  createTutorRequest,
  createTutorRunner,
  extractGuidancePreview,
  exportTutorTrace,
  tutorRequestToAgentInput,
  useHintHistory,
  type HintEntry,
  type HintLevel,
  type TutorRunOutcome,
} from "./tutoring/index.js";

const initialProblem = PROBLEMS[0];
if (!initialProblem) {
  throw new Error("The interview tutor requires at least one catalog problem.");
}

type DifficultyFilter = "all" | Problem["difficulty"];

interface PendingRun {
  readonly snapshot: AttemptSnapshot;
  readonly bundle: AttemptBundle;
}

interface HintRunContext {
  readonly problemId: string;
  readonly reportStatus: RunReport["status"];
  readonly hintLevel: HintLevel;
}

interface TutorShellProps {
  readonly setReview: (review: SandboxReview | null) => void;
}

async function* unavailableTutorWorkflow(
  message: string,
): AsyncIterable<WorkflowEvent> {
  throw new Error(message);
}

/** Build deliberately small context for a fresh run; never send full history. */
function compactPriorSummary(entries: readonly HintEntry[]): string {
  if (entries.length === 0) return "";

  return JSON.stringify({
    recentGuidance: entries
      .slice(0, 3)
      .reverse()
      .map((entry) => ({
        level: entry.hintLevel,
        concept: entry.guidance.concept,
        nextStep: entry.guidance.next_steps[0] ?? "",
      })),
  });
}

/**
 * Phase 3 learner shell.
 *
 * Test execution and tutoring remain separate workflow runs: tests use only
 * the deterministic sandbox agent; hints use a fresh WebLLM-backed agent over
 * the exact source/report snapshot that produced the visible result.
 */
function TutorShell({ setReview }: TutorShellProps) {
  const uiHost = useWebUiHost();
  const { entries: hintEntries, append: appendHint, clear: clearHints } = useHintHistory();

  const [selectedProblemId, setSelectedProblemId] = useState(initialProblem.id);
  const [drafts, setDrafts] = useState<Record<string, string>>(() =>
    Object.fromEntries(PROBLEMS.map((problem) => [problem.id, problem.starterCode])),
  );
  const [search, setSearch] = useState("");
  const [difficulty, setDifficulty] = useState<DifficultyFilter>("all");
  const [topic, setTopic] = useState("all");
  const [pendingRun, setPendingRun] = useState<PendingRun | null>(null);
  const [lastSnapshot, setLastSnapshot] = useState<AttemptSnapshot | null>(null);
  const [testOutcome, setTestOutcome] = useState<RunOutcome | null>(null);
  const [testPreflightError, setTestPreflightError] = useState<string | null>(null);

  const [modelSupportEnabled, setModelSupportEnabled] = useState(false);
  const [storageWarningDismissed, setStorageWarningDismissed] = useState(false);
  const [requestedHintLevel, setRequestedHintLevel] = useState<HintLevel>("targeted");
  // Default 0.7: low enough for coherent structured output, high enough to
  // break the near-deterministic retry loop that 0.2 caused (identical output
  // on every retry). Grammar-constrained decoding now guarantees valid JSON
  // regardless of temperature, so the old reason to keep it low is gone.
  const [temperature, setTemperature] = useState(0.7);
  const [hintRunContext, setHintRunContext] = useState<HintRunContext | null>(null);
  const [hintOutcome, setHintOutcome] = useState<TutorRunOutcome | null>(null);
  const [hintPreflightError, setHintPreflightError] = useState<string | null>(null);
  const [selectedHistoryEntry, setSelectedHistoryEntry] = useState<HintEntry | null>(null);
  const [resetAllStatus, setResetAllStatus] = useState<string | null>(null);

  const selectedProblem = getProblemById(selectedProblemId) ?? initialProblem;
  const source = drafts[selectedProblem.id] ?? selectedProblem.starterCode;
  const model = useWebLlmSession(modelSupportEnabled);

  const testRunner = useMemo(
    () =>
      createTestRunner({
        uiHost,
        sandboxTimeoutMs: selectedProblem.executionLimits.timeoutMs,
      }),
    [selectedProblem.executionLimits.timeoutMs, uiHost],
  );

  const testWorkflowRunner = useCallback(
    (inputs: Record<string, unknown>, signal: AbortSignal) =>
      testRunner.run(inputs.attempt_bundle as AttemptBundle, signal),
    [testRunner],
  );

  const testWorkflow = useWorkflowRun(testWorkflowRunner);

  // The factory is stable; each deterministic browser.js.run invocation
  // requests a fresh worker from it and the runtime terminates that worker on
  // completion, cancellation, or timeout.
  const tutorJsWorkerFactory = useCallback(
    () =>
      new Worker(
        new URL(
          "./generated/interview-tutor/src/js.worker.ts",
          import.meta.url,
        ),
        { type: "module" },
      ),
    [],
  );

  const tutorRunner = useMemo(() => {
    if (!model.session || !model.isModelLoaded) return null;

    return createTutorRunner({
      modelAdapter: model.session.adapter,
      uiHost,
      jsWorkerFactory: tutorJsWorkerFactory,
      temperature,
    });
  }, [model.isModelLoaded, model.session, tutorJsWorkerFactory, uiHost, temperature]);

  const tutorWorkflowRunner = useCallback<WorkflowRunner>(
    (inputs, signal) => {
      if (!tutorRunner) {
        return unavailableTutorWorkflow(
          "Load a local WebLLM model before requesting tutor guidance.",
        );
      }
      return tutorRunner.run(inputs, signal);
    },
    [tutorRunner],
  );

  const tutorWorkflow = useWorkflowRun(tutorWorkflowRunner);
  const testRunActive = pendingRun !== null || testWorkflow.running;
  const editorLocked = testRunActive || tutorWorkflow.running;

  const filteredProblems = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    return PROBLEMS.filter((problem) => {
      if (difficulty !== "all" && problem.difficulty !== difficulty) return false;
      if (topic !== "all" && !problem.topics.includes(topic)) return false;
      if (!normalizedSearch) return true;
      const haystack = [
        problem.id,
        problem.title,
        problem.statement,
        ...problem.topics,
      ]
        .join(" ")
        .toLocaleLowerCase();
      return haystack.includes(normalizedSearch);
    });
  }, [difficulty, search, topic]);

  const stale = Boolean(
    lastSnapshot &&
      (lastSnapshot.problemId !== selectedProblem.id ||
        lastSnapshot.evaluatorVersion !== selectedProblem.evaluatorVersion ||
        isSnapshotStale(lastSnapshot, source)),
  );

  const report = testOutcome?.kind === "completed" ? testOutcome.report : null;
  const testRunnerError = testPreflightError ?? (!testOutcome ? testWorkflow.error : null);
  const modelReady = Boolean(tutorRunner && model.session && model.isModelLoaded);
  const hintRequestEligible = Boolean(
    report &&
      lastSnapshot &&
      !stale &&
      !testRunActive &&
      !tutorWorkflow.running &&
      modelReady,
  );

  const displayedGuidance = selectedHistoryEntry?.guidance ?? (
    hintOutcome?.kind === "completed" ? hintOutcome.guidance : null
  );
  const hintPreview = useMemo(
    () => tutorWorkflow.running ? extractGuidancePreview(tutorWorkflow.events) : null,
    [tutorWorkflow.events, tutorWorkflow.running],
  );
  const hintError = hintPreflightError ?? (
    hintOutcome?.kind === "infrastructure_error"
      ? hintOutcome.error
      : (!hintOutcome ? tutorWorkflow.error : null)
  );
  const hintCancelled = hintOutcome?.kind === "cancelled";

  const hintActionHelp = useMemo(() => {
    if (!report) return "Run the current solution first. Hints use the resulting evidence.";
    if (stale) return "The editor changed after this report. Run tests again for an evidence-backed hint.";
    if (!modelSupportEnabled) return "Enable the optional local tutor, then explicitly load a WebLLM model.";
    if (model.status.kind === "unavailable") return model.status.message;
    if (model.status.kind === "error") return model.status.message;
    if (!modelReady) return "Select and load a local model before requesting guidance.";
    if (report.status === "passed") {
      return "All public tests passed. Request a review of complexity, robustness, or readability.";
    }
    return "The tutor receives this exact source and deterministic report; it does not rerun your code.";
  }, [model.status, modelReady, modelSupportEnabled, report, stale]);

  // Reset a dismissed storage warning whenever the learner chooses a model.
  useEffect(() => {
    setStorageWarningDismissed(false);
  }, [model.selectedModel]);

  // Starting in an effect ensures the sandbox review context has committed
  // before the workflow reaches its policy-gated user.confirm call.
  useEffect(() => {
    if (!pendingRun) return;
    testWorkflow.start({ attempt_bundle: pendingRun.bundle });
    setPendingRun(null);
  }, [pendingRun, testWorkflow.start]);

  // A finished deterministic stream supplies the authoritative test outcome.
  useEffect(() => {
    if (testWorkflow.running || testWorkflow.events.length === 0) return;
    const classified = testRunner.classifyOutcome(testWorkflow.events);
    if (classified) {
      setTestOutcome(classified);
      setReview(null);
    }
  }, [setReview, testRunner, testWorkflow.events, testWorkflow.running]);

  // A preflight/runtime failure can have no stream events. It should still
  // close the sandbox review context rather than leave stale dialog payload.
  useEffect(() => {
    if (!testWorkflow.running && testWorkflow.events.length === 0 && testWorkflow.error) {
      setReview(null);
    }
  }, [setReview, testWorkflow.error, testWorkflow.events.length, testWorkflow.running]);

  // Resolve the fresh tutor run from its event trace exactly once. Only the
  // validated terminal guidance becomes history or learner-facing content.
  useEffect(() => {
    if (
      tutorWorkflow.running ||
      tutorWorkflow.events.length === 0 ||
      !hintRunContext ||
      hintOutcome
    ) {
      return;
    }

    const classified = classifyTutorOutcome(
      tutorWorkflow.events,
      hintRunContext.reportStatus,
    );
    if (!classified) return;

    setHintOutcome(classified);
    if (classified.kind === "completed") {
      appendHint(
        classified.guidance,
        hintRunContext.hintLevel,
        hintRunContext.problemId,
        hintRunContext.reportStatus,
      );
    }
  }, [
    appendHint,
    hintOutcome,
    hintRunContext,
    tutorWorkflow.events,
    tutorWorkflow.running,
  ]);

  useEffect(() => {
    if (
      !tutorWorkflow.running &&
      tutorWorkflow.events.length === 0 &&
      tutorWorkflow.error &&
      hintRunContext &&
      !hintOutcome
    ) {
      setHintOutcome({
        kind: "infrastructure_error",
        error: tutorWorkflow.error,
      });
    }
  }, [
    hintOutcome,
    hintRunContext,
    tutorWorkflow.error,
    tutorWorkflow.events.length,
    tutorWorkflow.running,
  ]);

  const clearTutorPresentation = useCallback(() => {
    setHintRunContext(null);
    setHintOutcome(null);
    setHintPreflightError(null);
    setSelectedHistoryEntry(null);
  }, []);

  const clearTutorAttempt = useCallback(() => {
    clearTutorPresentation();
    clearHints();
  }, [clearHints, clearTutorPresentation]);

  const selectProblem = useCallback(
    (problemId: string) => {
      if (editorLocked) return;
      setSelectedProblemId(problemId);
      setTestPreflightError(null);
      clearTutorAttempt();
    },
    [clearTutorAttempt, editorLocked],
  );

  const updateSource = useCallback(
    (nextSource: string) => {
      if (editorLocked) return;
      setDrafts((current) => ({ ...current, [selectedProblem.id]: nextSource }));
      setTestPreflightError(null);
      if (nextSource !== source) clearTutorAttempt();
    },
    [clearTutorAttempt, editorLocked, selectedProblem.id, source],
  );

  const resetSource = useCallback(() => {
    if (editorLocked) return;
    setDrafts((current) => ({
      ...current,
      [selectedProblem.id]: selectedProblem.starterCode,
    }));
    setTestPreflightError(null);
    clearTutorAttempt();
  }, [clearTutorAttempt, editorLocked, selectedProblem.id, selectedProblem.starterCode]);

  const startTests = useCallback(() => {
    if (editorLocked) return;

    try {
      const snapshot = createAttemptSnapshot(selectedProblem, source);
      const bundle = buildAttemptBundle(snapshot);
      assertBundleValid(bundle);

      clearTutorAttempt();
      setLastSnapshot(snapshot);
      setTestOutcome(null);
      setTestPreflightError(null);
      setReview({
        submissionSource: snapshot.source,
        entryFunctionName: snapshot.entryFunctionName,
        tests: snapshot.tests,
        executionLimits: selectedProblem.executionLimits,
      });
      setPendingRun({ snapshot, bundle });
    } catch (error) {
      setTestOutcome(null);
      setReview(null);
      setTestPreflightError(
        error instanceof AttemptValidationError || error instanceof Error
          ? error.message
          : String(error),
      );
    }
  }, [clearTutorAttempt, editorLocked, selectedProblem, setReview, source]);

  const startTutor = useCallback((level: HintLevel = requestedHintLevel) => {
    if (testRunActive || tutorWorkflow.running) return;

    if (!report || !lastSnapshot || stale) {
      setHintPreflightError(
        stale
          ? "Run tests again after editing before requesting a hint."
          : "Run the current solution successfully through the deterministic test workflow first.",
      );
      return;
    }
    if (!tutorRunner) {
      setHintPreflightError("Load a local WebLLM model before requesting guidance.");
      return;
    }

    try {
      const request = createTutorRequest(
        selectedProblem,
        lastSnapshot,
        report,
        {
          hintLevel: level,
          priorSummary: compactPriorSummary(hintEntries),
        },
      );

      setHintRunContext({
        problemId: selectedProblem.id,
        reportStatus: report.status,
        hintLevel: request.hintLevel,
      });
      setHintOutcome(null);
      setHintPreflightError(null);
      setSelectedHistoryEntry(null);
      tutorWorkflow.start(tutorRequestToAgentInput(request));
    } catch (error) {
      setHintRunContext(null);
      setHintOutcome(null);
      setHintPreflightError(error instanceof Error ? error.message : String(error));
    }
  }, [
    hintEntries,
    lastSnapshot,
    report,
    requestedHintLevel,
    selectedProblem,
    stale,
    testRunActive,
    tutorRunner,
    tutorWorkflow,
  ]);

  const handleHistorySelect = useCallback((entry: HintEntry) => {
    if (tutorWorkflow.running) return;
    setSelectedHistoryEntry(entry);
    setHintOutcome(null);
    setHintPreflightError(null);
  }, [tutorWorkflow.running]);

  const enableLocalTutor = useCallback(() => {
    setModelSupportEnabled(true);
    setHintPreflightError(null);
  }, []);

  const loadSelectedModel = useCallback(() => {
    void model.loadModel();
  }, [model]);

  const loadDespiteStorageWarning = useCallback(() => {
    setStorageWarningDismissed(true);
    void model.loadModel();
  }, [model]);

  const handleResetAllCaches = useCallback(async () => {
    setResetAllStatus(null);
    try {
      const maybeReset = (model as unknown as { deleteAllCachedArtifacts?: () => Promise<{ deletedIds?: string[]; deleted?: string[]; failures?: Array<{ modelId: string; message: string }>; failed?: Array<{ id: string; error: string }> }> }).deleteAllCachedArtifacts;
      if (!maybeReset) {
        setResetAllStatus("Reset not available in this browser build.");
        return;
      }
      const result = await maybeReset.call(model);
      const deleted = (result as unknown as { deletedIds?: string[]; deleted?: string[] }).deletedIds ?? (result as unknown as { deleted?: string[] }).deleted ?? [];
      const failures = (result as unknown as { failures?: Array<{ modelId: string; message: string }>; failed?: Array<{ id: string; error: string }> }).failures ?? (result as unknown as { failed?: Array<{ id: string }> }).failed ?? [];
      if (deleted.length === 0 && failures.length === 0) {
        setResetAllStatus("No cached models to reset.");
      } else if (failures.length === 0) {
        setResetAllStatus(`Reset ${deleted.length} cached model(s).`);
      } else {
        const failedIds = failures.map((f: unknown) => (f as { modelId?: string; id?: string }).modelId ?? (f as { id?: string }).id ?? String(f)).join(", ");
        setResetAllStatus(`Reset ${deleted.length} model(s), ${failures.length} failed: ${failedIds}`);
      }
    } catch (error) {
      setResetAllStatus(error instanceof Error ? error.message : String(error));
    }
  }, [model]);

  const downloadTutorTrace = useCallback(() => {
    if (tutorWorkflow.events.length === 0) return;
    exportTutorTrace(tutorWorkflow.events, selectedProblem.id);
  }, [selectedProblem.id, tutorWorkflow.events]);

  return (
    <main className="tutor-shell">
      <header className="tutor-header">
        <div>
          <p className="eyebrow">NemoIR compiled workflow demo</p>
          <h1>JavaScript Interview Tutor</h1>
          <p className="tutor-lede">
            Practice one pure JavaScript function at a time. Correctness is
            determined by a local, deterministic sandbox workflow—not by a model.
          </p>
        </div>
        <p className="local-disclosure">
          Local-only: code runs only after your approval in an isolated browser
          sandbox. Hints use a separate, optional local WebLLM workflow and the
          exact test evidence shown here.
        </p>
      </header>

      <div className="tutor-layout">
        <aside className="problem-sidebar" aria-labelledby="problem-catalog-title">
          <div className="sidebar-heading">
            <p className="eyebrow">Practice set</p>
            <h2 id="problem-catalog-title">Problems</h2>
          </div>

          <label className="catalog-control">
            <span>Search</span>
            <input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search title or topic"
            />
          </label>

          <div className="catalog-filters" aria-label="Problem filters">
            <label className="catalog-control">
              <span>Difficulty</span>
              <select
                value={difficulty}
                onChange={(event) => setDifficulty(event.target.value as DifficultyFilter)}
              >
                <option value="all">All levels</option>
                <option value="beginner">Beginner</option>
                <option value="intermediate">Intermediate</option>
                <option value="advanced">Advanced</option>
              </select>
            </label>
            <label className="catalog-control">
              <span>Topic</span>
              <select value={topic} onChange={(event) => setTopic(event.target.value)}>
                <option value="all">All topics</option>
                {ALL_TOPICS.map((entry) => (
                  <option key={entry} value={entry}>
                    {entry}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <nav className="problem-list" aria-label="Interview problems">
            {filteredProblems.length > 0 ? (
              filteredProblems.map((problem) => (
                <button
                  type="button"
                  key={problem.id}
                  className={`problem-list-item ${problem.id === selectedProblem.id ? "is-selected" : ""}`}
                  onClick={() => selectProblem(problem.id)}
                  disabled={editorLocked}
                  aria-current={problem.id === selectedProblem.id ? "page" : undefined}
                >
                  <span>{problem.title}</span>
                  <small>
                    {problem.difficulty} · {problem.topics.join(", ")}
                  </small>
                </button>
              ))
            ) : (
              <p className="catalog-empty">No problems match those filters.</p>
            )}
          </nav>
        </aside>

        <section className="learning-pane" aria-labelledby="problem-title">
          <article className="problem-statement">
            <div className="problem-title-row">
              <div>
                <p className="eyebrow">
                  {selectedProblem.difficulty} · {selectedProblem.topics.join(" · ")}
                </p>
                <h2 id="problem-title">{selectedProblem.title}</h2>
              </div>
              <span className="public-tests-badge">
                {selectedProblem.visibleTests.length} public tests
              </span>
            </div>
            <p>{selectedProblem.statement}</p>

            <div className="problem-details-grid">
              <section>
                <h3>Constraints</h3>
                <ul>
                  {selectedProblem.constraints.map((constraint) => (
                    <li key={constraint}>{constraint}</li>
                  ))}
                </ul>
              </section>
              <section>
                <h3>Examples</h3>
                {selectedProblem.examples.map((example) => (
                  <div className="problem-example" key={`${example.input}-${example.output}`}>
                    <code>Input: {example.input}</code>
                    <code>Output: {example.output}</code>
                    {example.explanation && <p>{example.explanation}</p>}
                  </div>
                ))}
              </section>
            </div>
          </article>

          <SolutionEditor
            value={source}
            entryFunctionName={selectedProblem.entryFunctionName}
            disabled={editorLocked}
            onChange={updateSource}
            onReset={resetSource}
          />

          <div className="run-controls">
            <button
              type="button"
              className="button button-primary run-tests-button"
              onClick={startTests}
              disabled={editorLocked}
            >
              {testRunActive ? "Running tests…" : "Run Tests"}
            </button>
            <p>
              You will review the submission, public tests, and evaluator source
              before any sandbox execution begins. No model is loaded or invoked
              by this action.
            </p>
          </div>

          <TestResultsPanel
            report={report}
            outcome={testOutcome}
            stale={stale}
            running={testRunActive}
            error={testRunnerError}
            onRetry={startTests}
            onCancel={testWorkflow.cancel}
          />
        </section>

        <aside className="feedback-pane" aria-label="Local tutor feedback">
          <ModelStatus status={model.status} />

          <section className="model-controls card" aria-labelledby="model-controls-title">
            <div className="card-heading">
              <div>
                <p className="eyebrow">Optional setup</p>
                <h2 id="model-controls-title">Local model</h2>
              </div>
            </div>
            {!modelSupportEnabled ? (
              <>
                <p>
                  WebLLM is optional. Enable it only when you want local AI
                  guidance; deterministic test execution remains available.
                </p>
                <button
                  type="button"
                  className="button button-secondary"
                  onClick={enableLocalTutor}
                  disabled={tutorWorkflow.running}
                >
                  Enable local tutor
                </button>
              </>
            ) : model.session && (model.status.kind === "ready" || model.status.kind === "error") ? (
              <ModelLoader
                models={model.sortedModels}
                cachedIds={model.cachedIds}
                selectedModel={model.selectedModel}
                onSelectModel={(modelId) => {
                  setStorageWarningDismissed(false);
                  model.setSelectedModel(modelId);
                }}
                loading={model.loading}
                isLoaded={model.isModelLoaded}
                progress={model.progress}
                storageAssessment={model.storageAssessment}
                storageWarningDismissed={storageWarningDismissed}
                onDismissStorageWarning={loadDespiteStorageWarning}
                onLoad={loadSelectedModel}
                loadFailure={model.loadFailure}
                fitAssessments={model.fitAssessments}
                onRetryFreshWorker={() => {
                  void model.retryWithFreshWorker();
                }}
                onRetryCleanDownload={() => {
                  void model.retryCleanDownload();
                }}
                onDeleteCache={() => {
                  void model.deleteModelArtifacts();
                }}
                disabled={testRunActive || tutorWorkflow.running}
              />
            ) : (
              <p className="model-controls-message">
                {model.status.kind === "starting"
                  ? "Preparing model choices…"
                  : "Model controls become available when this browser supports the local tutor."}
              </p>
            )}
            {modelSupportEnabled && model.session && (
              <div className="reset-all-caches" style={{ marginTop: "0.75rem" }}>
                <button
                  type="button"
                  className="button button-secondary"
                  onClick={() => void handleResetAllCaches()}
                  disabled={testRunActive || tutorWorkflow.running || model.loading || !model.cachedIds || model.cachedIds.length === 0}
                  title="Delete all cached WebLLM model artifacts (weights, wasm, tokenizer) via the wasm-backed cache — frees browser storage"
                >
                  Reset all cached models
                </button>
                {model.cachedIds && model.cachedIds.length > 0 && (
                  <small className="cache-count" style={{ marginLeft: "0.5rem" }}>
                    {model.cachedIds.length} cached
                  </small>
                )}
                {resetAllStatus && (
                  <p className="model-controls-message" role="status" style={{ marginTop: "0.5rem" }}>
                    {resetAllStatus}
                  </p>
                )}
              </div>
            )}
          </section>

          <section className="hint-request-card card" aria-labelledby="hint-request-title">
            <div className="card-heading">
              <div>
                <p className="eyebrow">Evidence-backed feedback</p>
                <h2 id="hint-request-title">
                  {report?.status === "passed" ? "Review solution" : "Get a hint"}
                </h2>
              </div>
            </div>

            {report && report.status !== "passed" && (
              <label className="hint-level-control">
                <span>Guidance level</span>
                <select
                  value={requestedHintLevel}
                  onChange={(event) => setRequestedHintLevel(event.target.value as HintLevel)}
                  disabled={tutorWorkflow.running}
                >
                  <option value="nudge">Nudge</option>
                  <option value="targeted">Targeted</option>
                  <option value="plan">Plan</option>
                </select>
              </label>
            )}

            <label className="hint-level-control">
              <span>
                Temperature <code className="temperature-value">{temperature.toFixed(2)}</code>
              </span>
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={temperature}
                onChange={(event) => setTemperature(Number.parseFloat(event.target.value))}
                disabled={tutorWorkflow.running}
                aria-label="Model sampling temperature"
              />
            </label>

            <button
              type="button"
              className="button button-primary hint-request-button"
              onClick={() => startTutor(report?.status === "passed" ? "review" : requestedHintLevel)}
              disabled={!hintRequestEligible}
            >
              {tutorWorkflow.running
                ? "Generating guidance…"
                : report?.status === "passed"
                  ? "Review solution"
                  : "Get Hint"}
            </button>
            <p className="hint-action-help">{hintActionHelp}</p>
          </section>

          <HintPanel
            guidance={displayedGuidance}
            preview={hintPreview}
            running={tutorWorkflow.running}
            error={hintError}
            cancelled={hintCancelled}
            onCancel={tutorWorkflow.cancel}
            onDismiss={clearTutorPresentation}
            onRequestHintLevel={startTutor}
          />

          <HintHistory
            onSelectEntry={handleHistorySelect}
            onClear={() => setSelectedHistoryEntry(null)}
          />

          <section className="trace-export-card card" aria-labelledby="trace-export-title">
            <div className="card-heading">
              <div>
                <p className="eyebrow">Troubleshooting</p>
                <h2 id="trace-export-title">Tutor trace</h2>
              </div>
            </div>
            <p>
              Download the current tutor workflow events as JSONL for debugging.
              It includes your source snapshot, public run report, stage events,
              and raw local-model output—review it before sharing.
            </p>
            <button
              type="button"
              className="button button-secondary"
              onClick={downloadTutorTrace}
              disabled={tutorWorkflow.events.length === 0}
            >
              Download JSONL trace{tutorWorkflow.events.length > 0
                ? ` (${tutorWorkflow.events.length} events)`
                : ""}
            </button>
          </section>
        </aside>
      </div>
    </main>
  );
}

export function App() {
  const [review, setReview] = useState<SandboxReview | null>(null);

  return (
    <SandboxReviewProvider review={review}>
      <WebUiHostProvider
        renderConfirm={SandboxReviewDialog}
        renderElicit={ClarificationDialog}
      >
        <HintHistoryProvider>
          <TutorShell setReview={setReview} />
        </HintHistoryProvider>
      </WebUiHostProvider>
    </SandboxReviewProvider>
  );
}
