# NemoIR

NemoIR is a **research compiler toolchain for structured agent workflows**.
It compiles the workflow—not the model—into a validated Agent Workflow IR that
can be inspected and lowered to multiple runtime targets.

> **Research-pilot status.** NemoIR is intended for experimentation,
> reproducible research, and lab prototypes. Its DSL, IR, and generated APIs
> may evolve; it is not presented as a production agent platform or a security
> guarantee.

For user-facing compiler documentation, start with [docs/README.md](docs/README.md).

```text
.nemo workflow → DSL frontend → Agent Workflow IR → validation → backends
                                                             ├─ HTML visualizer
                                                             ├─ Python package
                                                             └─ browser application
```

## What is here

This repository contains the Rust compiler workspace:

- `nemoir-ir` — Agent Workflow IR, capability catalog, and static validation.
- `nemoir-dsl-fe` — the `.nemo` parser, resolver, validator, and lowerer.
- `nemoir-cli` — the `nemo check` and `nemo compile` command-line interface.
- `nemoir-backend-visualizer` — standalone HTML workflow visualizations.
- `nemoir-backend-python` — generated Python workflow packages.
- `nemoir-backend-web` — generated Vite/TypeScript browser applications.

The generated Python and web targets use their respective NemoIR runtime
packages. The compiler remains backend-neutral: the validated IR is the
boundary between workflow authoring and runtime execution.

## Prerequisites

Rust is pinned in [`rust-toolchain.toml`](rust-toolchain.toml). Install it via
[rustup](https://rustup.rs/), then clone and build this repository:

```bash
git clone https://github.com/hkalexling/nemoir.git
cd nemoir
cargo build --release
```

The compiler binary is `target/release/nemo`. To install it into your Cargo
bin directory instead:

```bash
cargo install --path crates/nemoir-cli
```

## Quick start

Validate a workflow:

```bash
cargo run --package nemoir-cli -- check examples/hello-workflow/hello.nemo
```

Lower it to IR and render a standalone workflow graph:

```bash
cargo run --package nemoir-cli -- compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html \
  --dump-ir
```

The CLI supports these compilation targets:

| Target | Output |
| --- | --- |
| `none` | Validate and lower only; optionally print YAML IR with `--dump-ir`. |
| `visualizer` | A standalone HTML workflow graph. |
| `python` | An installable, typed Python workflow package. |
| `web` | A Vite/TypeScript browser application using the NemoIR web runtime. |

Run `nemo --help` or `nemo compile --help` for the complete CLI surface.

## Examples

The curated, public workflow examples live in [`examples/`](examples/):

- [`hello-workflow`](examples/hello-workflow/) — the smallest model-driven
  workflow and the recommended first compiler invocation.
- [`policy-gated-edit`](examples/policy-gated-edit/) — a Python-targeted
  coding workflow with explicit capability policies.
- [`web-hint-tutor`](examples/web-hint-tutor/) — a browser-compatible workflow
  that demonstrates conditional transitions and user elicitation.

Every example is checked by CI so its documented syntax stays valid.

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

NemoIR is released under the [MIT License](LICENSE).
