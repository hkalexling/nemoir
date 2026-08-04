#!/usr/bin/env node

/**
 * Measure the release WASM bundle size and record a baseline.
 *
 * Uses Node's built-in `zlib` for deterministic, platform-independent
 * compression measurements (gzip + Brotli).  No system CLI tools needed.
 *
 * Usage:
 *   node scripts/measure-package-size.mjs [pkg-dir]
 */

import { createBrotliCompress, createGzip, constants } from "node:zlib";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname, join, basename } from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");
const pkgDir = resolve(process.argv[2] ?? join(rootDir, "pkg"));
const baselinePath = join(rootDir, "size-baseline.json");

// Find the .wasm file
const { readdirSync } = await import("node:fs");
const wasmFiles = readdirSync(pkgDir).filter((f) => f.endsWith(".wasm"));
if (wasmFiles.length === 0) {
  console.error("No .wasm file found in", pkgDir);
  process.exit(1);
}
if (wasmFiles.length > 1) {
  console.warn("Multiple .wasm files found, using first:", wasmFiles[0]);
}
const wasmPath = join(pkgDir, wasmFiles[0]);
const wasmBytes = readFileSync(wasmPath);

// -----------------------------------------------------------------------
// Measure
// -----------------------------------------------------------------------

async function compressSize(algo, bytes, options = {}) {
  const chunks = [];
  const source = Readable.from([bytes]);
  let stream;
  if (algo === "gzip") {
    stream = createGzip({ level: options.level ?? 9 });
  } else if (algo === "brotli") {
    stream = createBrotliCompress({
      params: {
        [constants.BROTLI_PARAM_QUALITY]: options.quality ?? 11,
      },
    });
  }
  await pipeline(source, stream, async function* (source) {
    for await (const chunk of source) chunks.push(chunk);
  });
  return Buffer.concat(chunks).length;
}

const gzipBytes = await compressSize("gzip", wasmBytes, { level: 9 });
const brotliBytes = await compressSize("brotli", wasmBytes, { quality: 11 });

const baseline = {
  measuredAt: new Date().toISOString(),
  nodeVersion: process.version,
  file: basename(wasmPath),
  rawBytes: wasmBytes.length,
  gzipLevel9Bytes: gzipBytes,
  brotliQuality11Bytes: brotliBytes,
};

// -----------------------------------------------------------------------
// Write
// -----------------------------------------------------------------------

writeFileSync(baselinePath, JSON.stringify(baseline, null, 2) + "\n");
console.log(`✅ Size baseline written to ${baselinePath}`);

// -----------------------------------------------------------------------
// Print
// -----------------------------------------------------------------------

const fmt = (n) => `${(n / 1024).toFixed(1)} KB (${n.toLocaleString()} bytes)`;

console.log(`   File:       ${baseline.file}`);
console.log(`   Raw:        ${fmt(baseline.rawBytes)}`);
console.log(`   Gzip (L9):  ${fmt(baseline.gzipLevel9Bytes)}`);
console.log(`   Brotli (11):${fmt(baseline.brotliQuality11Bytes)}`);
