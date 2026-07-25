//! Static template emitters for the generated web app files that do not
//! depend on IR variant translation.
//!
//! Mirrors `nemoir-backend-python::emit` (which emits `pyproject.toml`).
//! Each function renders a complete static file parameterized by the
//! package directory name and version.

use crate::escape::ts_template_body;

/// Render `package.json` for a generated web app package.
///
/// - `package_dir`: kebab-case npm package name (e.g. `judge-candidate`).
/// - `version`: package version string.
/// - `runtime_dep`: dependency spec for `@nemoir/web-runtime`
///   (e.g. `"^0.1.0"` or `"file:../../web/nemoir-runtime"`).
pub fn emit_package_json(package_dir: &str, version: &str, runtime_dep: &str) -> String {
    format!(
        r#"{{
  "name": "{pkg}",
  "private": true,
  "version": "{ver}",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit"
  }},
  "dependencies": {{
    "@nemoir/web-runtime": "{runtime}",
    "@mlc-ai/web-llm": "^0.2.84",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }},
  "devDependencies": {{
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0"
  }}
}}
"#,
        pkg = package_dir,
        ver = version,
        runtime = runtime_dep,
    )
}

/// Render `tsconfig.json` — Vite + React strict config.
pub fn emit_tsconfig_json() -> String {
    r#"{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable", "WebWorker"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": false,
    "noFallthroughCasesInSwitch": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "esModuleInterop": true
  },
  "include": ["src"]
}
"#
    .to_string()
}

/// Render `tsconfig.node.json` for Vite config type-checking.
pub fn emit_tsconfig_node_json() -> String {
    r#"{
  "compilerOptions": {
    "target": "ES2023",
    "lib": ["ES2023"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": false,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["vite.config.ts"]
}
"#
    .to_string()
}

/// Render `vite.config.ts` with React plugin and COOP/COEP headers on the
/// dev server (required for WebLLM's SharedArrayBuffer).
pub fn emit_vite_config() -> String {
    r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Cross-origin isolation headers required for SharedArrayBuffer (WebLLM).
const crossOriginIsolationHeaders = {
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Resource-Policy": "cross-origin",
};

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  worker: {
    format: "es",
  },
  server: {
    // Allow dev access through tunnels (e.g. trycloudflare) and LAN hosts.
    // The COOP/COEP headers below still apply; this only lifts Vite's
    // DNS-rebinding host allowlist during development. Production builds
    // are served statically and are unaffected.
    allowedHosts: true,
    headers: crossOriginIsolationHeaders,
  },
  preview: {
    headers: crossOriginIsolationHeaders,
  },
});
"#
    .to_string()
}

/// Render `index.html` — Vite entry point with a React mount root.
pub fn emit_index_html(package_dir: &str, workflow_id: &str) -> String {
    let title = format!("{workflow_id} — NemoIR Web App");
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{title}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#,
        title = html_escape(&title),
    )
    .replace("__PKG__", package_dir)
}

/// Render `netlify.toml` at the app root for static deployment with
/// cross-origin isolation headers.
pub fn emit_netlify_toml() -> String {
    r#"[build]
  command = "npm run build"
  publish = "dist"

[[headers]]
  for = "/*"
  [headers.values]
    Cross-Origin-Embedder-Policy = "require-corp"
    Cross-Origin-Opener-Policy = "same-origin"
    Cross-Origin-Resource-Policy = "cross-origin"
"#
    .to_string()
}

/// Render `vercel.json` for static deployment with cross-origin isolation
/// headers (alternative to Netlify).
pub fn emit_vercel_json() -> String {
    r#"{
  "$schema": "https://openapi.vercel.sh/vercel.json",
  "buildCommand": "npm run build",
  "outputDirectory": "dist",
  "headers": [
    {
      "source": "/(.*)",
      "headers": [
        { "key": "Cross-Origin-Embedder-Policy", "value": "require-corp" },
        { "key": "Cross-Origin-Opener-Policy", "value": "same-origin" },
        { "key": "Cross-Origin-Resource-Policy", "value": "cross-origin" }
      ]
    }
  ]
}
"#
    .to_string()
}

/// Render `public/_headers` (Netlify-style, also copied into `dist/` by
/// Vite) so the static build also carries the isolation headers.
pub fn emit_public_headers() -> String {
    "/*\n  Cross-Origin-Embedder-Policy: require-corp\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Resource-Policy: cross-origin\n".to_string()
}

/// Render the default `.gitignore` for a generated web app.
pub fn emit_gitignore() -> String {
    r#"node_modules
dist
dist-ssr
*.local
.vite
"#
    .to_string()
}

/// Render a minimal `README.md` documenting the generated app.
pub fn emit_readme(workflow_id: &str, _package_dir: &str, workflow_entry: &str) -> String {
    let body = format!(
        "# {workflow_id}\n\n\
A NemoIR-compiled browser application generated by `nemo compile --target web`.\n\n\
## Development\n\n\
```bash\n\
npm install\n\
npm run dev\n\
```\n\n\
The dev server serves cross-origin isolation headers (COOP/COEP) required\n\
by WebLLM's `SharedArrayBuffer`.\n\n\
## Requirements\n\n\
- **WebGPU:** Chrome/Edge 113+, or Opera 99+. The runner UI detects this and\n\
  shows a clear message if unavailable.\n\
- **Cross-origin isolation (COOP/COEP):** required for `SharedArrayBuffer`.\n\
  The dev/preview servers already set these headers (see `vite.config.ts`).\n\
  Hosts: Netlify, Vercel, Cloudflare Pages (config files are included).\n\
  GitHub Pages **cannot** set these headers — use a `coi-serviceworker` shim.\n\
- **First-visit model download:** small models are ~1–5 GB. WebLLM caches\n\
  them (OPFS preferred, IndexedDB fallback) so subsequent loads are faster.\n\n\
## Building against a local runtime checkout\n\n\
`@nemoir/web-runtime` is not yet published to npm. During development, point\n\
the compile step at a local checkout so `npm install` resolves:\n\n\
```bash\n\
nemo compile <file>.nemo --target web \\
\
  --web-runtime-dependency file:../../web/nemoir-runtime -o out/\n\
```\n\n\
## Build\n\n\
```bash\n\
npm run build\n\
```\n\n\
Serve `dist/` on any static host that sets the COOP/COEP headers.\n\n\
## Workflow\n\n\
- **Workflow ID:** `{workflow_id}`\n\
- **Entry stage:** `{entry}`\n\
- **Compiled IR:** `src/workflow.json`\n\n\
The compiled workflow is embedded as inspectable JSON. The generic runner UI\n\
in `src/main.tsx` loads it, detects WebGPU, lets you pick and load a WebLLM\n\
model, runs the workflow locally (no data leaves the browser), and streams\n\
stage/event output with a JSONL trace export.\n",
        entry = workflow_entry,
    );
    // Use template body escaping defensively even though workflow ids are
    // constrained — this README is markdown, not a template literal, so we
    // just return the body directly.
    let _ = ts_template_body(&body); // no-op; kept to document the escape path
    body
}

/// Minimal HTML entity escaping for text embedded in HTML.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Render the generic React runner UI (`src/main.tsx`).
///
/// The full Phase-3 runner:
/// - schema-derived input form (one control per IR input)
/// - WebGPU + cross-origin-isolation detection
/// - WebLLM model selector (prebuilt list, sorted by VRAM, load progress)
/// - run/cancel via AbortController
/// - React-based WebUiHost (elicit/confirm modals)
/// - streamed event timeline; reasoning channel opt-in
/// - typed result panel; user-initiated JSONL trace export
/// - local-only data disclosure indicator
///
/// The UI reads `workflow.json` at runtime, so it remains generic across
/// workflows. The runtime package stays framework-neutral; this file is the
/// only React consumer in the generated app. WebLLM is lazy-imported by the
/// runtime so it is code-split into a separate chunk (not on the first-paint
/// critical path).
pub fn emit_main_tsx() -> String {
    r##"import { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { Agent, WORKFLOW_ID, type AgentInput } from "./agent";
import workflowManifest from "./workflow.json";
import {
  createWebllmSession,
  isCrossOriginIsolated,
  isWebGPUAvailable,
  type ModelAdapter,
  type WebLlmModelInfo,
  type WebLlmProgressReport,
  type WebLlmSession,
  type WebUiHost,
  type WorkflowEvent,
} from "@nemoir/web-runtime";
import "./app.css";

type InputSpec = { id: string; type: string };

interface PendingElicit {
  question: string;
  options?: string[];
  resolve: (v: string) => void;
  reject: (e: unknown) => void;
}
interface PendingConfirm {
  message: string;
  resolve: (v: boolean) => void;
  reject: (e: unknown) => void;
}

function defaultFor(type: string): unknown {
  switch (type) {
    case "string":
      return "";
    case "number":
      return 0;
    case "bool":
      return false;
    case "string[]":
      return "";
    default:
      return "";
  }
}

function parseInputValue(type: string, raw: unknown): unknown {
  switch (type) {
    case "number": {
      const n = Number(raw);
      return Number.isFinite(n) ? n : 0;
    }
    case "bool":
      return Boolean(raw);
    case "string[]":
      return typeof raw === "string"
        ? raw
            .split("\n")
            .map((s) => s.trim())
            .filter((s) => s.length > 0)
        : [];
    default:
      return raw;
  }
}

function App() {
  const inputs = workflowManifest.inputs as InputSpec[];
  const [values, setValues] = useState<Record<string, unknown>>(() => {
    const init: Record<string, unknown> = {};
    for (const inp of inputs) init[inp.id] = defaultFor(inp.type);
    return init;
  });

  const webgpu = useMemo(() => isWebGPUAvailable(), []);
  const coi = useMemo(() => isCrossOriginIsolated(), []);

  const [session, setSession] = useState<WebLlmSession | null>(null);
  const [sessionError, setSessionError] = useState<string | null>(null);
  const [progress, setProgress] = useState<WebLlmProgressReport | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [cachedIds, setCachedIds] = useState<readonly string[] | null>(null);

  const refreshCached = (s: WebLlmSession | null) => {
    if (!s) return;
    s.cachedModelIds().then(setCachedIds).catch(() => {});
  };

  const [running, setRunning] = useState(false);
  const [events, setEvents] = useState<WorkflowEvent[]>([]);
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const [pendingElicit, setPendingElicit] = useState<PendingElicit | null>(null);
  const [pendingConfirm, setPendingConfirm] = useState<PendingConfirm | null>(null);

  useEffect(() => {
    if (!webgpu) return;
    let cancelled = false;
    createWebllmSession({
      workerFactory: () =>
        new Worker(new URL("./webllm.worker.ts", import.meta.url), { type: "module" }),
      onProgress: (r) => !cancelled && setProgress(r),
    })
      .then((s) => {
        if (cancelled) {
          void s.dispose();
          return;
        }
        setSession(s);
        refreshCached(s);
        const smallest = [...s.models].sort(
          (a, b) => (a.vramRequiredMb ?? 0) - (b.vramRequiredMb ?? 0),
        )[0];
        if (smallest) setSelectedModel(smallest.modelId);
      })
      .catch((e) => !cancelled && setSessionError(e instanceof Error ? e.message : String(e)));
    return () => {
      cancelled = true;
    };
  }, [webgpu]);

  const uiHost: WebUiHost = useMemo(
    () => ({
      elicit(question, options, signal) {
        return new Promise<string>((resolve, reject) => {
          setPendingElicit({ question, options, resolve, reject });
          signal?.addEventListener("abort", () =>
            reject(new DOMException("Aborted", "AbortError")),
          );
        });
      },
      confirm(message, signal) {
        return new Promise<boolean>((resolve, reject) => {
          setPendingConfirm({ message, resolve, reject });
          signal?.addEventListener("abort", () =>
            reject(new DOMException("Aborted", "AbortError")),
          );
        });
      },
    }),
    [],
  );

  const handleLoadModel = async () => {
    if (!session || !selectedModel) return;
    setLoading(true);
    setError(null);
    try {
      await session.ensureLoaded(selectedModel);
      setProgress(null);
      refreshCached(session);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleRun = async () => {
    setError(null);
    setEvents([]);
    setResult(null);
    setRunning(true);
    const ac = new AbortController();
    abortRef.current = ac;
    try {
      if (!session) throw new Error("WebLLM session is not ready.");
      await session.ensureLoaded(selectedModel, ac.signal);
      const adapter: ModelAdapter = session.adapter;
      const agent = new Agent({ modelAdapter: adapter, uiHost });
      const parsed: Record<string, unknown> = {};
      for (const inp of inputs) {
        parsed[inp.id] = parseInputValue(inp.type, values[inp.id]);
      }
      for await (const event of agent.stream(parsed as unknown as AgentInput, { signal: ac.signal })) {
        setEvents((prev) => [...prev, event]);
        if (event.kind === "run_completed") {
          setResult((event.result as { output?: Record<string, unknown> })?.output ?? null);
        }
        if (event.kind === "run_failed") {
          setError(event.error ?? "run failed");
        }
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // AbortError from cancellation is expected; don't show as an error.
      if (!(e instanceof DOMException && e.name === "AbortError")) {
        setError(msg);
      }
    } finally {
      setRunning(false);
      abortRef.current = null;
    }
  };

  const handleCancel = () => {
    abortRef.current?.abort();
  };

  const exportTrace = () => {
    const jsonl = events.map((e) => JSON.stringify(e)).join("\n");
    const blob = new Blob([jsonl], { type: "application/x-ndjson" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${WORKFLOW_ID}-trace.jsonl`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const modelLoaded = session?.isModelLoaded(selectedModel) ?? false;

  // Coalesce consecutive model_delta events (same stage + channel) into a
  // single readable message block, instead of one timeline row per token.
  // Non-delta events stay as individual rows.
  const timeline = useMemo(() => {
    type DeltaRun = {
      kind: "delta_run";
      firstSeq: number;
      lastSeq: number;
      stageId?: string;
      channel?: string | null;
      text: string;
    };
    type Item = DeltaRun | { kind: "event"; seq: number; event: WorkflowEvent };
    const items: Item[] = [];
    let run: DeltaRun | null = null;
    const flush = () => {
      if (run) {
        items.push(run);
        run = null;
      }
    };
    for (const e of events) {
      if (e.kind === "model_delta") {
        const ch = e.channel ?? null;
        if (
          run &&
          run.stageId === e.stageId &&
          ((run.channel ?? null) === ch)
        ) {
          run.text += e.text ?? "";
          run.lastSeq = e.sequence;
        } else {
          flush();
          run = {
            kind: "delta_run",
            firstSeq: e.sequence,
            lastSeq: e.sequence,
            stageId: e.stageId,
            channel: ch,
            text: e.text ?? "",
          };
        }
      } else {
        flush();
        items.push({ kind: "event", seq: e.sequence, event: e });
      }
    }
    flush();
    return items;
  }, [events]);

  return (
    <div className="app">
      <header className="app-header">
        <h1>{WORKFLOW_ID}</h1>
        <p className="muted">
          Entry: {workflowManifest.workflow.entry} · NemoIR-compiled browser workflow
        </p>
      </header>

      <section className="disclosure local">
        🔒 Running locally via WebLLM — prompts and outputs stay in this browser.
      </section>

      {!webgpu && (
        <section className="card warn">
          <strong>WebGPU unavailable.</strong> Use Chrome/Edge 113+ or Opera 99+. A cloud
          endpoint (Phase 4) is not wired in this build.
        </section>
      )}
      {webgpu && !coi && (
        <section className="card warn">
          Cross-origin isolation (COOP/COEP) is not active. WebLLM requires
          SharedArrayBuffer. Re-check your hosting headers.
        </section>
      )}
      {sessionError && <section className="card warn">Session error: {sessionError}</section>}

      <section className="card">
        <h2>Model</h2>
        {!session && webgpu && <p className="muted">Loading model catalog…</p>}
        {session && (
          <>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              disabled={running || loading}
            >
              {(() => {
                const cached = new Set(cachedIds ?? []);
                const byVram = [...session.models].sort(
                  (a, b) => (a.vramRequiredMb ?? 0) - (b.vramRequiredMb ?? 0),
                );
                const cachedModels = byVram.filter((m) => cached.has(m.modelId));
                const availableModels = byVram.filter((m) => !cached.has(m.modelId));
                const opt = (m: WebLlmModelInfo) => (
                  <option key={m.modelId} value={m.modelId}>
                    {m.label} (~{Math.round((m.vramRequiredMb ?? 0) / 1024)} GB)
                  </option>
                );
                return (
                  <>
                    {cachedModels.length > 0 && (
                      <optgroup label="Cached models">
                        {cachedModels.map(opt)}
                      </optgroup>
                    )}
                    {/* Only show this group if there are uncached models (or
                        if cache state isn't resolved yet, render everything under
                        it so the list isn't empty on first paint). */}
                    {(availableModels.length > 0 || cachedIds === null) && (
                      <optgroup label={cachedIds === null ? "Available models" : "Available models (need download)"}>
                        {(cachedIds === null ? byVram : availableModels).map(opt)}
                      </optgroup>
                    )}
                  </>
                );
              })()}
            </select>{" "}
            <button
              onClick={handleLoadModel}
              disabled={modelLoaded || loading || running || !selectedModel}
            >
              {modelLoaded ? "Loaded" : loading ? "Loading…" : "Load model"}
            </button>
            {progress && (
              <div className="progress">
                <div className="bar" style={{ width: `${Math.round(progress.progress * 100)}%` }} />
                <span>{progress.text}</span>
              </div>
            )}
          </>
        )}
      </section>

      <section className="card">
        <h2>Inputs</h2>
        {inputs.length === 0 && <p className="muted">This workflow takes no inputs.</p>}
        {inputs.map((inp) => (
          <div key={inp.id} className="field">
            <label htmlFor={`in-${inp.id}`}>{inp.id}</label>
            {inp.type === "bool" ? (
              <input
                id={`in-${inp.id}`}
                type="checkbox"
                checked={Boolean(values[inp.id])}
                onChange={(e) => setValues((v) => ({ ...v, [inp.id]: e.target.checked }))}
                disabled={running}
              />
            ) : inp.type === "number" ? (
              <input
                id={`in-${inp.id}`}
                type="number"
                value={Number(values[inp.id])}
                onChange={(e) => setValues((v) => ({ ...v, [inp.id]: e.target.value }))}
                disabled={running}
              />
            ) : inp.type === "string[]" ? (
              <textarea
                id={`in-${inp.id}`}
                value={String(values[inp.id] ?? "")}
                onChange={(e) => setValues((v) => ({ ...v, [inp.id]: e.target.value }))}
                disabled={running}
                placeholder="One value per line"
              />
            ) : (
              <input
                id={`in-${inp.id}`}
                type="text"
                value={String(values[inp.id] ?? "")}
                onChange={(e) => setValues((v) => ({ ...v, [inp.id]: e.target.value }))}
                disabled={running}
              />
            )}
            <span className="type-hint">{inp.type}</span>
          </div>
        ))}
      </section>

      <section className="controls">
        <button className="primary" onClick={handleRun} disabled={running || !modelLoaded}>
          {running ? "Running…" : "Run"}
        </button>
        <button onClick={handleCancel} disabled={!running}>
          Cancel
        </button>
        <button onClick={exportTrace} disabled={events.length === 0}>
          Export trace (JSONL)
        </button>
      </section>

      {error && <section className="card error">❌ {error}</section>}

      {events.length > 0 && (
        <section className="card">
          <h2>Events</h2>
          <ol className="timeline">
            {timeline.map((item) =>
              item.kind === "delta_run" ? (
                <li key={`r${item.firstSeq}`} className="event delta-run">
                  <span className="seq">{item.firstSeq}{item.lastSeq !== item.firstSeq ? `–${item.lastSeq}` : ""}</span>{" "}
                  <span className="kind">{item.channel === "reasoning" ? "reasoning" : "assistant"}</span>
                  {item.stageId && <span className="stage">{item.stageId}</span>}
                  <span className={`delta ${item.channel === "reasoning" ? "reasoning" : ""}`}>{item.text}</span>
                </li>
              ) : (
                <li key={`e${item.seq}`} className={`event ${item.event.kind}`}>
                  <span className="seq">{item.seq}</span>{" "}
                  <span className="kind">{item.event.kind}</span>
                  {item.event.stageId && <span className="stage">{item.event.stageId}</span>}
                  {item.event.kind === "transition_selected" && item.event.transitionTo && (
                    <span className="arrow">→ {item.event.transitionTo}</span>
                  )}
                  {item.event.error && <span className="err-text">{item.event.error}</span>}
                </li>
              ),
            )}
          </ol>
        </section>
      )}

      {result && (
        <section className="card">
          <h2>Result</h2>
          <pre>{JSON.stringify(result, null, 2)}</pre>
        </section>
      )}

      {pendingElicit && (
        <div className="modal-backdrop">
          <div className="modal">
            <p>{pendingElicit.question}</p>
            {pendingElicit.options && pendingElicit.options.length > 0 ? (
              <div className="modal-actions">
                {pendingElicit.options.map((opt) => (
                  <button
                    key={opt}
                    onClick={() => {
                      pendingElicit.resolve(opt);
                      setPendingElicit(null);
                    }}
                  >
                    {opt}
                  </button>
                ))}
              </div>
            ) : (
              <>
                <input
                  type="text"
                  id="elicit-input"
                  autoFocus
                  placeholder="Type your answer…"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      pendingElicit.resolve(
                        (document.getElementById("elicit-input") as HTMLInputElement)?.value ?? "",
                      );
                      setPendingElicit(null);
                    }
                  }}
                />
                <div className="modal-actions">
                  <button
                    className="primary"
                    onClick={() => {
                      pendingElicit.resolve(
                        (document.getElementById("elicit-input") as HTMLInputElement)?.value ?? "",
                      );
                      setPendingElicit(null);
                    }}
                  >
                    Submit
                  </button>
                  <button
                    onClick={() => {
                      pendingElicit.reject(new DOMException("Cancelled by user", "AbortError"));
                      setPendingElicit(null);
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {pendingConfirm && (
        <div className="modal-backdrop">
          <div className="modal">
            <p>{pendingConfirm.message}</p>
            <div className="modal-actions">
              <button
                onClick={() => {
                  pendingConfirm.resolve(true);
                  setPendingConfirm(null);
                }}
              >
                Yes
              </button>
              <button
                onClick={() => {
                  pendingConfirm.resolve(false);
                  setPendingConfirm(null);
                }}
              >
                No
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
"##.to_string()
}

/// Render the generic runner UI stylesheet (`src/app.css`).
pub fn emit_app_css() -> String {
    r##":root {
  --bg: #f7f7f8;
  --card: #ffffff;
  --border: #e2e2e6;
  --text: #1a1a1a;
  --muted: #71717a;
  --accent: #4f46e5;
  --warn-bg: #fff7ed;
  --warn-border: #f59e0b;
  --err-bg: #fef2f2;
  --err-border: #ef4444;
  font-family: system-ui, -apple-system, sans-serif;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--text); }
.app { max-width: 760px; margin: 0 auto; padding: 1.5rem 1rem 4rem; }
.app-header h1 { margin: 0 0 0.25rem; font-size: 1.6rem; }
.muted { color: var(--muted); margin: 0; }
.card {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 1rem 1.1rem;
  margin: 0.75rem 0;
}
.card h2 { margin: 0 0 0.75rem; font-size: 1.1rem; }
.card.warn { background: var(--warn-bg); border-color: var(--warn-border); }
.card.error { background: var(--err-bg); border-color: var(--err-border); }
.disclosure { font-size: 0.85rem; padding: 0.5rem 0.75rem; border-radius: 8px; margin: 0.5rem 0; }
.disclosure.local { background: #ecfdf5; color: #065f46; }
.field { margin-bottom: 0.6rem; display: flex; align-items: center; gap: 0.5rem; }
.field label { min-width: 120px; font-weight: 600; }
.field input[type="text"], .field input[type="number"], .field textarea {
  flex: 1; padding: 0.4rem 0.5rem; border: 1px solid var(--border); border-radius: 6px; font: inherit;
}
.type-hint { color: var(--muted); font-size: 0.8rem; }
.controls { display: flex; gap: 0.5rem; margin: 1rem 0; }
button {
  padding: 0.5rem 0.9rem; border: 1px solid var(--border); border-radius: 7px; background: var(--card);
  cursor: pointer; font: inherit;
}
button:disabled { opacity: 0.5; cursor: not-allowed; }
button.primary { background: var(--accent); color: #fff; border-color: var(--accent); }
.progress { margin-top: 0.5rem; position: relative; height: 1.4rem; background: #eee; border-radius: 6px; overflow: hidden; }
.progress .bar { position: absolute; inset: 0; background: var(--accent); opacity: 0.35; transition: width 0.2s; }
.progress span { position: relative; padding: 0.2rem 0.5rem; font-size: 0.8rem; display: block; }
.timeline { list-style: none; padding: 0; margin: 0; max-height: 400px; overflow-y: auto; font-size: 0.82rem; }
.timeline li { padding: 0.2rem 0; border-bottom: 1px solid #f0f0f0; }
.timeline .seq { color: var(--muted); font-variant-numeric: tabular-nums; }
.timeline .kind { font-weight: 600; margin-left: 0.3rem; }
.timeline .stage { color: var(--accent); margin-left: 0.4rem; }
.timeline .delta-run .delta { display: block; margin: 0.25rem 0 0 1.4rem; white-space: pre-wrap; word-break: break-word; }
.timeline .delta { color: #166534; margin-left: 0.4rem; }
.timeline .delta.reasoning { color: #7c3aed; font-style: italic; }
.timeline .arrow { color: var(--accent); margin-left: 0.4rem; }
.timeline .err-text { color: #dc2626; margin-left: 0.4rem; }
pre { background: #f4f4f5; padding: 0.75rem; border-radius: 6px; overflow-x: auto; font-size: 0.85rem; }
.modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; z-index: 50; }
.modal { background: var(--card); padding: 1.25rem; border-radius: 10px; min-width: 320px; }
.modal p { margin: 0 0 0.75rem; }
.modal input { width: 100%; padding: 0.4rem; border: 1px solid var(--border); border-radius: 6px; }
.modal-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 0.75rem; }
"##.to_string()
}

/// Render the WebLLM Web Worker entry (`src/webllm.worker.ts`).
///
/// Hosts `WebWorkerMLCEngineHandler` so the main thread can proxy
/// `MLCEngineInterface` calls via `CreateWebWorkerMLCEngine`.
pub fn emit_webllm_worker() -> String {
    r#"import { WebWorkerMLCEngineHandler } from "@mlc-ai/web-llm";

// The handler resides in the worker thread and processes all model
// computation off the main UI thread. The main thread holds a
// WebWorkerMLCEngine proxy that sends messages here.
const handler = new WebWorkerMLCEngineHandler();
self.onmessage = (msg: MessageEvent) => {
  handler.onmessage(msg);
};
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_json_contains_runtime_dep() {
        let s = emit_package_json("judge-candidate", "0.1.0", "^0.1.0");
        assert!(s.contains(r#""name": "judge-candidate""#));
        assert!(s.contains(r#""@nemoir/web-runtime": "^0.1.0""#));
        assert!(s.contains(r#""@mlc-ai/web-llm": "^0.2.84""#));
        assert!(s.contains(r#""react": "^19.0.0""#));
        assert!(s.contains(r#""version": "0.1.0""#));
    }

    #[test]
    fn package_json_supports_file_dep() {
        let s = emit_package_json("foo", "1.2.3", "file:../../web/nemoir-runtime");
        assert!(s.contains(r#""@nemoir/web-runtime": "file:../../web/nemoir-runtime""#));
    }

    #[test]
    fn vite_config_includes_coop_coep() {
        let s = emit_vite_config();
        assert!(s.contains("Cross-Origin-Embedder-Policy"));
        assert!(s.contains("Cross-Origin-Opener-Policy"));
        assert!(s.contains("Cross-Origin-Resource-Policy"));
        assert!(s.contains("server"));
        assert!(s.contains("preview"));
        assert!(s.contains("allowedHosts: true"));
    }

    #[test]
    fn netlify_toml_has_headers() {
        let s = emit_netlify_toml();
        assert!(s.contains("[build]"));
        assert!(s.contains("Cross-Origin-Embedder-Policy = \"require-corp\""));
        assert!(s.contains("publish = \"dist\""));
    }

    #[test]
    fn public_headers_file() {
        let s = emit_public_headers();
        assert!(s.contains("Cross-Origin-Embedder-Policy: require-corp"));
    }

    #[test]
    fn index_html_has_root_and_script() {
        let s = emit_index_html("judge-candidate", "JudgeCandidate");
        assert!(s.contains("<div id=\"root\"></div>"));
        assert!(s.contains("src/main.tsx"));
        assert!(s.contains("JudgeCandidate"));
    }

    #[test]
    fn main_tsx_imports_agent_and_manifest() {
        let s = emit_main_tsx();
        assert!(s.contains("from \"./agent\""));
        assert!(s.contains("import workflowManifest from \"./workflow.json\""));
        assert!(s.contains("createWebllmSession"));
        assert!(s.contains("WebLLM"));
        assert!(s.contains("./app.css"));
        // Coalescing of consecutive model_delta chunks into one readable block.
        assert!(s.contains("delta_run"));
        assert!(s.contains("model_delta"));
    }

    #[test]
    fn app_css_has_styles() {
        let s = emit_app_css();
        assert!(s.contains("--accent"));
        assert!(s.contains(".timeline"));
        assert!(s.contains(".modal"));
        assert!(s.contains(".delta-run"));
        assert!(s.contains(".reasoning"));
    }

    #[test]
    fn webllm_worker_has_handler() {
        let s = emit_webllm_worker();
        assert!(s.contains("WebWorkerMLCEngineHandler"));
        assert!(s.contains("self.onmessage"));
    }

    #[test]
    fn readme_has_workflow_id() {
        let s = emit_readme("JudgeCandidate", "judge-candidate", "Baseline");
        assert!(s.contains("JudgeCandidate"));
        assert!(s.contains("npm run dev"));
        assert!(s.contains("src/workflow.json"));
        assert!(s.contains("GitHub Pages"));
    }
}
