# Policy-gated edit

A Python-targeted workflow that makes file access and write authorization
explicit in the IR:

- reads and writes must remain inside the supplied `workspace`;
- every `fs.write` first requires an `fs.read` of the same path; and
- a user confirmation is required before the write proceeds.

Compile it to a generated Python package:

```bash
cargo run --package nemoir-cli -- compile \
  examples/policy-gated-edit/policy-gated-edit.nemo \
  --target python \
  --output /tmp/nemoir-policy-example
```

Compilation produces the workflow package only. Running it additionally
requires a NemoIR Python runtime, a model adapter, and implementations of the
capabilities declared by the workflow.
