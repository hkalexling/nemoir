# NemoIR

NemoIR is a **research compiler toolchain for structured agent workflows**.
It compiles the workflow—not the model—into a validated Agent Workflow IR that
can be inspected and lowered to multiple runtime targets.

> **Research-pilot status.** NemoIR is intended for experimentation,
> reproducible research, and lab prototypes. Its DSL, IR, and generated APIs
> may evolve; it is not presented as a production agent platform or a security
> guarantee.

```text
.nemo workflow → DSL frontend → Agent Workflow IR → validation → backends
                                                             ├─ HTML visualizer
                                                             ├─ Python package
                                                             └─ browser application
```

## Getting started

For user-facing compiler documentation, start with [docs/README.md](docs/README.md).

### Download the compiler

Prebuilt `nemo` binaries are available on the [GitHub Releases](https://github.com/hkalexling/nemoir/releases) page for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64).

### npm package

The browser-callable compiler facade is published as [`@nemoir/compiler-wasm`](https://www.npmjs.com/package/@nemoir/compiler-wasm) on npm. Its npm page is the canonical public source for package metadata and API details.

## Examples

Curated workflow examples live in [`examples/`](examples/):

- [`hello-workflow`](examples/hello-workflow/) — the smallest model-driven
  workflow and the recommended first compiler invocation.
- [`policy-gated-edit`](examples/policy-gated-edit/) — a Python-targeted
  coding workflow with explicit capability policies.
- [`web-hint-tutor`](examples/web-hint-tutor/) — a browser-compatible workflow
  that demonstrates conditional transitions and user elicitation.

## Documentation

- [Getting started](docs/getting-started.md)
- [DSL and IR reference](docs/dsl-and-ir.md)
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
