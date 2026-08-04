# The `.nemo` DSL and Agent Workflow IR

Status: public normative reference for the current NemoIR compiler pilot.
This document describes the semantics implemented by the compiler in this
repository. When this document and the compiler diverge, the implementation and
its tests are the source of truth until the document is updated.

Primary implementation sources:

- Grammar: [`../crates/nemoir-dsl-fe/src/grammar.pest`](../crates/nemoir-dsl-fe/src/grammar.pest)
- DSL frontend validation and transition inference: [`../crates/nemoir-dsl-fe/src/validate.rs`](../crates/nemoir-dsl-fe/src/validate.rs)
- DSL resolution and lowering: [`../crates/nemoir-dsl-fe/src/resolve.rs`](../crates/nemoir-dsl-fe/src/resolve.rs), [`../crates/nemoir-dsl-fe/src/lower.rs`](../crates/nemoir-dsl-fe/src/lower.rs)
- IR types: [`../crates/nemoir-ir/src/lib.rs`](../crates/nemoir-ir/src/lib.rs)
- IR validation: [`../crates/nemoir-ir/src/validate.rs`](../crates/nemoir-ir/src/validate.rs)
- Capability catalog: [`../crates/nemoir-ir/src/capabilities.rs`](../crates/nemoir-ir/src/capabilities.rs)

Useful public examples and fixtures:

- [`../examples/hello-workflow/hello.nemo`](../examples/hello-workflow/hello.nemo)
- [`../examples/policy-gated-edit/policy-gated-edit.nemo`](../examples/policy-gated-edit/policy-gated-edit.nemo)
- [`../examples/web-hint-tutor/hint-tutor.nemo`](../examples/web-hint-tutor/hint-tutor.nemo)
- [`../crates/nemoir-dsl-fe/tests/fixtures/coding-agent.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/coding-agent.nemo)
- [`../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate.nemo)
- [`../crates/nemoir-dsl-fe/tests/fixtures/policy_command_allowlist.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/policy_command_allowlist.nemo)
- Lowered IR fixtures: [`../crates/nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml`](../crates/nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml), [`../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate-ir.yml`](../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate-ir.yml)

## 1. Compiler model

NemoIR treats a workflow as a typed state machine:

```text
frontend -> validated Agent Workflow IR -> backend/runtime
```

For the text frontend, the authoring surface is the `.nemo` DSL. The compiler
resolves names, validates the workflow, infers transitions where needed, and
lowers the result into a target-neutral IR.

## 2. DSL surface

A `.nemo` file declares one workflow.

```ebnf
workflow  = "workflow" ident "{" input_block? policy_block? stage* "}"
ident     = ASCII_ALPHA (ASCII_ALPHANUMERIC | "_")*
```

Line comments use `//`.

High-level structure:

```nemo
workflow Name {
  input { ... }
  policy { ... }

  stage @entry A { ... }
  stage B { ... }
  stage @exit C { ... }
}
```

Notes:

- `input { ... }` is optional.
- `policy { ... }` is optional.
- A workflow with no stages is invalid.
- If no stage is marked `@entry`, the first stage becomes the entry stage.
- If no stage is marked `@exit`, the last stage becomes an exit stage.
- Multiple `@entry` stages are invalid.
- Multiple `@exit` stages are allowed.
- Within a stage, each of `prompt:`, `input:`, `output:`, `requires:`, and
  `exec:` may appear at most once.

## 3. Types and literals

### 3.1 Base types

Supported base types are:

- `string`
- `bool`
- `path`
- `number`
- `json`

A type reference is:

```text
type_ref = type_base array_marker? optional_marker?
```

Examples:

- `string`
- `string[]`
- `string?`
- `string[]?`
- `json`

Current restrictions:

- Workflow inputs cannot be optional.
- Unknown type names are rejected.
- In the IR, optionality is stored separately from the base type string.

### 3.2 Strings and JSON

The grammar distinguishes three common cases:

- Prompt strings: `"..."` or `"""..."""`
- Policy string literals: `"..."`
- Deterministic `exec:` arguments: strings or structured JSON literals

Actual parser behavior matters:

- Prompt strings are trimmed.
- Multi-line prompt strings are additionally trimmed line-by-line.
- Policy string literals preserve leading and trailing whitespace; only `\"`
  is unescaped.
- Multi-line `exec:` strings preserve their interior content verbatim after the
  outer `"""` delimiters are stripped.
- JSON exec literals are parsed structurally as objects, arrays, strings,
  numbers, booleans, or `null`.

## 4. Stages

A stage is either model-driven or deterministic.

```ebnf
stage = "stage" annotation? ident "{" stage_body_item* "}"
annotation = "@entry" | "@exit"
```

### 4.1 Model stages

A stage without `exec:` is a model stage.

- `prompt:` is required.
- `execution` lowers to `StageExecution::Model`.
- In serialized IR, model execution is omitted because it is the default.

### 4.2 Deterministic stages

A stage with `exec:` is deterministic.

```nemo
stage ReadConfig {
  exec: fs.read(path: config_path)
  output: { content: string }
}
```

Semantics:

- No model call occurs for the stage.
- `prompt:` is optional. If omitted, the lowered IR prompt is the empty string.
- The exec capability is auto-added to the stage's `requires` list if missing.
- The exec capability is also auto-added to top-level IR `capabilities`.
- `Stage.field` refs used in exec args are auto-added to the stage's reads.

Exec arg values currently support only:

- workflow input refs: `input_name`
- prior stage output refs: `Stage.field`
- string literals
- multi-line string literals
- JSON literals

Expressions such as arithmetic, boolean operators, method calls, and bound refs
are rejected in exec args.

### 4.3 Reads and writes

A stage may declare:

```nemo
input: Prior.field, Other.value?
output: {
  x: string
  y: bool?
}
```

Semantics:

- `input:` can reference only stage outputs, not workflow inputs.
- `Stage.field?` marks the read itself optional.
- Entry stages implicitly read every workflow input, even without `input:`.
- Output fields may be optional with `?`.
- Output blocks are optional; empty-write stages are allowed.

### 4.4 `requires:`

`requires:` lists stage capabilities:

```nemo
requires: fs.read, os.shell
```

The DSL frontend collects all stage requirements, exec capabilities, policy
trigger capabilities, and policy-required capabilities into the top-level IR
`capabilities` list in first-seen order with duplicates removed.

### 4.5 Bool branches

A single non-optional `bool` output field may carry a branch block:

```nemo
output: {
  ok: bool { true => Accept false => Retry }
}
```

Rules:

- Only one output field per stage may have bool branches.
- The field must be exactly `bool`, not `bool?` and not an array.
- Bool branches and explicit `transition` statements cannot be mixed.
- Bool branches are desugared into explicit transitions:
  - `transition if ok => Accept`
  - `transition if not ok => Retry`

## 5. Transitions

Exit stages have no outgoing transitions.

For non-exit stages, transitions come from either explicit syntax or inference.

### 5.1 Explicit transitions

```nemo
transition if score - Best.score > eps => Accept
transition if score > Best.score       => Confirm
transition else                        => Reject
```

Rules:

- A stage may declare one or more `transition if ...` statements.
- A stage may declare at most one `transition else`.
- All `if` transitions are prioritized by source order.
- `transition else` always gets the lowest priority, even if written earlier.
- `transition else` lowers to `Guard::Always`.

### 5.2 Transition-condition expression subset

Transition conditions reuse much of the policy-expression grammar, but the
accepted semantics are narrower.

Supported forms:

- bare identifiers
- `Stage.field` refs
- number literals
- unary `-`
- `+`, `-`, `*`, `/`
- `>`, `>=`, `<`, `<=`
- `not`, `and`, `or`
- parentheses

Name resolution in transition conditions:

- bare `x` resolves to the current stage's output `x` if that output exists
- otherwise bare `x` resolves to workflow input `x`
- `Stage.field` resolves to another stage's output

Not supported in transition conditions:

- method calls such as `path.contains(...)`
- `in [ ... ]`
- `==` and `!=`

Numeric equality is intentionally unsupported. Use comparisons and arithmetic
instead.

### 5.3 Inferred transitions

If a stage has no explicit transitions after bool-branch desugaring, the
frontend applies these rules in order.

1. Backward-reference loop.
   If exactly one prior stage reads an output from the current stage, infer an
   unconditional transition back to that prior stage.
2. Optional-skip.
   If the next stage has exactly one required read of an optional output from an
   earlier-or-current stage, infer:
   - `has_value` -> next stage
   - `missing` -> stage after next
3. Fallthrough.
   Otherwise infer an unconditional transition to the next stage.

Current edge cases:

- If multiple prior stages read outputs from the current stage, backward-loop
  inference is ambiguous and rejected.
- If optional-skip would require combining multiple optional guards, it is
  rejected.
- If optional-skip would skip past the final stage, it is rejected.
- A final non-exit stage with no valid inferred target is rejected.

### 5.4 Optional-output safety

A required read of an optional output is valid only when every incoming control
path to the consumer is guarded by the matching `has_value` check. The compiler
verifies this both during DSL validation and IR validation.

## 6. Policies

The DSL supports two policy kinds:

```nemo
policy {
  before fs.write(path) requires fs.read(path), user.confirm
  deny fs.write(path) if not workspace.contains(path)
}
```

### 6.1 Triggers and binding

A policy trigger is a capability call shape such as:

```text
fs.write(path)
os.shell(command)
http.fetch(url, method)
```

In the DSL, the identifiers inside the trigger become bound variables for that
policy. Their types are inferred from the capability catalog.

Only required catalog parameters are bindable.

### 6.2 `before`

`before` policies require other capabilities to run before the trigger
capability.

Current DSL behavior:

- `requires fs.read(path)` forwards the bound trigger variable `path` to the
  required capability parameter of the same name.
- `requires user.confirm` forwards no arguments.
- The DSL does not currently expose arbitrary policy argument expressions.

The IR is slightly more general: required-capability args are represented as a
map from parameter names to refs, and those refs may point to inputs or bound
variables. The text DSL currently lowers only same-name bound-variable
forwarding.

### 6.3 `deny`

A `deny` policy blocks the trigger capability when its condition evaluates true.

Policy conditions may reference:

- workflow inputs
- bound trigger variables

Policy conditions may not reference stage outputs.

## 7. Policy expression language

### 7.1 Operators

The parser supports:

- unary `-`
- `*`, `/`
- `+`, `-`
- `>`, `>=`, `<`, `<=`
- `not`
- `and`
- `or`
- method calls on a bare receiver identifier
- `in [ ... ]`

`==` and `!=` are not part of the grammar.

### 7.2 Method semantics

Supported methods are:

| Form | Valid receiver | Valid argument | Result |
| --- | --- | --- | --- |
| `x.contains(y)` | `path` | `path` or `string` | `bool` |
| `x.contains(y)` | `string` | `string` | `bool` |
| `x.eq(y)` | `path` | `path` or `string` | `bool` |
| `x.eq(y)` | `string` | `string` | `bool` |
| `x.starts_with(y)` | `string` | `string` | `bool` |

Rejected combinations include:

- `starts_with` on `path`
- `string.contains(path)`
- `string.eq(path)`
- `contains` or `eq` on `bool`, `number`, `json`, or arrays
- wrong arity for any method

### 7.3 `in [ ... ]`

`x in [a, b, c]` is DSL sugar.

- It is allowed in policy conditions.
- It lowers to an `or` of `eq` method calls.
- Empty lists are rejected.
- Type compatibility follows the same rules as `eq`.
- Numeric `in` is not supported.

### 7.4 Arithmetic and comparisons

`number` expressions support:

- arithmetic: `+`, `-`, `*`, `/`
- comparisons: `>`, `>=`, `<`, `<=`

Validation rules:

- arithmetic operands must be `number`
- comparison operands must be `number`
- `eq()` does not support `number` operands
- boolean operators require boolean operands
- `not` requires a boolean operand

## 8. Capability catalog

The core catalog is a closed set of ten capability names.

| Capability | Required params | Optional params |
| --- | --- | --- |
| `fs.read` | `path: path` | — |
| `fs.write` | `path: path`, `content: string` | — |
| `os.shell` | `command: string` | — |
| `user.elicit` | `question: string` | — |
| `user.confirm` | `message: string` | — |
| `http.fetch` | `url: string`, `method: string` | `headers: json`, `body: json` |
| `browser.storage.read` | `key: string` | — |
| `browser.storage.write` | `key: string`, `value: json` | — |
| `browser.js.run` | `code: string`, `input: json` | — |
| `browser.js.sandbox` | `code: string`, `input: json` | — |

Catalog notes:

- The catalog fixes capability names and parameter shapes, not return schemas.
- Optional catalog params are valid exec args but are not bindable in policy
  triggers.
- `CapabilityParamType::Bool` exists in the core type enum, but no catalog entry
  currently uses it.

Bound-variable types in policies come directly from the required parameters of
these catalog entries.

## 9. IR shape

The lowered IR type is `WorkflowIr`.

```rust
WorkflowIr {
    ir_version: String,
    kind: String,
    source: Source,
    workflow: Workflow,
    inputs: Vec<Input>,
    capabilities: Vec<String>,
    policies: Vec<Policy>,
    nodes: Vec<Node>,
}
```

Fixed top-level constants:

- `ir_version == "0.1"`
- `kind == "workflow_ir"`
- `workflow.transition_semantics.selection == "first_match_by_priority"`
- `workflow.transition_semantics.no_match == "error_unless_exit"`

### 9.1 Refs and expressions

```rust
Ref::Input { name }
Ref::NodeOutput { node, field }
Ref::Bound { name }
```

```rust
Expr::Not
Expr::MethodCall
Expr::Ref
Expr::Literal
Expr::And
Expr::Or
Expr::Compare
Expr::BinOp
```

Important scope rules:

- `Ref::Bound` is policy-local.
- Node reads cannot use `Ref::Bound`.
- Transition guards cannot use `Ref::Bound`.
- Exec args cannot use `Ref::Bound`.

### 9.2 Nodes

```rust
Node {
    id,
    annotations,
    prompt,
    reads,
    writes,
    requires,
    transitions,
    execution,
}
```

Lowering notes:

- Entry-node reads for workflow inputs are synthesized with origin
  `implicit_entry_input`.
- `input:` reads lower with origin `dsl_stage_input`.
- `Stage.field` exec-arg reads lower with origin `exec_arg`.
- `Write.type` stores the base type string plus array marker if present.
- `Write.optional` carries `?` separately.

### 9.3 Stage execution

```rust
StageExecution::Model
StageExecution::Tool { capability, args }
```

- Model is the default.
- Tool execution denotes a deterministic stage.
- The current DSL emits only `Ref` and `Literal` expressions in tool args.

### 9.4 Policies in IR

```rust
Policy {
    id,
    kind,
    trigger,
    requires,
    condition,
}
```

Notes:

- `Policy.id` is informational source text.
- `kind` is `"before"` or `"deny"`.
- `before` policies use `requires` and no `condition`.
- `deny` policies use `condition` and no required condition-less form.

### 9.5 Guards

```rust
Guard::Always
Guard::HasValue { ref }
Guard::Missing { ref }
Guard::Eq { left, right }
Guard::If { cond }
```

Current DSL lowering behavior:

- explicit `transition if ...` -> `Guard::If`
- explicit `transition else` -> `Guard::Always`
- inferred optional-skip -> `Guard::HasValue` and `Guard::Missing`
- the text DSL does not currently emit `Guard::Eq`

## 10. Validation

Validation happens at two layers.

### 10.1 DSL/frontend validation

The frontend checks, among other things:

- duplicate workflow inputs, stages, and output fields
- unknown type names
- unknown stage refs and output refs
- missing `prompt:` on model stages
- unknown capabilities in `requires:`, `exec:`, and policies
- duplicate stage keys
- invalid bool-branch usage
- mixing bool branches with explicit transitions
- invalid transition targets
- invalid transition-condition typing
- disallowed method calls or `in [ ... ]` in transition conditions
- invalid policy-expression names, arity, refs, and types
- transition graph shape and reachability

### 10.2 IR validation

The IR validator checks, among other things:

- fixed top-level constants
- unique, non-empty node and input ids
- entry and exit existence
- known capability names and top-level declaration coverage
- valid reads and writes
- valid transition targets and unique per-node priorities
- `has_value` and `missing` only on optional node outputs
- no self-reads
- producer-to-consumer control-flow reachability for node-output reads
- every node reachable from entry
- every non-exit node can reach an exit
- exit nodes have no outgoing transitions
- exec capability presence in both top-level capabilities and node requires
- exec args limited to the allowed `Ref`/`Literal` subset
- policy refs restricted to inputs and bound vars
- deny conditions must be boolean

## 11. Target-neutral execution contract

A validated IR describes a workflow state machine that targets are expected to
preserve.

Minimum contract:

1. Start at `workflow.entry`.
2. Treat each node as either a model step or a deterministic tool step according
   to `execution`.
3. Materialize the node's declared writes for use by later nodes and guards.
4. Evaluate outgoing transitions by ascending `priority`; the first matching
   guard wins.
5. If a non-exit node has no matching transition, execution errors.
6. If an exit node is reached, the run terminates.
7. Enforce `before` and `deny` policies on capability calls according to the IR.

The IR does not standardize backend-specific concerns such as model provider
selection, prompt rendering beyond the stored prompt text, or target-specific
tool plumbing. It does standardize workflow structure, refs, policy intent,
transition selection, and validation constraints.

## 12. Public example workflows

For worked examples of the current semantics, prefer these public files:

- Basic linear workflow: [`../examples/hello-workflow/hello.nemo`](../examples/hello-workflow/hello.nemo)
- Policy-gated file edit: [`../examples/policy-gated-edit/policy-gated-edit.nemo`](../examples/policy-gated-edit/policy-gated-edit.nemo)
- Hint tutor with optional clarification path: [`../examples/web-hint-tutor/hint-tutor.nemo`](../examples/web-hint-tutor/hint-tutor.nemo)
- Full fixture with inferred loops and optional-skip: [`../crates/nemoir-dsl-fe/tests/fixtures/coding-agent.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/coding-agent.nemo)
- Numeric transition guards: [`../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/judge_candidate.nemo)
- Policy-expression coverage: [`../crates/nemoir-dsl-fe/tests/fixtures/policy_command_allowlist.nemo`](../crates/nemoir-dsl-fe/tests/fixtures/policy_command_allowlist.nemo)
