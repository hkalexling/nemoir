# Extending NemoIR

NemoIR is organized around one central contract: user-facing workflows lower into a shared Agent Workflow IR, and targets consume that IR after validation. Safe extension work preserves that boundary.

## Core architecture

At a high level, the compiler stack has three layers:

1. frontends that accept some authoring surface and lower it into workflow IR;
2. shared validation and analysis over that IR; and
3. backends that emit target-specific artifacts from validated IR.

The goal is not to tie workflow semantics to one runtime or one editor. New work should make the compiler boundary clearer, more analyzable, or more reusable across targets.

## Frontends

A frontend should own parsing, authoring-surface rules, and lowering from source into the shared IR.

Good frontend extensions:

- keep authoring-surface-specific rules in the frontend layer;
- lower deterministically into the same workflow concepts used elsewhere;
- rely on shared IR validation for backend-neutral guarantees; and
- avoid baking target-specific runtime assumptions into authoring syntax.

If a rule should hold for every workflow regardless of how it was authored, it probably belongs in shared IR validation rather than in one frontend alone.

## Backends

A backend should consume validated IR and emit artifacts for one target environment.

Good backend extensions:

- keep target-specific compatibility checks local to that target;
- reject unsupported workflow shapes explicitly rather than silently changing semantics;
- preserve the workflow meaning established by the IR; and
- treat emitted files as a rendering of validated workflow structure, not as a second source of truth.

Target-specific constraints are expected. The important part is to keep them out of the shared workflow model unless every target must share them.

## Capabilities and policies

Capabilities and policies are part of the shared workflow contract, not private runtime implementation details.

When extending them:

- describe workflow-visible behavior rather than one backend's helper APIs;
- keep policy meaning explicit in the IR;
- update compiler-side validation where the shared contract changes; and
- update every affected target guide and public example when user-visible behavior changes.

## Language and IR changes

Changes to the language or IR have the widest blast radius because they can affect every frontend, every backend, and the public documentation.

Prefer small end-to-end changes:

- update lowering and validation together;
- update every affected backend together;
- refresh examples and reference docs together; and
- keep the IR explainable as a backend-neutral workflow description.

A useful test is whether the change can be described cleanly at the IR boundary. If not, it may be coupling one frontend or one runtime too tightly into the compiler core.

## Browser-related surfaces

The browser editor, the browser-callable compiler package, and the generated web target are separate surfaces.

When extending them:

- keep compiler semantics in the shared compiler pipeline;
- keep browser-application UX concerns in the browser application itself; and
- keep generated web-target restrictions in the web target.

That separation helps NemoIR stay compile-first rather than turning into one runtime-specific product.
