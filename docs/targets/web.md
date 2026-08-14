# Web target

The web target compiles a validated workflow into a generated Vite/TypeScript browser application. Running that application depends on the public [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) and [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui) projects.

This generated application is distinct from the browser-hosted NemoIR authoring editor. That editor invokes this backend through the WASM compiler and offers downloadable source artifacts. See [Browser compiler](../browser-compiler.md).

See also: [DSL and IR](../dsl-and-ir.md), [Safety and limitations](../safety-and-limitations.md), and [Compatibility](../compatibility.md).

## Compile

With `nemo` on your `PATH`:

```bash
nemo compile \
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
- `src/main.tsx` is the generated runner entry point.
- `src/webllm.worker.ts` is emitted for model-stage browser execution.
- `src/js.worker.ts` is emitted only for trusted `browser.js.run` stages.
- `package.json` declares the runtime and UI dependencies used by the generated app.

## Compiler-side dependency and naming rules

Generated `package.json` files depend on:

- `@nemoir/web-runtime`: `^0.4.0`
- `@nemoir/web-ui`: `^0.2.0`

During integrated development you can override either dependency string at compile time:

```bash
nemo compile \
  path/to/workflow.nemo \
  --target web \
  --web-runtime-dependency <spec> \
  --web-ui-dependency <spec> \
  --output /tmp/nemoir-web-out
```

The web backend derives names from the workflow id:

- workflow id -> kebab-case app directory name
- workflow inputs and exit-stage output fields -> TypeScript property names in `src/agent.ts`

Compilation fails if the workflow id cannot be converted to a valid package directory name, or if generated TypeScript field names would be invalid.

## Web compatibility checks

`nemo compile --target web` applies an extra compile-time compatibility check. In particular, the web target rejects workflows that require features the browser backend does not support.

Current compile-time restrictions include:

- no `fs.read`, `fs.write`, or `os.shell`
- no `path`-typed workflow inputs or stage writes
- `browser.js.run` and `browser.js.sandbox` are deterministic-stage-only; they cannot be exposed as model-stage capabilities
- `browser.js.run` requires its `code` argument to be a compile-time string literal
- `browser.js.sandbox` requires an explicit approval policy of the form `before browser.js.sandbox(code) requires user.confirm`, and `user.confirm` must be declared as a top-level capability

If the workflow violates the contract, compilation fails before any output app is written.

## Runtime docs

This compiler guide covers emitted app structure and compile-time checks only. Runtime behavior, browser execution details, and UI package APIs belong to the public [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) and [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui) pages.
