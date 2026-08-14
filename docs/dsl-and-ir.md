# The `.nemo` DSL and Agent Workflow IR

Status: public normative reference for released NemoIR compiler behavior.
This document defines the supported public `.nemo` authoring surface, its
lowering semantics, and the resulting Agent Workflow IR contract.
Implementation files, private fixtures, and internal source layout are not part
of the public reference.

Useful public examples and reference fixtures:

- [`../examples/hello-workflow/hello.nemo`](../examples/hello-workflow/hello.nemo)
- [`../examples/policy-gated-edit/policy-gated-edit.nemo`](../examples/policy-gated-edit/policy-gated-edit.nemo)
- [`../examples/web-hint-tutor/hint-tutor.nemo`](../examples/web-hint-tutor/hint-tutor.nemo)
- [`../examples/reference-fixtures/coding-agent.nemo`](../examples/reference-fixtures/coding-agent.nemo)
- [`../examples/reference-fixtures/coding-agent-ir.yml`](../examples/reference-fixtures/coding-agent-ir.yml)
- [`../examples/reference-fixtures/judge_candidate.nemo`](../examples/reference-fixtures/judge_candidate.nemo)
- [`../examples/reference-fixtures/judge_candidate-ir.yml`](../examples/reference-fixtures/judge_candidate-ir.yml)
- [`../examples/reference-fixtures/policy_command_allowlist.nemo`](../examples/reference-fixtures/policy_command_allowlist.nemo)
- [`../examples/reference-fixtures/hint_tutor.nemo`](../examples/reference-fixtures/hint_tutor.nemo)
- [`../examples/reference-fixtures/hint_tutor-ir.yml`](../examples/reference-fixtures/hint_tutor-ir.yml)

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
- Line comments use `//` and run to the end of the line.
- Within a stage, each of `prompt:`, `input:`, `output:`, `requires:`, and
  `exec:` may appear at most once.
- The grammar accepts any `@ident` annotation spelling; released compiler
  behavior recognizes only `@entry` and `@exit`.

## 3. Types and literals

### 3.1 Base types

Supported base types are:

- `string`
- `bool`
- `path`
- `number`
- `json`

A type reference is a base type with optional array and optional markers.
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

### 3.2 Strings

The DSL distinguishes three common string contexts:

- prompt strings: `"..."` or `"""..."""`
- policy string literals: `"..."`
- deterministic `exec:` string arguments: `"..."` or `"""..."""`

Current behavior:

- Prompt strings are trimmed.
- Multi-line prompt strings are additionally trimmed line-by-line.
- Policy string literals preserve leading and trailing whitespace; only `\"`
  is unescaped.
- Multi-line `exec:` strings preserve their interior content verbatim after the
  outer `"""` delimiters are stripped.
- In non-JSON DSL strings, `\"` is the only escape sequence.

### 3.3 Numbers and structured JSON exec literals

The DSL has two numeric surfaces:

- Policy and transition number literals support integers and decimal fractions.
- Structured JSON literals in `exec:` arguments additionally support exponent
  notation such as `1e6`, `3.5E-2`, and `-2E+4`.

Structured JSON exec literals are available for `json`-typed deterministic
arguments and may be objects, arrays, strings, numbers, booleans, or `null`.
Current public behavior is JSON-shaped but narrower than full JSON string
escaping: JSON strings accept plain characters and `\"`, but other backslash
escapes are not part of the DSL surface.

## 4. Stages

A stage is either model-driven or deterministic.

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
- `prompt:` is optional.
- If omitted, the lowered IR prompt is the empty string.
- If present, the prompt is documentation-only; it is preserved in IR but does
  not change the stage into a model call.
- The exec capability is auto-added to the stage's `requires` list if missing.
- The exec capability is also auto-added to top-level IR `capabilities`.
- `Stage.field` refs used in exec args are auto-added to the stage's reads.

Exec arg values currently support only:

- workflow input refs: `input_name`
- prior stage output refs: `Stage.field`
- string literals
- multi-line string literals
- structured JSON literals

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
policy. Their types are inferred from the normative capability catalog in
Section 8.

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

### 7.1 Operators and precedence

From highest to lowest precedence, the parser supports:

1. parentheses and primaries
2. unary `-`
3. `*`, `/`
4. `+`, `-`
5. `>`, `>=`, `<`, `<=`
6. `not`
7. `and`
8. `or`

Additional forms:

- method calls on a bare receiver identifier
- `in [ ... ]` on a bare receiver identifier

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

The receiver must be a bare identifier, not `Stage.field` and not an arbitrary
subexpression.

### 7.3 `in [ ... ]`

`x in [a, b, c]` is DSL sugar.

- It is allowed in policy conditions.
- It lowers to an `or` of `eq` method calls.
- Empty lists are rejected.
- The left-hand side must be a bare identifier.
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

<!-- normative-capability-catalog:start -->
## 8. Normative capability catalog

This section is the normative public capability catalog for released NemoIR DSL
and IR authoring. The core catalog is a closed set of ten capability names.

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
- Only required parameters are policy-bindable. In `before fs.write(path)` the
  bound name `path` is valid because `path` is a required catalog parameter of
  `fs.write`.
- Optional parameters are valid in deterministic `exec:` argument lists but are
  not bindable in policy triggers or policy-required capability parameter lists.
- `http.fetch(headers: ...)` and `http.fetch(body: ...)` are therefore valid in
  deterministic `exec:` stages, but `before http.fetch(url, method, headers)` is
  not part of the public DSL surface.
- `browser.js.run` is for trusted workflow-author code and, on the web target,
  requires `code` to be a compile-time string literal in `exec:`. Input or
  output refs are not allowed for that parameter.
- `browser.js.sandbox` is the explicit dynamic-code path. On the web target, its
  `code` argument may come from a workflow input or a prior stage output,
  subject to deterministic-stage-only and approval-policy rules.
- For web-target safety, approval, and isolation details, see the
  [Web target guide](targets/web.md).
- The compiler has a boolean capability-parameter type, but no catalog entry
  currently uses it.

Bound-variable types in policies come directly from the required parameters of
these catalog entries.
<!-- normative-capability-catalog:end -->

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
- A deterministic-stage prompt, if present, is descriptive metadata only.

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
- Hint-tutor golden source and IR pair: [`../examples/reference-fixtures/hint_tutor.nemo`](../examples/reference-fixtures/hint_tutor.nemo) and [`../examples/reference-fixtures/hint_tutor-ir.yml`](../examples/reference-fixtures/hint_tutor-ir.yml)
- Full fixture with inferred loops and optional-skip: [`../examples/reference-fixtures/coding-agent.nemo`](../examples/reference-fixtures/coding-agent.nemo)
- Lowered IR for the full fixture: [`../examples/reference-fixtures/coding-agent-ir.yml`](../examples/reference-fixtures/coding-agent-ir.yml)
- Numeric transition guards: [`../examples/reference-fixtures/judge_candidate.nemo`](../examples/reference-fixtures/judge_candidate.nemo)
- Lowered IR for numeric transitions: [`../examples/reference-fixtures/judge_candidate-ir.yml`](../examples/reference-fixtures/judge_candidate-ir.yml)
- Policy-expression coverage: [`../examples/reference-fixtures/policy_command_allowlist.nemo`](../examples/reference-fixtures/policy_command_allowlist.nemo)

## Appendix A. Complete normative EBNF grammar

<!-- complete-normative-ebnf:start -->
<!-- grammar-pest-sha256: 2f55f4f9bb4a392fa8ec23f8970f88a13e0f14e6ab7fbc2e2951c17585dae03c -->

The following grammar is a readable EBNF rendering of the released public DSL
surface. Whitespace and `//` line comments may appear between tokens unless a
rule says otherwise.

```ebnf
workflow            = "workflow", ident, "{",
                      [ input_block ],
                      [ policy_block ],
                      { stage },
                      "}" ;

comment             = "//", { not_newline }, ( newline | end_of_file ) ;

ident               = ascii_alpha, { ascii_alnum | "_" } ;
dotted_ident        = ident, { ".", ident } ;
annotation          = "@", ident ;

type_ref            = type_base, [ array_marker ], [ optional_marker ] ;
type_base           = ident ;
array_marker        = "[]" ;
optional_marker     = "?" ;

input_block         = "input", "{", { input_field }, "}" ;
input_field         = ident, ":", type_ref ;

policy_block        = "policy", "{", { before_policy | deny_policy }, "}" ;
before_policy       = "before", cap_call, "requires", require_list ;
deny_policy         = "deny", cap_call, "if", policy_expr ;
cap_call            = dotted_ident, "(", ident, { ",", ident }, ")" ;
require_list        = require_item, { ",", require_item } ;
require_item        = dotted_ident, [ "(", ident, { ",", ident }, ")" ] ;

policy_expr         = or_expr ;
or_expr             = and_expr, { "or", and_expr } ;
and_expr            = not_expr, { "and", not_expr } ;
not_expr            = { not_kw }, compare_expr ;
not_kw              = "not" not followed by ascii_alnum or "_" ;
compare_expr        = add_expr, [ compare_op, add_expr ] ;
compare_op          = ">=" | "<=" | ">" | "<" ;
add_expr            = mul_expr, { add_op, mul_expr } ;
add_op              = "+" | "-" ;
mul_expr            = unary_expr, { mul_op, unary_expr } ;
mul_op              = "*" | "/" ;
unary_expr          = { unary_minus }, primary ;
unary_minus         = "-" ;

primary             = "(", or_expr, ")"
                    | call_or_in
                    | node_ref
                    | number_literal
                    | policy_ref ;

policy_ref          = ident ;
node_ref            = ident, ".", ident ;
number_literal      = digit, { digit }, [ ".", digit, { digit } ] ;

call_or_in          = ident,
                      ( ".", ident, "(", [ policy_arg_list ], ")"
                      | "in", policy_array ) ;
policy_arg_list     = policy_value, { ",", policy_value } ;
policy_array        = "[", policy_value, { ",", policy_value }, "]" ;
policy_value        = number_literal | single_line_string | ident ;

stage               = "stage", [ annotation ], ident, "{",
                      { stage_body_item },
                      "}" ;

stage_body_item     = transition_block
                    | prompt_decl
                    | stage_input
                    | output_block
                    | requires_block
                    | exec_decl ;

transition_block    = transition_decl, [ "," ],
                      { transition_decl, [ "," ] } ;
transition_decl     = "transition", ( transition_cond | transition_else ),
                      "=>", ident ;
transition_cond     = "if", or_expr ;
transition_else     = "else" ;

prompt_decl         = "prompt", ":", string ;
stage_input         = "input", ":", input_ref, { ",", input_ref } ;
input_ref           = ident, ".", ident, [ optional_marker ] ;

output_block        = "output", ":", "{", { output_field }, "}" ;
output_field        = ident, ":", type_ref, [ bool_branches ] ;
bool_branches       = "{", "true", "=>", ident,
                      "false", "=>", ident, "}" ;

requires_block      = "requires", ":", dotted_ident, { ",", dotted_ident } ;

exec_decl           = "exec", ":", dotted_ident, "(", [ exec_arg_list ], ")" ;
exec_arg_list       = exec_arg, { ",", exec_arg } ;
exec_arg            = ident, ":", exec_value ;
exec_value          = multiline_string
                    | single_line_string
                    | json_value
                    | ident
                    | ident, ".", ident ;

json_value          = json_object
                    | json_array
                    | json_string
                    | json_number
                    | json_bool
                    | json_null ;
json_object         = "{", [ json_member, { ",", json_member } ], "}" ;
json_member         = json_string, ":", json_value ;
json_array          = "[", [ json_value, { ",", json_value } ], "]" ;
json_string         = '"', { json_char }, '"' ;
json_char           = '\\"' | json_plain_char ;
json_number         = [ "-" ], digit, { digit },
                      [ ".", digit, { digit } ],
                      [ ( "e" | "E" ), [ "+" | "-" ], digit, { digit } ] ;
json_bool           = "true" | "false" ;
json_null           = "null" ;

string              = multiline_string | single_line_string ;
single_line_string  = '"', { single_line_char }, '"' ;
multiline_string    = '"""', { multiline_char }, '"""' ;
```

Lexical notes:

- `ascii_alpha` means `A`-`Z` or `a`-`z`.
- `ascii_alnum` means `ascii_alpha` or `0`-`9`.
- `digit` means `0`-`9`.
- `not_newline` means any character except a line break.
- `single_line_char` means any non-newline character except `"` and bare `\\`,
  plus the escape `\"`.
- `multiline_char` means any character sequence other than the closing `"""`.
- `json_plain_char` means any character except `"` and bare `\\`.
- The grammar accepts any annotation spelling of the form `@ident`; semantic
  validation currently recognizes only `@entry` and `@exit`.
- `policy_array` contains one or more elements.
- `cap_call` requires at least one bound parameter name.
- `compare_expr` supports at most one comparison operator per expression level;
  chained comparisons such as `a < b < c` are not part of the DSL surface.
- Expression number literals do not use exponent notation; exponent syntax is
  available only inside structured JSON exec literals.

<!-- complete-normative-ebnf:end -->
