import type { WebLlmSessionStatus } from "./useWebLlmSession.js";

interface ModelStatusProps {
  readonly status: WebLlmSessionStatus;
}

const statusLabels: Record<WebLlmSessionStatus["kind"], string> = {
  idle: "Optional",
  unavailable: "Unavailable",
  starting: "Starting",
  ready: "Ready",
  error: "Error",
};

/** Compact, learner-facing status for the optional local WebLLM boundary. */
export function ModelStatus({ status }: ModelStatusProps) {
  return (
    <section className="card model-status" aria-labelledby="model-status-title">
      <div className="card-heading">
        <div>
          <p className="eyebrow">Local model boundary</p>
          <h2 id="model-status-title">WebLLM tutor</h2>
        </div>
        <span className={`status-badge status-${status.kind}`}>
          {statusLabels[status.kind]}
        </span>
      </div>
      <p>{status.message}</p>
      {status.kind !== "idle" && (
        <dl className="environment-grid">
          <div>
            <dt>WebGPU</dt>
            <dd>{status.webGpuAvailable ? "Available" : "Unavailable"}</dd>
          </div>
          <div>
            <dt>Cross-origin isolation</dt>
            <dd>{status.crossOriginIsolated ? "Active" : "Inactive"}</dd>
          </div>
          {status.kind === "ready" && (
            <>
              <div>
                <dt>Available model profiles</dt>
                <dd>{status.modelCount}</dd>
              </div>
              <div>
                <dt>Selected model</dt>
                <dd>{status.modelLoaded ? "Loaded" : status.loading ? "Loading" : "Not loaded"}</dd>
              </div>
            </>
          )}
        </dl>
      )}
    </section>
  );
}
