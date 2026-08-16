# NemoIR examples

These are small, self-contained workflows intended for learning and public compiler validation. They do not include provider credentials, generated packages, or runtime-specific setup.

All commands below assume the `nemo` binary is already installed and available on your `PATH`.

| Example | Demonstrates | Suitable target |
| --- | --- | --- |
| [`hello-workflow`](hello-workflow/) | Inputs, entry/exit stages, typed outputs, and a matching visual semantic document | `visualizer`, `python`, `web` |
| [`policy-gated-edit`](policy-gated-edit/) | Capability declarations and `before`/`deny` policies | `python` |
| [`web-hint-tutor`](web-hint-tutor/) | Conditional transitions and browser-safe user elicitation | `web` |

Validate the canonical public examples from the repository root:

```bash
nemo check examples/hello-workflow/hello.nemo
nemo check examples/policy-gated-edit/policy-gated-edit.nemo
nemo check examples/web-hint-tutor/hint-tutor.nemo
```

The CLI commands apply to the `.nemo` examples. The matching
[`hello.visual.json`](hello-workflow/hello.visual.json) document is consumed by
the visual WASM API and covered by the visual frontend and package smoke tests.

The CI workflow also compiles one representative artifact for each supported backend. Keep new examples small, deterministic to compile, and free of private data or credentials.
