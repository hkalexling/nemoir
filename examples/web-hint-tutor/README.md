# Web hint tutor

A browser-compatible workflow that demonstrates conditional transitions,
optional data flow, and `user.elicit`. It uses only capabilities supported by
the web backend.

```bash
cargo run --package nemoir-cli -- compile \
  examples/web-hint-tutor/hint-tutor.nemo \
  --target web \
  --output /tmp/nemoir-web-example

cd /tmp/nemoir-web-example/hint-tutor
npm install
npm run dev
```

Running the generated application requires a WebGPU-capable browser because its
model stages use the NemoIR web runtime's local WebLLM path.
