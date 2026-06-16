# WASM compiler package

`@nemoir/compiler-wasm` is the browser-callable NemoIR compiler as
WebAssembly. It wraps the DSL frontend, IR validator, and backend code
generators in a single module consumed by the Monaco editor Worker.

This document is a **build and release guide** for maintainers. For the
browser architecture and ownership boundaries, see [Browser compiler](browser-compiler.md).
For the public package API, see the [crate README](../crates/nemoir-wasm/README.md).

## Quick build

Run these commands from the compiler checkout root (the directory containing
`Cargo.toml`):

```bash
rustup target add wasm32-unknown-unknown
wasm-pack build crates/nemoir-wasm --target web --release --scope nemoir
node crates/nemoir-wasm/scripts/finalize-package.mjs crates/nemoir-wasm/pkg
```

The generated `pkg/` directory is a ready-to-publish npm package.

## Package contents

| File | Purpose |
|------|---------|
| `nemoir_wasm_bg.wasm` | Compiled WebAssembly binary |
| `nemoir_wasm.js` | ESM glue (wasm-bindgen `--target web`) |
| `api.d.ts` | Hand-authored public TypeScript declarations |
| `README.md` | Package README (copied from crate) |
| `LICENSE` | MIT license (copied from workspace root) |
| `package.json` | Finalized npm manifest |

The finalizer replaces wasm-bindgen's generated declaration file with the
hand-authored [`npm/api.d.ts`](../crates/nemoir-wasm/npm/api.d.ts), updates the
`types` and ESM export fields, and publishes `api.d.ts` with the package.

## Local override for integrated development

During integrated development, point the browser editor application's
`package.json` at the generated `pkg/` directory. The path is relative to the
application's `package.json`; in a NemoIR meta checkout it is:

```json
"@nemoir/compiler-wasm": "file:../../compiler/crates/nemoir-wasm/pkg"
```

Re-run `wasm-pack build` and `finalize-package.mjs` after compiler changes,
then reinstall in the browser app. A standalone clone may need a different
relative path.

## Verifying the package

### Smoke test

```bash
node crates/nemoir-wasm/tests/package-smoke.mjs crates/nemoir-wasm/pkg
```

Runs `initSync`, `analyze`, `generate`, and `metadata` against real
workflow fixtures and asserts every response is a plain JavaScript object.

### Vite consumer fixture

```bash
node crates/nemoir-wasm/tests/run-vite-fixture.mjs crates/nemoir-wasm/pkg
```

Packs the package into a tarball, installs it into a standalone Vite
project, and runs `tsc --noEmit` + `vite build`. No Rust toolchain is
required for the consumer build — this is the proof that the package works
as a normal npm dependency.

For CI: after the `wasm` job builds and packs the package, the
`vite-consumer` job downloads the tarball artifact and runs:

```bash
node crates/nemoir-wasm/tests/run-vite-fixture.mjs --tarball <path-to-tgz>
```

## Bundle size

Current release baseline (recorded in
[`size-baseline.json`](../crates/nemoir-wasm/size-baseline.json)):

| Metric | Value |
|--------|-------|
| Raw `.wasm` | ~1.1 MB |
| Gzip (level 9) | ~390 KB |
| Brotli (quality 11) | ~283 KB |

Measure again after every compiler dependency change with:

```bash
node crates/nemoir-wasm/scripts/measure-package-size.mjs crates/nemoir-wasm/pkg
```

## Release workflow

Releases are fully automated via
[`.github/workflows/release.yml`](../.github/workflows/release.yml).
See [Releasing](releasing.md) for the canonical version source, trigger
rules, binary matrix, OIDC setup, and manual dry-run instructions.

### Automated checks (run in CI on every release)

The release workflow runs these checks before any publish step:

- [ ] All native compiler tests pass (`cargo test --workspace`).
- [ ] `cargo check -p nemoir-wasm --target wasm32-unknown-unknown` passes.
- [ ] `wasm-pack build` + finalizer succeed.
- [ ] Smoke test (24 checks) passes.
- [ ] Vite consumer fixture builds without Rust/Cargo.
- [ ] `npm pack --dry-run` shows expected files.
- [ ] Package version equals workspace Cargo version (asserted at build time).

### Local pre-release verification

Before bumping the workspace version, run these locally to catch
issues early:

```bash
# Rust checks
cargo test --workspace
cargo check -p nemoir-wasm --target wasm32-unknown-unknown

# WASM package path
wasm-pack build crates/nemoir-wasm --target web --release --scope nemoir
node crates/nemoir-wasm/scripts/finalize-package.mjs crates/nemoir-wasm/pkg
node crates/nemoir-wasm/tests/package-smoke.mjs crates/nemoir-wasm/pkg
node crates/nemoir-wasm/tests/run-vite-fixture.mjs crates/nemoir-wasm/pkg

# Size baseline (update before releasing)
node crates/nemoir-wasm/scripts/measure-package-size.mjs crates/nemoir-wasm/pkg
```

Never manually publish — the automated workflow is the only supported
path to npm and GitHub Releases.  It ensures the npm version, binary
archives and checksums are always consistent; public-source releases also receive
provenance attestations.

## Version compatibility

The `metadata()` function reports the compiler crate version and IR schema
version. The browser app should report these in its about/debug view.

When the IR schema version changes, previously exported workflow IR may become
incompatible. `metadata()` and each successful compiler response report the IR
version so browser clients can display or record the version that produced an
artifact.

## CI

See `.github/workflows/ci.yml` for the `wasm` and `vite-consumer` jobs.
