# Hello workflow

A minimal model-driven workflow with a typed input, an entry stage, and an exit
stage.

```bash
cargo run --package nemoir-cli -- check examples/hello-workflow/hello.nemo
cargo run --package nemoir-cli -- compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html
```

Open the generated HTML file in a browser to inspect the lowered workflow
graph. The same source can also be compiled to the Python or web targets.
