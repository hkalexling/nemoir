# Python target

The Python target compiles a validated workflow into a generated Python package. Running that package depends on the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) project.

See also: [DSL and IR](../dsl-and-ir.md), [Safety and limitations](../safety-and-limitations.md), and [Compatibility](../compatibility.md).

## Compile

With `nemo` on your `PATH`:

```bash
nemo compile \
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
- `types.py` defines typed `AgentInput`, `AgentOutput`, and `AgentResult` shapes.
- `_agent.py` defines the generated agent wrapper.
- `__init__.py` re-exports the generated entry points.

## Compiler-side naming and dependency rules

The Python backend derives names from the workflow id:

- workflow id -> snake_case import package name
- workflow id -> hyphenated distribution name in `pyproject.toml`

Compilation fails if the workflow id cannot be converted to a valid Python package or module name.

Compilation also fails if any workflow input id or exit-stage output field that must become a Python attribute is not a valid Python identifier or is a reserved keyword.

Generated packages currently declare:

- Python `>=3.11`
- `nemoir-runtime>=0.9.2`

## Runtime docs

This compiler guide covers emitted package structure only. Runtime setup, execution, tool registration, model adapters, and package APIs belong to the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) documentation.
