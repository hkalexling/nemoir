# Covertype XGBoost Autoresearch

A NemoIR research proof of concept for a bounded, auditable XGBoost
configuration search on UCI Covertype. It is not an AutoML product and makes
no ecological or medical claim.

## Trust boundary

- `candidate.json` is the only model-writable artifact. It is declarative JSON
  with a strict, frozen schema.
- `harness/`, `configs/baseline.json`, split manifests, metrics, and models are
  trusted evaluator inputs/outputs. The workflow restricts model reads to
  `candidate.json` and sanitized aggregate history, and restricts model writes
  to `candidate.json`.
- The harness owns Covertype retrieval, labels, split generation, feature
  implementations, XGBoost invariants, metrics, acceptance inputs, model
  snapshots, and final test evaluation.
- NemoIR policy gates model-visible tool calls. Run real experiments under an
  appropriately restricted account/container; policies do not sandbox the
  trusted harness or other local processes.

> **Viewer first:** open [`demo.ipynb`](demo.ipynb) on GitHub — the full 32-trial trace + 5 dashboard figures are pre-rendered, no dataset or API key needed.

## Setup

Use a Python 3.11 or 3.12 environment (the host's Python 3.14 may not have
compatible ML wheels):

```bash
git clone https://github.com/hkalexling/nemoir.git
cd nemoir/public/demos/xgboost-autoresearch
python -m venv .venv
.venv/bin/pip install -r requirements.txt
```

`requirements/lock/py312-linux.txt` records the environment used for the
local CPU feasibility check; regenerate a lock for other platforms.

Prepare and fingerprint the public data before an agent run:

```bash
.venv/bin/python harness/data.py prepare
```

This creates ignored `data/` artifacts, including `dataset_manifest.json`,
locked split indices, and `split_manifest.json`. The harness records UCI
Covertype attribution/source/license and a canonical content fingerprint.

Compile the workflow; generated files are intentionally not hand-edited or
committed:

```bash
nemo check autoresearch.nemo
nemo compile autoresearch.nemo --target python -o . --dump-ir
# or: cargo run -p nemoir-cli --manifest-path ../../../compiler/Cargo.toml -- check autoresearch.nemo
```

Set `NEMOIR_MODEL` and `NEMOIR_API_KEY` in `.env` (copy `.env.example`), then
start a bounded run:

```bash
.venv/bin/python run.py --device cpu --max-trials 10 --eps 0.002
```

CUDA is optional. `--device cuda` is accepted only by the frozen driver/harness
and is never candidate-controlled.

## Protocol

The harness creates fixed stratified `fit`, `early_stop`, `selection`,
`confirmation`, and `final` partitions. It trains against early-stop data,
optimizes primary selection macro-F1, confirms every positive primary result on
the separate confirmation split, and lets the compiled numeric guard decide
whether the combined result clears `eps`. The final split is evaluated only at
workflow exit for the frozen baseline and final accepted incumbent.

Each unique run directory contains a launch manifest, event JSONL trace,
frozen-file hashes, sanitized history, candidate/parent snapshots, metrics, and
accept/reject decisions. Every rejection records a stable reason code plus the
relevant candidate/incumbent scores, delta and epsilon, or explicitly marks a
score unavailable when a preflight/train/evaluation stage failed. The next
analysis stage receives the same evidence. Rejected trial models are not
retained as incumbents.

## Inline post-run dashboard

After the workflow finishes, install the optional plotting dependency and call
the read-only helper from a notebook or IPython cell:

```bash
.venv/bin/pip install -r requirements/plots.txt
```

```python
from plot_run import plot_run

# Use the run id printed by run.py.
dashboard = plot_run("runs/20260719T075416Z-b08eef4e")
dashboard.summary
```

In Colab, run the same Python cell from the extracted demo directory after
`run.py` exits. The helper renders five inline Matplotlib figures: validation
score trajectory, trial outcomes/training cost, hyperparameter trajectory,
selection-vs-confirmation comparison, and a separately labelled held-out final
test comparison. It only reads the run evidence bundle and does not invoke the
model, evaluator, or data loader.

For non-interactive validation or scripts, suppress rendering with
`plot_run(run_dir, show=False)` or run:

```bash
.venv/bin/python plot_run.py runs/20260719T075416Z-b08eef4e --no-show --print-summary
```

## Viewer notebook

[`demo.ipynb`](demo.ipynb) (and the legacy [`xgboost-autoresearch.ipynb`](xgboost-autoresearch.ipynb)) contain **pre-rendered outputs**: the `harness/data.py prepare` fingerprint, the full `run.py --device cuda --max-trials 32` stream, and `plot_run(run_dir)`'s 5 figures + `dashboard.summary`. View on GitHub without downloading Covertype. To re-run locally, execute the notebook after `harness/data.py prepare`.

## Development checks

```bash
nemo check autoresearch.nemo
nemo compile autoresearch.nemo --target python -o /tmp/xgb-package
pytest -m "not slow"  # synthetic harness, no Covertype download
```

The demo tests use synthetic data, a fake model, and a fake deterministic
harness; they do not download Covertype or contact an LLM provider.
