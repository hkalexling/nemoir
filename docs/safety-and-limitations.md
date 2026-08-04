# Safety and limitations

## Research-pilot status

NemoIR is a research compiler stack for structured agent workflows. The compiler, DSL, IR, and generated package layouts may change as the project evolves.

This documentation does not claim that NemoIR is a production platform or that compilation provides a security guarantee.

## What the compiler checks

Today, the compiler gives you static workflow checks:

- `nemo check` verifies frontend correctness: parse, resolve, and DSL validation.
- `nemo compile` additionally lowers to IR, validates the IR structure, and runs backend-specific checks when a target needs them.

These checks help catch malformed workflows, bad references, incompatible capabilities, and some unsupported target combinations before runtime.

## What the compiler does not guarantee

Compilation does not guarantee:

- correct model behavior or truthful model outputs;
- safe prompts or safe tool implementations;
- runtime sandboxing or host isolation;
- correct enforcement by external runtimes or capability handlers; or
- suitability for production security boundaries.

Policies and capability declarations make intended behavior explicit in the workflow and IR, but actual enforcement still depends on the generated runtime package and the environment that runs it.

## Target-specific limits

Current backends have real compatibility limits:

- the web target rejects workflows that depend on unsupported capabilities such as `fs.read`, `fs.write`, or `os.shell`;
- the web target also rejects `path`-typed workflow inputs and outputs;
- generated artifacts depend on separate runtime packages; and
- generated artifacts are intended as compiler outputs, not as standalone proof that a workflow is safe.

Use the public examples in [`../examples/`](../examples/) as the supported starting point for experimentation.
