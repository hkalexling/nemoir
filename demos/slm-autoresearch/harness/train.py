#!/usr/bin/env python3
"""SLM autoresearch harness — MNLI LoRA/QLoRA training driver.

The harness imports the single mutable artifact, candidate.py, but all training
mechanics live here and remain immutable.  Training uses a shared MNLI prompt
from candidate.py and masks prompt tokens with -100 so the LoRA is optimized
only for the answer token(s), not for reproducing the premise/hypothesis text.

Called by the compiled workflow as:
    python harness/train.py

Environment overrides used by run.py / Colab:
    NEMOIR_TRAIN_EXAMPLES

Output: adapter saved to adapter/ and a final JSON line on stdout.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import shutil
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
ADAPTER_DIR = ROOT / "adapter"
CHECKPOINT_DIR = ROOT / "checkpoints"

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def _log(msg: str) -> None:
    print(f"[train] {msg}", flush=True)


def _json_line(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, sort_keys=True), flush=True)


def _parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Train MNLI LoRA adapter")
    p.add_argument("--profile", default=os.environ.get("NEMOIR_PROFILE", "mnli_demo"))
    p.add_argument("--train-examples", type=int, default=None)
    return p.parse_args()


def _int_env(name: str, default: int) -> int:
    val = os.environ.get(name)
    if val is None or val == "":
        return default
    return int(val)


class AnswerOnlyCollator:
    """Pad causal-LM features while preserving precomputed -100 labels."""

    def __init__(self, tokenizer) -> None:
        self.tokenizer = tokenizer

    def __call__(self, features: list[dict[str, list[int]]]) -> dict[str, Any]:
        import torch

        max_len = max(len(f["input_ids"]) for f in features)
        pad_id = self.tokenizer.pad_token_id
        if pad_id is None:
            pad_id = self.tokenizer.eos_token_id

        batch_input_ids: list[list[int]] = []
        batch_attention: list[list[int]] = []
        batch_labels: list[list[int]] = []

        for f in features:
            input_ids = list(f["input_ids"])
            labels = list(f["labels"])
            pad_len = max_len - len(input_ids)
            batch_input_ids.append(input_ids + [pad_id] * pad_len)
            batch_attention.append([1] * len(input_ids) + [0] * pad_len)
            batch_labels.append(labels + [-100] * pad_len)

        return {
            "input_ids": torch.tensor(batch_input_ids, dtype=torch.long),
            "attention_mask": torch.tensor(batch_attention, dtype=torch.long),
            "labels": torch.tensor(batch_labels, dtype=torch.long),
        }


def _build_answer_only_features(candidate, tokenizer, dataset, max_seq_length: int) -> tuple[list[dict[str, list[int]]], int]:
    """Tokenize MNLI examples; mask all prompt tokens in labels."""
    records: list[dict[str, list[int]]] = []
    skipped = 0
    eos = [tokenizer.eos_token_id] if tokenizer.eos_token_id is not None else []

    for raw in dataset:
        example = dict(raw)
        try:
            prompt_text, answer_text = candidate.format_training_example(example)
            answer_ids = tokenizer(answer_text, add_special_tokens=False).input_ids
            if not answer_ids:
                skipped += 1
                continue

            budget_for_prompt = max_seq_length - len(answer_ids) - len(eos)
            if budget_for_prompt < 8:
                skipped += 1
                continue
            prompt_ids = tokenizer(
                prompt_text,
                add_special_tokens=True,
                truncation=True,
                max_length=budget_for_prompt,
            ).input_ids

            input_ids = prompt_ids + answer_ids + eos
            labels = [-100] * len(prompt_ids) + answer_ids + eos
            records.append({"input_ids": input_ids, "labels": labels})
        except Exception:
            skipped += 1

    return records, skipped


def main() -> int:
    args = _parse_args()
    _log(f"Profile: {args.profile}")
    if args.profile != "mnli_demo":
        _log(f"WARNING: unknown profile {args.profile!r}; using MNLI harness")

    _log("Importing candidate module...")
    try:
        import candidate  # noqa: F401
    except Exception:
        import traceback

        traceback.print_exc()
        _log("FATAL: cannot import candidate.py")
        _json_line({"ok": False, "error": "candidate_import_failed"})
        return 1

    try:
        import torch
        from datasets import Dataset, load_dataset
        from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
        from transformers import (
            AutoModelForCausalLM,
            AutoTokenizer,
            BitsAndBytesConfig,
            Trainer,
            TrainingArguments,
        )
    except ImportError as e:
        _log(f"Missing dependency: {e}")
        _json_line({"ok": False, "error": f"missing_dependency: {e}"})
        return 1

    if not torch.cuda.is_available():
        _log("ERROR: CUDA not available")
        _json_line({"ok": False, "error": "cuda_unavailable"})
        return 1
    _log(f"GPU: {torch.cuda.get_device_name(0)}")

    train_examples = args.train_examples
    if train_examples is None:
        train_examples = _int_env("NEMOIR_TRAIN_EXAMPLES", int(candidate.TRAIN_EXAMPLES))
    _log(f"Base model: {candidate.BASE_MODEL}")
    _log(f"Train examples: {train_examples}")

    # Clean per-trial outputs so eval never sees a stale adapter.
    for path in (ADAPTER_DIR, CHECKPOINT_DIR):
        if path.exists():
            shutil.rmtree(path)

    _log("Loading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(candidate.BASE_MODEL, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"

    _log("Loading model...")
    compute_dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
    model_kwargs: dict[str, Any] = {
        "device_map": "auto",
        "trust_remote_code": True,
    }
    if candidate.USE_QLORA:
        model_kwargs["quantization_config"] = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=compute_dtype,
            bnb_4bit_use_double_quant=True,
        )
        model_kwargs["torch_dtype"] = compute_dtype

    model = AutoModelForCausalLM.from_pretrained(candidate.BASE_MODEL, **model_kwargs)
    if candidate.USE_QLORA:
        model = prepare_model_for_kbit_training(model)

    lora_config = LoraConfig(
        r=int(candidate.LORA_R),
        lora_alpha=int(candidate.LORA_ALPHA),
        lora_dropout=float(candidate.LORA_DROPOUT),
        target_modules=list(candidate.LORA_TARGET_MODULES),
        bias="none",
        task_type="CAUSAL_LM",
    )
    model = get_peft_model(model, lora_config)
    model.print_trainable_parameters()

    _log("Loading GLUE/MNLI train split...")
    raw = load_dataset("nyu-mll/glue", "mnli", split="train")
    raw = raw.shuffle(seed=int(candidate.SEED)).select(range(min(train_examples, len(raw))))

    _log("Tokenizing with answer-only loss mask...")
    records, skipped = _build_answer_only_features(
        candidate,
        tokenizer,
        raw,
        max_seq_length=int(candidate.MAX_SEQ_LENGTH),
    )
    if not records:
        _log("ERROR: no training examples after formatting/tokenization")
        _json_line({"ok": False, "error": "no_training_examples"})
        return 1
    _log(f"Usable training examples: {len(records)} (skipped={skipped})")
    train_dataset = Dataset.from_list(records)

    training_args = TrainingArguments(
        output_dir=str(CHECKPOINT_DIR),
        max_steps=int(candidate.MAX_STEPS),
        per_device_train_batch_size=int(candidate.PER_DEVICE_BATCH_SIZE),
        gradient_accumulation_steps=int(candidate.GRAD_ACCUM_STEPS),
        learning_rate=float(candidate.LEARNING_RATE),
        lr_scheduler_type=str(candidate.LR_SCHEDULER),
        warmup_steps=int(candidate.WARMUP_STEPS),
        optim=str(candidate.OPTIMIZER),
        logging_steps=10,
        save_strategy="no",
        report_to="none",
        bf16=bool(torch.cuda.is_bf16_supported()),
        fp16=not bool(torch.cuda.is_bf16_supported()),
        seed=int(candidate.SEED),
        remove_unused_columns=False,
    )

    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        data_collator=AnswerOnlyCollator(tokenizer),
    )

    _log("Training...")
    output = trainer.train()
    train_loss = float(getattr(output, "training_loss", 0.0) or 0.0)

    ADAPTER_DIR.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(ADAPTER_DIR))
    tokenizer.save_pretrained(str(ADAPTER_DIR))
    _log(f"Adapter saved to {ADAPTER_DIR}")

    trial_info = {
        "timestamp": _dt.datetime.now().isoformat(),
        "profile": args.profile,
        "base_model": candidate.BASE_MODEL,
        "train_examples": len(records),
        "skipped_examples": skipped,
        "train_loss": train_loss,
        "lora_r": candidate.LORA_R,
        "lora_alpha": candidate.LORA_ALPHA,
        "lora_dropout": candidate.LORA_DROPOUT,
        "lora_target_modules": list(candidate.LORA_TARGET_MODULES),
        "learning_rate": candidate.LEARNING_RATE,
        "lr_scheduler": candidate.LR_SCHEDULER,
        "warmup_steps": candidate.WARMUP_STEPS,
        "max_steps": candidate.MAX_STEPS,
        "batch_size": candidate.PER_DEVICE_BATCH_SIZE,
        "grad_accum": candidate.GRAD_ACCUM_STEPS,
        "max_seq_length": candidate.MAX_SEQ_LENGTH,
    }
    (ROOT / "last_trial.json").write_text(json.dumps(trial_info, indent=2, sort_keys=True))

    _json_line({
        "ok": True,
        "train_examples": len(records),
        "skipped_examples": skipped,
        "train_loss": round(train_loss, 6),
        "adapter_dir": str(ADAPTER_DIR),
    })
    _log("Training complete.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
