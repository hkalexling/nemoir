# Web Interview Tutor — external demo

This demo is **not** vendored in the `nemoir` public repo. It lives in its own repository to keep its GitHub Actions deploy pipeline at the repo root (GitHub does not run workflows from subdirectories).

**Source:** [`hkalexling/nemoir-web-interview-tutor`](https://github.com/hkalexling/nemoir-web-interview-tutor)

**Live deployments:**
- **Cloudflare Pages (full, including WebLLM):** https://nemoir-web-interview-tutor.pages.dev/
- **GitHub Pages (deterministic runner only, WebLLM needs COOP/COEP):** https://hkalexling.github.io/nemoir-web-interview-tutor/

**Workflows:**
- `workflows/interview_tutor.nemo` — evidence-backed WebLLM tutoring (`browser.js.run` + `browser.storage` + `user.elicit`)
- `workflows/interview_test_runner.nemo` — deterministic sandbox evaluator (`browser.js.sandbox` with `before browser.js.sandbox(code) requires user.confirm`)

**Run locally:**
```bash
git clone https://github.com/hkalexling/nemoir-web-interview-tutor
cd nemoir-web-interview-tutor/app
npm ci && npm run dev
```

Private meta checkout still vendors it as a submodule at `demos/web-interview-tutor/` for local `rsync` sync during development, but `public/demos/web-interview-tutor/` in this repo is intentionally a pointer — see the standalone repo for `app/src/generated/` and CI.

Live demo built from the standalone repo, not from this directory.
