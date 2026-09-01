# candidate.py — Sole mutable artifact for the SLM autoresearch demo.
#
# The immutable harness (harness/*.py) imports these values and hooks.
# The autoresearch agent may edit this file between trials; NemoIR policies
# deny writes to the evaluator/harness.
#
# Benchmark profile: GLUE MNLI natural-language inference.
# Base model:      HuggingFaceTB/SmolLM2-360M (weaker → more headroom for LoRA).
# Search space:    LoRA rank/alpha/dropout/targets, learning rate, scheduler,
# warmup, max steps, batch size, max sequence length, train examples,
# and prompt/label wording.
#
# Label contract: LABELS, ID_TO_LABEL, label_from_example, format_answer,
# and answer_options must be mutually consistent (preflight checks this).
# You may use single-letter labels ("A"/"B"/"C") or natural-language labels
# ("entailment"/"neutral"/"contradiction") — the harness supports both.
# If all answer options are single tokens, a fast batched eval path is used;
# multi-token answers use a slightly slower batched path.

BASE_MODEL = "HuggingFaceTB/SmolLM2-360M"

# ── LoRA / QLoRA -------------------------------------------------------------
# Very weak defaults — the autoresearch agent should discover better values.
LORA_R = 8
LORA_ALPHA = 16
LORA_DROPOUT = 0.0
LORA_TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj"]
USE_QLORA = True

# ── Optimisation -------------------------------------------------------------
LEARNING_RATE = 2e-4
LR_SCHEDULER = "cosine"
WARMUP_STEPS = 10
MAX_STEPS = 50
PER_DEVICE_BATCH_SIZE = 4
GRAD_ACCUM_STEPS = 2
MAX_SEQ_LENGTH = 512
OPTIMIZER = "adamw_8bit"
SEED = 42

# ── Data budget --------------------------------------------------------------
# Kept in candidate.py so the agent can trade compute against quality.
TRAIN_EXAMPLES = 500
EVAL_EXAMPLES = 2000

# ── MNLI label contract ------------------------------------------------------
# HuggingFace GLUE/MNLI label ids: 0=entailment, 1=neutral, 2=contradiction.
LABELS = ["A", "B", "C"]
LABEL_TO_TEXT = {
    "A": "entailment",
    "B": "neutral",
    "C": "contradiction",
}
ID_TO_LABEL = {
    0: "A",
    1: "B",
    2: "C",
}

# Deliberately minimal: no task description, no options listed.
# The agent should discover that listing options with their meanings helps.
SYSTEM_INSTRUCTION = ""


def format_prompt(example: dict) -> str:
    """Return the exact prompt used by both training and evaluation."""
    premise = str(example["premise"]).strip()
    hypothesis = str(example["hypothesis"]).strip()
    # No options listed — the model must "know" that A/B/C are the only choices.
    return (
        f"Premise:\n{premise}\n\n"
        f"Hypothesis:\n{hypothesis}\n\n"
        "Answer:"
    )


def label_from_example(example: dict) -> str:
    """Map a GLUE/MNLI example to A/B/C."""
    label_id = int(example["label"])
    if label_id not in ID_TO_LABEL:
        raise ValueError(f"unexpected MNLI label id: {label_id}")
    return ID_TO_LABEL[label_id]


def format_answer(example_or_label) -> str:
    """Return the answer string. Leading space is intentional for LM scoring."""
    if isinstance(example_or_label, str):
        label = example_or_label
    else:
        label = label_from_example(example_or_label)
    if label not in LABELS:
        raise ValueError(f"unexpected MNLI label: {label}")
    return f" {label}"


def answer_options() -> list[tuple[str, str]]:
    """Return (answer_text, label) options for constrained log-prob eval."""
    return [(format_answer(label), label) for label in LABELS]


def format_training_example(example: dict) -> tuple[str, str]:
    """Return (prompt_text, answer_text) for answer-only SFT."""
    return format_prompt(example), format_answer(example)
