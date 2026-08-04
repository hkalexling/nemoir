# Web target

The web target compiles a validated workflow into a static Vite/TypeScript browser application that runs on the public [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) package repository and the generated React runner from [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui).

See also: [DSL and IR](../dsl-and-ir.md), [Safety and limitations](../safety-and-limitations.md), and [Compatibility](../compatibility.md).

## Compile

From the compiler workspace:

```bash
cargo run --package nemoir-cli -- compile \
  path/to/workflow.nemo \
  --target web \
  --output /tmp/nemoir-web-out
```

`--output` points to the parent directory. The compiler then creates one app directory named from the workflow id. If you omit `--output`, the compiler writes next to the source file. When reading the workflow from stdin (`-`), `--output` is required.

## What gets emitted

For a workflow whose id lowers to `hint-tutor`, the output looks like this:

```text
/tmp/nemoir-web-out/
└── hint-tutor/
    ├── package.json
    ├── tsconfig.json
    ├── tsconfig.node.json
    ├── vite.config.ts
    ├── index.html
    ├── netlify.toml
    ├── vercel.json
    ├── public/_headers
    ├── src/agent.ts
    ├── src/workflow.json
    ├── src/main.tsx
    ├── src/app.css
    ├── src/webllm.worker.ts
    └── src/js.worker.ts   # only when the workflow uses browser.js.run
```

At a high level:

- `src/workflow.json` is the compiled workflow IR as inspectable JSON.
- `src/agent.ts` is the typed workflow facade.
- `src/main.tsx` is the generic React runner UI.
- `src/webllm.worker.ts` hosts local WebLLM model execution.
- `src/js.worker.ts` is emitted only for trusted `browser.js.run` stages.
- `vite.config.ts`, `netlify.toml`, `vercel.json`, and `public/_headers` carry the cross-origin-isolation setup the model path needs.

## Build, run, and deploy

Typical local flow:

```bash
cd /tmp/nemoir-web-out/hint-tutor
npm install
npm run dev
```

Build a production bundle with:

```bash
npm run build
```

You can preview the built app locally with:

```bash
npm run preview
```

Deploy `dist/` to a static host that serves the required COOP/COEP headers. The generated app includes `netlify.toml`, `vercel.json`, and `public/_headers` to help with that deployment step.

## Requirements

The generated app assumes:

- a working Node.js/npm environment for `npm install`, `npm run dev`, and `npm run build`
- a WebGPU-capable browser for workflows with model stages
- cross-origin isolation (COOP/COEP) for workflows with model stages, because WebLLM needs `SharedArrayBuffer`

From the generated app and runtime:

- model-stage workflows need WebGPU
- deterministic-only workflows do not need WebGPU, WebLLM, or a model adapter
- the Vite dev and preview servers set the isolation headers for local development
- production hosting must also set those headers

## Runtime and UI dependencies

Generated `package.json` files depend on:

- `@nemoir/web-runtime`: `^0.4.0`
- `@nemoir/web-ui`: `^0.2.0`

During development you can override either dependency at compile time:

```bash
cargo run --package nemoir-cli -- compile \
  path/to/workflow.nemo \
  --target web \
  --web-runtime-dependency file:../../web/nemoir-runtime \
  --web-ui-dependency file:../../web/nemoir-ui \
  --output /tmp/nemoir-web-out
```

Each override is independent.

## Model and tool responsibility

The generated app embeds the workflow, but it does not invent new browser capabilities beyond the shared runtime contract.

- Model stages require a `modelAdapter`; the generated web facade defaults to the runtime's tagged-envelope action protocol for this target.
- `user.elicit` and `user.confirm` are satisfied through a `uiHost`, unless you provide your own tools for those capabilities.
- Browser-native capabilities such as `http.fetch`, `browser.storage.*`, `browser.js.run`, and `browser.js.sandbox` come from the shared web runtime.

As on the Python target, the model is not the workflow runtime: policies, transitions, stage visibility, and output validation stay in NemoIR's runtime layer.

## Web capability contract and unsupported workflows

`nemo compile --target web` applies an extra compile-time compatibility check. In particular, the web target rejects workflows that require features the browser backend does not support.

Current compile-time restrictions include:

- no `fs.read`, `fs.write`, or `os.shell`
- no `path`-typed workflow inputs or stage writes
- `browser.js.run` and `browser.js.sandbox` are deterministic-stage-only; they cannot be exposed as model-stage capabilities
- `browser.js.run` requires its `code` argument to be a compile-time string literal
- `browser.js.sandbox` requires an explicit approval policy of the form `before browser.js.sandbox(code) requires user.confirm`, and `user.confirm` must be declared as a top-level capability

If the workflow violates the contract, compilation fails before any output app is written.

## Deterministic-only and dynamic-code safety boundaries

Two browser JavaScript capabilities have intentionally different trust boundaries:

- `browser.js.run` is for trusted workflow-author code only. It runs in a fresh same-origin Worker and is not an untrusted-code sandbox.
- `browser.js.sandbox` is the explicit dynamic-code path. It is deterministic-stage-only, never model-callable, requires the explicit `user.confirm` approval policy above, and runs in the runtime's isolated opaque-origin iframe-plus-Worker sandbox.

Even with that isolation, do not pass secrets or credentials to dynamic code. For broader limits and safety posture, see [Safety and limitations](../safety-and-limitations.md).

## Naming caveats

The web backend derives names from the workflow id:

- workflow id -> kebab-case app directory name
- workflow inputs and exit-stage output fields -> TypeScript property names in `src/agent.ts`

Compilation fails if the workflow id cannot be converted to a valid package directory name, or if generated TypeScript field names would be invalid.

## Public runtime sources

- [`hkalexling/nemoir-web-runtime`](https://github.com/hkalexling/nemoir-web-runtime)
- [`hkalexling/nemoir-web-ui`](https://github.com/hkalexling/nemoir-web-ui)
