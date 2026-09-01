#!/usr/bin/env python3
"""SLM autoresearch harness — accept/reject judge.

Compares the current trial score against the best seen score.
Used by baselines B and C (prompt-only and hand-coded controllers).
NemoIR replaces this with a compiled numeric guard in JudgeCandidate.

Reads:  results.json (from eval) and best_score.txt (if exists)
Writes: decision.json with accept/reject

Exit code 0 always (decision is in the output file).
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
RESULTS_FILE = ROOT / "results.json"
BEST_SCORE_FILE = ROOT / "best_score.txt"
DECISION_FILE = ROOT / "decision.json"


def main() -> int:
    # Read current score
    if not RESULTS_FILE.exists():
        _fail("results.json not found — eval must run first")
        return 0

    results = json.loads(RESULTS_FILE.read_text())
    current_score = float(results["score"])

    # Read best score
    best_score = None
    if BEST_SCORE_FILE.exists():
        best_score = float(BEST_SCORE_FILE.read_text().strip())

    # Decision
    eps = 0.005  # minimum improvement threshold
    if best_score is None:
        decision = "accept"
        reason = "first_trial"
    elif current_score > best_score + eps:
        decision = "accept"
        reason = "improved"
    elif current_score > best_score:
        decision = "confirm"
        reason = "near_frontier"
    else:
        decision = "reject"
        reason = "no_improvement"

    # Update best if accepted
    if decision == "accept":
        BEST_SCORE_FILE.write_text(str(current_score))

    # Write decision
    output = {
        "decision": decision,
        "reason": reason,
        "current_score": current_score,
        "best_score": best_score or current_score,
        "eps": eps,
    }
    DECISION_FILE.write_text(json.dumps(output) + "\n")
    print(json.dumps(output), flush=True)
    return 0


def _fail(msg: str) -> None:
    print(json.dumps({"decision": "reject", "reason": msg}), flush=True)
    DECISION_FILE.write_text(json.dumps({"decision": "reject", "reason": msg}) + "\n")


if __name__ == "__main__":
    sys.exit(main())
