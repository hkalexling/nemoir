# Web hint tutor

A browser-compatible workflow that demonstrates conditional transitions, optional data flow, and `user.elicit`. It uses only capabilities supported by the web backend. With `nemo` on your `PATH`, run this command from the repository root.

```bash
nemo compile \
  examples/web-hint-tutor/hint-tutor.nemo \
  --target web \
  --output /tmp/nemoir-web-example
```

Compilation produces generated browser-application source. Runtime and UI details for running that output live in the public [`@nemoir/web-runtime`](https://github.com/hkalexling/nemoir-web-runtime) and [`@nemoir/web-ui`](https://github.com/hkalexling/nemoir-web-ui) projects.
