# Writing workflows

The public examples in [`../examples/`](../examples/) are the canonical executable references for the current DSL.

## Minimal structure

[`../examples/hello-workflow/hello.nemo`](../examples/hello-workflow/hello.nemo) shows the smallest useful workflow:

```nemo
workflow HelloWorkflow {
  input {
    message: string
  }

  stage @entry Compose {
    prompt: "Reply with a concise, friendly greeting for the user's message."
    output: {
      greeting: string
    }
  }

  stage @exit Done {
    input: Compose.greeting
    prompt: "Return the greeting unchanged."
    output: {
      greeting: string
    }
  }
}
```

A workflow consists of:

- a `workflow` identifier;
- an `input` block with typed workflow inputs;
- one `@entry` stage where execution begins;
- one or more `@exit` stages where execution may end; and
- stage `prompt`, `input`, and `output` declarations.

## Moving data between stages

Stage inputs read from earlier stage outputs by name:

```nemo
input: Compose.greeting
```

Optional reads use `?`. [`../examples/web-hint-tutor/hint-tutor.nemo`](../examples/web-hint-tutor/hint-tutor.nemo) uses:

```nemo
input: Diagnose.suspected_bug, AskClarify.clarification?
```

That means `Hint` may read `AskClarify.clarification` when it exists.

## Conditional transitions

The hint tutor example also shows stage-to-stage branching from a boolean output:

```nemo
needs_clarify: bool { true => AskClarify false => Hint }
```

This keeps control flow explicit in the workflow source and in the lowered IR.

## Capabilities and policies

Stages can declare required capabilities:

```nemo
requires: user.elicit
```

Policies make workflow-level constraints explicit. [`../examples/policy-gated-edit/policy-gated-edit.nemo`](../examples/policy-gated-edit/policy-gated-edit.nemo) shows both `before` and `deny` policies:

```nemo
policy {
  before fs.write(path) requires fs.read(path), user.confirm
  deny fs.read(path) if not workspace.contains(path)
  deny fs.write(path) if not workspace.contains(path)
}
```

Use capabilities and policies to describe what a workflow may do, not to hide runtime assumptions inside prompts.

## Practical advice

- Start from a public example and change one feature at a time.
- Keep stage inputs and outputs explicitly typed.
- Prefer clear stage boundaries over large prompts with implicit state.
- Run `nemo check` often while editing, then `nemo compile` for full IR and target validation.
