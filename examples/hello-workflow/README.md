# Hello workflow

A minimal model-driven workflow with a typed input, an entry stage, and an exit stage. With `nemo` on your `PATH`, run these commands from the repository root.

```bash
nemo check examples/hello-workflow/hello.nemo
nemo compile \
  examples/hello-workflow/hello.nemo \
  --target visualizer \
  --output /tmp/hello-workflow.html
```

Open the generated HTML file in a browser to inspect the lowered workflow graph. The same source can also be compiled to the Python or web targets.
