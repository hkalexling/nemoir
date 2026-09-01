"""Compiled workflow integration tests.

Compiles the actual ``autoresearch.nemo`` workflow into a temporary directory,
imports the generated package, and runs it with a fake model and fake
``os.shell`` harness tool.

Covers:
- Fully successful accepting path (Init -> Baseline -> AdoptBaseline ->
  StartTrial -> ... -> AcceptCandidate -> CheckBudget -> FinalEval -> FinalReport)
- Numeric accept transition (score > best_score in JudgePrimary,
  score - best_score > eps in JudgeConfirmed)
- Event trace lifecycle (stage_started, tool_call_started,
  transition_selected, run_completed)
- Fixed commands map (no model call for deterministic stages)
- Invalid preflight / repair / reject branch
- Policy denial: direct fs.write to protected file raises PolicyDeniedError
- Policy denial: direct fs.read of outside path raises PolicyDeniedError,
  emits policy_denied event
"""

from __future__ import annotations

import asyncio
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from nemoir_runtime import (
    PolicyDeniedError,
    Tool,
    ToolContext,
    ToolRegistry,
)
from nemoir_runtime.events import WorkflowEvent
from nemoir_runtime.models import ModelResponse, ModelToolCall

from tests.conftest import DEMO_ROOT, RUNTIME_SRC


# ---------------------------------------------------------------------------
# Compilation helpers
# ---------------------------------------------------------------------------


def _nemo_binary() -> Path:
    from tests.conftest import NEMO_PATH

    if NEMO_PATH is None:
        pytest.skip("nemo binary not found (build with `cargo build --release` in compiler/)")
    return NEMO_PATH


def _compile_autoresearch(out_dir: Path) -> None:
    """Compile autoresearch.nemo into a Python package in out_dir."""
    nemo = _nemo_binary()
    nemo_src = DEMO_ROOT / "autoresearch.nemo"
    result = subprocess.run(  # noqa: S603
        [
            str(nemo),
            "compile",
            str(nemo_src),
            "--target", "python",
            "-o", str(out_dir),
        ],
        capture_output=True,
        cwd=str(DEMO_ROOT),
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode() if result.stderr else "(no stderr)\n")
        sys.stderr.write(result.stdout.decode() if result.stdout else "(no stdout)\n")
        result.check_returncode()


def _import_compiled(out_dir: Path) -> Any:
    """Import the generated covertype_xgb_autoresearch package."""
    for mod_name in list(sys.modules):
        if mod_name == "covertype_xgb_autoresearch" or mod_name.startswith("covertype_xgb_autoresearch."):
            del sys.modules[mod_name]

    if str(out_dir) not in sys.path:
        sys.path.insert(0, str(out_dir))
    if str(RUNTIME_SRC) not in sys.path:
        sys.path.insert(0, str(RUNTIME_SRC))

    import covertype_xgb_autoresearch  # type: ignore[import-not-found]

    return covertype_xgb_autoresearch


# ---------------------------------------------------------------------------
# Output dataclass matching HarnessResult schema
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class FakeHarnessResult:
    ok: bool
    state_json: str
    trial_count: int
    score: float
    best_score: float
    selection_score: float
    confirmation_score: float
    repair_allowed: bool
    continue_search: bool
    report: str
    metrics: str
    log: str
    candidate_hash: str
    mutable_candidate_path: str
    history_summary: str
    history_path: str
    agent_view_path: str


# Fixed commands in autoresearch.nemo
ALLOWED_COMMANDS = {
    "python harness/state.py init",
    "python harness/baseline.py",
    "python harness/state.py adopt-baseline",
    "python harness/state.py start-trial",
    "python harness/preflight.py",
    "python harness/state.py repair-gate",
    "python harness/train.py",
    "python harness/evaluate.py --split selection --model current",
    "python harness/state.py judge-primary",
    "python harness/evaluate.py --split confirmation --model current",
    "python harness/state.py judge-confirm",
    "python harness/state.py accept",
    "python harness/state.py reject",
    "python harness/state.py should-continue",
    "python harness/final_eval.py",
}

HARNESS_OUTPUT_SCHEMA: dict[str, type] = {
    "ok": bool, "state_json": str, "trial_count": int,
    "score": float, "best_score": float, "selection_score": float,
    "confirmation_score": float, "repair_allowed": bool,
    "continue_search": bool, "report": str, "metrics": str,
    "log": str, "candidate_hash": str, "mutable_candidate_path": str,
    "history_summary": str, "history_path": str, "agent_view_path": str,
}


# ---------------------------------------------------------------------------
# Fake model adapter
# ---------------------------------------------------------------------------


class FakeModelAdapter:
    """Returns stage-appropriate outputs for model stages."""

    def __init__(self) -> None:
        self.calls: list[Any] = []
        self._responses: dict[str, str] = {
            "AnalyzeHistory": '{"analysis": "tuned max_depth", "direction": "increase tree complexity"}',
            "ProposeCandidate": '{"patch_description": "increase max_depth from 6 to 8"}',
            "ApplyCandidate": '{"applied": true, "summary": "increased max_depth to 8"}',
            "FinalReport": '{"summary": "search completed successfully"}',
            "RepairCandidate": '{"summary": "repaired candidate"}',
        }

    async def complete(self, request: Any) -> ModelResponse:
        self.calls.append(request)
        stage_id = request.stage_id
        content = self._responses.get(stage_id, '{"summary": "ok"}')
        return ModelResponse(content=content)


# ---------------------------------------------------------------------------
# Tool factory helpers
# ---------------------------------------------------------------------------


def _make_basic_tools(
    harness_handler: Any,
    *,
    read_handler: Any = None,
    write_handler: Any = None,
    confirm_handler: Any = None,
    elicit_handler: Any = None,
) -> ToolRegistry:
    """Create a ToolRegistry with os.shell + required fs.read/fs.write/user.confirm tools."""

    async def _default_read(*, path: Path, ctx: ToolContext) -> str:
        return '{"schema_version": 1, "max_depth": 6}'

    async def _default_write(*, path: Path, content: str, ctx: ToolContext) -> None:
        pass

    async def _default_confirm(*, message: str, ctx: ToolContext) -> bool:
        return True

    async def _default_elicit(*, question: str, ctx: ToolContext) -> str:
        return "yes"

    harness_tool = Tool(
        name="run_harness",
        capability="os.shell",
        description="harness",
        input_schema={"command": str},
        handler=harness_handler,
        output_schema=HARNESS_OUTPUT_SCHEMA,
    )
    read_tool = Tool(
        name="read_file", capability="fs.read", description="r",
        input_schema={"path": Path},
        handler=read_handler if read_handler is not None else _default_read,
    )
    write_tool = Tool(
        name="write_file", capability="fs.write", description="w",
        input_schema={"path": Path, "content": str},
        handler=write_handler if write_handler is not None else _default_write,
    )
    confirm_tool = Tool(
        name="confirm", capability="user.confirm", description="c",
        input_schema={"message": str},
        handler=confirm_handler if confirm_handler is not None else _default_confirm,
    )
    elicit_tool = Tool(
        name="elicit", capability="user.elicit", description="e",
        input_schema={"question": str},
        handler=elicit_handler if elicit_handler is not None else _default_elicit,
    )
    return ToolRegistry([harness_tool, read_tool, write_tool, confirm_tool, elicit_tool])


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def compiled_package(tmp_path_factory: pytest.TempPathFactory) -> Any:
    """Compile the autoresearch workflow once per module."""
    _ = _nemo_binary()  # skip if nemo not available
    out_dir = tmp_path_factory.mktemp("compiled_pkg")
    _compile_autoresearch(out_dir)
    return _import_compiled(out_dir)


# ---------------------------------------------------------------------------
# Harness state machine
# ---------------------------------------------------------------------------


class _HarnessState:
    """Pure state tracking for the fake harness — keeps the async handler thin."""

    def __init__(
        self,
        baseline_selection: float = 0.70,
        baseline_confirmation: float = 0.68,
        trial_selection: float = 0.80,
        trial_confirmation: float = 0.78,
        max_trials: int = 10,
        preflight_ok: bool = True,
        train_ok: bool = True,
        should_continue_after_first: bool = False,
        max_repairs: int = 2,
    ) -> None:
        self.commands: list[str] = []
        self.trial_count = 0
        self.accepted = False
        self.repair_attempts = 0
        self.baseline_selection = baseline_selection
        self.baseline_confirmation = baseline_confirmation
        self.baseline_combined = (baseline_selection + baseline_confirmation) / 2.0
        self.trial_selection = trial_selection
        self.trial_confirmation = trial_confirmation
        self.trial_combined = (trial_selection + trial_confirmation) / 2.0
        self.max_trials = max_trials
        self.max_repairs = max_repairs
        self.preflight_ok = preflight_ok
        self.train_ok = train_ok
        self.should_continue_after_first = should_continue_after_first

    @property
    def incumbent_score(self) -> float:
        return self.trial_combined if self.accepted else self.baseline_combined

    @property
    def incumbent_selection(self) -> float:
        return self.trial_selection if self.accepted else self.baseline_selection

    def _result(self, **kwargs: object) -> FakeHarnessResult:
        defaults: dict[str, Any] = {
            "ok": True, "score": 0.0, "best_score": 0.0,
            "selection_score": 0.0, "confirmation_score": 0.0,
            "repair_allowed": False, "continue_search": False,
            "report": "", "metrics": "{}", "log": "",
            "candidate_hash": "test-hash-1234",
            "mutable_candidate_path": "/tmp/candidate.json",
            "history_summary": "Budget: 0/3 trials used. Best: 0.8923 (trial 0).",
            "history_path": "/tmp/agent_view/history.jsonl",
            "agent_view_path": "/tmp/agent_view",
        }
        defaults.update(kwargs)
        # Build a minimal state_json
        state = {
            "trial_count": self.trial_count,
            "accepted_count": 1 if self.accepted else 0,
            "max_trials": self.max_trials,
        }
        defaults["state_json"] = json.dumps(state, sort_keys=True)
        defaults["trial_count"] = self.trial_count
        return FakeHarnessResult(**defaults)  # type: ignore[arg-type]

    def handle(self, command: str) -> FakeHarnessResult:
        self.commands.append(command)

        if command not in ALLOWED_COMMANDS:
            return self._result(ok=False, report=f"unrecognized: {command}")

        if command == "python harness/state.py init":
            return self._result(ok=True, report="initialized")

        elif command == "python harness/baseline.py":
            return self._result(
                score=self.baseline_combined,
                selection_score=self.baseline_selection,
                confirmation_score=self.baseline_confirmation,
                report=f"baseline sel={self.baseline_selection} conf={self.baseline_confirmation}",
            )

        elif command == "python harness/state.py adopt-baseline":
            return self._result(
                score=self.baseline_combined,
                best_score=self.baseline_combined,
                selection_score=self.baseline_selection,
                confirmation_score=self.baseline_confirmation,
                report=f"adopted baseline score={self.baseline_combined}",
            )

        elif command == "python harness/state.py start-trial":
            self.trial_count += 1
            self.repair_attempts = 0  # reset repair counter per trial
            return self._result(
                best_score=self.incumbent_score,
                report=f"started trial {self.trial_count}",
            )

        elif command == "python harness/preflight.py":
            return self._result(
                ok=self.preflight_ok,
                report="preflight passed" if self.preflight_ok else "preflight failed",
            )

        elif command == "python harness/state.py repair-gate":
            self.repair_attempts += 1
            allowed = self.repair_attempts <= self.max_repairs
            return self._result(
                ok=False,
                repair_allowed=allowed,
                report=f"repair {self.repair_attempts}/{self.max_repairs}: {'allowed' if allowed else 'denied'}",
            )

        elif command == "python harness/train.py":
            return self._result(ok=self.train_ok, report="trained")

        elif command == "python harness/evaluate.py --split selection --model current":
            return self._result(score=self.trial_selection, report="selection eval")

        elif command == "python harness/state.py judge-primary":
            if self.trial_selection > self.incumbent_selection:
                return self._result(
                    score=self.trial_selection,
                    best_score=self.incumbent_selection,
                    selection_score=self.trial_selection,
                    report=f"primary score={self.trial_selection} > best={self.incumbent_selection}",
                )
            else:
                return self._result(
                    score=self.trial_selection,
                    best_score=self.incumbent_selection,
                    selection_score=self.trial_selection,
                    report=f"primary score={self.trial_selection} <= best={self.incumbent_selection}",
                )

        elif command == "python harness/evaluate.py --split confirmation --model current":
            return self._result(score=self.trial_confirmation, report="confirmation eval")

        elif command == "python harness/state.py judge-confirm":
            delta = self.trial_combined - self.incumbent_score
            return self._result(
                score=self.trial_combined,
                best_score=self.incumbent_score,
                selection_score=self.trial_selection,
                confirmation_score=self.trial_confirmation,
                report=f"combined={self.trial_combined} incumbent={self.incumbent_score} delta={delta:+.6f}",
            )

        elif command == "python harness/state.py accept":
            self.accepted = True
            return self._result(
                score=self.trial_combined,
                best_score=self.trial_combined,
                selection_score=self.trial_selection,
                confirmation_score=self.trial_confirmation,
                report=f"accepted trial {self.trial_count}",
            )

        elif command == "python harness/state.py reject":
            return self._result(best_score=self.incumbent_score, report="rejected")

        elif command == "python harness/state.py should-continue":
            cs = self.trial_count < self.max_trials if self.should_continue_after_first else False
            return self._result(
                best_score=self.incumbent_score,
                continue_search=cs,
                report=f"continue_search={cs}",
            )

        elif command == "python harness/final_eval.py":
            return self._result(score=0.79, report="final eval")

        return self._result(ok=False, report=f"unhandled: {command}")


async def _make_harness_handler(state: _HarnessState, *, command: str, ctx: ToolContext) -> FakeHarnessResult:
    return state.handle(command)


# ---------------------------------------------------------------------------
# Test: Accepting path with event trace
# ---------------------------------------------------------------------------


class TestAcceptingPath:
    def test_full_accepting_workflow(self, compiled_package: Any) -> None:
        """Complete accepting path: Init -> ... -> AcceptCandidate -> FinalEval -> FinalReport."""
        state = _HarnessState()

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        candidate_path = DEMO_ROOT / "candidate.json"
        agent_view_dir = Path("/tmp/test_agent_view")
        agent_view_dir.mkdir(parents=True, exist_ok=True)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=candidate_path,
            agent_view_dir=agent_view_dir,
            eps=0.002,
        )

        result = asyncio.run(agent.run(inputs))

        assert isinstance(result, pkg.AgentResult)
        assert isinstance(result.output, pkg.AgentOutput)
        assert result.output.summary == "search completed successfully"

        # Verify expected command sequence
        expected_cmds = [
            "python harness/state.py init",
            "python harness/baseline.py",
            "python harness/state.py adopt-baseline",
            "python harness/state.py start-trial",
            "python harness/preflight.py",
            "python harness/train.py",
            "python harness/evaluate.py --split selection --model current",
            "python harness/state.py judge-primary",
        ]
        for cmd in expected_cmds:
            assert cmd in state.commands, f"missing command: {cmd}"

        assert "python harness/evaluate.py --split confirmation --model current" in state.commands
        assert "python harness/state.py judge-confirm" in state.commands
        assert "python harness/state.py accept" in state.commands
        assert "python harness/state.py should-continue" in state.commands
        assert "python harness/final_eval.py" in state.commands

        model_stages = {c.stage_id for c in model.calls if hasattr(c, 'stage_id')}
        assert "AnalyzeHistory" in model_stages
        assert "ProposeCandidate" in model_stages
        assert "ApplyCandidate" in model_stages
        assert "FinalReport" in model_stages

    def test_event_trace_lifecycle(self, compiled_package: Any) -> None:
        """Event trace contains all lifecycle events through accepting path."""
        state = _HarnessState()

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        agent_view_dir = Path("/tmp/test_agent_view_lifecycle")
        agent_view_dir.mkdir(parents=True, exist_ok=True)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=agent_view_dir,
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            async for event in agent.stream(inputs):
                events.append(event)

        asyncio.run(collect())

        kinds = [e.kind for e in events]
        assert "run_started" in kinds
        assert "stage_started" in kinds
        assert "stage_completed" in kinds
        assert "tool_call_started" in kinds
        assert "tool_call_completed" in kinds
        assert "transition_selected" in kinds
        assert "run_completed" in kinds

        transitions = [e for e in events if e.kind == "transition_selected"]
        to_stages = [t.transition_to for t in transitions]
        assert "AcceptCandidate" in to_stages, f"transitions: {to_stages}"

        rc = [e for e in events if e.kind == "run_completed"]
        assert len(rc) == 1
        assert rc[0].result.output.summary == "search completed successfully"

    def test_numeric_accept_transition(self, compiled_package: Any) -> None:
        """When trial score > best_score, the workflow transitions to AcceptCandidate."""
        state = _HarnessState()

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=Path("/tmp/test_agent_view_num"),
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            async for event in agent.stream(inputs):
                events.append(event)

        asyncio.run(collect())

        transitions = [e for e in events if e.kind == "transition_selected"]
        to_stages = [t.transition_to for t in transitions]

        # JudgePrimary -> ConfirmCandidate (score 0.80 > best_score 0.70)
        assert "ConfirmCandidate" in to_stages
        # JudgeConfirmed -> AcceptCandidate (0.79 - 0.69 = 0.10 > eps 0.002)
        assert "AcceptCandidate" in to_stages


# ---------------------------------------------------------------------------
# Test: No model call for deterministic stages
# ---------------------------------------------------------------------------


class TestNoModelCallForDeterministicStages:
    def test_deterministic_stages_do_not_call_model(self, compiled_package: Any) -> None:
        """Stages with exec: os.shell(...) must not trigger model completion."""
        state = _HarnessState()

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=Path("/tmp/test_agent_view_det"),
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            async for event in agent.stream(inputs):
                events.append(event)

        asyncio.run(collect())

        model_completed_stages = [
            e.stage_id for e in events
            if e.kind == "model_completed"
        ]
        deterministic_ids = {
            "Init", "Baseline", "AdoptBaseline", "StartTrial",
            "Preflight", "RepairGate", "TrainCandidate",
            "EvaluateSelection", "JudgePrimary",
            "ConfirmCandidate", "JudgeConfirmed",
            "AcceptCandidate", "RejectCandidate",
            "CheckBudget", "FinalEval",
        }
        for stage_id in model_completed_stages:
            assert stage_id not in deterministic_ids, (
                f"deterministic stage {stage_id} triggered model_completed"
            )


# ---------------------------------------------------------------------------
# Test: Fixed commands
# ---------------------------------------------------------------------------


class TestFixedCommands:
    def test_rejection_evidence_is_forwarded_to_analysis(self, compiled_package: Any) -> None:
        """The next analysis receives both the direct report and durable summary."""
        analyze = next(stage for stage in compiled_package.Agent.manifest.stages if stage.id == "AnalyzeHistory")
        reads = {
            (read.ref.node, read.ref.field, read.optional)
            for read in analyze.reads
            if read.ref.kind == "node_output"
        }
        assert ("RejectCandidate", "report", True) in reads
        assert ("StartTrial", "history_summary", False) in reads

    def test_all_commands_in_allowlist(self, compiled_package: Any) -> None:
        """Every os.shell command invoked matches the fixed command set."""
        state = _HarnessState()

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=Path("/tmp/test_agent_view_cmds"),
            eps=0.002,
        )

        asyncio.run(agent.run(inputs))

        for cmd in state.commands:
            assert cmd in ALLOWED_COMMANDS, (
                f"command {cmd!r} not in fixed allowlist"
            )


# ---------------------------------------------------------------------------
# Test: Invalid preflight / repair / reject branch
# ---------------------------------------------------------------------------


class TestPreflightRepairReject:
    def test_preflight_failure_triggers_reject_path(self, compiled_package: Any) -> None:
        """When preflight fails and repair is exhausted, workflow rejects."""
        state = _HarnessState(preflight_ok=False)
        # Use max_repairs=1 so second repair-gate call denies repair

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        registry = _make_basic_tools(harness_handler)
        model = FakeModelAdapter()
        pkg = compiled_package

        agent = pkg.Agent(model=model, tools=registry)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=Path("/tmp/test_agent_view_reject"),
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            async for event in agent.stream(inputs):
                events.append(event)

        asyncio.run(collect())

        transitions = [e for e in events if e.kind == "transition_selected"]
        to_stages = [t.transition_to for t in transitions]
        # Preflight failure -> RepairGate
        assert "RepairGate" in to_stages, f"got: {to_stages}"
        # Should see reject command
        assert "python harness/state.py reject" in state.commands
        assert "RejectCandidate" in to_stages


# ---------------------------------------------------------------------------
# Test: Policy denial for fs.write to protected harness file
# ---------------------------------------------------------------------------


class TestPolicyDenial:
    def test_fs_write_to_protected_file_raises_policy_denied(self, compiled_package: Any) -> None:
        """Direct fs.write to a protected harness file is denied and retried.

        The compiled policy `deny fs.write(path) if not path.eq(candidate_path)`
        blocks writing to any path other than candidate_path. With the runtime
        fix, the denial is fed back to the model as a tool error (retryable)
        rather than killing the run.
        """
        state = _HarnessState(preflight_ok=False)
        pkg = compiled_package

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        async def fake_read(*, path: Path, ctx: ToolContext) -> str:
            return "ok"

        async def fake_write(*, path: Path, content: str, ctx: ToolContext) -> None:
            pass  # policy should block before this runs

        async def fake_confirm(*, message: str, ctx: ToolContext) -> bool:
            return True

        async def fake_elicit(*, question: str, ctx: ToolContext) -> str:
            return "yes"

        registry = _make_basic_tools(
            harness_handler,
            read_handler=fake_read,
            write_handler=fake_write,
            confirm_handler=fake_confirm,
            elicit_handler=fake_elicit,
        )

        class MaliciousModel:
            def __init__(self) -> None:
                self._stage_idx: dict[str, int] = {}
                self.calls: list[Any] = []
                self._outputs: dict[str, str] = {
                    "AnalyzeHistory": '{"analysis": "ok", "direction": "no change"}',
                    "ProposeCandidate": '{"patch_description": "increase max_depth"}',
                    "ApplyCandidate": '{"applied": true, "summary": "ok"}',
                    "FinalReport": '{"summary": "done"}',
                    "RepairCandidate": '{"summary": "repaired"}',
                }

            async def complete(self, request: Any) -> ModelResponse:
                self.calls.append(request)
                stage_id = request.stage_id
                count = self._stage_idx.get(stage_id, 0)
                self._stage_idx[stage_id] = count + 1

                if stage_id == "ApplyCandidate" and count == 0:
                    # Try to write to harness/evaluate.py (protected)
                    return ModelResponse(
                        content=None,
                        tool_calls=(
                            ModelToolCall(
                                id="bad_write",
                                name="write_file",
                                arguments={
                                    "path": str(DEMO_ROOT / "harness" / "evaluate.py"),
                                    "content": "malicious",
                                },
                            ),
                        ),
                    )
                return ModelResponse(content=self._outputs.get(stage_id, '{"summary": "ok"}'))

        agent = pkg.Agent(model=MaliciousModel(), tools=registry)

        candidate_path = DEMO_ROOT / "candidate.json"
        agent_view_dir = Path("/tmp/test_agent_view_policy")
        agent_view_dir.mkdir(parents=True, exist_ok=True)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=candidate_path,
            agent_view_dir=agent_view_dir,
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            try:
                async for event in agent.stream(inputs):
                    events.append(event)
            except PolicyDeniedError:
                pass

        asyncio.run(collect())

        policy_denied_events = [e for e in events if e.kind == "policy_denied"]
        assert len(policy_denied_events) >= 1, (
            f"expected at least one policy_denied event, got kinds: "
            f"{[e.kind for e in events]}"
        )

    def test_policy_denied_event_emitted(self, compiled_package: Any) -> None:
        """A denied read emits a policy_denied event."""
        state = _HarnessState()
        pkg = compiled_package

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        async def fake_read(*, path: Path, ctx: ToolContext) -> str:
            return "ok"

        async def fake_write(*, path: Path, content: str, ctx: ToolContext) -> None:
            pass

        async def fake_confirm(*, message: str, ctx: ToolContext) -> bool:
            return True

        async def fake_elicit(*, question: str, ctx: ToolContext) -> str:
            return "yes"

        registry = _make_basic_tools(
            harness_handler,
            read_handler=fake_read,
            write_handler=fake_write,
            confirm_handler=fake_confirm,
            elicit_handler=fake_elicit,
        )

        class MaliciousModel:
            def __init__(self) -> None:
                self._stage_idx: dict[str, int] = {}
                self.calls: list[Any] = []
                self._outputs: dict[str, str] = {
                    "AnalyzeHistory": '{"analysis": "ok", "direction": "no change"}',
                    "ProposeCandidate": '{"patch_description": "increase max_depth"}',
                    "ApplyCandidate": '{"applied": true, "summary": "ok"}',
                    "FinalReport": '{"summary": "done"}',
                    "RepairCandidate": '{"summary": "repaired"}',
                }

            async def complete(self, request: Any) -> ModelResponse:
                self.calls.append(request)
                stage_id = request.stage_id
                count = self._stage_idx.get(stage_id, 0)
                self._stage_idx[stage_id] = count + 1

                if stage_id == "AnalyzeHistory" and count == 0:
                    # Try to read outside allowed path
                    return ModelResponse(
                        content=None,
                        tool_calls=(
                            ModelToolCall(
                                id="bad_read",
                                name="read_file",
                                arguments={"path": "/etc/passwd"},
                            ),
                        ),
                    )
                return ModelResponse(content=self._outputs.get(stage_id, '{"summary": "ok"}'))

        agent = pkg.Agent(model=MaliciousModel(), tools=registry)

        agent_view_dir = Path("/tmp/test_agent_view_policy2")
        agent_view_dir.mkdir(parents=True, exist_ok=True)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=agent_view_dir,
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            try:
                async for event in agent.stream(inputs):
                    events.append(event)
            except PolicyDeniedError:
                pass

        asyncio.run(collect())

        policy_denied_events = [e for e in events if e.kind == "policy_denied"]
        assert len(policy_denied_events) >= 1, (
            f"expected at least one policy_denied event, got kinds: "
            f"{[e.kind for e in events]}"
        )

    def test_fs_read_outside_agent_view_raises_policy_denied(self, compiled_package: Any) -> None:
        """fs.read of a path outside agent_view_dir and candidate_path is denied.

        With the runtime fix, the denial is fed back as a tool error (retryable)
        rather than killing the run. The policy_denied event is still emitted.
        """
        state = _HarnessState()
        pkg = compiled_package

        async def harness_handler(*, command: str, ctx: ToolContext) -> FakeHarnessResult:
            return state.handle(command)

        async def fake_read(*, path: Path, ctx: ToolContext) -> str:
            return "ok"

        async def fake_write(*, path: Path, content: str, ctx: ToolContext) -> None:
            pass

        async def fake_confirm(*, message: str, ctx: ToolContext) -> bool:
            return True

        async def fake_elicit(*, question: str, ctx: ToolContext) -> str:
            return "yes"

        registry = _make_basic_tools(
            harness_handler,
            read_handler=fake_read,
            write_handler=fake_write,
            confirm_handler=fake_confirm,
            elicit_handler=fake_elicit,
        )

        class OutsideReadModel:
            def __init__(self) -> None:
                self._stage_idx: dict[str, int] = {}
                self.calls: list[Any] = []
                self._outputs: dict[str, str] = {
                    "AnalyzeHistory": '{"analysis": "ok", "direction": "ok"}',
                    "ProposeCandidate": '{"patch_description": "increase max_depth"}',
                    "ApplyCandidate": '{"applied": true, "summary": "ok"}',
                    "FinalReport": '{"summary": "done"}',
                    "RepairCandidate": '{"summary": "repaired"}',
                }

            async def complete(self, request: Any) -> ModelResponse:
                self.calls.append(request)
                stage_id = request.stage_id
                count = self._stage_idx.get(stage_id, 0)
                self._stage_idx[stage_id] = count + 1

                if stage_id == "AnalyzeHistory" and count == 0:
                    return ModelResponse(
                        content=None,
                        tool_calls=(
                            ModelToolCall(
                                id="outside_read",
                                name="read_file",
                                arguments={"path": "/etc/shadow"},
                            ),
                        ),
                    )
                return ModelResponse(content=self._outputs.get(stage_id, '{"summary": "ok"}'))

        agent = pkg.Agent(model=OutsideReadModel(), tools=registry)

        agent_view_dir = Path("/tmp/test_agent_view_deny_read")
        agent_view_dir.mkdir(parents=True, exist_ok=True)

        inputs = pkg.AgentInput(
            cwd=DEMO_ROOT,
            candidate_path=DEMO_ROOT / "candidate.json",
            agent_view_dir=agent_view_dir,
            eps=0.002,
        )

        events: list[WorkflowEvent] = []

        async def collect() -> None:
            try:
                async for event in agent.stream(inputs):
                    events.append(event)
            except PolicyDeniedError:
                pass

        asyncio.run(collect())

        policy_denied_events = [e for e in events if e.kind == "policy_denied"]
        assert len(policy_denied_events) >= 1, (
            f"expected at least one policy_denied event, got kinds: "
            f"{[e.kind for e in events]}"
        )
