# Extending NemoIR

Status: the compiler crates in `compiler/` are experimental research interfaces. Treat the validated Agent Workflow IR as the stable boundary inside this workspace, and keep frontend, IR, and backend changes separated as much as possible.

## Workspace roles

| Crate | Role |
| --- | --- |
| [`../crates/nemoir-ir/`](../crates/nemoir-ir/) | Core IR types, capability catalog, and backend-neutral IR validation. |
| [`../crates/nemoir-dsl-fe/`](../crates/nemoir-dsl-fe/) | `.nemo` parsing, name resolution, DSL validation, transition inference, and lowering to IR. |
| [`../crates/nemoir-cli/`](../crates/nemoir-cli/) | Public `nemo check` and `nemo compile` entry points. |
| [`../crates/nemoir-wasm/`](../crates/nemoir-wasm/) | Browser-callable WASM facade around the existing frontend, IR validator, and backends. |
| [`../crates/nemoir-backend-visualizer/`](../crates/nemoir-backend-visualizer/) | Standalone HTML graph emission from validated IR. |
| [`../crates/nemoir-backend-python/`](../crates/nemoir-backend-python/) | Python package generation from validated IR. |
| [`../crates/nemoir-backend-web/`](../crates/nemoir-backend-web/) | Web-app generation plus web-specific compatibility validation. |

Useful source entry points:

- [`../crates/nemoir-dsl-fe/src/lib.rs`](../crates/nemoir-dsl-fe/src/lib.rs)
- [`../crates/nemoir-ir/src/lib.rs`](../crates/nemoir-ir/src/lib.rs)
- [`../crates/nemoir-cli/src/main.rs`](../crates/nemoir-cli/src/main.rs)
- [`../crates/nemoir-wasm/src/lib.rs`](../crates/nemoir-wasm/src/lib.rs)

## Browser distribution facade

`nemoir-wasm` is a distribution adapter, not a new language frontend or backend.
It must call the existing library APIs, preserve the IR boundary, and keep
browser-only UI, Worker, DOM, and ZIP behavior in the separate browser
application. See [Browser compiler](browser-compiler.md).

## Safe extension principles

- Preserve the IR as the boundary between authoring and runtime targets.
- Prefer backend-neutral semantics in `nemoir-ir` and backend-specific restrictions inside backend crates.
- Reuse the IR validator instead of duplicating structural checks in multiple places.
- Update public examples and fixtures when a public language or target behavior changes.
- Keep compiler docs in [`README.md`](README.md) and the pages it links, not in private notes or target runtime repos.

## Adding a new frontend

A new frontend should lower into `nemoir_ir::WorkflowIr`, not directly into a target runtime.

Safe path:

1. Create a new crate under [`../crates/`](../crates/) that owns parsing/import logic for the new authoring surface.
2. Resolve names and frontend-local invariants before lowering.
3. Lower into `WorkflowIr` and run [`../crates/nemoir-ir/src/validate.rs`](../crates/nemoir-ir/src/validate.rs).
4. Add focused fixtures and tests near the new frontend crate.
5. Only after the frontend can produce valid IR should you expose it through the CLI or a backend.

If a rule is specific to one authoring surface, keep it in that frontend. If it must hold for every frontend, move it into IR validation instead.

## Adding a new backend

A backend should consume validated IR and keep target-specific compatibility checks local.

Safe path:

1. Add a backend crate under [`../crates/`](../crates/).
2. Accept `&nemoir_ir::WorkflowIr` as input.
3. Re-run IR validation defensively, as the existing backends do.
4. If the target only supports part of the IR, add a backend validator similar to [`../crates/nemoir-backend-web/src/validate_web.rs`](../crates/nemoir-backend-web/src/validate_web.rs).
5. Emit files or artifacts without changing IR semantics.
6. Wire the backend into [`../crates/nemoir-cli/src/main.rs`](../crates/nemoir-cli/src/main.rs) only after codegen and tests are stable.

Do not push target-specific assumptions into `nemoir-ir` unless every backend must share them.

## Adding or changing a capability

Capabilities are part of the shared compiler contract, so change them carefully.

Safe path:

1. Update the catalog in [`../crates/nemoir-ir/src/capabilities.rs`](../crates/nemoir-ir/src/capabilities.rs).
2. Update DSL/frontend checks for `requires:`, `exec:`, and policy trigger/require validation in [`../crates/nemoir-dsl-fe/src/validate.rs`](../crates/nemoir-dsl-fe/src/validate.rs).
3. Update IR validation if new structural guarantees are required.
4. Update backend-specific compatibility checks, especially the web target if the capability is not browser-safe.
5. Add positive and negative tests.
6. Update [DSL and IR](dsl-and-ir.md), [Compatibility](compatibility.md), and any affected target guide.

A capability should describe workflow-visible behavior, not one backend's private runtime helper.

## Changing the language or IR

This is the riskiest class of change because it can affect every frontend, every backend, and the public docs.

### DSL-only change

If the IR shape stays the same:

1. Update grammar and AST handling in [`../crates/nemoir-dsl-fe/src/`](../crates/nemoir-dsl-fe/src/).
2. Update resolution, validation, and lowering together.
3. Refresh relevant fixtures in [`../crates/nemoir-dsl-fe/tests/fixtures/`](../crates/nemoir-dsl-fe/tests/fixtures/).
4. Update [DSL and IR](dsl-and-ir.md) and any public example that demonstrates the changed syntax.

### IR change

If the IR shape or semantics change:

1. Update IR types in [`../crates/nemoir-ir/src/lib.rs`](../crates/nemoir-ir/src/lib.rs).
2. Update backend-neutral validation in [`../crates/nemoir-ir/src/validate.rs`](../crates/nemoir-ir/src/validate.rs).
3. Update every lowering path that produces the changed IR.
4. Update every backend that consumes the changed IR.
5. Refresh golden fixtures such as [`../crates/nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml`](../crates/nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml) and [`../crates/nemoir-dsl-fe/tests/fixtures/hint_tutor-ir.yml`](../crates/nemoir-dsl-fe/tests/fixtures/hint_tutor-ir.yml).
6. Update [DSL and IR](dsl-and-ir.md), [Compatibility](compatibility.md), and any target page whose emitted layout or constraints changed.

Prefer small end-to-end slices. A change that cannot be explained at the IR boundary is usually coupling too much frontend or runtime behavior into the wrong layer.
