#!/usr/bin/env python3
"""SLM autoresearch harness — preflight smoke checks.

Verifies candidate.py, MNLI formatting hooks, CUDA, tokenizer, model config,
and writable adapter/run directories before GPU time is spent on a trial.
"""

import sys
import traceback
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def main() -> int:
    print("[preflight] Running smoke checks...", flush=True)
    ok = 0
    fail = 0

    def _check(label: str, fn) -> None:
        nonlocal ok, fail
        try:
            fn()
            print(f"[preflight]   OK  {label}", flush=True)
            ok += 1
        except Exception:
            print(f"[preflight]   FAIL {label}", flush=True)
            traceback.print_exc()
            fail += 1

    _check("import candidate", lambda: __import__("candidate"))
    _check("MNLI formatting hooks", _check_formatting)
    _check("CUDA available", _check_cuda)
    _check("tokenizer loads", _load_tokenizer)
    _check("model config loads", _check_config)
    _check("adapter dir writable", lambda: _check_writable(ROOT / "adapter"))
    _check("run dir writable", lambda: _check_writable(ROOT / "runs" / "current"))

    total = ok + fail
    print(f"[preflight] {ok}/{total} checks passed", flush=True)
    return 0 if fail == 0 else 1


def _check_formatting() -> None:
    """Validate that candidate.py's label contract is self-consistent.

    Makes no assumption about what the labels actually are (A/B/C,
    entailment/neutral/contradiction, yes/no/maybe, etc). Only checks:
      - Prompt has the required structure markers.
      - LABELS is a unique non-empty list.
      - ID_TO_LABEL maps 0..N-1 to entries in LABELS.
      - label_from_example returns ID_TO_LABEL[id] for each MNLI label id.
      - format_answer returns a non-empty string for each label.
      - answer_options returns (answer_text, label) pairs matching LABELS.
      - Training example's answer is consistent with format_answer.
    """
    import candidate

    ex = {
        "premise": "A soccer game with multiple males playing.",
        "hypothesis": "Some men are playing a sport.",
        "label": 0,
    }
    prompt, answer = candidate.format_training_example(ex)
    assert "Premise:" in prompt, "prompt must contain 'Premise:'"
    assert "Hypothesis:" in prompt, "prompt must contain 'Hypothesis:'"
    assert "Answer:" in prompt, "prompt must contain 'Answer:'"

    labels = candidate.LABELS
    assert isinstance(labels, list), "LABELS must be a list"
    assert len(labels) >= 2, "LABELS must have at least 2 entries"
    assert len(set(labels)) == len(labels), "LABELS must be unique"

    id_map = candidate.ID_TO_LABEL
    assert set(id_map.keys()) == set(range(len(labels))), \
        "ID_TO_LABEL must map 0..N-1"
    for i, lbl in id_map.items():
        assert lbl in labels, f"ID_TO_LABEL[{i}]={lbl!r} not in LABELS"

    for i in range(len(labels)):
        got = candidate.label_from_example({
            "label": i, "premise": "x", "hypothesis": "y",
        })
        assert got == id_map[i], \
            f"label_from_example(label={i}) returned {got!r}, expected {id_map[i]!r}"

    for lbl in labels:
        ans = candidate.format_answer(lbl)
        assert isinstance(ans, str) and len(ans.strip()) > 0, \
            f"format_answer({lbl!r}) must return a non-empty string"

    opts = candidate.answer_options()
    assert len(opts) == len(labels), \
        f"answer_options() returned {len(opts)} options, expected {len(labels)}"
    for (ans_text, lbl), expected in zip(opts, labels):
        assert lbl == expected, \
            f"answer_options() label mismatch: {lbl!r} != {expected!r}"
        assert ans_text == candidate.format_answer(expected), \
            f"answer_options() text mismatch for {expected!r}"

    assert answer == candidate.format_answer(candidate.label_from_example(ex)), \
        "Training example answer not consistent with format_answer(label_from_example(ex))"


def _check_cuda() -> None:
    import torch

    if not torch.cuda.is_available():
        raise RuntimeError("CUDA not available")
    print(f"[preflight]          GPU: {torch.cuda.get_device_name(0)}", flush=True)


def _load_tokenizer() -> None:
    import candidate
    from transformers import AutoTokenizer

    tok = AutoTokenizer.from_pretrained(candidate.BASE_MODEL, trust_remote_code=True)
    ids = tok.encode("Hello!")
    tok.decode(ids)
    print(f"[preflight]          model={candidate.BASE_MODEL}", flush=True)


def _check_config() -> None:
    import candidate
    from transformers import AutoConfig

    cfg = AutoConfig.from_pretrained(candidate.BASE_MODEL, trust_remote_code=True)
    assert cfg is not None


def _check_writable(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    test = path / ".write_test"
    test.write_text("ok")
    test.unlink()


if __name__ == "__main__":
    sys.exit(main())
