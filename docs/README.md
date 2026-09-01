# NemoIR compiler docs

These docs cover the public compiler surface: using the installed `nemo` CLI, writing `.nemo` workflows or visual semantic documents, running `nemo check` and `nemo compile`, understanding current backend compatibility, and discovering the browser-related public packages.

NemoIR is a research compiler for structured agent workflows. This documentation describes the current public compiler behavior. It does not present NemoIR as a production product or a security guarantee.

## Start here

- [Getting started](getting-started.md)
- [Writing workflows](writing-workflows.md)
- [CLI reference](cli.md)
- [DSL and IR reference](dsl-and-ir.md)
- [Visual frontend](visual-frontend.md)
- [Python target guide](targets/python.md)
- [Web target guide](targets/web.md)
- [Browser compiler](browser-compiler.md)
- [WASM compiler package](wasm-package.md)
- [Safety and limitations](safety-and-limitations.md)
- [Compatibility](compatibility.md)
- [Troubleshooting](troubleshooting.md)
- [Extending NemoIR](extending.md)

## Canonical public examples

- [`../examples/hello-workflow/`](../examples/hello-workflow/) — the smallest end-to-end workflow and the best first compiler run.
- [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) — capability policies and a Python-oriented workflow.
- [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) — conditional transitions, optional data flow, and a web-compatible user interaction.

## Demos (full apps)

- [`../demos/web-interview-tutor/`](../demos/web-interview-tutor/) — browser interview tutor (`web` target, sandboxed evaluator, optional WebLLM).
- [`../demos/xgboost-autoresearch/`](../demos/xgboost-autoresearch/) — bounded Covertype XGBoost search (pre-rendered `demo.ipynb` + 5-figure dashboard).
- [`../demos/slm-autoresearch/`](../demos/slm-autoresearch/) — MNLI LoRA loop (pre-rendered 25-trial `demo.ipynb`).

`examples/` are toys for the DSL; `demos/` are executable systems with harnesses, policies, and notebook viewers. See [`../demos/README.md`](../demos/README.md).

## Scope and ownership

- [`../README.md`](../README.md) stays short and points here for user-facing compiler documentation.
- Public docs here assume the `nemo` binary is already available; they do not include source-build or release runbooks.
- `docs/` contains the published compiler documentation: workflow authoring (both the `.nemo` DSL and the visual document frontend), CLI behavior, validation stages, the normative DSL/IR reference, and backend target guides.
- Runtime package APIs should be documented in their own public repositories or npm pages. This folder links to those pages rather than duplicating package-level API references.
