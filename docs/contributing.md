# Contributing

This workspace is a public research compiler pilot. Contributions should keep the compiler stack explicit, testable, and backend-neutral.

## Prerequisites

From the `compiler/` workspace root:

- install the pinned Rust toolchain from [`../rust-toolchain.toml`](../rust-toolchain.toml) with `rustup`
- have Python 3 available for the local Markdown link checker in [`../scripts/check_markdown_links.py`](../scripts/check_markdown_links.py)
- use Node.js/npm only when you need to inspect generated web artifacts locally

Start with [Getting started](getting-started.md) if you have not built the workspace before.

## Local checks

These are the main local checks used by CI in [`../.github/workflows/ci.yml`](../.github/workflows/ci.yml):

```bash
python3 scripts/check_markdown_links.py README.md docs examples
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For compiler-facing validation of the public examples, CI also runs `nemo check` on each example and compiles one representative artifact per backend.

## Public examples and fixtures

Public examples in [`../examples/`](../examples/) are part of the published compiler surface.

Expectations:

- keep examples small, self-contained, and free of private data, credentials, or unpublished paths
- prefer examples that demonstrate one or two public language features clearly
- if a public syntax or lowering behavior changes, update the affected example and its README
- do not replace public examples with private demos or internal-only workflows

Compiler fixtures in [`../crates/nemoir-dsl-fe/tests/fixtures/`](../crates/nemoir-dsl-fe/tests/fixtures/) are the regression corpus for frontend and lowering behavior.

When changing compiler semantics:

- add or update a positive fixture for new valid behavior
- add or update an invalid fixture and test when diagnostics or rejection rules change
- refresh golden IR files such as `*-ir.yml` when lowering output changes
- keep tests in sync with the public docs, especially [DSL and IR](dsl-and-ir.md)

## Documentation boundaries

Public compiler documentation lives in [`README.md`](README.md) and the pages under `docs/`.

Rules for documentation changes:

- keep shared compiler, DSL, IR, and target docs in `compiler/docs/`
- keep [`../README.md`](../README.md) short and link into `docs/`
- link to public runtime repositories and target guides instead of copying runtime package APIs into compiler docs
- do not add private master paths, private demos, credentials, or unpublished operational details
- do not make product, SLA, or security claims that are not supported by the source and tests

If you introduce a new public page, make sure its local Markdown links resolve and add it to [docs index](README.md).

## Change scope

Prefer narrow end-to-end changes:

- frontend changes should still lower to valid `WorkflowIr`
- IR changes should update every affected backend and fixture
- backend-specific restrictions should stay in backend crates unless they are truly backend-neutral

For extension-oriented work, see [Extending NemoIR](extending.md).
