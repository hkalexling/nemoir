#!/usr/bin/env python3
"""Run the compiled Covertype XGBoost autoresearch workflow.

Run ``demos/nemo compile autoresearch.nemo --target python -o .`` first, then
install the demo requirements in a Python 3.11+ environment.  The driver does
not download data or make a model-provider call until the compiled workflow is
started.
"""

from __future__ import annotations

import argparse
import asyncio
import dataclasses
import json
import os
import sys
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default=os.environ.get("NEMOIR_MODEL"), help="LiteLLM model name")
    parser.add_argument("--api-key", default=None, help="provider key; defaults to environment")
    parser.add_argument("--api-base", default=os.environ.get("NEMOIR_API_BASE"))
    parser.add_argument("--eps", type=float, default=0.002, help="combined macro-F1 improvement threshold")
    parser.add_argument("--max-trials", type=int, default=10)
    parser.add_argument("--max-repairs", type=int, default=2)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--n-jobs", type=int, default=min(4, os.cpu_count() or 1))
    parser.add_argument("--data-dir", type=Path, default=HERE / "data")
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument("--train-timeout-seconds", type=float, default=20 * 60)
    parser.add_argument("--eval-timeout-seconds", type=float, default=10 * 60)
    parser.add_argument("--command-timeout-seconds", type=float, default=120)
    parser.add_argument("--max-steps", type=int, default=512)
    parser.add_argument("--max-model-retries", type=int, default=4)
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
    return None


def _new_run_dir(value: Path | None) -> Path:
    if value is not None:
        path = value if value.is_absolute() else HERE / value
        path = path.resolve()
        path.mkdir(parents=True, exist_ok=False)
        return path
    stamp = datetime.now(tz=UTC).strftime("%Y%m%dT%H%M%SZ")
    path = HERE / "runs" / f"{stamp}-{uuid.uuid4().hex[:8]}"
    path.mkdir(parents=True, exist_ok=False)
    return path.resolve()


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


def _event_payload(event: Any) -> dict[str, Any]:
    if dataclasses.is_dataclass(event):
        raw = dataclasses.asdict(event)
    else:
        raw = dict(event)
    return _jsonable(raw)


def _print_event(event: Any) -> None:
    if event.kind == "stage_started":
        print(f"\n-- {event.stage_id} --", flush=True)
    elif event.kind == "model_delta":
        # Live-stream model text so the operator sees analysis/proposal/reports.
        if event.text:
            print(event.text, end="", flush=True)
    elif event.kind == "tool_call_started":
        print(f"  [{event.capability}] {event.tool_name or event.capability}", flush=True)
    elif event.kind == "policy_denied":
        print(f"  POLICY DENIED: {event.error}", flush=True)
    elif event.kind == "transition_selected":
        print(f"  -> {event.transition_to}", flush=True)
    elif event.kind == "stage_completed":
        output = event.output or {}
        fragments: list[str] = []
        for name in ("score", "best_score", "selection_score", "confirmation_score", "trial_count"):
            if name in output:
                value = output[name]
                if isinstance(value, float):
                    fragments.append(f"{name}={value:.6f}")
                else:
                    fragments.append(f"{name}={value}")
        if "continue_search" in output:
            fragments.append(f"continue={output['continue_search']}")
        if fragments:
            print(f"  output: {', '.join(fragments)}", flush=True)
    elif event.kind == "run_failed":
        print(f"\nFAILED: {event.error}", flush=True)
    elif event.kind == "run_completed":
        result = event.result
        summary = getattr(getattr(result, "output", None), "summary", None)
        print(f"\nDONE{': ' + summary if summary else ''}", flush=True)


async def _run(args: argparse.Namespace, *, run_dir: Path, api_key: str) -> int:
    # Path-policy equality intentionally resolves relative paths from the demo root.
    os.chdir(HERE)
    sys.path.insert(0, str(HERE))
    try:
        from nemoir_runtime import RunOptions, ToolRegistry
        from covertype_xgb_autoresearch import Agent, AgentInput
        from harness_tools import (
            edit_agent_file,
            read_agent_file,
            run_harness,
            write_agent_file,
        )
    except ImportError as exc:
        print(
            "error: generated package/runtime unavailable. Run `demos/nemo compile "
            "demos/xgboost-autoresearch/autoresearch.nemo --target python "
            "-o demos/xgboost-autoresearch` and install requirements. "
            f"({exc})",
            file=sys.stderr,
        )
        return 2

    agent_view_dir = run_dir / "agent_view"
    agent_view_dir.mkdir(parents=True, exist_ok=True)
    candidate_path = (HERE / "candidate.json").resolve()
    data_dir = args.data_dir.resolve()
    os.environ.update(
        {
            "NEMOIR_RUN_DIR": str(run_dir),
            "NEMOIR_DATA_DIR": str(data_dir),
            "NEMOIR_EPS": str(args.eps),
            "NEMOIR_MAX_TRIALS": str(max(0, args.max_trials)),
            "NEMOIR_MAX_REPAIRS": str(max(0, args.max_repairs)),
            "NEMOIR_DEVICE": args.device,
            "NEMOIR_N_JOBS": str(max(1, args.n_jobs)),
            "NEMOIR_TRAIN_TIMEOUT_SECONDS": str(args.train_timeout_seconds),
            "NEMOIR_EVAL_TIMEOUT_SECONDS": str(args.eval_timeout_seconds),
            "NEMOIR_COMMAND_TIMEOUT_SECONDS": str(args.command_timeout_seconds),
        }
    )

    model: dict[str, Any] = {"name": args.model, "temperature": 0.2, "api_key": api_key}
    if args.api_base:
        model["api_base"] = args.api_base
    # Provider reasoning is suppressed from output; only assistant text streams.
    model["reasoning"] = "none"

    tools = ToolRegistry([read_agent_file, write_agent_file, edit_agent_file, run_harness])
    agent = Agent(model=model, tools=tools)
    inputs = AgentInput(
        cwd=HERE.resolve(),
        candidate_path=candidate_path,
        agent_view_dir=agent_view_dir.resolve(),
        eps=args.eps,
    )
    options = RunOptions(
        max_steps=max(16, args.max_steps),
        max_model_retries=max(0, args.max_model_retries),
        max_tool_rounds=max(1, args.max_tool_rounds),
    )

    launch = {
        "created_at": datetime.now(tz=UTC).isoformat(),
        "workflow": agent.workflow_id,
        "model": args.model,
        "device": args.device,
        "max_trials": args.max_trials,
        "max_repairs": args.max_repairs,
        "eps": args.eps,
        "data_dir": str(data_dir),
        "candidate_path": str(candidate_path),
        "agent_view_dir": str(agent_view_dir),
    }
    (run_dir / "launch.json").write_text(json.dumps(launch, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print(
        f"Covertype XGBoost autoresearch: run={run_dir.name} device={args.device} "
        f"trials={args.max_trials} eps={args.eps}",
        flush=True,
    )
    failed = False
    with (run_dir / "events.jsonl").open("a", encoding="utf-8") as trace:
        try:
            async for event in agent.stream(inputs, options=options):
                trace.write(json.dumps(_event_payload(event), sort_keys=True) + "\n")
                trace.flush()
                _print_event(event)
        except Exception as exc:
            failed = True
            trace.write(json.dumps({
                "kind": "driver_error",
                "timestamp": datetime.now(tz=UTC).isoformat(),
                "error": f"{type(exc).__name__}: {exc}",
            }, sort_keys=True) + "\n")
            trace.flush()
            print(f"\nFAILED: {type(exc).__name__}: {exc}", file=sys.stderr)
    return 1 if failed else 0


def main() -> int:
    # --model defaults are read by argparse, so load .env first.
    _load_dotenv()
    args = _parser().parse_args()
    if not args.model:
        print("error: provide --model or set NEMOIR_MODEL", file=sys.stderr)
        return 2
    if args.eps <= 0:
        print("error: --eps must be positive", file=sys.stderr)
        return 2
    key = _api_key(args.api_key)
    if not key:
        print("error: set NEMOIR_API_KEY, OPENAI_API_KEY, DEEPSEEK_API_KEY, or --api-key", file=sys.stderr)
        return 2
    try:
        run_dir = _new_run_dir(args.run_dir)
    except FileExistsError:
        print("error: --run-dir already exists; use a new unique directory", file=sys.stderr)
        return 2
    return asyncio.run(_run(args, run_dir=run_dir, api_key=key))


if __name__ == "__main__":
    raise SystemExit(main())
