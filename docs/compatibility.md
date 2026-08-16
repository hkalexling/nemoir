# Compatibility

This page records the current compiler-side compatibility contract.

## IR version

The current Agent Workflow IR version is `0.1`.

`nemo compile` lowers `.nemo` workflows to this IR and then validates that the result matches the expected IR structure and semantics. The [visual frontend](visual-frontend.md) lowers visual semantic documents into the same IR version and applies the same validation and target rules.

## Visual document schema

The released visual semantic-document schema is `0.1`. It is versioned
independently from the IR: programmatic consumers should obtain the accepted
schema version and capability catalogue from `visualMetadata()` rather than
assuming that an IR-version change implies a visual-schema change, or vice
versa.

## Target outputs

| Target | Generated output | Current compatibility notes |
| --- | --- | --- |
| `none` | No artifact | Useful for lowering and optional IR dumping only. |
| `visualizer` | Standalone HTML file | No separate NemoIR runtime package is required for the generated HTML artifact. |
| `python` | Python package | Generated `pyproject.toml` currently requires Python `>=3.11` and [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) `>=0.9.2`. |
| `web` | Vite/TypeScript app | Generated `package.json` currently uses [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) `^0.4.0` and [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui) `^0.2.0` by default. |

## Web target compatibility

The web backend is intentionally narrower than the IR.

It accepts workflows that stay within the browser-oriented capability set used by the current compiler:

- `user.elicit`
- `user.confirm`
- `http.fetch`
- `browser.storage.read`
- `browser.storage.write`
- `browser.js.run`
- `browser.js.sandbox`

It rejects workflows that require unsupported capabilities such as `fs.read`, `fs.write`, or `os.shell`, and it rejects `path`-typed workflow inputs and outputs.

`browser.js.run` and `browser.js.sandbox` are deterministic-stage-only capabilities on the web target. In addition, `browser.js.sandbox` requires an explicit approval policy of the form `before browser.js.sandbox(code) requires user.confirm`.

## Example-level guidance

The public examples reflect current target intent:

- [`../examples/hello-workflow/`](../examples/hello-workflow/) is suitable for `visualizer`, `python`, and `web`. It ships both `hello.nemo` and the equivalent `hello.visual.json` visual document.
- [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) is Python-oriented and not web-compatible because it uses `path`, `fs.read`, and `fs.write`.
- [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) is web-compatible and also serves as a good frontend validation example.

## Documentation boundary

This page records compiler-emitted dependency and compatibility facts. It does not duplicate runtime or package API documentation.
