# @nemoir/compiler-wasm

Browser-callable NemoIR compiler as WebAssembly — the Worker-side bridge
between the Monaco DSL editor and the NemoIR compiler stack.

## What this package is

`@nemoir/compiler-wasm` wraps the NemoIR DSL frontend, IR validator, and
backend code generators in a single WebAssembly module. It exposes three
typed entry points:

- **`analyze(request)`** — parse, lower, and validate `.nemo` source; returns
  structured diagnostics and optionally the lowered IR.
- **`generate(request)`** — run the full compiler pipeline and invoke a
  selected backend to produce downloadable source artifacts (Python package,
  web app, visualizer HTML).
- **`metadata()`** — return compiler crate version, IR schema version, and the
  list of supported targets.

All responses are plain JavaScript objects (no `Map` instances), so they are
safe for `structuredClone`, `JSON.stringify`, and `postMessage`.

## Installation

```bash
npm install @nemoir/compiler-wasm
```

## Browser usage

```js
import init, { analyze, generate, metadata } from "@nemoir/compiler-wasm";

await init(); // fetches and instantiates the WebAssembly module

const result = analyze({
  source: 'workflow Hello { stage @entry A { prompt:"hi" } }',
  filename: "workflow.nemo",
  includeIr: true,
});

console.log(result.ok);          // true
console.log(result.diagnostics); // []
console.log(result.ir);          // the validated WorkflowIr as a plain object
```

The package uses `wasm-pack --target web`, so the `.wasm` asset is loaded
via `fetch`. For synchronous initialization (e.g. in a Worker with raw
bytes), use `initSync`:

```js
import { initSync, analyze } from "@nemoir/compiler-wasm";

const wasmBytes = await fetch("/nemoir_wasm_bg.wasm").then(r => r.arrayBuffer());
initSync(wasmBytes);
```

## Targets

| Target | Output |
|--------|--------|
| `none` | Validated IR JSON (no artifact file) |
| `visualizer` | Standalone workflow visualization HTML |
| `python` | Installable Python package source tree |
| `web` | Vite/TypeScript browser-app source tree |

## API types

All request and response types are documented in the companion TypeScript
declaration file. The canonical compiler semantics live in the
[NemoIR compiler docs](https://github.com/hkalexling/nemoir/blob/master/docs/),
including the [browser compiler guide](https://github.com/hkalexling/nemoir/blob/master/docs/browser-compiler.md).

## Local development

Build this package from the compiler checkout root (the directory containing
`Cargo.toml`):

```bash
rustup target add wasm32-unknown-unknown
wasm-pack build crates/nemoir-wasm --target web --release --scope nemoir
# The generated pkg/ directory is ready for local consumption.
```

For integrated development with the browser editor app, point its
`package.json` at the generated `pkg/` directory. In a NemoIR meta checkout,
this path is relative to `web/nemoir-compiler/package.json`:

```json
"@nemoir/compiler-wasm": "file:../../compiler/crates/nemoir-wasm/pkg"
```

See also [`web/nemoir-compiler/README.md`](https://github.com/hkalexling/nemoir-web-compiler) for the full browser-application
build instructions.

## License

MIT
