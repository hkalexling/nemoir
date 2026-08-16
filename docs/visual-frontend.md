# Visual frontend

Status: canonical public guide for the direct visual workflow frontend.

The visual frontend is a second authoring surface alongside the `.nemo` DSL. It
accepts a versioned **visual semantic document** (JSON) and lowers it
**directly** into the same validated `WorkflowIr` used by every backend. It is
not a DSL generator, not a `.nemo` serializer, and not a raw-IR passthrough.

```text
visual semantic document (JSON, schema v0.1)
        │  deserialize + visual validation
        ▼
Rust visual frontend ──► WorkflowIr ──► shared IR validation ──► backends
        (nemoir-visual-fe)                              ├─ none / visualizer
                                                        ├─ python
                                                        └─ web
```

## What this frontend is and is not

- **Direct lowering.** `nemoir_visual_fe::lower(document, filename)` validates
  the document and lowers it straight into the shared `WorkflowIr`
  (`source.frontend == "nemo_visual"`). `filename` is display metadata only and
  is never treated as a host path.
- **No DSL round-trip.** The visual frontend has no dependency on the text DSL
  frontend and never serializes or parses `.nemo` source.
- **No raw-IR input.** The wire request carries a parsed `document` object, not
  a serialized `WorkflowIr` and not `.nemo` source text. The document is its own
  authoring surface.
- **Layout is editor-only.** Canvas coordinates and editor state are
  deliberately absent from the semantic document and are not compiler
  semantics.
- **One pipeline for all targets.** After lowering, visual documents go through
  the same shared IR validation and backend dispatch as DSL workflows. There are
  no visual-specific backend branches.
- **Direct diagnostics.** Visual errors surface as `phase: "visual"`
  diagnostics addressed to a canvas entity (`visualLocation` with an `id` and
  optional `field`), not to source-line ranges.

## The document

The document is the `VisualWorkflowDocument` type. The exhaustive, exact
field-by-field contract ships as `api.d.ts` with the
[`@nemoir/compiler-wasm`](https://www.npmjs.com/package/@nemoir/compiler-wasm)
package. This section summarizes the semantic rules. Where this page and the
packaged declaration differ, the declaration is the type authority.

Top-level fields (camelCase): `schemaVersion`, `workflow`, `states`,
`controlEdges`, `dataEdges`, and `policies`. Unknown fields are rejected at
deserialization.

### Identities and names

Most graph objects carry two kinds of identity:

- **`id`** — an opaque, stable visual identity used only for cross-references
  inside the document (edges reference states, data edges reference
  inputs/outputs, tool arguments reference outputs). Ids must be non-empty and
  unique within their scope, and are not lowered into IR identifiers.
- **`name`** — the user-facing identifier lowered into the shared IR
  (`Input.id`, `Node.id`, `Write.name`). Names must match the portable
  identifier form `[A-Za-z][A-Za-z0-9_]*` and must be unique within their scope
  (workflow inputs, states, and outputs within a state).

The workflow `id` is the exception: it is lowered directly into IR
`workflow.id` and is validated as a portable identifier.

### Workflow inputs

Each input has `id`, `name`, and `type`. Workflow inputs cannot be optional:
there is no `optional` field on an input, and supplying one is rejected.

### States, entry, and exit

Each state has `id`, `name`, `isEntry`, `isExit`, `prompt`, `requires`,
`outputs`, and `execution`.

- Exactly one state must be `isEntry`; at least one must be `isExit`.
- Exit states cannot have outgoing control edges.
- `execution` is tagged by `kind`: `{ "kind": "model" }` or
  `{ "kind": "tool", "capability": ..., "args": [...] }`.
- Model states require a non-empty `prompt`; tool states may omit it (an
  omitted prompt lowers to an empty, descriptive-only IR prompt).

### Types

Value types are `string`, `bool`, `path`, `number`, `json`, and the array forms
`string[]`, `bool[]`, `path[]`, `number[]`, `json[]`. Optionality is not part of
the type string; it is carried by explicit `optional` booleans on outputs and
data edges.

### Outputs

Each output has `id`, `name`, `type`, and `optional`. `name` lowers to
`Write.name`; `optional` lowers to `Write.optional`.

### Control edges and explicit order

Each control edge has `id`, `sourceStateId`, `targetStateId`, `order`, and
`guard`.

- `order` is a unique `u32` key within the source state's outgoing edges.
  Sparse values are sorted and lowered to consecutive transition priorities
  (`0, 1, 2, ...`), so the author controls guard evaluation order explicitly.
- A duplicate source-to-target relationship is rejected; combine conditions
  into one guard instead.
- Guards are evaluated in ascending priority and the first match wins; see the
  target-neutral execution contract in the [DSL and IR reference](dsl-and-ir.md).

### Data edges

Each data edge has `id`, `source` (an input or node-output reference), a
`targetStateId`, and `optional`.

- Data edges are explicit. Unlike the DSL, the visual frontend does not
  synthesize implicit entry-input reads. Lowered reads come from data edges
  (origin `visual_data_edge`) and from tool arguments (origin
  `visual_tool_arg`).
- `optional` lowers to `Read.optional`.

### Tool execution and derived reads

A tool state's `args` may reference a workflow input, a prior node output, or a
typed literal.

- A node-output tool argument synthesizes a read for that output (origin
  `visual_tool_arg`) unless a data edge already creates the same dependency.
  Declaring both for the same dependency is rejected.
- The tool capability is auto-added to the state's `requires` and to the
  top-level IR `capabilities` if missing.
- Literals are validated against their declared type.

### Expressions and guards

Guards:

- `always`
- `has_value` / `missing` — take a node-output reference
  (`{ "kind": "node_output", "stateId": ..., "outputId": ... }`) and are used
  for optional-output branching.
- `eq` — two expressions.
- `if` — a boolean condition expression.

Expressions:

- `ref` (input, node_output, or bound)
- `literal` (typed JSON value)
- `not`, `method_call`, `and`, `or`
- `compare` with op `gt`/`gte`/`lt`/`lte`
- `binop` with op `add`/`sub`/`mul`/`div` (the wire spelling is `binop`, not
  `bin_op`)

Method and type-checking semantics follow the same expression rules and
capability catalogue documented in the [DSL and IR reference](dsl-and-ir.md).
`bound` refs are policy-local: they are valid only inside policy conditions and
policy-required arguments, never in transition guards, node reads, or tool
arguments.

### Policies

Each policy has `id` (opaque), `label` (lowered to `Policy.id`; must be
non-empty), `kind`, `trigger`, `requires`, and `condition`.

- `kind` is `before` or `deny`.
- `trigger` is a capability plus `bindings`, the names of required parameters
  of that capability. Only required parameters are bindable.
- `before` policies declare `requires` (capabilities with input or bound refs)
  and must not have a `condition`.
- `deny` policies require a `condition` and must not declare `requires`.

## Validation and lowering boundaries

Lowering runs two layers.

1. **Visual frontend validation** (`nemoir_visual_fe::check`): deserialization
   shape (unknown fields rejected), schema version, identifier and name
   validity/uniqueness, entry/exit shape, exit-state edge rules, capability and
   parameter checks against the closed catalogue, reference resolution, type
   and literal checks, control-order/relationship checks, guard and expression
   operator checks, and policy shape checks.
2. **Shared IR validation** (`nemoir_ir::validate`): the same validator used by
   the DSL runs over the lowered `WorkflowIr` — graph reachability,
   optional-guard safety, transition targets, and the other IR-level rules
   listed in the [DSL and IR reference](dsl-and-ir.md).

`check(document)` runs only layer 1. `lower(document, filename)` runs layer 1
and then lowers. The WASM pipeline runs both and maps layer-2 errors back to
visual locations.

## Diagnostics

- Visual frontend diagnostics carry `phase: "visual"` and a `visualLocation`
  (`entity` + `id` + optional `field`). They have no source range.
- Shared IR diagnostics carry `phase: "ir"` and are mapped back through visual
  provenance to a `visualLocation` when the failed IR path corresponds to a
  source visual entity.
- A request that cannot be deserialized into a document produces a global
  `phase: "visual"` diagnostic with `code: "visual_request"` and no location.
- Backend/target failures carry `phase: "target"`, identical to the DSL path.

## WASM APIs

The browser-callable package exposes three **additive** visual entry points; the
`.nemo` `analyze`/`generate`/`metadata` functions are unchanged.

- `analyzeVisual(request)` — validate, lower, and optionally return the shared
  `WorkflowIr`. Intended to be called on a debounce while editing.
- `generateVisual(request)` — re-runs analysis, then dispatches to the selected
  backend to produce an artifact.
- `visualMetadata()` — returns the schema version and the canonical capability
  catalogue (name plus parameter name/type/required) used for tool-stage and
  policy validation.

`request` carries a parsed `document` (not `.nemo` source text), an optional
display-only `filename`, an optional `target`, `includeIr`, and `options`. See
the [WASM package page](wasm-package.md) and `api.d.ts` for exact shapes.

## Backends

The visual path shares the DSL path's target set and compatibility rules:

| Target | Output |
| --- | --- |
| `none` | Validate and lower only; no artifact. |
| `visualizer` | Standalone HTML workflow graph. |
| `python` | Python workflow package. |
| `web` | Vite/TypeScript browser app. |

Target-specific restrictions (for example the web target's browser-only
capability set) apply identically; see [Compatibility](compatibility.md).

## Browser application boundaries

The browser editor is a separate surface. This guide documents the
compiler-side document contract and the WASM entry points above; its UI behavior
is documented in [Browser compiler](browser-compiler.md). The current browser
app authors the document in-memory and has no persistence, import/export, or
DSL/visual conversion. These UI limits do not change the programmatic WASM
contract.

## Example

The public hello workflow ships both authoring forms:

- [`hello.nemo`](../examples/hello-workflow/hello.nemo) — the `.nemo` source,
  compiled with the CLI.
- [`hello.visual.json`](../examples/hello-workflow/hello.visual.json) — the
  same workflow as a visual semantic document; the canonical tested public
  visual sample.

## Related pages

- [DSL and IR reference](dsl-and-ir.md) — shared IR shape, capability
  catalogue, expression and type semantics.
- [WASM compiler package](wasm-package.md)
- [Browser compiler](browser-compiler.md)
- [Compatibility](compatibility.md)
- [`../examples/hello-workflow/`](../examples/hello-workflow/)
