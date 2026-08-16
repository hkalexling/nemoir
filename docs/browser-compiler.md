# Browser compiler

The NemoIR browser compiler is a browser-hosted editor for `.nemo` workflows and visual semantic documents. It uses the same compiler pipeline as the CLI, but runs it locally in the browser through the published `@nemoir/compiler-wasm` package rather than invoking a native `nemo` binary.

This is a research-pilot authoring surface, not a runtime host or a separate workflow language. The visual document path lowers directly into the same workflow IR as the DSL; see the [visual frontend](visual-frontend.md) guide.

## Authoring modes

The browser app offers a Monaco editor for `.nemo` source and a visual canvas
for a semantic document. Both compile locally in a Worker and feed the same
validated IR and target backends. The visual mode is a direct frontend: it does
not convert to `.nemo` source or construct raw IR in TypeScript. See the
[web compiler README](https://github.com/hkalexling/nemoir-web-compiler#editor-modes)
for current editor UX, including mode-switch and persistence boundaries.

## Public pages

- Browser editor application: [`hkalexling/nemoir-web-compiler`](https://github.com/hkalexling/nemoir-web-compiler)
- Browser compiler package: [`@nemoir/compiler-wasm`](https://www.npmjs.com/package/@nemoir/compiler-wasm)
- Compiler-emitted web target: [Web target guide](targets/web.md)
- Compiler-emitted Python target: [Python target guide](targets/python.md)
- Visual document contract: [Visual frontend](visual-frontend.md)

## What the browser compiler produces

The browser compiler exposes the same target choices as the main compiler surface:

| Selection | Result |
| --- | --- |
| `none` | Validated IR for inspection or export. |
| `visualizer` | A standalone workflow graph artifact. |
| `python` | Generated Python package source. |
| `web` | Generated browser-application source. |

For Python and web outputs, this repository documents only compiler behavior and emitted artifacts. Runtime, UI, and package API details live on their own public GitHub or npm pages.

## Documentation boundary

This page is intentionally high level. It does not duplicate the browser application's UI documentation or the `@nemoir/compiler-wasm` API reference.
