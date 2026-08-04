#!/usr/bin/env node

/**
 * Finalize a `wasm-pack --target web` build into a publishable npm package.
 *
 * Usage:
 *   node scripts/finalize-package.mjs [pkg-dir]
 *
 * When `pkg-dir` is omitted, defaults to `pkg/` relative to this script.
 *
 * This script:
 *   1. Renames the npm package name from `@nemoir/nemoir-wasm` to
 *      `@nemoir/compiler-wasm`.
 *   2. Adds `publishConfig.access: "public"`.
 *   3. Replaces the wasm-bindgen generated `.d.ts` with the hand-authored
 *      `npm/api.d.ts` and points the `types` field at it.
 *   4. Adds ESM `exports` pointing at the generated JS and `.d.ts` files.
 *   5. Adds `api.d.ts`, `LICENSE`, and `README.md` to `files` so they
 *      are included when publishing.
 *   6. Ensures `keywords` are present.
 */

import { copyFileSync, readFileSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { resolve, dirname, basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(process.argv[2] ?? resolve(__dirname, "..", "pkg"));

const manifestPath = resolve(pkgDir, "package.json");
if (!existsSync(manifestPath)) {
  console.error(`package.json not found at ${manifestPath}`);
  process.exit(1);
}

const pkg = JSON.parse(readFileSync(manifestPath, "utf-8"));

// 1. Rename package from @nemoir/nemoir-wasm to @nemoir/compiler-wasm
if (pkg.name === "@nemoir/nemoir-wasm") {
  pkg.name = "@nemoir/compiler-wasm";
} else {
  console.warn(`Unexpected package name "${pkg.name}" — leaving unchanged`);
}

// 2. public access for scoped package
pkg.publishConfig = { access: "public" };

// 3. Replace wasm-bindgen's generated .d.ts with the hand-authored declarations
const npmDir = resolve(__dirname, "..", "npm");
const handAuthoredDts = join(npmDir, "api.d.ts");
const typedDtsName = "api.d.ts";

if (!existsSync(handAuthoredDts)) {
  console.error(`Hand-authored declarations not found at ${handAuthoredDts}`);
  process.exit(1);
}

// Remove the wasm-bindgen generated .d.ts so only the typed one ships
const generatedDts = resolve(pkgDir, basename(pkg.types));
if (existsSync(generatedDts)) {
  rmSync(generatedDts);
  console.log(`Removed generated .d.ts: ${generatedDts}`);
}

copyFileSync(handAuthoredDts, resolve(pkgDir, typedDtsName));
console.log(`Copied hand-authored ${handAuthoredDts} → ${resolve(pkgDir, typedDtsName)}`);

// Point "types" at the hand-authored declarations and remove the old .d.ts from files
pkg.types = typedDtsName;
const oldDtsFile = basename(generatedDts);
if (pkg.files) {
  pkg.files = pkg.files.filter((f) => f !== oldDtsFile);
}

// 4. ESM exports — derive filenames from the generated main/types fields
if (!pkg.main) {
  console.error("package.json is missing main field");
  process.exit(1);
}
const mainFile = basename(pkg.main);
pkg.exports = {
  ".": {
    types: `./${typedDtsName}`,
    import: `./${mainFile}`,
    default: `./${mainFile}`,
  },
};

// 5. Ensure LICENSE, README.md, and api.d.ts are in files
//    wasm-pack already puts the wasm/js/d.ts in files; we add docs.
const extraFiles = [];
for (const candidate of [typedDtsName, "LICENSE", "README.md"]) {
  if (existsSync(resolve(pkgDir, candidate))) {
    extraFiles.push(candidate);
  }
}
// Deduplicate
const existing = new Set(pkg.files ?? []);
for (const f of extraFiles) {
  if (!existing.has(f)) {
    pkg.files.push(f);
  }
}

// 6. Ensure keywords
if (!pkg.keywords || pkg.keywords.length === 0) {
  pkg.keywords = ["nemoir", "compiler", "wasm", "agent", "workflow"];
}

writeFileSync(manifestPath, JSON.stringify(pkg, null, 2) + "\n");
console.log(`Finalized ${manifestPath} → name: ${pkg.name}`);
