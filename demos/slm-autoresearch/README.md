# SLM Autoresearch — MNLI LoRA Demo

A NemoIR research proof of concept that compiles a Karpathy-style autoresearch loop for small-language-model LoRA post-training into a policy-constrained, statically checkable workflow. The mutable artifact is `candidate.py` only; the frozen evaluator is `harness/*` + `harness/benchmarks.yml`.

> **Viewer first:** open [`demo.ipynb`](demo.ipynb) on GitHub — the full 25-trial log (base 0.3415 → best 0.7405) is pre-rendered, no GPU or API key needed. The notebook itself is documentation for the workflow's behavior.

## Trust boundary

| Owner | Surface |
|---|---|
| model (read) | `candidate.py`, sanitized `runs/current/trial_history.jsonl` via `fs.read` |
| model (write) | `candidate.py` only (`deny fs.write(path) if not path.eq("candidate.py")`) |
| harness (exclusive) | `harness/train.py`, `harness/eval.py`, `harness/state.py`, `harness/preflight.py`, `harness/benchmarks.yml`, adapter snapshots, trial accounting, `runs/current/` |
| compiled workflow | `score - best > eps` numeric guard on `JudgeCandidate`; `before fs.write` policy; `os.shell` literal allowlist |

NemoIR policies gate model-visible tool calls. Candidate training runs under `harness/train.py`'s wall-clock timeout (`19m` by default). Deploy real campaigns under an appropriately restricted account/container — policies do not sandbox the harness.

## Setup

Python 3.11+ plus a modest HF stack. On a fresh checkout:

```bash
git clone https://github.com/hkalexling/nemoir.git
cd nemoir/public/demos/slm-autoresearch

python -m venv .venv
.venv/bin/pip install -e .            # installs nemoir-runtime + demo package
.venv/bin/pip install bitsandbytes    # Colab parity; optional for CPU-only eval
# or: .venv/bin/pip install -r requirements.txt  # if present
```

`harness/benchmarks.yml` pins the public profile: **GLUE/MNLI** `nyu-mll/glue` `mnli` — `train` (4000 reserve, `TRAIN_EXAMPLES=500` default) and `validation_matched` (2000 eval, `EVAL_EXAMPLES=2000`).

Compile the workflow (generated files are intentionally not committed):

```bash
nemo check autoresearch.nemo
nemo compile autoresearch.nemo --target python -o . --dump-ir
# or: cargo run -p nemoir-cli --manifest-path ../../..//compiler/Cargo.toml -- check autoresearch.nemo
```

Set `NEMOIR_MODEL` and `NEMOIR_API_KEY` in `.env` (copy `.env.example`) or in the environment — any LiteLLM-compatible provider works (`openai/gpt-4o-mini`, `deepseek/deepseek-chat`, `anthropic/claude-3-5-sonnet`, or an OpenAI-compatible base URL via `NEMOIR_API_BASE`), just like `xgboost-autoresearch` and `cvxpygen-autoresearch`. Then start a bounded run:

```bash
cp .env.example .env  # then edit NEMOIR_MODEL / NEMOIR_API_KEY
.venv/bin/python run.py --model openai/gpt-4o-mini --max-trials 5 --eps 0.01
# knobs: --model, --api-key, --api-base, --profile mnli_demo, --eval-split validation_matched, --train-timeout-seconds 1140, --max-trials 25
```

CUDA is not required. Candidate `USE_QLORA=true` is the harness default; the demo runs on CPU for evaluation but benefits from GPU for `bitsandbytes`.

## Protocol

The harness fixes the dataset, label contract (`ID_TO_LABEL: 0=entailment→A, 1=neutral→B, 2=contradiction→C`), prompt (`format_prompt`), answer scoring (`format_answer` leading-space single-token vs multi-token), and metric (accuracy on `validation_matched`). Each trial:

1. `Setup` reads `candidate.py` + `benchmarks.yml`.
2. `StateInit` + `BaseModelEval` (zero-shot) + `InitialRecipeTrain/Eval` → `AcceptInitial`.
3. Loop: `StateStartTrial` (restore incumbent) → `AnalyzeHistory`→`ProposeRecipePatch`→`ApplyPatch`→`Preflight`→`TrainAdapter`→`BenchmarkAdapter`→`JudgeCandidate` (`score - best > eps` compiled guard) → `Accept/Reject` → `should-continue`.

Every rejection is logged with a reason code and visible to the next `AnalyzeHistory`. `runs/current/trial_history.jsonl` contains the full audit.

## Viewer notebook

[`demo.ipynb`](demo.ipynb) contains **pre-rendered outputs** for the entire 25-trial campaign, so you can see the model's tool calls, scores, and final report on GitHub without running anything:

- Cells 1–2 are the Colab `gdown` bootstrap — **skip on local checkout** (you already have the code).
- Last cell's log is the evidence viewer: `Setup 0.3415 → Accepts at 0.377/0.398/0.4945/0.557/0.6425/0.66/0.722/0.7405`.

For a local re-run that regenerates the log, execute the notebook's final `!python run.py --model $NEMOIR_MODEL --max-trials 25` cell after setting `NEMOIR_API_KEY` (or `OPENAI_API_KEY` / `DEEPSEEK_API_KEY`).

## Development checks

```bash
nemo check autoresearch.nemo
nemo compile autoresearch.nemo --target python -o /tmp/slm-pkg
python -m py_compile harness/*.py candidate.py
# tests are in the private meta repo; public demo ships the viewer notebook as its test
```
