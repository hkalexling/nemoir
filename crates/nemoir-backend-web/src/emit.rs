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
///   (e.g. `"^0.4.0"` or `"file:../../web/nemoir-runtime"`).
/// - `ui_dep`: dependency spec for `@nemoir/web-ui`
///   (e.g. `"^0.2.0"` or `"file:../../web/nemoir-ui"`).
pub fn emit_package_json(
    package_dir: &str,
    version: &str,
    runtime_dep: &str,
    ui_dep: &str,
) -> String {
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
    "@nemoir/web-ui": "{ui_dep}",
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
        ui_dep = ui_dep,
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
The dev server serves cross-origin isolation headers (COOP/COEP).\n\n\
## Requirements\n\n\
- **WebGPU:** Chrome/Edge 113+, or Opera 99+. Required only for workflows\n\
  with model stages. For deterministic-only workflows, WebGPU is not needed.\n\
- **Cross-origin isolation (COOP/COEP):** required for WebLLM's\n\
  `SharedArrayBuffer`. Deterministic-only workflows do not need this.\n\
  The dev/preview servers set these headers (see `vite.config.ts`).\n\
  Hosts: Netlify, Vercel, Cloudflare Pages (config files are included).\n\
  GitHub Pages **cannot** set these headers — use a `coi-serviceworker` shim.\n\
- **Model download (model-stage workflows only):** small models are ~1–5 GB.\n\
  WebLLM caches them (OPFS preferred, IndexedDB fallback). Before downloading\n\
  a large model the runner UI checks available storage and warns if it looks\n\
  insufficient. Free browser storage or choose a smaller model. Cached models\n\
  never warn. For deterministic-only workflows this section does not apply.\n\n\
## Building against local runtime / UI checkouts\n\n\
`@nemoir/web-runtime` and `@nemoir/web-ui` are published to npm, so `npm install`\n\
resolves them directly. To build against an in-repo checkout during development,\n\
point the compile step at both packages:\n\n\
```bash\n\
nemo compile <file>.nemo --target web \\
\
  --web-runtime-dependency file:../../web/nemoir-runtime \\
\
  --web-ui-dependency file:../../web/nemoir-ui \\
\
  -o out/\n\
```\n\n\
Each override is independent — you can override only the runtime, only the UI,\n\
or both.\n\n\
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
in `src/main.tsx` loads it and runs the workflow locally (no data leaves the\n\
browser). For model-stage workflows, it detects WebGPU and lets you pick and\n\
load a WebLLM model. For deterministic-only workflows, it runs without any\n\
model setup. All runs stream stage/event output with a JSONL trace export.\n",
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
/// Generated app uses shared primitives from `@nemoir/web-ui` for:
/// - `useWebLlmSession` — WebLLM lifecycle/model UI (disabled entirely for
///   deterministic-only workflows via `enabled: HAS_MODEL_STAGES`)
/// - `useWorkflowRun` — generic async stream runner, cancellation,
///   coalesced timeline derivation, result/error capture
/// - `WebUiHostProvider`/`useWebUiHost` — elicit/confirm dialog host
/// - `ModelLoader` — model selection/loading UI
/// - `WorkflowTraceDrawer` — scrollable event timeline display
/// - `downloadJsonl` — JSONL trace export
///
/// Schema-driven input form, generated Agent construction, worker/sandbox
/// wiring, disclosure, and styling remain app-specific.
///
/// The emitted app avoids model session creation for deterministic-only
/// workflows by passing `enabled: HAS_MODEL_STAGES` to `useWebLlmSession`.
/// Model-storage safety: ordinary Load assesses storage before starting an
/// insufficient download; the explicit Load anyway path bypasses the warning.
pub fn emit_main_tsx(has_js_run: bool, has_js_sandbox: bool) -> String {
    let src = r##"import { useCallback, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Agent,
  HAS_JS_SANDBOX,
  HAS_MODEL_STAGES,
  WORKFLOW_ID,
  type AgentInput,
} from "./agent";
import workflowManifest from "./workflow.json";
import {
  __NEMOIR_SANDBOX_IMPORT__
  type ModelAdapter,
  type WebUiHost,
} from "@nemoir/web-runtime";
import {
  useWebLlmSession,
  useWorkflowRun,
  useWebUiHost,
  WebUiHostProvider,
  ModelLoader,
  WorkflowTraceDrawer,
  downloadJsonl,
  type WorkflowRunner,
} from "@nemoir/web-ui";

// Build the browser.js.run worker factory from the emitted static worker.
const jsWorkerFactory = () =>
  new Worker(new URL("./js.worker.ts", import.meta.url), { type: "module" });
__NEMOIR_SANDBOX_FACTORY__
import "./app.css";

type InputSpec = { id: string; type: string };

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
    case "json":
      return "{}";
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
    case "json": {
      const text = typeof raw === "string" ? raw : "";
      const trimmed = text.trim();
      if (trimmed.length === 0) return null;
      try {
        return JSON.parse(trimmed);
      } catch {
        return null;
      }
    }
    default:
      return raw;
  }
}

function App() {
  return (
    <WebUiHostProvider>
      <AppInner />
    </WebUiHostProvider>
  );
}

function AppInner() {
  const inputs = workflowManifest.inputs as InputSpec[];
  const [values, setValues] = useState<Record<string, unknown>>(() => {
    const init: Record<string, unknown> = {};
    for (const inp of inputs) init[inp.id] = defaultFor(inp.type);
    return init;
  });

  // Compute which `json` inputs currently hold invalid JSON text.
  const invalidJsonInputs = useMemo(() => {
    const bad = new Set<string>();
    for (const inp of inputs) {
      if (inp.type !== "json") continue;
      const text = String(values[inp.id] ?? "").trim();
      if (text.length === 0) continue;
      try { JSON.parse(text); } catch { bad.add(inp.id); }
    }
    return bad;
  }, [inputs, values]);
  const hasInvalidJson = invalidJsonInputs.size > 0;

  // ---- WebLLM lifecycle via shared hook (disabled for deterministic-only)
  const {
    webgpuAvailable: webgpu,
    crossOriginIsolated: coi,
    session,
    models,
    selectedModel,
    setSelectedModel,
    loading,
    progress,
    error: sessionError,
    cachedIds,
    storageAssessment,
    isModelLoaded: modelLoaded,
    loadModel,
    assessStorageForSelected,
  } = useWebLlmSession({
    enabled: HAS_MODEL_STAGES,
    workerFactory: () =>
      new Worker(new URL("./webllm.worker.ts", import.meta.url), { type: "module" }),
  });

  // Storage-warning dismissal tracks per-model and resets on selection.
  const [dismissedStorageWarning, setDismissedStorageWarning] = useState(false);
  useEffect(() => { setDismissedStorageWarning(false); }, [selectedModel]);

  // ---- Run lifecycle via shared hook
  const uiHost = useWebUiHost();

  const runner: WorkflowRunner = useCallback(
    async function* (input: Record<string, unknown>, signal: AbortSignal) {
      let modelAdapter: ModelAdapter | undefined;
      if (HAS_MODEL_STAGES) {
        if (!session) throw new Error("WebLLM session is not ready.");
        await session.ensureLoaded(selectedModel, signal);
        modelAdapter = session.adapter;
      }
      const agent = new Agent({
        modelAdapter,
        uiHost: uiHost as WebUiHost,
        browserTools: __NEMOIR_BROWSER_TOOLS__,
      });
      yield* agent.stream(input as unknown as AgentInput, { signal });
    },
    [session, selectedModel, uiHost],
  );

  const {
    running,
    events,
    timeline,
    result,
    error,
    start: startRun,
    cancel,
  } = useWorkflowRun(runner);

  // ---- Load handler with storage assessment guard
  const handleLoadModel = useCallback(async () => {
    if (!selectedModel) return;
    // Re-assess storage immediately before download (quota may have changed).
    // The shared hook also publishes this fresh result to ModelLoader.
    const assessment = await assessStorageForSelected();
    if (assessment && !assessment.likelySufficient) {
      setDismissedStorageWarning(false);
      return; // wait for explicit "Load anyway"
    }
    await loadModel();
  }, [selectedModel, assessStorageForSelected, loadModel]);

  // "Load anyway" bypasses storage assessment.
  const handleLoadAnyway = useCallback(async () => {
    setDismissedStorageWarning(true);
    await loadModel();
  }, [loadModel]);

  // ---- Run handler
  const handleRun = useCallback(() => {
    if (running) return;
    const parsed: Record<string, unknown> = {};
    for (const inp of inputs) {
      parsed[inp.id] = parseInputValue(inp.type, values[inp.id]);
    }
    startRun(parsed);
  }, [running, inputs, values, startRun]);

  // ---- JSONL export
  const handleExportTrace = useCallback(() => {
    if (events.length === 0) return;
    downloadJsonl(events, `${WORKFLOW_ID}-trace.jsonl`);
  }, [events]);

  // ---- Render
  return (
    <div className="app">
      <header className="app-header">
        <h1>{WORKFLOW_ID}</h1>
        <p className="muted">
          Entry: {workflowManifest.workflow.entry} · NemoIR-compiled browser workflow
        </p>
      </header>

      <section className="disclosure local">
        {HAS_MODEL_STAGES
          ? "🔒 Running locally via WebLLM — prompts and outputs stay in this browser."
          : "🔒 Running locally in this browser — no model calls, no cloud dependency."}
      </section>

      {HAS_JS_SANDBOX && (() => {
        const sandboxStages = (workflowManifest.nodes ?? [])
          .filter((n: any) => n.execution?.kind === "tool" && n.execution?.capability === "browser.js.sandbox")
          .map((n: any) => ({ id: n.id as string, writes: (n.writes ?? []).map((w: any) => w.name as string) }));
        return (
          <section className="disclosure sandbox">
            <p>⚠️ This workflow may ask to run user- or model-provided JavaScript. Source is shown for confirmation and runs in an isolated, network-restricted sandbox with strict limits.</p>
            <p>Dynamic code is an async function body receiving <code>input</code>; it must return a plain JSON object with the stage's declared write names:</p>
            <ul>
              {sandboxStages.map((s: { id: string; writes: string[] }) => (
                <li key={s.id}><strong>{s.id}</strong>: <code>return {`{ ${s.writes.join(", ")} }`}</code></li>
              ))}
            </ul>
          </section>
        );
      })()}

      {HAS_MODEL_STAGES && !webgpu && (
        <section className="card warn">
          <strong>WebGPU unavailable.</strong> Use Chrome/Edge 113+ or Opera 99+.
        </section>
      )}
      {HAS_MODEL_STAGES && webgpu && !coi && (
        <section className="card warn">
          Cross-origin isolation (COOP/COEP) is not active. WebLLM requires
          SharedArrayBuffer. Re-check your hosting headers.
        </section>
      )}
      {sessionError && <section className="card warn">Session error: {sessionError}</section>}

      {HAS_MODEL_STAGES && (
        <section className="card">
          <h2>Model</h2>
          {!session && webgpu && <p className="muted">Loading model catalog…</p>}
          {session && (
            <>
              <ModelLoader
                models={models}
                cachedIds={cachedIds}
                selectedModel={selectedModel}
                onSelectModel={setSelectedModel}
                loading={loading}
                isLoaded={modelLoaded}
                progress={progress}
                storageAssessment={storageAssessment}
                storageWarningDismissed={dismissedStorageWarning}
                onDismissStorageWarning={handleLoadAnyway}
                onLoad={handleLoadModel}
                disabled={running}
              />
            </>
          )}
        </section>
      )}

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
            ) : inp.type === "json" ? (
              <textarea
                id={`in-${inp.id}`}
                value={String(values[inp.id] ?? "")}
                onChange={(e) => setValues((v) => ({ ...v, [inp.id]: e.target.value }))}
                disabled={running}
                placeholder='{"key": "value"}'
                className={invalidJsonInputs.has(inp.id) ? "invalid-json" : ""}
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
        <button className="primary" onClick={handleRun} disabled={running || (HAS_MODEL_STAGES ? !modelLoaded : false) || hasInvalidJson}>
          {running ? "Running…" : "Run"}
        </button>
        <button onClick={cancel} disabled={!running}>
          Cancel
        </button>
        <button onClick={handleExportTrace} disabled={events.length === 0}>
          Export trace (JSONL)
        </button>
      </section>

      {error && <section className="card error">❌ {error}</section>}

      <WorkflowTraceDrawer items={timeline} />

      {result && (
        <section className="card">
          <h2>Result</h2>
          <pre>{JSON.stringify(result, null, 2)}</pre>
        </section>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
"##.to_string();

    let sandbox_import = if has_js_sandbox {
        "createOpaqueOriginJsSandbox,\n"
    } else {
        ""
    };
    let sandbox_factory = if has_js_sandbox {
        "// Build the opaque-origin runner used only for dynamic browser.js.sandbox stages.\nconst jsSandboxRunner = createOpaqueOriginJsSandbox();\n"
    } else {
        ""
    };
    let browser_tools = match (has_js_run, has_js_sandbox) {
        (true, true) => "{ jsWorkerFactory, jsSandboxRunner }",
        (true, false) => "{ jsWorkerFactory }",
        (false, true) => "{ jsSandboxRunner }",
        (false, false) => "{}",
    };

    let src = src
        .replace("__NEMOIR_SANDBOX_IMPORT__\n", sandbox_import)
        .replace("__NEMOIR_SANDBOX_FACTORY__\n", sandbox_factory)
        .replace("__NEMOIR_BROWSER_TOOLS__", browser_tools);

    if has_js_run {
        src
    } else {
        // No trusted `browser.js.run` stage: drop the static-worker factory so
        // Vite does not try to resolve an asset that was not emitted.
        src.replace(
            "// Build the browser.js.run worker factory from the emitted static worker.\nconst jsWorkerFactory = () =>\n  new Worker(new URL(\"./js.worker.ts\", import.meta.url), { type: \"module\" });\n",
            "",
        )
    }
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
.disclosure.sandbox { background: #fff7ed; color: #9a3412; border: 1px solid #fdba74; }
.field { margin-bottom: 0.6rem; display: flex; align-items: center; gap: 0.5rem; }
.field label { min-width: 120px; font-weight: 600; }
.field input[type="text"], .field input[type="number"], .field textarea {
  flex: 1; padding: 0.4rem 0.5rem; border: 1px solid var(--border); border-radius: 6px; font: inherit;
}
.type-hint { color: var(--muted); font-size: 0.8rem; }
.invalid-json { border-color: var(--err-border) !important; }
.controls { display: flex; gap: 0.5rem; margin: 1rem 0; }
button {
  padding: 0.5rem 0.9rem; border: 1px solid var(--border); border-radius: 7px; background: var(--card);
  cursor: pointer; font: inherit;
}
button:disabled { opacity: 0.5; cursor: not-allowed; }
button.primary, button.nemoir-primary { background: var(--accent); color: #fff; border-color: var(--accent); }
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
.modal-backdrop, .nemoir-modal-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; z-index: 50; }
.modal, .nemoir-modal { background: var(--card); padding: 1.25rem; border-radius: 10px; min-width: 320px; }
.modal p, .nemoir-modal p { margin: 0 0 0.75rem; white-space: pre-wrap; max-height: 50vh; overflow: auto; }
.modal input, .nemoir-modal input { width: 100%; padding: 0.4rem; border: 1px solid var(--border); border-radius: 6px; }
.modal-actions, .nemoir-modal-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 0.75rem; }
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

/// Render the JS-run worker (`src/js.worker.ts`) for `browser.js.run`.
///
/// This is a static worker (emitted at codegen time) that receives
/// `{ code, input }` via postMessage, executes the trusted code inside
/// a `new Function`, and posts back the result. The worker is isolated
/// from the DOM and from the page's JavaScript context.
pub fn emit_js_run_worker() -> String {
    r#"// Auto-generated static worker for browser.js.run.
// Receives `{ code, input }` via postMessage and executes the trusted code.
self.onmessage = async (e: MessageEvent) => {
  const { code, input } = e.data as { code: string; input: unknown };
  try {
    const fn = new Function("input", code);
    const result = await fn(input);
    self.postMessage(result);
  } catch (err) {
    self.postMessage({
      __error: err instanceof Error ? err.message : String(err),
    });
  }
};
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_json_contains_runtime_dep() {
        let s = emit_package_json("judge-candidate", "0.1.0", "^0.1.0", "^0.1.0");
        assert!(s.contains(r#""name": "judge-candidate""#));
        assert!(s.contains(r#""@nemoir/web-runtime": "^0.1.0""#));
        assert!(s.contains(r#""@nemoir/web-ui": "^0.1.0""#));
        assert!(s.contains(r#""@mlc-ai/web-llm": "^0.2.84""#));
        assert!(s.contains(r#""react": "^19.0.0""#));
        assert!(s.contains(r#""version": "0.1.0""#));
    }

    #[test]
    fn package_json_supports_file_dep() {
        let s = emit_package_json(
            "foo",
            "1.2.3",
            "file:../../web/nemoir-runtime",
            "file:../../web/nemoir-ui",
        );
        assert!(s.contains(r#""@nemoir/web-runtime": "file:../../web/nemoir-runtime""#));
        assert!(s.contains(r#""@nemoir/web-ui": "file:../../web/nemoir-ui""#));
    }

    #[test]
    fn package_json_contains_ui_dep_default() {
        let s = emit_package_json("test-app", "0.1.0", "^0.4.0", "^0.2.0");
        assert!(s.contains(r#""@nemoir/web-ui": "^0.2.0""#));
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
        let s = emit_main_tsx(true, false);
        assert!(s.contains("from \"./agent\""));
        assert!(s.contains("import workflowManifest from \"./workflow.json\""));
        assert!(s.contains("@nemoir/web-ui"));
        assert!(s.contains("@nemoir/web-runtime"));
        assert!(s.contains("./app.css"));
        // Shared primitives imported from @nemoir/web-ui.
        assert!(s.contains("useWebLlmSession"));
        assert!(s.contains("useWorkflowRun"));
        assert!(s.contains("WebUiHostProvider"));
        assert!(s.contains("ModelLoader"));
        assert!(s.contains("WorkflowTraceDrawer"));
        assert!(s.contains("downloadJsonl"));
        // Model session disabled for deterministic-only via enabled flag.
        assert!(s.contains("enabled: HAS_MODEL_STAGES"));
        // Storage assessment guard + Load anyway bypass.
        assert!(s.contains("assessStorage"));
        assert!(s.contains("dismissedStorageWarning"));
        // Phase-6: js-worker wiring is present when has_js_run=true.
        assert!(s.contains("jsWorkerFactory"));
        assert!(s.contains("./js.worker.ts"));
        assert!(s.contains("browserTools: { jsWorkerFactory }"));
    }

    #[test]
    fn main_tsx_omits_js_worker_when_unused() {
        let s = emit_main_tsx(false, false);
        // No js.run stage → no jsWorkerFactory definition, no js.worker.ts import,
        // and an empty browserTools object.
        assert!(!s.contains("jsWorkerFactory"));
        assert!(!s.contains("./js.worker.ts"));
        assert!(s.contains("browserTools: {}"));
        // The json form control is always present (harmless when no json inputs).
        assert!(s.contains(r###"case "json":"###));
        assert!(s.contains("invalidJsonInputs"));
        // Shared UI primitives still imported.
        assert!(s.contains("@nemoir/web-ui"));
        assert!(s.contains("useWebLlmSession"));
        assert!(s.contains("useWorkflowRun"));
    }

    #[test]
    fn main_tsx_wires_opaque_origin_sandbox_only_when_used() {
        let s = emit_main_tsx(false, true);
        assert!(s.contains("createOpaqueOriginJsSandbox"));
        assert!(s.contains("jsSandboxRunner"));
        assert!(s.contains("browserTools: { jsSandboxRunner }"));
        assert!(!s.contains("jsWorkerFactory"));
        assert!(!s.contains("./js.worker.ts"));
        // Shared UI primitives still imported.
        assert!(s.contains("@nemoir/web-ui"));
        assert!(s.contains("ModelLoader"));
    }

    #[test]
    fn main_tsx_renders_sandbox_return_envelope_contract() {
        // When has_js_sandbox=true, the generated disclosure derives each
        // sandbox stage's write names from the compiled IR so the required
        // return envelope is visible where code is authored.
        let s = emit_main_tsx(false, true);
        assert!(s.contains("browser.js.sandbox"));
        assert!(s.contains("Dynamic code is an async function body"));
        assert!(s.contains("must return a plain JSON object"));
        assert!(s.contains("return {`{ ${s.writes.join("));
        assert!(s.contains("sandboxStages"));
    }

    #[test]
    fn main_tsx_web_ui_host_provider_wraps_app() {
        let s = emit_main_tsx(false, false);
        assert!(s.contains("WebUiHostProvider"));
        assert!(s.contains("<AppInner"));
        assert!(s.contains("useWebUiHost"));
    }

    #[test]
    fn main_tsx_uses_workflow_run_hook() {
        let s = emit_main_tsx(false, false);
        assert!(s.contains("useWorkflowRun"));
        assert!(s.contains("WorkflowRunner"));
        assert!(s.contains("WorkflowTraceDrawer"));
        assert!(s.contains("downloadJsonl"));
    }

    #[test]
    fn app_css_has_styles() {
        let s = emit_app_css();
        assert!(s.contains("--accent"));
        assert!(s.contains(".timeline"));
        assert!(s.contains(".modal"));
        assert!(s.contains(".nemoir-modal-backdrop"));
        assert!(s.contains(".nemoir-modal-actions"));
        assert!(s.contains("button.nemoir-primary"));
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
        assert!(s.contains("--web-runtime-dependency"));
        assert!(s.contains("--web-ui-dependency"));
        assert!(s.contains("Each override is independent"));
    }
}
