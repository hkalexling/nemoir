#!/usr/bin/env node

/**
 * Compile the demo's two workflow sources into disposable, read-only inputs
 * for the custom Vite application. This script intentionally never patches
 * generated files.
 *
 * The generated package.json files reference the published npm versions of
 * @nemoir/web-runtime (^0.4.0) and @nemoir/web-ui (^0.2.0) via the compiler's
 * defaults. Set NEMOIR_COMPILER_MANIFEST to a NemoIR checkout's Cargo.toml
 * when regenerating this standalone repository outside the meta repository.
 */
import { spawnSync } from "node:child_process";
import { access, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const appDirectory = path.resolve(scriptDirectory, "..");
const demoDirectory = path.resolve(appDirectory, "..");
// When hosted inside public/demos/web-interview-tutor, the meta-repo root is
// three levels above the demo (public/demos/web-interview-tutor -> public -> repo root).
// Fall back through both layouts so the script works in the standalone mirror
// and in the public nemoir checkout.
const candidates = [
  path.join(demoDirectory, "compiler", "Cargo.toml"), // standalone mirror layout (demo is repo root)
  path.resolve(demoDirectory, "..", "..", "..", "compiler", "Cargo.toml"), // public/demos/web-interview-tutor -> repo root
  path.resolve(demoDirectory, "..", "..", "compiler", "Cargo.toml"), // fallback
];
import { existsSync as _existsSync } from "node:fs";
const bundledCompilerManifest = candidates.find((p) => _existsSync(p)) ?? candidates[0];
const compilerManifest = process.env.NEMOIR_COMPILER_MANIFEST ?? bundledCompilerManifest;
const generatedDirectory = path.join(appDirectory, "src", "generated");

const workflows = [
  {
    source: path.join(demoDirectory, "workflows", "interview_test_runner.nemo"),
    packageDirectory: "interview-test-runner",
    requiresWebLlmWorker: false,
    requiresJsRunWorker: false,
  },
  {
    source: path.join(demoDirectory, "workflows", "interview_tutor.nemo"),
    packageDirectory: "interview-tutor",
    requiresWebLlmWorker: true,
    requiresJsRunWorker: true,
  },
];

async function requireFile(filePath, label) {
  try {
    await access(filePath);
  } catch {
    throw new Error(`${label} is missing: ${filePath}`);
  }
}

function compileWorkflow(workflow) {
  const args = [
    "run",
    "--quiet",
    "--manifest-path",
    compilerManifest,
    "-p",
    "nemoir-cli",
    "--",
    "compile",
    workflow.source,
    "--target",
    "web",
    "--output",
    generatedDirectory,
  ];

  console.log(`Compiling ${path.basename(workflow.source)} -> ${workflow.packageDirectory}/`);
  const result = spawnSync("cargo", args, {
    cwd: appDirectory,
    stdio: "inherit",
  });

  if (result.error) {
    throw new Error(`Unable to run cargo: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`Workflow compilation failed: ${path.basename(workflow.source)}`);
  }
}

async function verifyArtifacts(workflow) {
  const sourceDirectory = path.join(generatedDirectory, workflow.packageDirectory, "src");
  const requiredFiles = ["agent.ts", "workflow.json"];
  if (workflow.requiresWebLlmWorker) requiredFiles.push("webllm.worker.ts");
  if (workflow.requiresJsRunWorker) requiredFiles.push("js.worker.ts");

  for (const name of requiredFiles) {
    await requireFile(path.join(sourceDirectory, name), `Generated ${name}`);
  }
}

async function main() {
  await requireFile(
    compilerManifest,
    "NemoIR compiler manifest (set NEMOIR_COMPILER_MANIFEST to a checkout's compiler/Cargo.toml)",
  );
  for (const workflow of workflows) {
    await requireFile(workflow.source, "Workflow source");
  }

  await mkdir(generatedDirectory, { recursive: true });
  // Remove all targeted packages first so a failed compile cannot leave a
  // previously generated facade available to a later typecheck/build.
  await Promise.all(
    workflows.map((workflow) =>
      rm(path.join(generatedDirectory, workflow.packageDirectory), {
        recursive: true,
        force: true,
      }),
    ),
  );

  for (const workflow of workflows) {
    compileWorkflow(workflow);
    await verifyArtifacts(workflow);
  }

  console.log("Generated workflow artifacts are ready.");
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
