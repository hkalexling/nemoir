# NemoIR

NemoIR is a **research compiler toolchain for structured agent workflows**.
It compiles the workflow—not the model—into a validated Agent Workflow IR that
can be inspected and lowered to multiple runtime targets.

> **Research-pilot status.** NemoIR is intended for experimentation,
> reproducible research, and lab prototypes. Its authoring surfaces, IR, and
> generated APIs may evolve; it is not presented as a production agent platform
> or a security guarantee.

```text
.nemo workflow ────► DSL frontend ───────┐
                                         ├─► Agent Workflow IR ─► validation ─► backends
visual document ──► visual frontend ──────┘                                     ├─ HTML visualizer
                                                                                ├─ Python package
                                                                                └─ browser application
```

The `nemo` CLI accepts `.nemo` source. Visual semantic documents are consumed
by the browser compiler or the `@nemoir/compiler-wasm` API; see the
[visual frontend guide](docs/visual-frontend.md).

## Getting started

For user-facing compiler documentation, start with [docs/README.md](docs/README.md).

### Download the compiler

Prebuilt `nemo` binaries are available on the [GitHub Releases](https://github.com/hkalexling/nemoir/releases) page for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64).

### npm package

The browser-callable compiler facade is published as [`@nemoir/compiler-wasm`](https://www.npmjs.com/package/@nemoir/compiler-wasm) on npm. Its npm page is the canonical public source for package metadata and API details.

## Examples

Curated workflow examples live in [`examples/`](examples/):

- [`hello-workflow`](examples/hello-workflow/) — the smallest model-driven
  workflow and the recommended first compiler invocation. Ships both
  `hello.nemo` and the equivalent `hello.visual.json` visual document.
- [`policy-gated-edit`](examples/policy-gated-edit/) — a Python-targeted
  coding workflow with explicit capability policies.
- [`web-hint-tutor`](examples/web-hint-tutor/) — a browser-compatible workflow
  that demonstrates conditional transitions and user elicitation.

## Demos

Full applications built with NemoIR — see [`demos/`](demos/) for the curated set. Each demo ships a `*.nemo` workflow, its harness/runtime, and a pre-rendered notebook where applicable so you can view a complete run on GitHub without installing anything:

- [`web-interview-tutor`](demos/web-interview-tutor/) — browser-only interview tutor (`web` target, sandboxed evaluator, optional local WebLLM) — **live:** [nemoir-web-interview-tutor.pages.dev](https://nemoir-web-interview-tutor.pages.dev/) ([GitHub Pages fallback](https://hkalexling.github.io/nemoir-web-interview-tutor/))
- [`xgboost-autoresearch`](demos/xgboost-autoresearch/) — bounded Covertype XGBoost search (declarative `candidate.json`, 32-trial trace + 5-figure dashboard in `demo.ipynb`)
- [`slm-autoresearch`](demos/slm-autoresearch/) — MNLI LoRA post-training loop (`candidate.py` only writable, 25-trial `demo.ipynb`)

`examples/` are single-file toys for learning the DSL; `demos/` are executable research systems with policies and auditable traces.

## Documentation

- [Getting started](docs/getting-started.md)
- [DSL and IR reference](docs/dsl-and-ir.md)
- [Visual frontend](docs/visual-frontend.md)
- [Writing workflows](docs/writing-workflows.md)
- [CLI reference](docs/cli.md)
- [WASM package](docs/wasm-package.md)
- [Browser compiler](docs/browser-compiler.md)
- [Backend targets](docs/targets/)
- [Compatibility](docs/compatibility.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Extending the compiler](docs/extending.md)

## License

NemoIR is released under the [MIT License](LICENSE).
