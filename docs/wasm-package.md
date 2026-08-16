# WASM compiler package

NemoIR publishes a browser-callable compiler package as [`@nemoir/compiler-wasm`](https://www.npmjs.com/package/@nemoir/compiler-wasm).

The package is used by the browser editor application for in-browser validation and artifact generation, for both `.nemo` source and visual semantic documents:

- Browser compiler app: [`hkalexling/nemoir-web-compiler`](https://github.com/hkalexling/nemoir-web-compiler)
- Browser compiler overview: [Browser compiler](browser-compiler.md)
- Visual document contract and the additive `analyzeVisual` / `generateVisual` / `visualMetadata` entry points: [Visual frontend](visual-frontend.md)

The package's public README and npm page are the canonical places for package metadata and API details. Compiler docs in this workspace intentionally do not duplicate the package API or maintainer build/release runbooks.
