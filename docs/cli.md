# CLI reference

The compiler CLI exposes two public commands:

- `nemo check` for frontend validation
- `nemo compile` for lowering, IR validation, and target generation

Use `nemo --help` for the top-level help text and `nemo compile --help` for flag details.

## `nemo check`

```bash
nemo check <file>
```

`<file>` may be a filesystem path or `-` for stdin.

What it does:

1. parses the `.nemo` source;
2. resolves references; and
3. runs DSL validation.

On success it prints `OK: <file>`.

`nemo check` does not lower to IR, does not run IR validation, and does not generate any target artifact.

Example:

```bash
cargo run --package nemoir-cli -- check examples/hello-workflow/hello.nemo
```

## `nemo compile`

```bash
nemo compile <file> [--target <target>] [-o <path>] [--dump-ir]
```

`<file>` may be a filesystem path or `-` for stdin.

What it does:

1. parses the `.nemo` source;
2. resolves references;
3. runs DSL validation;
4. lowers the workflow to Agent Workflow IR;
5. runs full IR validation; and
6. generates the requested target artifact.

Supported targets:

| Target | Behavior |
| --- | --- |
| `none` | Default. Validate and lower only. With `--dump-ir`, emit YAML IR to stdout. |
| `visualizer` | Generate a standalone HTML workflow graph. |
| `python` | Generate an installable Python workflow package. |
| `web` | Generate a Vite/TypeScript browser app. |

### Output rules

- `--dump-ir` writes YAML IR to stdout before target generation.
- `--target none` generates no artifact. Without `--dump-ir`, the CLI reports that IR validation succeeded.
- `--target visualizer` writes to `--output` when provided; otherwise it writes `<input-stem>.html` in the current working directory.
- `--target python` and `--target web` write into `--output` when provided; otherwise they use the input file's parent directory. Each target creates its generated package directory inside that output directory.
- When the input is `-`, the `visualizer`, `python`, and `web` targets require `--output`.

### Web-only dependency overrides

For local development against unpublished runtime checkouts, `nemo compile --target web` also accepts:

- `--web-runtime-dependency <spec>`
- `--web-ui-dependency <spec>`

These override the dependency strings written into the generated `package.json`.

## Common commands

Validate a workflow:

```bash
cargo run --package nemoir-cli -- check examples/hello-workflow/hello.nemo
```

Print lowered IR without generating an artifact:

```bash
cargo run --package nemoir-cli -- compile \
  examples/hello-workflow/hello.nemo \
  --dump-ir
```

Generate a Python package:

```bash
cargo run --package nemoir-cli -- compile \
  examples/policy-gated-edit/policy-gated-edit.nemo \
  --target python \
  --output /tmp/nemoir-python-example
```

Generate a web app:

```bash
cargo run --package nemoir-cli -- compile \
  examples/web-hint-tutor/hint-tutor.nemo \
  --target web \
  --output /tmp/nemoir-web-example
```
