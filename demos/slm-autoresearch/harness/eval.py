#!/usr/bin/env python3
"""SLM autoresearch harness — GLUE/MNLI evaluation driver.

Evaluates either the base model or the current LoRA adapter on a fixed MNLI
slice. Prediction is constrained log-prob scoring over candidate.py's answer
options using the exact same prompt formatter as training.

Supports arbitrary label strings (A/B/C, entailment/neutral/contradiction,
etc). Two batched scoring paths:
  - Single-token fast path: 1 forward pass per batch of 32 examples.
  - Multi-token batched path: 1 forward pass per (batch x option).

Called by the compiled workflow as:
    python harness/eval.py --adapter none
    python harness/eval.py --adapter adapter

Output: JSON result line on stdout, plus results.json and trial_log.jsonl.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import sys
from pathlib import Path
from typing import Any

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
ADAPTER_DIR = ROOT / "adapter"
BENCHMARKS_PATH = HERE / "benchmarks.yml"
RESULTS_FILE = ROOT / "results.json"

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def _log(msg: str) -> None:
    print(f"[eval] {msg}", flush=True)


def _json_line(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, sort_keys=True), flush=True)


def _parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Evaluate MNLI base model or adapter")
    p.add_argument("--profile", default=os.environ.get("NEMOIR_PROFILE", "mnli_demo"))
    p.add_argument("--adapter", choices=["none", "adapter"], default="adapter")
    p.add_argument("--split", default=os.environ.get("NEMOIR_EVAL_SPLIT", None))
    p.add_argument("--num-examples", type=int, default=None)
    p.add_argument("--slice-offset", type=int, default=int(os.environ.get("NEMOIR_EVAL_OFFSET", "0")))
    return p.parse_args()


def _load_benchmark_defaults(profile: str) -> dict[str, Any]:
    if not BENCHMARKS_PATH.exists():
        return {}
    with open(BENCHMARKS_PATH) as f:
        cfg = yaml.safe_load(f) or {}
    if cfg.get("profile") != profile:
        return cfg
    task = next(iter((cfg.get("tasks") or {}).values()), {})
    return task


def _macro_f1(y_true: list[str], y_pred: list[str], labels: list[str]) -> float:
    scores: list[float] = []
    for label in labels:
        tp = sum(1 for y, p in zip(y_true, y_pred) if y == label and p == label)
        fp = sum(1 for y, p in zip(y_true, y_pred) if y != label and p == label)
        fn = sum(1 for y, p in zip(y_true, y_pred) if y == label and p != label)
        precision = tp / (tp + fp) if (tp + fp) else 0.0
        recall = tp / (tp + fn) if (tp + fn) else 0.0
        f1 = 2 * precision * recall / (precision + recall) if (precision + recall) else 0.0
        scores.append(f1)
    return sum(scores) / len(scores) if scores else 0.0


# ── Answer tokenization ──────────────────────────────────────────────────────


def _tokenize_answer_options(tokenizer, candidate) -> list[list[int]] | None:
    """Tokenize each answer option. Returns list of token ID lists.

    Returns None if any answer tokenizes to zero tokens.
    Does NOT require single tokens — each option may have multiple tokens.
    """
    result: list[list[int]] = []
    for answer_text, _label in candidate.answer_options():
        ids = tokenizer(answer_text, add_special_tokens=False).input_ids
        if not ids:
            return None
        result.append(ids)
    return result


# ── Single-token fast path ───────────────────────────────────────────────────


def _predict_batch_single_token(
    model,
    tokenizer,
    candidate,
    examples: list[dict[str, Any]],
    answer_token_ids: list[int],
    max_seq_length: int,
    batch_size: int = 32,
) -> list[str]:
    """Fast path: 1 forward pass per batch.

    All answer options must be single tokens. The next-token logits at the
    last real position are compared across the candidate answer token ids.
    """
    import torch
    import torch.nn.functional as F

    pad_id = tokenizer.pad_token_id or tokenizer.eos_token_id
    results: list[str] = []

    for start in range(0, len(examples), batch_size):
        batch = examples[start : start + batch_size]

        max_len = max_seq_length - 1
        all_ids: list[list[int]] = []
        for ex in batch:
            prompt = candidate.format_prompt(ex)
            ids = tokenizer(
                prompt, add_special_tokens=True, truncation=True, max_length=max_len,
            ).input_ids
            if not ids:
                ids = [tokenizer.eos_token_id]
            all_ids.append(ids)

        batch_max = max(len(ids) for ids in all_ids)
        input_ids: list[list[int]] = []
        attn_mask: list[list[int]] = []
        last_idx: list[int] = []
        for ids in all_ids:
            pad_len = batch_max - len(ids)
            input_ids.append(ids + [pad_id] * pad_len)
            attn_mask.append([1] * len(ids) + [0] * pad_len)
            last_idx.append(len(ids) - 1)

        input_tensor = torch.tensor(input_ids, device=model.device)
        attn_tensor = torch.tensor(attn_mask, device=model.device)
        last_tensor = torch.tensor(last_idx, device=model.device)
        answer_tensor = torch.tensor(answer_token_ids, device=model.device)

        with torch.inference_mode():
            logits = model(input_ids=input_tensor, attention_mask=attn_tensor).logits
            bsz = logits.shape[0]
            last_logits = logits[torch.arange(bsz), last_tensor, :]
            log_probs = F.log_softmax(last_logits.float(), dim=-1)
            option_scores = log_probs[:, answer_tensor]
            best = option_scores.argmax(dim=-1)

        for opt_idx in best.tolist():
            results.append(candidate.LABELS[int(opt_idx)])

    return results


# ── Multi-token batched path ─────────────────────────────────────────────────


def _predict_batch_multitoken(
    model,
    tokenizer,
    candidate,
    examples: list[dict[str, Any]],
    answer_token_ids_list: list[list[int]],
    max_seq_length: int,
    batch_size: int = 32,
) -> list[str]:
    """Batched multi-token path: 1 forward pass per (batch x option).

    Works for any answer length. For each option, builds prompt+answer
    sequences for a batch of examples, runs one forward pass, and extracts
    the answer-token log-probs at the correct positions.

    ~3 forward passes per batch (one per option) instead of 3 per example.
    """
    import torch
    import torch.nn.functional as F

    pad_id = tokenizer.pad_token_id or tokenizer.eos_token_id
    num_options = len(answer_token_ids_list)
    max_answer_len = max(len(ids) for ids in answer_token_ids_list)
    results: list[str] = []

    for start in range(0, len(examples), batch_size):
        batch = examples[start : start + batch_size]

        # Tokenize prompts once (truncated to leave room for the longest answer).
        prompt_budget = max_seq_length - max_answer_len
        if prompt_budget < 8:
            prompt_budget = 8
        prompt_ids_list: list[list[int]] = []
        for ex in batch:
            prompt = candidate.format_prompt(ex)
            ids = tokenizer(
                prompt, add_special_tokens=True, truncation=True, max_length=prompt_budget,
            ).input_ids
            if not ids:
                ids = [tokenizer.eos_token_id]
            prompt_ids_list.append(ids)

        # Score each option: scores[i] = list of per-option scores for example i.
        scores: list[list[float]] = [[float("-inf")] * num_options for _ in range(len(batch))]

        for opt_idx in range(num_options):
            answer_ids = answer_token_ids_list[opt_idx]
            ans_len = len(answer_ids)

            # Build prompt + answer sequences for this option.
            input_ids_list: list[list[int]] = []
            answer_starts: list[int] = []
            for pids in prompt_ids_list:
                max_p = max_seq_length - ans_len
                if len(pids) > max_p:
                    pids = pids[:max_p]
                input_ids_list.append(pids + answer_ids)
                # Logits at position (len(pids) - 1) predict the first answer token.
                answer_starts.append(len(pids) - 1)

            batch_max = max(len(ids) for ids in input_ids_list)
            padded = [ids + [pad_id] * (batch_max - len(ids)) for ids in input_ids_list]
            attn = [[1] * len(ids) + [0] * (batch_max - len(ids)) for ids in input_ids_list]

            input_tensor = torch.tensor(padded, device=model.device)
            attn_tensor = torch.tensor(attn, device=model.device)
            answer_tensor = torch.tensor(answer_ids, device=model.device).unsqueeze(1)  # [ans_len, 1]

            with torch.inference_mode():
                logits = model(input_ids=input_tensor, attention_mask=attn_tensor).logits
                for i in range(len(batch)):
                    s = answer_starts[i]
                    relevant = logits[i, s : s + ans_len, :]  # [ans_len, vocab]
                    log_probs = F.log_softmax(relevant.float(), dim=-1)  # [ans_len, vocab]
                    token_lps = log_probs.gather(1, answer_tensor).squeeze(1)  # [ans_len]
                    scores[i][opt_idx] = float(token_lps.sum().item()) / ans_len

        for i in range(len(batch)):
            best_opt = max(range(num_options), key=lambda o: scores[i][o])
            results.append(candidate.LABELS[best_opt])

    return results


# ── Main ──────────────────────────────────────────────────────────────────────


def main() -> int:
    args = _parse_args()
    defaults = _load_benchmark_defaults(args.profile)
    split = args.split or defaults.get("split", "validation_matched")

    _log("Importing candidate module...")
    try:
        import candidate  # noqa: F401
    except Exception:
        import traceback

        traceback.print_exc()
        _log("FATAL: cannot import candidate.py")
        _json_line({"ok": False, "score": 0.0, "error": "candidate_import_failed"})
        return 1

    n_examples = args.num_examples
    if n_examples is None:
        n_examples = int(os.environ.get("NEMOIR_EVAL_EXAMPLES", defaults.get("num_examples", candidate.EVAL_EXAMPLES)))

    try:
        import torch
        from datasets import load_dataset
        from peft import PeftModel
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    except ImportError as e:
        _log(f"Missing dependency: {e}")
        _json_line({"ok": False, "score": 0.0, "error": f"missing_dependency: {e}"})
        return 1

    if not torch.cuda.is_available():
        _log("ERROR: CUDA not available")
        _json_line({"ok": False, "score": 0.0, "error": "cuda_unavailable"})
        return 1

    _log(f"Profile: {args.profile}")
    _log(f"Split: {split}, n={n_examples}, offset={args.slice_offset}, adapter={args.adapter}")

    _log("Loading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(candidate.BASE_MODEL, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    _log("Loading model...")
    compute_dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
    model_kwargs: dict[str, Any] = {"device_map": "auto", "trust_remote_code": True}
    if candidate.USE_QLORA:
        model_kwargs["quantization_config"] = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=compute_dtype,
        )
        model_kwargs["torch_dtype"] = compute_dtype

    base_model = AutoModelForCausalLM.from_pretrained(candidate.BASE_MODEL, **model_kwargs)

    if args.adapter == "adapter":
        if not ADAPTER_DIR.exists():
            _log(f"ERROR: adapter directory not found: {ADAPTER_DIR}")
            _json_line({"ok": False, "score": 0.0, "error": "adapter_missing"})
            return 1
        _log(f"Loading adapter from {ADAPTER_DIR}...")
        model = PeftModel.from_pretrained(base_model, str(ADAPTER_DIR))
    else:
        _log("Evaluating base model without adapter")
        model = base_model
    model.eval()

    _log("Loading GLUE/MNLI eval split...")
    ds = load_dataset("nyu-mll/glue", "mnli", split=split)
    ds = ds.shuffle(seed=int(candidate.SEED))
    if args.slice_offset:
        max_start = max(0, len(ds) - 1)
        start = min(args.slice_offset, max_start)
    else:
        start = 0
    end = min(start + int(n_examples), len(ds))
    ds = ds.select(range(start, end))

    # Tokenize answer options once, then choose scoring path.
    answer_tokens = _tokenize_answer_options(tokenizer, candidate)
    if answer_tokens is None:
        _json_line({"ok": False, "score": 0.0, "error": "answer_options_empty"})
        return 1

    all_single = all(len(ids) == 1 for ids in answer_tokens)

    y_true: list[str] = []
    y_pred: list[str] = []
    correct = 0
    total = 0

    examples_list = list(ds)
    if all_single:
        _log("Scoring with batched single-token logits (fast path)...")
        single_ids = [ids[0] for ids in answer_tokens]
        y_pred = _predict_batch_single_token(
            model, tokenizer, candidate, examples_list,
            single_ids, int(candidate.MAX_SEQ_LENGTH), batch_size=32,
        )
    else:
        _log("Scoring with batched multi-token logits (multi-token path)...")
        y_pred = _predict_batch_multitoken(
            model, tokenizer, candidate, examples_list,
            answer_tokens, int(candidate.MAX_SEQ_LENGTH), batch_size=32,
        )

    for raw in examples_list:
        example = dict(raw)
        try:
            expected = candidate.label_from_example(example)
            y_true.append(expected)
            total += 1
        except Exception as e:
            _log(f"WARNING: example label failed: {e}")
            total += 1

    # Align y_pred length to y_true.
    y_pred = y_pred[: len(y_true)]
    correct = sum(1 for y, p in zip(y_true, y_pred) if y == p)

    accuracy = correct / total if total else 0.0
    macro_f1 = _macro_f1(y_true, y_pred, list(candidate.LABELS))
    score = accuracy

    result = {
        "ok": True,
        "profile": args.profile,
        "adapter": args.adapter,
        "split": split,
        "num_examples": total,
        "slice_offset": args.slice_offset,
        "scoring_path": "single_token" if all_single else "multi_token",
        "score": round(score, 6),
        "accuracy": round(accuracy, 6),
        "macro_f1": round(macro_f1, 6),
        "correct": correct,
        "label_counts": {label: y_true.count(label) for label in candidate.LABELS},
        "pred_counts": {label: y_pred.count(label) for label in candidate.LABELS},
    }

    RESULTS_FILE.write_text(json.dumps(result, indent=2, sort_keys=True))
    adapter_results = ROOT / ("base_results.json" if args.adapter == "none" else "adapter_results.json")
    adapter_results.write_text(json.dumps(result, indent=2, sort_keys=True))

    log_entry = dict(result)
    log_entry["timestamp"] = _dt.datetime.now().isoformat()
    with open(ROOT / "trial_log.jsonl", "a") as lf:
        lf.write(json.dumps(log_entry, sort_keys=True) + "\n")

    _json_line(result)
    _log(f"SLMScore/accuracy: {score:.4f}, macro_f1={macro_f1:.4f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
