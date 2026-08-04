# Browser compiler

The NemoIR browser compiler is a static, browser-hosted authoring surface for
`.nemo` workflows. It uses a Monaco text editor and runs the existing NemoIR
compiler locally in a WebAssembly Worker; workflow source is not sent to a
server merely to compile it.

This is a research-pilot tool. It is a text DSL editor, not a graphical
workflow frontend, runtime host, package manager, or generated-code preview.
For the application itself, see
[`hkalexling/nemoir-web-compiler`](https://github.com/hkalexling/nemoir-web-compiler).

## Architecture and boundaries

Three browser-related components have distinct responsibilities:

| Component | Responsibility |
| --- | --- |
| [`nemoir-dsl-fe`](../crates/nemoir-dsl-fe/) | Parses `.nemo` source and lowers it to validated `WorkflowIr`. |
| [`nemoir-backend-web`](../crates/nemoir-backend-web/) | Generates a Vite/TypeScript workflow application and enforces web-target compatibility. |
| [`nemoir-wasm`](../crates/nemoir-wasm/) | Browser-callable adapter around the existing frontend, IR validator, and backends. |

The separate browser application owns Monaco, Worker messaging, editor state,
ZIP creation, downloads, and static-site deployment. It must not duplicate DSL
parsing, IR validation, target compatibility, or code generation in TypeScript.

```text
.nemo source
  -> browser Worker
  -> @nemoir/compiler-wasm
  -> DSL frontend -> validated WorkflowIr -> selected backend
  -> virtual text files
  -> browser ZIP / Blob download
```

The Agent Workflow IR remains the frontend/backend boundary. The WASM package
uses compiler library APIs directly; it does not invoke the native CLI or
expose browser DOM, filesystem, terminal, or process-exit behavior through the
compiler core.

## Browser-callable compiler API

[`@nemoir/compiler-wasm`](wasm-package.md) exposes three operations:

- `analyze(request)` parses, lowers, validates, and optionally returns
  serialized IR for editor feedback.
- `generate(request)` repeats the trusted source-to-IR pipeline and generates
  an artifact only after an explicit user action.
- `metadata()` reports the compiler version, IR version, and supported targets.

Expected compiler failures are structured response diagnostics rather than
terminal reports. Ranged DSL diagnostics use 1-based UTF-16 columns, matching
Monaco. Browser clients normalize source line endings to LF before compilation;
IR and target diagnostics may legitimately have no source range and should be
shown without inventing a marker location.

The public TypeScript contract ships as `api.d.ts` with the npm package. The
crate README is the API-oriented companion to this guide.

## Targets and downloaded artifacts

The browser compiler exposes the compiler's existing targets; it does not add
new target semantics.

| Selection | Browser result | Follow-up |
| --- | --- | --- |
| `none` | Validated IR JSON | Inspection/export only; it is not runnable. |
| `visualizer` | ZIP containing visualization HTML | The current visualization uses Cytoscape from a CDN. |
| `python` | ZIP containing Python package source | Install and run it outside the browser; see the [Python target guide](targets/python.md). |
| `web` | ZIP containing Vite/TypeScript application source | Install and build it outside the browser; see the [web target guide](targets/web.md). |

Python and web downloads are generated **source**, not built deployments. The
browser compiler does not run generated artifacts, install dependencies, build
wheels, run Vite, or execute workflow-author JavaScript.

The `web` backend remains authoritative for browser compatibility. A workflow
that needs unsupported filesystem capabilities, `path` values, or invalid
browser-JavaScript forms is rejected by the backend and the editor must show
that diagnostic rather than approximating the rule in UI code.

## Safety and trust boundaries

Compiling locally does not make a workflow or generated artifact safe. In
particular, generated source can intentionally contain trusted workflow-author
code such as `browser.js.run`; downloading it is not permission for the
compiler application to execute it. Follow [Safety and limitations](safety-and-limitations.md)
and the target-specific guidance before running generated artifacts.

ZIP assembly validates relative artifact paths both in the WASM facade and in
the browser application. This prevents archive-path confusion but is not a
substitute for reviewing generated source or applying runtime security policy.

## Development and release ownership

- The compiler repository owns the WASM facade, its package contract, package
  tests, and release workflow. See [WASM compiler package](wasm-package.md).
- The browser application repository owns its React/Vite/Monaco UI, Worker
  lifecycle, browser tests, CSP, and deployment instructions.
- DSL, IR, and generated-target semantics remain canonical in this `docs/`
  directory and its target guides.

When changing the WASM facade, preserve this separation: improve the shared
compiler library APIs when semantics need to change, and keep browser-only UX
logic in the separate application.
