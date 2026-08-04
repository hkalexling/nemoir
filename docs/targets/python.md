# Python target

The Python target compiles a validated workflow into an installable Python package that runs on the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) repository.

See also: [DSL and IR](../dsl-and-ir.md), [Safety and limitations](../safety-and-limitations.md), and [Compatibility](../compatibility.md).

## Compile

From the compiler workspace:

```bash
cargo run --package nemoir-cli -- compile \
  path/to/workflow.nemo \
  --target python \
  --output /tmp/nemoir-python-out
```

`--output` points to the parent directory. The compiler then creates one package directory named from the workflow id. If you omit `--output`, the compiler writes next to the source file. When reading the workflow from stdin (`-`), `--output` is required.

## What gets emitted

For a workflow whose id lowers to `policy_gated_edit`, the output looks like this:

```text
/tmp/nemoir-python-out/
├── pyproject.toml
└── policy_gated_edit/
    ├── __init__.py
    ├── _agent.py
    ├── _manifest.py
    └── types.py
```

At a high level:

- `pyproject.toml` declares the generated distribution and its runtime dependency.
- `_manifest.py` embeds the compiled workflow manifest as Python data.
- `types.py` defines typed `AgentInput`, `AgentOutput`, and `AgentResult` dataclasses.
- `_agent.py` defines the generated `Agent` wrapper.
- `__init__.py` re-exports the generated entry points plus selected runtime types.

## Requirements and install/use flow

Generated packages require:

- Python `>=3.11`
- `nemoir-runtime>=0.9.2`

The generated `pyproject.toml` declares both requirements for you.

A typical local flow is:

```bash
cd /tmp/nemoir-python-out
python -m venv .venv
. .venv/bin/activate
pip install -e .
```

Then import the generated package and provide:

- a `model` for model-driven stages; and
- a `ToolRegistry` covering the workflow's declared capabilities.

The generated package exposes a small typed surface (`Agent`, `AgentInput`, `AgentOutput`, `AgentResult`). Runtime behavior, tool registration, model adapters, events, and official tools live in the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) repository; this guide does not duplicate that API.

## Model and tool responsibility

NemoIR compiles the workflow structure, not the model provider. The generated package does not bundle tools or a model backend for you.

- Model stages run through the shared runtime's model executor.
- Deterministic `exec:` stages run a selected tool directly, with no model call.
- Policies, stage visibility, transitions, and output validation stay in the runtime.

In other words, the model proposes stage output and tool calls, but the workflow semantics remain compiler- and runtime-controlled.

## Naming and output caveats

The Python backend derives names from the workflow id:

- workflow id -> snake_case import package name
- workflow id -> hyphenated distribution name in `pyproject.toml`

Compilation fails if the workflow id cannot be converted to a valid Python package/module name.

Compilation also fails if any workflow input id or exit-stage output field that must become a Python attribute is not a valid Python identifier or is a reserved keyword.

## Deterministic vs. model behavior

Generated Python packages support both kinds of stages:

- model stages use the supplied model adapter through `nemoir-runtime`
- deterministic `exec:` stages use matching registered tools and still go through normal policy checks and output validation

Deterministic tool selection is validated eagerly when the generated agent/runtime is constructed, so missing or ambiguous matches fail before the first run.

## Public runtime source

For the shared runtime package and its public API, see [`hkalexling/nemoir-python-runtime`](https://github.com/hkalexling/nemoir-python-runtime).
