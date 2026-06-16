# Troubleshooting

Use the public examples in [`../examples/`](../examples/) and the command split in [CLI reference](cli.md) to isolate where a problem starts:

- `nemo check` stops after parse, resolve, and DSL validation.
- `nemo compile` additionally lowers to IR, runs IR validation, and then runs target-specific checks.
- `nemo compile --dump-ir --target none` is the quickest way to inspect lowered IR without generating an artifact.

## Compiler and DSL diagnostics

If `nemo check` fails, the problem is in the source workflow or the DSL frontend.

Common patterns:

| Symptom | Usually means | What to check next |
| --- | --- | --- |
| parse error near `==` or `!=` | numeric equality is not part of the current DSL | Use ordering comparisons instead and check [DSL and IR](dsl-and-ir.md). |
| unknown stage, output field, or workflow input | a reference does not resolve | Compare names against the source workflow and the small examples in [`../examples/`](../examples/). |
| unknown capability | the capability is not in the current catalog | Check the catalog in [DSL and IR](dsl-and-ir.md) and [`../crates/nemoir-ir/src/capabilities.rs`](../crates/nemoir-ir/src/capabilities.rs). |
| missing or unknown `exec:` arg | the deterministic stage does not match the capability signature | Check [`../crates/nemoir-dsl-fe/tests/exec_stages.rs`](../crates/nemoir-dsl-fe/tests/exec_stages.rs) and the capability catalog. |
| unreachable stage, ambiguous backward reference, or transition error | inferred control flow is not valid | Make transitions explicit or simplify the stage graph; [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) is the best public branching example. |

If `nemo check` passes but `nemo compile` fails before target generation, the lowered IR or a backend-neutral rule is the likely issue.

Useful references:

- [DSL and IR](dsl-and-ir.md)
- [Safety and limitations](safety-and-limitations.md)
- [`../crates/nemoir-dsl-fe/tests/invalid_dsls.rs`](../crates/nemoir-dsl-fe/tests/invalid_dsls.rs)
- [`../crates/nemoir-ir/tests/validate_tests.rs`](../crates/nemoir-ir/tests/validate_tests.rs)

## Python target and runtime integration

If `nemo compile --target python` fails, first read [Python target guide](targets/python.md).

Common patterns:

| Symptom | Usually means | What to check next |
| --- | --- | --- |
| compile error about workflow id or generated field names | the workflow id, input id, or exit-output field cannot become a Python identifier | Follow the naming rules in [Python target guide](targets/python.md). |
| generated package will not install | the local Python environment does not meet the generated package requirements | Use Python `>=3.11` and install in a fresh virtual environment. |
| generated package imports but cannot run a workflow | the compiler emitted a package, but no model or tools were supplied at runtime | Wire the generated package into the public runtime and register the workflow's declared capabilities. |
| looking for runtime classes or tool APIs | those live outside the compiler repo | Use the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) repository instead of expecting compiler docs to duplicate that API. |

A good compiler-only sanity check is to compile [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) and confirm that `pyproject.toml` and the generated package directory are written.

## Web target and environment problems

If `nemo compile --target web` fails, start with [Web target guide](targets/web.md) and [Compatibility](compatibility.md).

Common patterns:

| Symptom | Usually means | What to check next |
| --- | --- | --- |
| compile error mentioning `fs.read`, `fs.write`, `os.shell`, or `path` | the workflow is valid IR, but not web-compatible | The web backend accepts a narrower capability and type subset; compare with [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/). |
| compile error about `browser.js.run` or `browser.js.sandbox` | the JavaScript capability is being used outside the web backend's contract | Recheck the deterministic-stage-only rules and approval-policy rules in [Web target guide](targets/web.md). |
| `npm install` or `npm run dev` fails in a generated app | the local Node.js/npm environment is missing or mismatched | Run commands inside the generated app directory and inspect the emitted `package.json`. |
| app loads but model stages do not run | the browser environment does not provide WebGPU or the required isolation headers | Follow the local and deployment requirements in [Web target guide](targets/web.md). |
| deterministic-only workflow still needs browser checks | deterministic-only web workflows do not need WebGPU, but they still need a working browser/Node toolchain | Use a minimal workflow first, then add model stages later. |

Runtime and UI package APIs live in their public repositories:

- [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime)
- [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui)
