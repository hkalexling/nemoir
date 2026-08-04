# NemoIR compiler docs

These docs cover the public compiler surface in this workspace: building or installing `nemo`, writing `.nemo` workflows, running `nemo check` and `nemo compile`, and understanding current backend compatibility.

NemoIR is a research compiler for structured agent workflows. This documentation describes the current compiler behavior in the public `compiler/` workspace. It does not present NemoIR as a production product or a security guarantee.

## Start here

- [Getting started](getting-started.md)
- [Writing workflows](writing-workflows.md)
- [CLI reference](cli.md)
- [DSL and IR reference](dsl-and-ir.md)
- [Python target guide](targets/python.md)
- [Web target guide](targets/web.md)
- [Safety and limitations](safety-and-limitations.md)
- [Compatibility](compatibility.md)
- [Troubleshooting](troubleshooting.md)
- [Extending NemoIR](extending.md)
- [Contributing](contributing.md)

## Canonical public examples

- [`../examples/hello-workflow/`](../examples/hello-workflow/) — the smallest end-to-end workflow and the best first compiler run.
- [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) — capability policies and a Python-oriented workflow.
- [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) — conditional transitions, optional data flow, and a web-compatible user interaction.

## Scope and ownership

- [`../README.md`](../README.md) stays short and points here for user-facing compiler documentation.
- `docs/` owns compiler-facing docs in this published workspace: workflow authoring, CLI behavior, validation stages, the normative DSL/IR reference, and backend target guides.
- Runtime package APIs should be documented in their own public runtime repositories or linked target-specific guides. This folder should link to those docs rather than duplicate package-level API references.
- New compiler reference pages can still be added alongside this index later, but the core public language and target guides already live here.
