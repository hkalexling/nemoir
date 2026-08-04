#!/usr/bin/env node

/**
 * Vite consumer fixture test — proves that a standalone Vite project can
 * consume `@nemoir/compiler-wasm` from its packaged tarball without any
 * Rust toolchain, Cargo checkout, or sibling-path dependency.
 *
 * Usage:
 *   node tests/run-vite-fixture.mjs [pkg-dir]
 *
 * Steps:
 *   1. `npm pack` the finalized pkg/ into a tarball.
 *   2. Copy the fixture to a temp directory.
 *   3. Place the tarball under `local-packages/`.
 *   4. `npm install` the fixture.
 *   5. `npx tsc --noEmit` (typecheck).
 *   6. `npx vite build` (production build).
 *   7. Assert `dist/assets/` contains one `.wasm` file.
 */

import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { resolve, dirname, join, basename } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const fixtureDir = join(__dirname, "vite-fixture");

// Accept either a pkg-dir (default) or an explicit --tarball path.
let pkgDir = null;
let prebuiltTarball = null;
for (let i = 2; i < process.argv.length; i++) {
  if (process.argv[i] === "--tarball" && i + 1 < process.argv.length) {
    prebuiltTarball = resolve(process.argv[++i]);
  } else if (!pkgDir) {
    pkgDir = resolve(process.argv[i]);
  }
}
if (!pkgDir && !prebuiltTarball) {
  pkgDir = join(__dirname, "..", "pkg");
}

// -----------------------------------------------------------------------
// helpers
// -----------------------------------------------------------------------

function sh(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
  if (r.status !== 0) {
    const msg = `${cmd} ${args.join(" ")} exited with ${r.status}`;
    if (r.error) throw r.error;
    throw new Error(msg);
  }
}

function mkdtemp(prefix) {
  const dir =
    prefix + "-" + process.pid + "-" + Math.random().toString(36).slice(2, 8);
  mkdirSync(dir, { recursive: true });
  return dir;
}

// -----------------------------------------------------------------------
// 1. Pack the pkg into a tarball
// -----------------------------------------------------------------------

// ---- 1. Obtain tarball ----

const tmp = mkdtemp(join(tmpdir(), "nemoir-vite-fixture-"));
let tarballName;
let tarballSourcePath;

if (prebuiltTarball) {
  console.log("📦 Using prebuilt tarball:", prebuiltTarball);
  if (!existsSync(prebuiltTarball)) {
    console.error("prebuilt tarball not found:", prebuiltTarball);
    process.exit(1);
  }
  tarballName = basename(prebuiltTarball);
  tarballSourcePath = prebuiltTarball;
} else {
  console.log("📦 Packing", pkgDir);
  if (!existsSync(resolve(pkgDir, "package.json"))) {
    console.error("package.json not found in", pkgDir);
    process.exit(1);
  }

  const tarballsDir = join(tmp, "tarballs");
  mkdirSync(tarballsDir, { recursive: true });

  try {
    sh("npm", ["pack", "--pack-destination", tarballsDir], { cwd: pkgDir });
  } catch {
    console.error("npm pack failed");
    process.exit(1);
  }

  const tarballs = readdirSync(tarballsDir).filter((f) => f.endsWith(".tgz"));
  if (tarballs.length !== 1) {
    console.error("Expected exactly 1 tarball, got:", tarballs);
    process.exit(1);
  }
  tarballName = tarballs[0];
  tarballSourcePath = join(tarballsDir, tarballName);
}
console.log("   tarball:", tarballName, `(${statSync(tarballSourcePath).size} bytes)`);

// -----------------------------------------------------------------------
// 2–3. Set up fixture in temp workspace
// -----------------------------------------------------------------------

const ws = join(tmp, "fixture");
cpSync(fixtureDir, ws, { recursive: true });

// Replace the dependency entry with the actual tarball path.
const localPkgsDir = join(ws, "local-packages");
mkdirSync(localPkgsDir, { recursive: true });
cpSync(tarballSourcePath, join(localPkgsDir, tarballName));

const fixturePkgPath = join(ws, "package.json");
const fixturePkg = JSON.parse(readFileSync(fixturePkgPath, "utf-8"));
fixturePkg.dependencies["@nemoir/compiler-wasm"] =
  `file:./local-packages/${tarballName}`;
writeFileSync(fixturePkgPath, JSON.stringify(fixturePkg, null, 2) + "\n");

// -----------------------------------------------------------------------
// 4. Install
// -----------------------------------------------------------------------

console.log("📥 Installing fixture dependencies...");
const nodeBin = dirname(process.execPath);
const pathWithBin = [nodeBin, join(ws, "node_modules", ".bin"), process.env.PATH].join(":");
sh("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund"], {
  cwd: ws,
  env: { ...process.env, PATH: pathWithBin },
});

// -----------------------------------------------------------------------
// 5. Typecheck
// -----------------------------------------------------------------------

console.log("🔍 Typechecking...");
sh("npx", ["tsc", "--noEmit"], {
  cwd: ws,
  env: { ...process.env, PATH: pathWithBin },
});

// -----------------------------------------------------------------------
// 6. Build
// -----------------------------------------------------------------------

console.log("🏗️  Building with Vite...");
sh("npx", ["vite", "build"], {
  cwd: ws,
  env: { ...process.env, PATH: pathWithBin },
});

// -----------------------------------------------------------------------
// 7. Assertions
// -----------------------------------------------------------------------

const distDir = join(ws, "dist");
if (!existsSync(distDir)) {
  console.error("dist/ directory not found after build");
  process.exit(1);
}

const assetsDir = join(distDir, "assets");
if (!existsSync(assetsDir)) {
  console.error("dist/assets/ directory not found");
  process.exit(1);
}

const wasmFiles = readdirSync(assetsDir).filter((f) => f.endsWith(".wasm"));
if (wasmFiles.length === 0) {
  console.error("No .wasm file in dist/assets/");
  process.exit(1);
}
console.log(
  `   ✅ WASM asset emitted: ${wasmFiles[0]} (${statSync(join(assetsDir, wasmFiles[0])).size} bytes)`,
);

const jsFiles = readdirSync(assetsDir).filter((f) => f.endsWith(".js"));
if (jsFiles.length === 0) {
  console.error("No .js file in dist/assets/ (bundle missing)");
  process.exit(1);
}
console.log(
  `   ✅ JS bundle emitted: ${jsFiles[0]} (${statSync(join(assetsDir, jsFiles[0])).size} bytes)`,
);

if (!existsSync(join(distDir, "index.html"))) {
  console.error("dist/index.html not found");
  process.exit(1);
}

console.log("\n✅ Vite consumer fixture test PASSED");
console.log("   No Cargo, Rust, or compiler source was required.");

// Cleanup
rmSync(tmp, { recursive: true, force: true });
