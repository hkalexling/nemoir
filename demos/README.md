# NemoIR Demos

Full applications built with NemoIR that compile **the workflow, not the model**. Each demo's `*.nemo` workflow lowers to the same Agent Workflow IR, is validated, and is emitted through a backend (`python` or `web`). Where `examples/` are single-file toys for learning the DSL, `demos/` have harnesses, policies, and auditable traces that showcase the research thesis: a formal, analyzable workflow boundary.

> All commands below assume `nemo` is on your `PATH` (prebuilt binaries in [Releases](https://github.com/hkalexling/nemoir/releases)) or `cargo run -p nemoir-cli --`. Notebooks contain **pre-rendered outputs** so you can see a complete run on GitHub without installing anything.

| Demo | Target | What it proves about the IR | Try it | Needs to re-run? |
|---|---|---|---|---|
| [`web-interview-tutor`](web-interview-tutor/) *(external)* | `web` | Browser-only capabilities (`browser.js.sandbox` with mandatory `before browser.js.sandbox(code) requires user.confirm`, `browser.js.run`, `browser.storage`), `user.elicit` Socratic loop, optional local WebLLM model — **source & CI live in** [`hkalexling/nemoir-web-interview-tutor`](https://github.com/hkalexling/nemoir-web-interview-tutor) | Live: [nemoir-web-interview-tutor.pages.dev](https://nemoir-web-interview-tutor.pages.dev/) · [hkalexling.github.io/nemoir-web-interview-tutor](https://hkalexling.github.io/nemoir-web-interview-tutor/) — or `git clone https://github.com/hkalexling/nemoir-web-interview-tutor && cd app && npm ci && npm run dev` | No — open the live demo; workflows in the standalone repo compile to `app/src/generated/` |
| [`xgboost-autoresearch`](xgboost-autoresearch/) | `python` | Declarative `candidate.json` sole model-writable file, handler-level read/write allowlists, `os.shell` literal allowlist, compiled numeric guard `score - best > eps` on paired selection/confirmation splits, frozen `final` split | `nemo check autoresearch.nemo` then `demo.ipynb` on GitHub | View `demo.ipynb` (5 figures, full trace) without data/API key; live run needs `harness/data.py prepare` + key |
| [`slm-autoresearch`](slm-autoresearch/) | `python` | Mutable `candidate.py` (LoRA + prompt contract) vs frozen `harness/*`, `before fs.write` guard, hill-climbing `score - best > eps`, answer-only SFT on GLUE/MNLI | `nemo check autoresearch.nemo` then `demo.ipynb` on GitHub | View `demo.ipynb` (25-trial log) without GPU; live run needs HF stack + key |

## `examples/` vs `demos/`

- `examples/*` — one `*.nemo` (<100 LOC), `nemo check` + `nemo compile --dump-ir` — no harness, no secrets.
- `demos/*` — executable research systems: `.nemo` + `harness/`, `run.py`, `requirements`, and a notebook with rendered outputs that doubles as documentation.

## Reproducing from source

Each demo's own `README.md` is the source of truth for setup. In general:

```bash
# Python demos (xgboost, slm)
nemo check demos/<name>/autoresearch.nemo
nemo compile demos/<name>/autoresearch.nemo --target python -o /tmp/<name>-pkg  # smoke
# then follow demos/<name>/README.md for `pip install` + `python run.py`

# Web demo — external (see web-interview-tutor/README.md)
# git clone https://github.com/hkalexling/nemoir-web-interview-tutor && cd nemoir-web-interview-tutor
# nemo check workflows/interview_tutor.nemo
# nemo check workflows/interview_test_runner.nemo
# app/src/generated/* are checked in so `npm run dev` works without a compiler checkout;
# to regenerate: NEMOIR_COMPILER_MANIFEST=/path/to/nemoir/compiler/Cargo.toml npm run compile:workflows
```
