#!/usr/bin/env node

/**
 * Smoke test: verify the generated `@nemoir/compiler-wasm` package works
 * end-to-end in Node with `initSync` (no fetch / no browser).
 *
 * Usage:
 *   node tests/package-smoke.mjs [pkg-dir]
 *
 * Default `pkg-dir` is `./pkg/` relative to this script's directory
 * (`compiler/crates/nemoir-wasm/pkg`).  When running from the compiler
 * workspace root, pass the explicit path, e.g.:
 *
 *   node crates/nemoir-wasm/tests/package-smoke.mjs crates/nemoir-wasm/pkg
 */

import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(process.argv[2] ?? join(__dirname, "..", "pkg"));

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

const PASS = "✅";
const FAIL = "❌";
const results = [];

function check(name, fn) {
  try {
    const detail = fn();
    results.push({ name, pass: true, detail });
    console.log(`${PASS} ${name}`);
  } catch (e) {
    results.push({ name, pass: false, detail: e.message });
    console.log(`${FAIL} ${name}: ${e.message}`);
  }
}

/** Assert val is a plain object, not null, and not a Map/Array. */
function isPlainObject(val) {
  return (
    typeof val === "object" &&
    val !== null &&
    !(val instanceof Map) &&
    !Array.isArray(val)
  );
}

function readNemo(relPath) {
  // Try relative to the compiler workspace root (pkgDir/../../..)
  const fromCompiler = resolve(pkgDir, "..", "..", "..", relPath);
  // Try relative to the wasm crate dir (pkgDir/..)
  const fromCrate = resolve(pkgDir, "..", relPath);
  const fromPkg = resolve(pkgDir, relPath);
  for (const p of [fromCompiler, fromCrate, fromPkg]) {
    if (existsSync(p)) return readFileSync(p, "utf-8");
  }
  throw new Error(`fixture not found: ${relPath}`);
}

// ---------------------------------------------------------------------------
// initialization
// ---------------------------------------------------------------------------

check("pkg directory exists", () => {
  assert(existsSync(pkgDir), `pkg dir not found: ${pkgDir}`);
  return pkgDir;
});

const jsPath = join(pkgDir, "nemoir_wasm.js");
const wasmPath = join(pkgDir, "nemoir_wasm_bg.wasm");

check("JS entry exists", () => {
  assert(existsSync(jsPath), `missing: ${jsPath}`);
});
check("WASM binary exists", () => {
  assert(existsSync(wasmPath), `missing: ${wasmPath}`);
});

const mod = await import(pathToFileURL(jsPath).href);
const wasmBytes = readFileSync(wasmPath);

check("initSync succeeds", () => {
  mod.initSync({ module: wasmBytes });
  return "ok";
});

// ---------------------------------------------------------------------------
// metadata
// ---------------------------------------------------------------------------

check("metadata returns plain object", () => {
  const m = mod.metadata();
  assert(isPlainObject(m));
  return JSON.stringify(m);
});

check("metadata.compilerVersion is string", () => {
  const m = mod.metadata();
  assert.strictEqual(typeof m.compilerVersion, "string");
  assert(m.compilerVersion.length > 0);
  return m.compilerVersion;
});

check("metadata.irVersion is string", () => {
  const m = mod.metadata();
  assert.strictEqual(typeof m.irVersion, "string");
  return m.irVersion;
});

check("metadata.supportedTargets includes all four targets", () => {
  const m = mod.metadata();
  for (const t of ["none", "visualizer", "python", "web"]) {
    assert(m.supportedTargets.includes(t), `missing target: ${t}`);
  }
  return m.supportedTargets.join(", ");
});

// ---------------------------------------------------------------------------
// analyze — valid source
// ---------------------------------------------------------------------------

const helloSource = readNemo("examples/hello-workflow/hello.nemo");

check("analyze valid → response is plain object", () => {
  const r = mod.analyze({ source: helloSource, filename: "hello.nemo", includeIr: true });
  assert(isPlainObject(r));
});

check("analyze valid → ok = true", () => {
  const r = mod.analyze({ source: helloSource, filename: "hello.nemo" });
  assert.strictEqual(r.ok, true);
  assert.deepStrictEqual(r.diagnostics, []);
});

check("analyze valid + includeIr → ir is present and plain object", () => {
  const r = mod.analyze({ source: helloSource, filename: "hello.nemo", includeIr: true });
  assert.notStrictEqual(r.ir, undefined);
  assert.notStrictEqual(r.ir, null);
  assert(isPlainObject(r.ir), "ir should be a plain object — not a Map");
  // Spot-check a known IR field
  assert.strictEqual(r.ir.kind, "workflow_ir");
  assert(r.ir.workflow?.id === "HelloWorkflow", `unexpected workflow id: ${r.ir.workflow?.id}`);
});

check("analyze valid + includeIr=false → ir omitted", () => {
  const r = mod.analyze({ source: helloSource, filename: "hello.nemo", includeIr: false });
  assert.strictEqual(r.ir, undefined);
});

// ---------------------------------------------------------------------------
// analyze — invalid source
// ---------------------------------------------------------------------------

check("analyze invalid → ok = false", () => {
  const r = mod.analyze({ source: "not a valid workflow", filename: "bad.nemo" });
  assert.strictEqual(r.ok, false);
});

check("analyze invalid → has diagnostics", () => {
  const r = mod.analyze({ source: "not a valid workflow", filename: "bad.nemo" });
  assert(r.diagnostics.length > 0, "expected at least one diagnostic");
  assert.strictEqual(r.diagnostics[0].phase, "dsl");
  assert.strictEqual(typeof r.diagnostics[0].message, "string");
});

// ---------------------------------------------------------------------------
// generate — python
// ---------------------------------------------------------------------------

check("generate python → response is plain object", () => {
  const r = mod.generate({ source: helloSource, filename: "hello.nemo", target: "python" });
  assert(isPlainObject(r));
});

check("generate python → ok = true", () => {
  const r = mod.generate({ source: helloSource, filename: "hello.nemo", target: "python" });
  assert.strictEqual(r.ok, true);
});

check("generate python → artifact has expected files", () => {
  const r = mod.generate({ source: helloSource, filename: "hello.nemo", target: "python" });
  assert.ok(r.artifact, "expected artifact");
  assert.strictEqual(r.artifact.target, "python");
  assert.strictEqual(typeof r.artifact.packageName, "string");
  assert.strictEqual(typeof r.artifact.archiveRoot, "string");
  const paths = r.artifact.files.map((f) => f.path);
  assert(paths.includes("pyproject.toml"), `missing pyproject.toml in ${paths}`);
  // Python artifacts preserve the import-package directory.
  assert(paths.includes("hello_workflow/_agent.py"), `missing hello_workflow/_agent.py in ${paths}`);
  assert(paths.includes("hello_workflow/__init__.py"), `missing hello_workflow/__init__.py in ${paths}`);
  assert(paths.includes("hello_workflow/_manifest.py"), `missing hello_workflow/_manifest.py in ${paths}`);
  assert(paths.includes("hello_workflow/types.py"), `missing hello_workflow/types.py in ${paths}`);
  return `${r.artifact.packageName} (${r.artifact.files.length} files)`;
});

// ---------------------------------------------------------------------------
// generate — web (valid)
// ---------------------------------------------------------------------------

const hintTutorSource = readNemo("crates/nemoir-dsl-fe/tests/fixtures/hint_tutor.nemo");

check("generate web valid → ok = true", () => {
  const r = mod.generate({ source: hintTutorSource, filename: "hint_tutor.nemo", target: "web" });
  assert.strictEqual(r.ok, true);
});

check("generate web valid → has expected artifact files", () => {
  const r = mod.generate({ source: hintTutorSource, filename: "hint_tutor.nemo", target: "web" });
  assert.ok(r.artifact, "expected artifact");
  assert.strictEqual(r.artifact.target, "web");
  const paths = r.artifact.files.map((f) => f.path);
  assert(paths.includes("package.json"), `missing package.json in ${paths}`);
  assert(paths.includes("src/agent.ts"), `missing src/agent.ts in ${paths}`);
  return `${r.artifact.packageName} (${r.artifact.files.length} files)`;
});

// ---------------------------------------------------------------------------
// generate — web (incompatible)
// ---------------------------------------------------------------------------

const codingAgentSource = readNemo("crates/nemoir-dsl-fe/tests/fixtures/coding-agent.nemo");

check("generate web incompatible → ok = false", () => {
  const r = mod.generate({ source: codingAgentSource, filename: "coding_agent.nemo", target: "web" });
  assert.strictEqual(r.ok, false);
});

check("generate web incompatible → has target-phase diagnostics", () => {
  const r = mod.generate({ source: codingAgentSource, filename: "coding_agent.nemo", target: "web" });
  const targetDiags = r.diagnostics.filter((d) => d.phase === "target");
  assert(targetDiags.length > 0, "expected at least one target-phase diagnostic");
});

check("generate web incompatible → no artifact", () => {
  const r = mod.generate({ source: codingAgentSource, filename: "coding_agent.nemo", target: "web" });
  // skip_serializing_if omits the field when None
  assert.ok(!r.artifact, `expected no artifact, got ${r.artifact}`);
});

// ---------------------------------------------------------------------------
// generate — none
// ---------------------------------------------------------------------------

check("generate none → ok = true, no artifact", () => {
  const r = mod.generate({ source: helloSource, filename: "hello.nemo", target: "none" });
  assert.strictEqual(r.ok, true);
  // skip_serializing_if omits the field when None
  assert.ok(!r.artifact, `expected no artifact, got ${r.artifact}`);
});

// ---------------------------------------------------------------------------
// generate — visualizer
// ---------------------------------------------------------------------------

check("generate visualizer → single HTML file", () => {
  const r = mod.generate({ source: helloSource, filename: "hello.nemo", target: "visualizer" });
  assert.strictEqual(r.ok, true);
  assert.ok(r.artifact, "expected artifact");
  assert.strictEqual(r.artifact.target, "visualizer");
  assert.strictEqual(r.artifact.files.length, 1);
  assert.strictEqual(r.artifact.files[0].path, "index.html");
  assert(r.artifact.files[0].content.includes("<html"));
});

// ---------------------------------------------------------------------------
// summary
// ---------------------------------------------------------------------------

const failed = results.filter((r) => !r.pass);
console.log(`\n${results.length - failed.length}/${results.length} checks passed.`);
if (failed.length > 0) {
  console.log("Failures:");
  for (const f of failed) {
    console.log(`  - ${f.name}: ${f.detail}`);
  }
  process.exit(1);
}
