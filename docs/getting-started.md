# Getting started

This guide assumes the `nemo` binary is already installed and available on your `PATH`. Run commands from a directory containing the workflow paths you supply; the examples below assume a checkout of this documentation repository.

## First workflow: validate the public hello example

```bash
nemo check examples/hello-workflow/hello.nemo
```

`nemo check` runs frontend validation only: parse, resolve, and DSL validation.

## Lower to IR and generate an artifact

Render the same workflow as a standalone HTML graph:

```bash
nemo compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html
```

`nemo compile` lowers the workflow to Agent Workflow IR, runs full IR validation, and then generates the requested target artifact.

If you only want the lowered IR, use the default `none` target with `--dump-ir`:

```bash
nemo compile \
  examples/hello-workflow/hello.nemo \
  --dump-ir > /tmp/hello-workflow.ir.yml
```

## Public examples to use next

| Example | What to try |
| --- | --- |
| [`../examples/hello-workflow/`](../examples/hello-workflow/) | `check`, `compile --target visualizer`, or `compile --target web` |
| [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) | `compile --target python` |
| [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) | `compile --target web` |

Compilation generates artifacts only. Target-specific runtime and package APIs live on their own public pages; see [Python target guide](targets/python.md), [Web target guide](targets/web.md), and [Compatibility](compatibility.md).
