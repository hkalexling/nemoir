# Hello workflow

A minimal model-driven workflow with a typed input, an entry stage, and an exit stage. It ships in two authoring forms that lower to the same validated workflow IR:

- [`hello.nemo`](hello.nemo) — the `.nemo` DSL source, compiled with the CLI.
- [`hello.visual.json`](hello.visual.json) — the same workflow as a visual semantic document, consumed by the browser compiler's visual WASM API.

## CLI (`.nemo` source)

With `nemo` on your `PATH`, run these commands from the repository root.

```bash
nemo check examples/hello-workflow/hello.nemo
nemo compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html
```

Open the generated HTML file in a browser to inspect the lowered workflow graph. The same source can also be compiled to the Python or web targets.

## Visual document / WASM API

`hello.visual.json` is the canonical tested public visual sample. It is the
same workflow expressed as a schema `0.1` visual semantic document: the entry
state `Compose` reads the `message` input over a data edge, and its `greeting`
output flows to the exit state `Done` over another data edge, with one explicit
`always` control edge.

It is not compiled with the `nemo` CLI. The browser compiler consumes the
parsed document through the additive `@nemoir/compiler-wasm` entry points:

- `analyzeVisual({ document, filename })` — validate, lower, and optionally
  return the shared workflow IR.
- `generateVisual({ document, target })` — re-run analysis and dispatch to a
  backend (`visualizer`, `python`, or `web`).
- `visualMetadata()` — return the visual schema version and capability
  catalogue.

See the [visual frontend guide](../../docs/visual-frontend.md) for the document
contract and the [WASM package page](../../docs/wasm-package.md) for the package
API.
