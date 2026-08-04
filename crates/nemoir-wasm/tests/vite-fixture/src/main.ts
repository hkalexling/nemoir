import init, { analyze, generate, metadata } from "@nemoir/compiler-wasm";

// Workflow source that is valid for all targets.
const HELLO_SOURCE = `workflow HelloWorkflow {
  input { message: string }

  stage @entry Compose {
    prompt: "Reply with a concise, friendly greeting for the user's message."
    output: { greeting: string }
  }

  stage @exit Done {
    input: Compose.greeting
    prompt: "Return the greeting unchanged."
    output: { greeting: string }
  }
}`;

async function main(): Promise<void> {
  await init();

  // 1. Analyze — should succeed
  const ar = analyze({ source: HELLO_SOURCE, filename: "hello.nemo", includeIr: true });
  if (!ar.ok) {
    document.body.textContent = `FAIL: analyze not ok — ${ar.diagnostics[0]?.message}`;
    return;
  }
  if (typeof ar.ir !== "object" || ar.ir === null) {
    document.body.textContent = `FAIL: IR is ${typeof ar.ir}`;
    return;
  }

  // 2. Generate Python — verify artifact
  const gr = generate({ source: HELLO_SOURCE, filename: "hello.nemo", target: "python" });
  if (!gr.ok || !gr.artifact) {
    document.body.textContent = `FAIL: generate python not ok or no artifact`;
    return;
  }
  const paths = gr.artifact.files.map((f: { path: string }) => f.path);
  if (!paths.includes("pyproject.toml")) {
    document.body.textContent = `FAIL: missing pyproject.toml in ${paths}`;
    return;
  }

  // 3. Metadata — verify typed return
  const m = metadata();
  if (typeof m.compilerVersion !== "string" || m.compilerVersion.length === 0) {
    document.body.textContent = `FAIL: bad compilerVersion`;
    return;
  }
  if (typeof m.irVersion !== "string" || m.irVersion.length === 0) {
    document.body.textContent = `FAIL: bad irVersion`;
    return;
  }
  if (!Array.isArray(m.supportedTargets) || !m.supportedTargets.includes("python")) {
    document.body.textContent = `FAIL: bad supportedTargets`;
    return;
  }

  document.body.textContent = "ok";
}

void main();
