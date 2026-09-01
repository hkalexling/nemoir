#!/usr/bin/env python3
"""Run the compiled NemoIR MNLI autoresearch workflow."""

from __future__ import annotations

import argparse
import asyncio
import os
import sys
from pathlib import Path
from datetime import UTC, datetime
import dataclasses
import json
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default=os.environ.get("NEMOIR_MODEL"), help="LiteLLM model name (e.g. openai/gpt-4o-mini, deepseek/deepseek-chat)")
    parser.add_argument("--api-key", default=None, help="provider key; defaults to NEMOIR_API_KEY / OPENAI_API_KEY / DEEPSEEK_API_KEY")
    parser.add_argument("--api-base", default=os.environ.get("NEMOIR_API_BASE"), help="provider base URL (for OpenAI-compatible endpoints)")
    parser.add_argument("--eps", type=float, default=0.01, help="minimum accepted accuracy gain")
    parser.add_argument("--cwd", type=Path, default=HERE)
    parser.add_argument("--profile", default="mnli_demo")
    parser.add_argument("--max-trials", type=int, default=5)
    parser.add_argument("--train-examples", type=int, default=None)
    parser.add_argument("--eval-examples", type=int, default=None)
    parser.add_argument("--eval-split", default="validation_matched")
    parser.add_argument("--run-dir", default="runs/current")
    parser.add_argument(
        "--train-timeout-seconds",
        type=float,
        default=19 * 60,
        help="training-stage wall-clock timeout; capped under 20 minutes",
    )
    parser.add_argument("--max-steps", type=int, default=512, help="workflow runtime step cap")
    parser.add_argument("--max-model-retries", type=int, default=8)
    parser.add_argument("--max-tool-rounds", type=int, default=16)
    return parser


def _load_dotenv() -> None:
    """Load local provider configuration before argparse resolves defaults."""
    try:
        from dotenv import load_dotenv

        load_dotenv(HERE / ".env")
    except ImportError:
        pass


def _api_key(explicit: str | None) -> str | None:
    if explicit:
        return explicit
    _load_dotenv()
    for name in ("NEMOIR_API_KEY", "OPENAI_API_KEY", "DEEPSEEK_API_KEY"):
        value = os.environ.get(name)
        if value:
            return value
    # Colab fallback
    try:
        from google.colab import userdata  # type: ignore[import-not-found]

        for name in ("NEMOIR_API_KEY", "OPENAI_API_KEY", "DEEPSEEK_API_KEY"):
            value = userdata.get(name)
            if value:
                return value
    except Exception:
        pass
    return None


def _jsonable(value: Any) -> Any:
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return _jsonable(dataclasses.asdict(value))
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _jsonable(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set, frozenset)):
        return [_jsonable(item) for item in value]
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return str(value)


async def main() -> None:
    _load_dotenv()
    parser = _parser()
    args = parser.parse_args()

    # Harness scripts consume runtime knobs through environment variables because
    # deterministic exec commands in the compiled workflow are intentionally fixed.
    os.environ["NEMOIR_PROFILE"] = args.profile
    os.environ["NEMOIR_MAX_TRIALS"] = str(args.max_trials)
    os.environ["NEMOIR_EVAL_SPLIT"] = args.eval_split
    os.environ["NEMOIR_RUN_DIR"] = args.run_dir
    os.environ["NEMOIR_TRAIN_TIMEOUT_SECONDS"] = str(args.train_timeout_seconds)
    os.environ["NEMOIR_EPS"] = str(args.eps)
    if args.train_examples is not None:
        os.environ["NEMOIR_TRAIN_EXAMPLES"] = str(args.train_examples)
    if args.eval_examples is not None:
        os.environ["NEMOIR_EVAL_EXAMPLES"] = str(args.eval_examples)

    if not args.model:
        print("error: provide --model or set NEMOIR_MODEL (e.g. openai/gpt-4o-mini)", file=sys.stderr)
        raise SystemExit(2)

    key = _api_key(args.api_key)
    if not key:
        print("error: set NEMOIR_API_KEY, OPENAI_API_KEY, DEEPSEEK_API_KEY, or --api-key", file=sys.stderr)
        raise SystemExit(2)

    from nemoir_runtime import RunOptions, ToolRegistry
    from nemoir_runtime.official_tools import edit_file, read_file, write_file

    from autoresearch import Agent, AgentInput
    from harness_tools import run_eval, run_harness, run_state

    model: dict[str, Any] = {"name": args.model, "temperature": 0.3, "api_key": key}
    if args.api_base:
        model["api_base"] = args.api_base

    tools = ToolRegistry([read_file, write_file, edit_file, run_harness, run_eval, run_state])

    cwd = args.cwd.resolve()
    os.chdir(cwd)
    agent = Agent(model=model, tools=tools)
    inputs = AgentInput(cwd=cwd, eps=args.eps)
    opts = RunOptions(max_steps=args.max_steps, max_model_retries=args.max_model_retries, max_tool_rounds=args.max_tool_rounds)
    print(
        "Autoresearch MNLI: "
        f"model={args.model} cwd={cwd} eps={args.eps} "
        f"profile={args.profile} max_trials={args.max_trials} "
        f"train_examples={os.environ.get('NEMOIR_TRAIN_EXAMPLES', 'candidate.py')} "
        f"eval_examples={os.environ.get('NEMOIR_EVAL_EXAMPLES', 'benchmarks.yml')} "
        f"train_timeout_seconds={os.environ.get('NEMOIR_TRAIN_TIMEOUT_SECONDS')}",
        flush=True,
    )

    # Persist launch metadata (mirrors xgboost/cvxpygen)
    try:
        run_dir = Path(args.run_dir)
        if not run_dir.is_absolute():
            run_dir = (HERE / run_dir).resolve()
        run_dir.mkdir(parents=True, exist_ok=True)
        (run_dir / "launch.json").write_text(
            json.dumps(
                {
                    "created_at": datetime.now(tz=UTC).isoformat(),
                    "workflow": getattr(agent, "workflow_id", "autoresearch"),
                    "model": args.model,
                    "eps": args.eps,
                    "profile": args.profile,
                    "max_trials": args.max_trials,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
    except Exception:
        pass

    cur = None
    ch = None
    eval_count = 0
    trial = 0

    # Stream with JSONL trace (like xgboost) if run_dir is available
    trace_path = None
    try:
        run_dir = Path(args.run_dir)
        if not run_dir.is_absolute():
            run_dir = HERE / run_dir
        trace_path = run_dir / "events.jsonl"
        trace_path.parent.mkdir(parents=True, exist_ok=True)
    except Exception:
        trace_path = None

    trace_file = open(trace_path, "a", encoding="utf-8") if trace_path else None

    try:
        async for event in agent.stream(inputs, options=opts):
            # Mirror to trace
            if trace_file:
                try:
                    trace_file.write(json.dumps(_jsonable(event.__dict__ if hasattr(event, "__dict__") else dict(event)), sort_keys=True) + "\n")
                    trace_file.flush()
                except Exception:
                    pass

            if event.kind == "stage_completed" and event.stage_id in ("StateStartTrial",):
                out = event.output or {}
                tc = out.get("trial_count")
                if tc is not None:
                    trial = int(tc)

            if event.stage_id and event.stage_id != cur:
                if ch:
                    print()
                    ch = None
                cur = event.stage_id
                if cur in ("BaseModelEval", "InitialRecipeEval", "BenchmarkAdapter"):
                    eval_count += 1
                parts = []
                if trial and cur not in ("Setup", "StateInit", "BaseModelEval", "InitialRecipeTrain", "InitialRecipeEval", "AcceptInitial", "FinalReport"):
                    parts.append(f"trial {trial}")
                if cur in ("BaseModelEval", "InitialRecipeEval", "BenchmarkAdapter"):
                    parts.append(f"eval {eval_count}")
                t = f" [{' '.join(parts)}]" if parts else ""
                print(f"\n-- {cur}{t} --", flush=True)

            if event.kind == "model_delta":
                if event.text:
                    if ch != event.channel:
                        if ch:
                            print()
                        ch = event.channel
                    print(event.text, end="", flush=True)
                continue

            if ch:
                print()
                ch = None

            if event.kind == "model_retry":
                m = event.metadata or {}
                print(f"  [retry {m.get('attempt','?')}/{m.get('max_retries','?')}]", flush=True)
            elif event.kind == "policy_denied":
                print(f"  POLICY DENIED: {event.error}", flush=True)
            elif event.kind == "tool_call_started":
                cap = event.capability or "?"
                print(f"  [{cap}] {event.tool_name or cap}", flush=True)
            elif event.kind == "tool_call_completed":
                out = event.output or {}
                suffix = ""
                if "score" in out:
                    suffix += f" score={float(out.get('score') or 0.0):.4f}"
                if "best_score" in out:
                    suffix += f" best={float(out.get('best_score') or 0.0):.4f}"
                if "continue_search" in out:
                    suffix += f" continue={out.get('continue_search')}"
                print(f"  => ok{suffix}", flush=True)
            elif event.kind == "tool_call_failed":
                print(f"  TOOL FAILED: {event.error}", flush=True)
            elif event.kind == "transition_selected":
                to = event.transition_to or "?"
                h = " ACCEPTED" if to == "AcceptCandidate" else (" REJECTED" if to == "RejectCandidate" else "")
                print(f"  -> {to}{h}", flush=True)
            elif event.kind == "stage_completed":
                out = event.output or {}
                ks = []
                for k in list(out.keys())[:6]:
                    v = out[k]
                    if isinstance(v, float):
                        ks.append(f"{k}={v:.4f}")
                    elif isinstance(v, int):
                        ks.append(f"{k}={v}")
                    elif isinstance(v, bool):
                        ks.append(f"{k}={v}")
                    elif isinstance(v, str) and len(v) > 50:
                        ks.append(f"{k}='{v[:47]}...'")
                    else:
                        ks.append(f"{k}={v!r}")
                if ks:
                    print(f"  output: {', '.join(ks)}", flush=True)
            elif event.kind == "run_completed":
                r = event.result
                print(f"\n=== DONE ===\n{r.output.summary if r else 'completed'}")
            elif event.kind == "run_failed":
                print(f"\n=== FAILED: {event.error} ===")
    finally:
        if trace_file:
            trace_file.close()


if __name__ == "__main__":
    asyncio.run(main())
