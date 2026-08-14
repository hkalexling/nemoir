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
| unknown capability | the capability is not in the current catalog | Check the catalog in [DSL and IR](dsl-and-ir.md). |
| missing or unknown `exec:` arg | the deterministic stage does not match the capability signature | Recheck the capability declaration and compare with the public examples. |
| unreachable stage, ambiguous backward reference, or transition error | inferred control flow is not valid | Make transitions explicit or simplify the stage graph; [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/) is the best public branching example. |

If `nemo check` passes but `nemo compile` fails before target generation, the lowered IR or a backend-neutral rule is the likely issue.

Useful references:

- [DSL and IR](dsl-and-ir.md)
- [Safety and limitations](safety-and-limitations.md)
- [Writing workflows](writing-workflows.md)

## Python target issues

If `nemo compile --target python` fails, first read [Python target guide](targets/python.md).

Common patterns:

| Symptom | Usually means | What to check next |
| --- | --- | --- |
| compile error about workflow id or generated field names | the workflow id, input id, or exit-output field cannot become a valid Python name | Follow the naming rules in [Python target guide](targets/python.md). |
| generated package metadata looks wrong | the compiler emitted the package, but the workflow-derived names or dependency expectations are not what you expected | Inspect the emitted `pyproject.toml` and generated package directory. |
| post-compile runtime questions | the compiler emitted a package, but execution setup belongs to the runtime layer | Use the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) docs. |

A good compiler-only sanity check is to compile [`../examples/policy-gated-edit/`](../examples/policy-gated-edit/) and confirm that `pyproject.toml` and the generated package directory are written.

## Web target issues

If `nemo compile --target web` fails, start with [Web target guide](targets/web.md) and [Compatibility](compatibility.md).

Common patterns:

| Symptom | Usually means | What to check next |
| --- | --- | --- |
| compile error mentioning `fs.read`, `fs.write`, `os.shell`, or `path` | the workflow is valid IR, but not web-compatible | The web backend accepts a narrower capability and type subset; compare with [`../examples/web-hint-tutor/`](../examples/web-hint-tutor/). |
| compile error about `browser.js.run` or `browser.js.sandbox` | the JavaScript capability is being used outside the web backend's contract | Recheck the deterministic-stage-only rules and approval-policy rules in [Web target guide](targets/web.md). |
| generated `package.json` dependency strings are not what you expected | you may need different compile-time dependency overrides for integration work | Recheck `--web-runtime-dependency` and `--web-ui-dependency` in [CLI reference](cli.md). |
| post-compile runtime or UI questions | the compiler emitted source successfully, but running it belongs to the public runtime and UI packages | Use the public [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) and [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui) pages. |

## Browser compiler questions

For the browser editor application and the published WASM package, start with:

- [Browser compiler](browser-compiler.md)
- [WASM compiler package](wasm-package.md)
