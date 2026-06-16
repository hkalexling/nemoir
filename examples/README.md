# NemoIR examples

These are small, self-contained workflows intended for learning and public
compiler validation. They do not include provider credentials, generated
packages, or runtime-specific setup.

| Example | Demonstrates | Suitable target |
| --- | --- | --- |
| [`hello-workflow`](hello-workflow/) | Inputs, entry/exit stages, typed outputs | `visualizer`, `python`, `web` |
| [`policy-gated-edit`](policy-gated-edit/) | Capability declarations and `before`/`deny` policies | `python` |
| [`web-hint-tutor`](web-hint-tutor/) | Conditional transitions and browser-safe user elicitation | `web` |

Validate every example from the compiler repository root:

```bash
for workflow in examples/*/*.nemo; do
  cargo run --package nemoir-cli -- check "$workflow"
done
```

The CI workflow also compiles one representative artifact for each supported
backend. Keep new examples small, deterministic to compile, and free of
private data or credentials.
