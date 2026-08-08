# Releasing

This document describes how the NemoIR compiler workspace is released:
the `nemo` CLI binary for five platform targets and the
`@nemoir/compiler-wasm` npm package.

## Canonical version

The single source of truth for the release version is
`[workspace.package].version` in the workspace root `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.0"
```

Every crate inherits that version via `version.workspace = true`. The npm
package version in `crates/nemoir-wasm/pkg/package.json` and the
`metadata().compilerVersion` reported by the WASM module are derived from
the same workspace version at build time.

**There is no manual version override.**  The Cargo workspace version is
authoritative.  `workflow_dispatch` only selects dry-run vs. publish.

## Trigger

Releases are triggered by two events in
[`.github/workflows/release.yml`](../.github/workflows/release.yml):

1. **Push to `master` that changes `Cargo.toml`** — the workflow
   compares `[workspace.package].version` in the pushed commit against
   `github.event.before`. Only an increasing canonical SemVer version
   proceeds.

2. **Manual `workflow_dispatch`** — defaults to **dry-run** (no publish).
   Set `dry_run` to `false` to publish.

### Example

```bash
# Bump version in Cargo.toml, then push to master.
# The release workflow detects the change and publishes.
sed -i 's/^version = "0\.1\.0"/version = "0.1.2"/' Cargo.toml
git add Cargo.toml
git commit -m "bump workspace version to 0.1.2"
git push origin master
```

## Release guard

The `version` job performs these checks before any build begins:

- The version string must be valid SemVer `X.Y.Z`.
- If a git tag `vX.Y.Z` already exists and points to a **different**
  commit, the workflow refuses — the version was already used.
- If a git tag `vX.Y.Z` exists at the **current** commit but a **final**
  (non-draft) GitHub Release already exists, the workflow refuses.
- If a git tag `vX.Y.Z` exists at the **current** commit and only a
  **draft** release exists, the workflow allows a rerun (partial draft
  recovery).

The `publish` job additionally refuses to publish to npm if the version
already exists on the registry.  npm is immutable; a version can never
be overwritten.

## What the workflow does

### Build jobs (unprivileged, `contents: read`)

These all run in parallel after version detection.  Every native build
job runs `nemo --help` and `nemo check` against `hello-workflow` as a
smoke test, and includes `LICENSE` and `README.md` in the archive.

| Job | Runner | What it proves |
|-----|--------|----------------|
| `verify` | `ubuntu-latest` | `cargo fmt`, `clippy`, `cargo test`, public example validation |
| `wasm` | `ubuntu-latest` | `wasm-pack build`, finalize, smoke test, version match assertion, `npm pack` |
| `vite-consumer` | `ubuntu-latest` | Consumes prebuilt tarball — no Rust/Cargo required |
| `native-linux-x64` | `ubuntu-latest` | `nemo` binary for `x86_64-unknown-linux-gnu` + smoke |
| `native-linux-arm64` | `ubuntu-24.04-arm` | `nemo` binary for `aarch64-unknown-linux-gnu` + smoke |
| `native-macos-x64` | `macos-15-intel` | `nemo` binary for `x86_64-apple-darwin` + smoke |
| `native-macos-arm64` | `macos-14` | `nemo` binary for `aarch64-apple-darwin` + smoke |
| `native-windows-x64` | `windows-latest` | `nemo.exe` binary for `x86_64-pc-windows-msvc` + smoke |

### Publish job (privileged, `contents: write` + `id-token: write`)

The publish job is the **only** job with write permissions.  It runs in
the `npm` GitHub environment and:

1. Sets up a current Node runtime (`node-version: 24`) with
   `registry-url: https://registry.npmjs.org` and ensures npm >= 11.5.1.
2. Downloads all prebuilt artifacts (does **not** check out the
   repository, does **not** execute any source code).
3. Merges artifacts into a flat `dist/` directory and generates
   `SHA256SUMS`.
4. Creates (or updates) a **draft** GitHub Release tagged `vX.Y.Z` at
   the exact release commit, and uploads all assets.
5. Publishes the prebuilt WASM tarball to npm via OIDC.
6. Creates a GitHub artifact provenance attestation when the repository is
   public, then finalizes the GitHub Release (removes draft status).

### Release assets

These files appear on each GitHub Release:

| File | Contents |
|------|----------|
| `nemo-linux-x86_64.tar.gz` | `nemo` binary, `LICENSE`, `README.md` |
| `nemo-linux-aarch64.tar.gz` | `nemo` binary, `LICENSE`, `README.md` |
| `nemo-macos-x86_64.tar.gz` | `nemo` binary, `LICENSE`, `README.md` |
| `nemo-macos-aarch64.tar.gz` | `nemo` binary, `LICENSE`, `README.md` |
| `nemo-windows-x86_64.zip` | `nemo.exe` binary, `LICENSE`, `README.md` |
| `nemoir-compiler-wasm-X.Y.Z.tgz` | The published npm tarball |
| `SHA256SUMS` | SHA-256 checksums of all of the above |

### Idempotency and re-runs

- **Partial draft recovery**: If a build or npm publish fails after the
  draft GitHub Release was created, re-running the workflow (or
  re-running the publish job) will upload assets to the existing draft
  and retry the remaining steps.
- **npm is never overwritten**: An existing registry version is accepted only
  to finish a matching draft release at the exact same commit; every other
  existing version is refused.
- **Final releases are never overwritten**: The publish job refuses to
  create or modify a release that is already published.
- **Concurrency** is set to `cancel-in-progress: false`, so two release
  workflows can never run in parallel.

## Environment and OIDC setup (one-time)

### GitHub environment

Create a GitHub environment named `npm` in the compiler repository
(Settings → Environments → New environment).  The release workflow's
`publish` job targets this environment.

### npm OIDC trust

In the npm organization that owns `@nemoir`, configure an OIDC
(OpenID Connect) trust relationship:

1. Go to the npm package or org **Access Control** settings.
2. Add a **GitHub OIDC** provider for **Trusted Publishing**.
3. Restrict it to the compiler repository and the `npm` environment.
4. Set workflow filename to `release.yml`, environment to `npm`, and grant
   **publish** permission for the `@nemoir/compiler-wasm` package.

Once configured, `npm publish` authenticates via OIDC token exchange
without any npm token stored in GitHub Secrets.  The `id-token: write`
permission on the publish job exchanges the GitHub Actions OIDC token
for a short-lived npm token automatically.

### Provenance and repository visibility

**Trusted publishing (OIDC authentication) works from private
repositories.**  npm will accept the OIDC token and publish the package
regardless of the source repository's visibility.

**npm provenance attestations require a public repository.** Trusted npm
publishing generates them automatically once the source is public. While the
repository remains private, publication still works through OIDC but npm does
not expose a public provenance attestation. No workflow change is needed when
the repository becomes public.

## No local token use

The release automation uses only the GitHub Actions `GITHUB_TOKEN` and an
OIDC exchange for npm. No long-lived npm token, Personal Access Token, or
other secret is used by the workflow. Local developer authentication remains
separate from CI.

## Linux ARM64 runner notes

The workflow uses `ubuntu-24.04-arm` for the native Linux aarch64 build.
This runner is generally available on GitHub Actions but may require
repository or organization opt-in for some plans.

If the ARM64 runner is unavailable, fall back to cross-compilation on
`ubuntu-latest`:

```yaml
native-linux-arm64:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions-rust-lang/setup-rust-toolchain@v1
      with:
        toolchain: "1.97.1"
        target: aarch64-unknown-linux-gnu
    - name: Install cross-linker
      run: |
        sudo apt-get update
        sudo apt-get install -y gcc-aarch64-linux-gnu
    - name: Build
      run: |
        cargo build --release --package nemoir-cli \
          --target aarch64-unknown-linux-gnu
    - name: Smoke test
      run: |
        # Smoke test requires either an ARM64 host or QEMU user-mode.
        # On x86_64 CI, skip the smoke test or use qemu-aarch64-static.
    - name: Archive
      run: |
        mkdir -p artifacts
        cp target/aarch64-unknown-linux-gnu/release/nemo artifacts/
        cp LICENSE README.md artifacts/
        tar -czf nemo-linux-aarch64.tar.gz -C artifacts nemo LICENSE README.md
```

The cross-compiled binary should be tested on real ARM64 hardware before
shipping as a release asset.

## Manual release checklist

When testing a release manually:

1. Open **Actions → Release → Run workflow**.
2. Keep **dry_run** checked (default).
3. Verify that all build jobs pass and artifacts are uploaded.
4. Inspect the uploaded workflow artifacts for the expected archive
   contents (each should contain `nemo` or `nemo.exe`, `LICENSE`, and
   `README.md`).
5. When ready to publish, re-run with **dry_run** unchecked.

## Related documents

- [WASM compiler package](wasm-package.md) — build and test the WASM
  package locally
- [Contributing](contributing.md) — local checks and change scope
- [CI workflow](../.github/workflows/ci.yml) — CI checks on every push
  and PR
