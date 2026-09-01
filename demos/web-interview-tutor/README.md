# NemoIR Web Interview Tutor

A custom React/Vite, JavaScript-only coding-interview tutor built around
NemoIR's compile-first workflow design.

**Live demo:** [nemoir-web-interview-tutor.pages.dev](https://nemoir-web-interview-tutor.pages.dev/) (Cloudflare Pages — full app, including local WebLLM) · [hkalexling.github.io/nemoir-web-interview-tutor](https://hkalexling.github.io/nemoir-web-interview-tutor/) (GitHub Pages — deterministic runner only)

> **Deploy source:** GitHub Actions live in the standalone repo [`hkalexling/nemoir-web-interview-tutor`](https://github.com/hkalexling/nemoir-web-interview-tutor). This copy at `public/demos/web-interview-tutor/` is the same source snapshot but is **source-only** — it intentionally has no `.github/workflows` (GitHub does not run workflows from subdirectories). See [Deployment](#deployment-standalone-repo) below.

## Phase 3 status

The deterministic learning path remains fully local and is now paired with an
optional, evidence-backed tutoring path:

- Eight curated, declarative interview problems with public JSON-compatible
  tests cover arrays/hash maps, two pointers, stacks, binary search, intervals,
  trees, BFS, and DFS.
- Learners select a problem, edit a JavaScript function in Monaco, and run its
  visible tests through the generated `InterviewTestRunner` facade.
- The workflow evaluates learner code only through `browser.js.sandbox`, with
  the mandatory inspectable policy:

  ```nemo
  before browser.js.sandbox(code) requires user.confirm
  ```

- Before every run, the UI shows the learner's source snapshot, public test
  bundle, execution limits, and the reviewed literal evaluator harness.
- Results distinguish passing tests, assertion failures, syntax errors,
  runtime errors, outer sandbox timeout, cancellation, policy denial, and
  operational failures. Editing code after a run marks its results stale.
- **Run Tests** remains deterministic: it does not require WebGPU, a model
  download, or model inference.
- Learners can explicitly enable a local WebLLM session, assess storage and
  device fit, choose/load a model, and request a fresh `InterviewTutor`
  workflow run from the exact source/report snapshot. Load diagnostics offer
  fresh-worker and clean-download recovery without touching other model caches.
- Model cache controls include per-model delete and **Reset all cached models** (via the wasm-backed WebLLM cache — `@nemoir/web-runtime` 0.5.0 / `@nemoir/web-ui` 0.3.0, `@nemoir/compiler-wasm` 0.1.7) to free browser storage.
- The tutor workflow deterministically validates and flattens its bounded
  request, loads a compact workflow-owned profile, diagnoses the deterministic
  evidence, optionally asks one clarification question through `user.elicit`,
  normalizes/saves only pedagogical topic data, then returns a typed Socratic
  hint or passing-code review.
- Tool-less model stages use grammar-constrained JSON with tolerant repair as a
  fallback. Semantic learner-safety checks run inside the model retry loop and
  again before display; raw operational JSON is never rendered directly.
- Guidance has `nudge`, `targeted`, and `plan` tiers; passing reports receive
  complexity/robustness review instead. A temperature slider (0.00–1.00,
  default 0.70) controls sampling variation without relaxing the structured
  output contract.
- The feedback pane keeps user-initiated JSONL trace export. The full collapsed
  workflow inspector and durable learner history remain Phase 4 work.

Tests are deliberately public/pedagogical. This static app does not claim to
hide them.

## Development

Prerequisites for the checked-in static demo: Node.js/npm.

```bash
git clone https://github.com/hkalexling/nemoir.git
cd nemoir/public/demos/web-interview-tutor/app
npm ci
npm run dev
```

> This copy lives in the public `nemoir` repository at `public/demos/web-interview-tutor/`. The standalone mirror at [`hkalexling/nemoir-web-interview-tutor`](https://github.com/hkalexling/nemoir-web-interview-tutor) remains available but this directory is the canonical public demo.

The generated agent facades and workers required by the static app are checked
in as reviewed compiler output, so ordinary development and the Pages build do
not require private compiler access.

To change a `.nemo` workflow, install Rust/Cargo and point the regeneration
script at a NemoIR compiler checkout:

```bash
NEMOIR_COMPILER_MANIFEST=/path/to/nemoir/compiler/Cargo.toml \
  npm run compile:workflows
npm run typecheck
npm test
npm run build
```

For a Pages-equivalent build that uses the checked-in generated artifacts:

```bash
npm run build:pages
```

For actual sandbox execution, use a modern browser. The configured COOP/COEP
headers remain in place for the optional WebLLM path, but Run Tests itself works
without WebGPU. A learner must approve every sandbox run; approval is not
cached.

By default, models use WebLLM's upstream records only. A deployer may configure
an audited private/institutional source profile in
`app/src/nemoir/model-sources.ts`; source-specific IDs prevent caches from
mixing artifacts from different origins. Public mirror/provider integration is
not enabled.

## Generated-artifact policy

The custom app needs only a small generated subset: typed agent facades,
workflow manifests, and worker entry points. This directory (and the standalone mirror) checks
that subset into `app/src/generated/` so builds can run without access
to the private compiler repository. The complete generic generated shells stay
ignored.

Never edit generated files. Change a `.nemo` workflow, then run
`NEMOIR_COMPILER_MANIFEST=/path/to/nemoir/compiler/Cargo.toml npm run
compile:workflows`, review the generated-artifact diff, and commit it with the
workflow source. This demo consumes the published `@nemoir/web-runtime` and
`@nemoir/web-ui` packages; use explicit compiler dependency overrides locally
when testing unreleased package changes.

## Deployment (standalone repo)

This snapshot at `public/demos/web-interview-tutor/` has **no** `.github/workflows` — GitHub Actions do not run from subdirectories. The live deployments are built from the standalone mirror [`hkalexling/nemoir-web-interview-tutor`](https://github.com/hkalexling/nemoir-web-interview-tutor):

- **GitHub Pages** (deterministic runner only): `.github/workflows/deploy-pages.yml` in the standalone repo builds `app/dist` on every push to `main`, sets `VITE_BASE_PATH` via `actions/configure-pages`, and deploys via `actions/deploy-pages`. No private compiler credential needed (uses checked-in `app/src/generated/`).

  > **WebLLM limitation:** GitHub Pages cannot set COOP/COEP headers, so the local WebLLM tutor shows its normal unavailable state there.

- **Cloudflare Pages** (full app, including WebLLM): `.github/workflows/deploy-cloudflare-pages.yml` in the standalone repo builds `app/dist` and deploys with `cloudflare/wrangler-action` (`pages deploy app/dist --project-name=nemoir-web-interview-tutor`). Unlike Pages, Cloudflare serves `app/public/_headers`, which provides the COOP/COEP headers WebLLM needs.

To keep this public snapshot in sync with the standalone deploy repo:

```bash
# from the meta-repo root
rsync -a --delete --exclude=.github --exclude=node_modules --exclude=dist --exclude=.vite \
  demos/web-interview-tutor/ public/demos/web-interview-tutor/
```

The workflows themselves are not copied here; see the standalone repo's `.github/workflows/` for the canonical CI.

## Safety boundary

Learner source is an input to a reviewed literal evaluator harness and runs
only inside the opaque-origin `browser.js.sandbox` iframe/worker boundary.
The host app, Monaco editor, and model workflow never evaluate it. The outer
sandbox enforces time and JSON-size limits; an uninterruptible loop is mapped
to a learner-facing timeout result rather than treated as a normal harness
report.
