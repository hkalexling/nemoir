# Getting started

Run all commands below from the `compiler/` repository root.

## Build or install the CLI

Rust is pinned in [`../rust-toolchain.toml`](../rust-toolchain.toml).

```bash
cargo build --release
```

This produces `target/release/nemo`.

To install the CLI into your Cargo bin directory instead:

```bash
cargo install --path crates/nemoir-cli
```

## First workflow: validate the public hello example

```bash
cargo run --package nemoir-cli -- check examples/hello-workflow/hello.nemo
```

`nemo check` runs frontend validation only: parse, resolve, and DSL validation.

## Lower to IR and generate an artifact

Render the same workflow as a standalone HTML graph:

```bash
cargo run --package nemoir-cli -- compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html
```

`nemo compile` lowers the workflow to Agent Workflow IR, runs full IR validation, and then generates the requested target artifact.

If you only want the lowered IR, use the default `none` target with `--dump-ir`:

```bash
cargo run --package nemoir-cli -- compile \
  examples/hello-workflow/hello.nemo \
  --dump-ir > /tmp/hello-workflow.ir.yml
```

## Public examples to use next

| Example | What to try |
| --- | --- |
| [`../examples/hello-workflow/`](../examples/hello-workflow/) | `check`, `compile --target visualizer`, or `compile --target web` |
| [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) | `compile --target python` |
| [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) | `compile --target web` |

Compilation generates artifacts only. Running generated Python or web outputs additionally requires the matching NemoIR runtime packages described in [Compatibility](compatibility.md).
