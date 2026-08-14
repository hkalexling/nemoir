# Policy-gated edit

A Python-targeted workflow that makes file access and write authorization explicit in the IR:

- reads and writes must remain inside the supplied `workspace`;
- every `fs.write` first requires an `fs.read` of the same path; and
- a user confirmation is required before the write proceeds.

With `nemo` on your `PATH`, compile it to a generated Python package from the repository root:

```bash
nemo compile \
  examples/policy-gated-edit/policy-gated-edit.nemo \
  --target python \
  --output /tmp/nemoir-policy-example
```

Compilation produces the workflow package only. Runtime setup and execution are documented in the public [`nemoir-runtime`](https://github.com/hkalexling/nemoir-python-runtime) project.
