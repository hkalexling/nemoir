//! IR -> Python source translation.
//!
//! Each function emits a Python source fragment constructing an instance of a
//! `nemoir_runtime.runtime.*` dataclass from a `nemoir_ir::*` value using
//! literal constructor arguments only. Module-level emitters compose per-IR-type
//! helpers to produce the full `_manifest.py`, `types.py`, `_agent.py`, and
//! `__init__.py` source files.

use nemoir_ir::{
    Expr, Guard, Policy, Read, Ref, RequiredCapability, Transition, WorkflowIr, Write,
};

use crate::escape::python_string_literal;
use crate::PythonBackendError;

// ---------------------------------------------------------------------------
// Module-level emitters (filled in by M6, M7, M8, M9)
// ---------------------------------------------------------------------------

pub fn emit_manifest_module(ir: &WorkflowIr) -> Result<String, PythonBackendError> {
    let mut out = String::new();

    out.push_str("from nemoir_runtime.runtime import (\n");
    out.push_str("    ExprSpec,\n");
    out.push_str("    GuardSpec,\n");
    out.push_str("    InputSpec,\n");
    out.push_str("    PolicySpec,\n");
    out.push_str("    ReadSpec,\n");
    out.push_str("    RefSpec,\n");
    out.push_str("    RequiredCapabilitySpec,\n");
    out.push_str("    StageExecutionSpec,\n");
    out.push_str("    StageSpec,\n");
    out.push_str("    TransitionSpec,\n");
    out.push_str("    TriggerSpec,\n");
    out.push_str("    WorkflowManifest,\n");
    out.push_str("    WriteSpec,\n");
    out.push_str(")\n\n");

    out.push_str(&format!(
        "WORKFLOW_ID = {}\n",
        python_string_literal(&ir.workflow.id)
    ));
    out.push_str(&format!(
        "ENTRY_STAGE_ID = {}\n",
        python_string_literal(&ir.workflow.entry)
    ));

    let exits: Vec<String> = ir
        .workflow
        .exits
        .iter()
        .map(|e| python_string_literal(e))
        .collect();
    out.push_str(&format!(
        "EXIT_STAGE_IDS = frozenset({{{}}})\n",
        exits.join(", ")
    ));

    let caps: Vec<String> = ir
        .capabilities
        .iter()
        .map(|c| python_string_literal(c))
        .collect();
    out.push_str(&format!(
        "REQUIRED_CAPABILITIES = frozenset({{{}}})\n\n",
        caps.join(", ")
    ));

    out.push_str("WORKFLOW_MANIFEST = WorkflowManifest(\n");
    out.push_str(&format!(
        "    workflow_id={},\n",
        python_string_literal(&ir.workflow.id)
    ));
    out.push_str(&format!(
        "    entry_stage_id={},\n",
        python_string_literal(&ir.workflow.entry)
    ));
    out.push_str(&format!(
        "    exit_stage_ids=frozenset({{{}}}),\n",
        exits.join(", ")
    ));

    // inputs
    if ir.inputs.is_empty() {
        out.push_str("    inputs=(),\n");
    } else {
        out.push_str("    inputs=(\n");
        for inp in &ir.inputs {
            out.push_str(&format!(
                "        InputSpec(name={}, type={}),\n",
                python_string_literal(&inp.id),
                python_string_literal(&inp.ty)
            ));
        }
        out.push_str("    ),\n");
    }

    out.push_str(&format!(
        "    capabilities=frozenset({{{}}}),\n",
        caps.join(", ")
    ));

    // policies
    if ir.policies.is_empty() {
        out.push_str("    policies=(),\n");
    } else {
        out.push_str("    policies=(\n");
        for p in &ir.policies {
            out.push_str(&format!("        {},\n", emit_policy(p)?));
        }
        out.push_str("    ),\n");
    }

    // stages
    if ir.nodes.is_empty() {
        out.push_str("    stages=(),\n");
    } else {
        out.push_str("    stages=(\n");
        for n in &ir.nodes {
            out.push_str(&format!("        {},\n", emit_stage(n)?));
        }
        out.push_str("    ),\n");
    }

    out.push_str(")\n");
    Ok(out)
}

pub fn emit_types_module(ir: &WorkflowIr) -> Result<String, PythonBackendError> {
    let mut out = String::new();
    out.push_str("from dataclasses import dataclass\n");
    // `from __future__ import annotations` lets us emit `list[str]` and `Optional[T]`
    // as plain strings even for older type-parsing consumers.
    out.push_str("from pathlib import Path\n");
    out.push_str("from typing import Optional\n");
    out.push_str("import typing\n\n");

    // -----------------------------------------------------------------------
    // AgentInput from ir.inputs
    // -----------------------------------------------------------------------
    out.push_str("@dataclass(frozen=True)\n");
    out.push_str("class AgentInput:\n");
    if ir.inputs.is_empty() {
        out.push_str("    pass\n\n");
    } else {
        for inp in &ir.inputs {
            out.push_str(&format!("    {}: {}\n", inp.id, ir_type_to_python(&inp.ty)));
        }
        out.push('\n');
    }

    // -----------------------------------------------------------------------
    // AgentOutput from exit-stage writes
    // -----------------------------------------------------------------------
    let exit_fields = collect_exit_fields(ir);

    out.push_str("@dataclass(frozen=True)\n");
    out.push_str("class AgentOutput:\n");
    if exit_fields.is_empty() {
        out.push_str("    pass\n\n");
    } else {
        let single_exit = ir.workflow.exits.len() == 1;
        for (name, py_type, optional_dynamic) in &exit_fields {
            // Multi-exit workflows make every field Optional[T] = None (Phase 3 simplification).
            // Single-exit: writes whose IR `optional=true` become Optional[T] = None; others
            // stay required.
            let optional = !single_exit || *optional_dynamic;
            if optional {
                out.push_str(&format!("    {}: Optional[{}] = None\n", name, py_type));
            } else {
                out.push_str(&format!("    {}: {}\n", name, py_type));
            }
        }
        out.push('\n');
    }

    // -----------------------------------------------------------------------
    // AgentResult -- holds only `output` in Phase 3. trace (Phase 7) and
    // snapshot (Phase 6) placeholders are commented as anchors.
    // -----------------------------------------------------------------------
    out.push_str("@dataclass(frozen=True)\n");
    out.push_str("class AgentResult:\n");
    out.push_str("    output: AgentOutput\n");
    out.push_str("    # trace: Trace  # added in Phase 7\n");
    out.push_str("    # snapshot: WorkflowSnapshot  # added in Phase 6\n");

    Ok(out)
}

/// Collect `(field_name, python_type, is_optional)` for every write across
/// all exit stages, preserving declaration order without duplicating names.
fn collect_exit_fields(ir: &WorkflowIr) -> Vec<(String, String, bool)> {
    let exit_set: std::collections::HashSet<&str> =
        ir.workflow.exits.iter().map(|s| s.as_str()).collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<(String, String, bool)> = Vec::new();
    for node in &ir.nodes {
        if !exit_set.contains(node.id.as_str()) {
            continue;
        }
        for w in &node.writes {
            if seen.insert(w.name.clone()) {
                out.push((w.name.clone(), ir_type_to_python(&w.ty), w.optional));
            }
        }
    }
    out
}

pub fn emit_agent_module(
    package_name: &str,
    ir: &WorkflowIr,
) -> Result<String, PythonBackendError> {
    let exit_fields = collect_exit_fields(ir);

    let mut input_assigns = String::new();
    for inp in &ir.inputs {
        input_assigns.push_str(&format!("        \"{}\": inputs.{},\n", inp.id, inp.id));
    }

    let mut output_assigns = String::new();
    // For multi-exit workflows the dataclass marks every unioned field
    // `Optional[T] = None`, since fields from exits that were not taken are
    // absent from the runtime output mapping. The converter must therefore
    // use `output.get(...)` (not `output[...]`) for every field, matching the
    // declared optionality contract.
    let multi_exit = ir.workflow.exits.len() > 1;
    for (name, _py_type, optional) in &exit_fields {
        if multi_exit || *optional {
            output_assigns.push_str(&format!("        {}=output.get(\"{}\"),\n", name, name));
        } else {
            output_assigns.push_str(&format!("        {}=output[\"{}\"],\n", name, name));
        }
    }

    let out = format!(
        concat!(
            "from __future__ import annotations\n",
            "\n",
            "from collections.abc import AsyncIterator\n",
            "from dataclasses import replace\n",
            "\n",
            "from typing import TYPE_CHECKING\n",
            "\n",
            "from nemoir_runtime import ModelStageExecutor, WorkflowEvent\n",
            "from nemoir_runtime.runtime import RunOptions, StageExecutor, WorkflowRuntime\n",
            "\n",
            "from {pkg}._manifest import (\n",
            "    REQUIRED_CAPABILITIES,\n",
            "    WORKFLOW_ID,\n",
            "    WORKFLOW_MANIFEST,\n",
            ")\n",
            "from {pkg}.types import AgentInput, AgentOutput, AgentResult\n",
            "\n",
            "if TYPE_CHECKING:\n",
            "    from nemoir_runtime.tools import ToolRegistry\n",
            "\n",
            "\n",
            "class Agent:\n",
            "    workflow_id = WORKFLOW_ID\n",
            "    required_capabilities = REQUIRED_CAPABILITIES\n",
            "    manifest = WORKFLOW_MANIFEST\n",
            "\n",
            "    def __init__(\n",
            "        self,\n",
            "        *,\n",
            "        model,\n",
            "        tools: ToolRegistry,\n",
            "        trace=None,\n",
            "        defaults: RunOptions | None = None,\n",
            "    ) -> None:\n",
            "        self._model = model\n",
            "        self._tools = tools\n",
            "        self._defaults = defaults\n",
            "        # Eagerly validate deterministic stages — fail fast\n",
            "        # before any run starts (plan §1.7).\n",
            "        WorkflowRuntime(\n",
            "            manifest=WORKFLOW_MANIFEST,\n",
            "            tools=tools,\n",
            "            stage_executor=ModelStageExecutor(model=model, tools=tools),\n",
            "        )\n",
            "\n",
            "    async def run(\n",
            "        self,\n",
            "        inputs: AgentInput,\n",
            "        *,\n",
            "        options: RunOptions | None = None,\n",
            "    ) -> AgentResult:\n",
            "        opts = options if options is not None else (self._defaults or RunOptions())\n",
            "        executor = ModelStageExecutor(model=self._model, tools=self._tools, max_tool_rounds=opts.max_tool_rounds)\n",
            "        return await self._run_with_executor(inputs, executor=executor, options=options)\n",
            "\n",
            "    async def stream(\n",
            "        self,\n",
            "        inputs: AgentInput,\n",
            "        *,\n",
            "        options: RunOptions | None = None,\n",
            "    ) -> AsyncIterator[WorkflowEvent]:\n",
            "        opts = options if options is not None else (self._defaults or RunOptions())\n",
            "        executor = ModelStageExecutor(model=self._model, tools=self._tools, max_tool_rounds=opts.max_tool_rounds)\n",
            "        async for event in self._stream_with_executor(\n",
            "            inputs, executor=executor, options=options\n",
            "        ):\n",
            "            yield event\n",
            "\n",
            "    async def _run_with_executor(\n",
            "        self,\n",
            "        inputs: AgentInput,\n",
            "        *,\n",
            "        executor: StageExecutor,\n",
            "        options: RunOptions | None = None,\n",
            "    ) -> AgentResult:\n",
            "        runtime = WorkflowRuntime(\n",
            "            manifest=WORKFLOW_MANIFEST,\n",
            "            tools=self._tools,\n",
            "            stage_executor=executor,\n",
            "        )\n",
            "        inputs_dict = _inputs_to_mapping(inputs)\n",
            "        opts = options if options is not None else (self._defaults or RunOptions())\n",
            "        result = await runtime.run(inputs_dict, options=opts)\n",
            "        return AgentResult(output=_output_from_mapping(result.output))\n",
            "\n",
            "    async def _stream_with_executor(\n",
            "        self,\n",
            "        inputs: AgentInput,\n",
            "        *,\n",
            "        executor: StageExecutor,\n",
            "        options: RunOptions | None = None,\n",
            "    ) -> AsyncIterator[WorkflowEvent]:\n",
            "        runtime = WorkflowRuntime(\n",
            "            manifest=WORKFLOW_MANIFEST,\n",
            "            tools=self._tools,\n",
            "            stage_executor=executor,\n",
            "        )\n",
            "        inputs_dict = _inputs_to_mapping(inputs)\n",
            "        opts = options if options is not None else (self._defaults or RunOptions())\n",
            "        async for event in runtime.stream(inputs_dict, options=opts):\n",
            "            if event.kind == \"run_completed\" and event.result is not None:\n",
            "                typed = AgentResult(\n",
            "                    output=_output_from_mapping(event.result.output)\n",
            "                )\n",
            "                yield replace(event, result=typed)\n",
            "            else:\n",
            "                yield event\n",
            "\n",
            "\n",
            "def _inputs_to_mapping(inputs: AgentInput) -> dict[str, object]:\n",
            "    return {{\n",
            "{inputs}    }}\n",
            "\n",
            "\n",
            "def _output_from_mapping(output: dict[str, object]) -> AgentOutput:\n",
            "    return AgentOutput(\n",
            "{outputs}    )\n",
        ),
        pkg = package_name,
        inputs = input_assigns,
        outputs = output_assigns,
    );

    Ok(out)
}

pub fn emit_init_module(
    package_name: &str,
    _ir: &WorkflowIr,
) -> Result<String, PythonBackendError> {
    Ok(format!(
        concat!(
            "from nemoir_runtime import ModelRouter, RunOptions, Tool, ToolContext, ToolRegistry, WorkflowEvent, tool\n",
            "\n",
            "from {pkg}._agent import Agent\n",
            "from {pkg}._manifest import WORKFLOW_MANIFEST\n",
            "from {pkg}.types import AgentInput, AgentOutput, AgentResult\n",
            "\n",
            "__all__ = [\n",
            "    \"Agent\",\n",
            "    \"AgentInput\",\n",
            "    \"AgentOutput\",\n",
            "    \"AgentResult\",\n",
            "    \"ModelRouter\",\n",
            "    \"RunOptions\",\n",
            "    \"Tool\",\n",
            "    \"ToolContext\",\n",
            "    \"ToolRegistry\",\n",
            "    \"WORKFLOW_MANIFEST\",\n",
            "    \"WorkflowEvent\",\n",
            "    \"tool\",\n",
            "]\n",
        ),
        pkg = package_name,
    ))
}

// ---------------------------------------------------------------------------
// Per-IR-variant emitters
// ---------------------------------------------------------------------------

/// RefSpec(kind="input", name="task"), RefSpec(kind="node_output", node="Triage", field="summary"),
/// RefSpec(kind="bound", name="path")
pub fn emit_ref(r: &Ref) -> String {
    match r {
        Ref::Input { name } => format!(
            "RefSpec(kind=\"input\", name={})",
            python_string_literal(name)
        ),
        Ref::NodeOutput { node, field } => format!(
            "RefSpec(kind=\"node_output\", node={}, field={})",
            python_string_literal(node),
            python_string_literal(field)
        ),
        Ref::Bound { name } => format!(
            "RefSpec(kind=\"bound\", name={})",
            python_string_literal(name)
        ),
    }
}

/// `True` / `False` / integer / float / quoted string. Other variants are unsupported.
pub fn emit_literal_value(value: &serde_yaml::Value) -> Result<String, PythonBackendError> {
    match value {
        serde_yaml::Value::Bool(b) => Ok(if *b { "True".into() } else { "False".into() }),
        serde_yaml::Value::Number(n) => {
            if n.is_i64() {
                Ok(format!("{}", n.as_i64().unwrap()))
            } else if n.is_u64() {
                Ok(format!("{}", n.as_u64().unwrap()))
            } else if n.is_f64() {
                Ok(format!("{}", n.as_f64().unwrap()))
            } else {
                Err(PythonBackendError::UnsupportedLiteral(format!("{:?}", n)))
            }
        }
        serde_yaml::Value::String(s) => Ok(python_string_literal(s)),
        other => Err(PythonBackendError::UnsupportedLiteral(format!(
            "{:?}",
            other
        ))),
    }
}

/// Literal ty -> Python type annotation string for `ExprSpec(kind="literal", type=..., value=...)`.
fn ir_type_as_annotation(ty: &str) -> &'static str {
    match ty {
        "string" => "str",
        "path" => "Path",
        "bool" => "bool",
        "string[]" => "list[str]",
        _ => "typing.Any",
    }
}

/// IR type string -> Python type string for `types.py` annotations.
pub fn ir_type_to_python(ty: &str) -> String {
    ir_type_as_annotation(ty).to_string()
}

/// ExprSpec(...) for an IR Expr. Recursively emits nested expressions.
pub fn emit_expr(expr: &Expr) -> Result<String, PythonBackendError> {
    match expr {
        Expr::Ref { r#ref } => Ok(format!("ExprSpec(kind=\"ref\", ref={})", emit_ref(r#ref))),
        Expr::Literal { ty, value } => Ok(format!(
            "ExprSpec(kind=\"literal\", type={}, value={})",
            python_string_literal(ty),
            emit_literal_value(value)?
        )),
        Expr::Not { expr: inner } => Ok(format!(
            "ExprSpec(kind=\"not\", expr={})",
            emit_expr(inner)?
        )),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let mut args_src = String::new();
            args_src.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    args_src.push_str(", ");
                }
                args_src.push_str(&emit_expr(a)?);
            }
            // Trailing comma is required when args has exactly one element so the
            // emitted form is a 1-tuple, not a parenthesized ExprSpec. The runtime
            // iterates `expr.args` (runtime.py:731) and would otherwise raise
            // `TypeError: 'ExprSpec' object is not iterable`.
            if args.len() == 1 {
                args_src.push(',');
            }
            args_src.push(')');
            Ok(format!(
                "ExprSpec(kind=\"method_call\", receiver={}, method={}, args={})",
                emit_expr(receiver)?,
                python_string_literal(method),
                args_src
            ))
        }
        Expr::And { exprs } => {
            let mut s = String::from("ExprSpec(kind=\"and\", exprs=(");
            for (i, e) in exprs.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&emit_expr(e)?);
            }
            if exprs.len() == 1 {
                s.push(',');
            }
            s.push(')');
            s.push(')');
            Ok(s)
        }
        Expr::Or { exprs } => {
            let mut s = String::from("ExprSpec(kind=\"or\", exprs=(");
            for (i, e) in exprs.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&emit_expr(e)?);
            }
            if exprs.len() == 1 {
                s.push(',');
            }
            s.push(')');
            s.push(')');
            Ok(s)
        }
    }
}

/// GuardSpec(...) for an IR Guard.
pub fn emit_guard(guard: &Guard) -> Result<String, PythonBackendError> {
    match guard {
        Guard::Always => Ok("GuardSpec(kind=\"always\")".into()),
        Guard::HasValue { r#ref } => Ok(format!(
            "GuardSpec(kind=\"has_value\", ref={})",
            emit_ref(r#ref)
        )),
        Guard::Missing { r#ref } => Ok(format!(
            "GuardSpec(kind=\"missing\", ref={})",
            emit_ref(r#ref)
        )),
        Guard::Eq { left, right } => Ok(format!(
            "GuardSpec(kind=\"eq\", left={}, right={})",
            emit_expr(left)?,
            emit_expr(right)?
        )),
    }
}

/// ReadSpec(ref=..., optional=...)
pub fn emit_read(read: &Read) -> String {
    format!(
        "ReadSpec(ref={}, optional={})",
        emit_ref(&read.ref_),
        if read.optional { "True" } else { "False" }
    )
}

/// WriteSpec(name=..., type=..., optional=...)
pub fn emit_write(write: &Write) -> String {
    format!(
        "WriteSpec(name={}, type={}, optional={})",
        python_string_literal(&write.name),
        python_string_literal(&write.ty),
        if write.optional { "True" } else { "False" }
    )
}

/// TransitionSpec(to=..., priority=..., reason=..., guard=...)
pub fn emit_transition(t: &Transition) -> Result<String, PythonBackendError> {
    Ok(format!(
        "TransitionSpec(to={}, priority={}, reason={}, guard={})",
        python_string_literal(&t.to),
        t.priority,
        python_string_literal(&t.reason),
        emit_guard(&t.guard)?
    ))
}

/// TriggerSpec(capability=..., bind={var: name, ...}) -- bind flattened to a dict literal.
pub fn emit_trigger(trigger: &nemoir_ir::Trigger) -> String {
    let mut bind_items: Vec<String> = Vec::new();
    for (var, arg) in &trigger.bind {
        bind_items.push(format!(
            "{}: {}",
            python_string_literal(var),
            python_string_literal(&arg.name)
        ));
    }
    format!(
        "TriggerSpec(capability={}, bind={{{}}})",
        python_string_literal(&trigger.capability),
        bind_items.join(", ")
    )
}

/// RequiredCapabilitySpec(capability=..., args={k: RefSpec, ...}) -- ArgValue::Ref is unwrapped.
pub fn emit_required_capability(req: &RequiredCapability) -> String {
    let mut arg_items: Vec<String> = Vec::new();
    for (k, av) in &req.args {
        // BindArg.kind is always "arg" -> the mapping value is the inner Ref of ArgValue::Ref.
        match av {
            nemoir_ir::ArgValue::Ref { r#ref } => {
                arg_items.push(format!("{}: {}", python_string_literal(k), emit_ref(r#ref)));
            }
        }
    }
    format!(
        "RequiredCapabilitySpec(capability={}, args={{{}}})",
        python_string_literal(&req.capability),
        arg_items.join(", ")
    )
}

/// PolicySpec(id=..., kind=..., trigger=..., requires=(...), condition=...)
pub fn emit_policy(policy: &Policy) -> Result<String, PythonBackendError> {
    let mut out = String::new();
    out.push_str(&format!(
        "PolicySpec(id={}, kind={}, trigger={}",
        python_string_literal(&policy.id),
        python_string_literal(&policy.kind),
        emit_trigger(&policy.trigger)
    ));
    if let Some(requires) = &policy.requires {
        if requires.is_empty() {
            out.push_str(", requires=()");
        } else {
            out.push_str(", requires=(");
            for (i, r) in requires.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&emit_required_capability(r));
            }
            // Runtime tuple requires trailing comma for length-1 tuples.
            if requires.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
    } else {
        out.push_str(", requires=()");
    }
    if let Some(condition) = &policy.condition {
        out.push_str(&format!(", condition={}", emit_expr(condition)?));
    }
    out.push(')');
    Ok(out)
}

/// StageSpec(id=..., prompt=..., reads=(...), writes=(...), requires=frozenset({...}),
/// transitions=(...))
pub fn emit_stage(node: &nemoir_ir::Node) -> Result<String, PythonBackendError> {
    let mut out = String::new();
    out.push_str(&format!(
        "StageSpec(id={}, prompt={}",
        python_string_literal(&node.id),
        python_string_literal(&node.prompt)
    ));

    // reads
    if node.reads.is_empty() {
        out.push_str(", reads=()");
    } else {
        out.push_str(", reads=(");
        for (i, r) in node.reads.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&emit_read(r));
        }
        if node.reads.len() == 1 {
            out.push(',');
        }
        out.push(')');
    }

    // writes
    if node.writes.is_empty() {
        out.push_str(", writes=()");
    } else {
        out.push_str(", writes=(");
        for (i, w) in node.writes.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&emit_write(w));
        }
        if node.writes.len() == 1 {
            out.push(',');
        }
        out.push(')');
    }

    // requires: unwrap StageCapability to plain strings
    out.push_str(", requires=frozenset({");
    let caps: Vec<String> = node
        .requires
        .iter()
        .map(|c| python_string_literal(&c.capability))
        .collect();
    out.push_str(&caps.join(", "));
    out.push_str("})");

    // transitions
    if node.transitions.is_empty() {
        out.push_str(", transitions=()");
    } else {
        out.push_str(", transitions=(");
        for (i, t) in node.transitions.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&emit_transition(t)?);
        }
        if node.transitions.len() == 1 {
            out.push(',');
        }
        out.push(')');
    }

    // execution
    if !node.execution.is_model() {
        out.push_str(", execution=");
        out.push_str(&emit_stage_execution(&node.execution)?);
    }

    out.push(')');
    Ok(out)
}

/// StageExecutionSpec(kind="tool", capability=..., args={...})
pub fn emit_stage_execution(exec: &nemoir_ir::StageExecution) -> Result<String, PythonBackendError> {
    match exec {
        nemoir_ir::StageExecution::Model => Ok("StageExecutionSpec()".into()),
        nemoir_ir::StageExecution::Tool { capability, args } => {
            let mut out = String::new();
            out.push_str(&format!(
                "StageExecutionSpec(kind=\"tool\", capability={}",
                python_string_literal(capability)
            ));
            out.push_str(", args={");
            for (i, (name, expr)) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!(
                    "{}: {}",
                    python_string_literal(name),
                    emit_expr(expr)?
                ));
            }
            out.push('}');
            out.push(')');
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use nemoir_ir::*;

    use super::*;

    fn ref_input(name: &str) -> Ref {
        Ref::Input { name: name.into() }
    }

    #[test]
    fn ref_input_emits_correct_kind() {
        let s = emit_ref(&ref_input("task"));
        assert!(s.contains("RefSpec(kind=\"input\""));
        assert!(s.contains("name=\"task\""));
        assert_eq!(s, "RefSpec(kind=\"input\", name=\"task\")");
    }

    #[test]
    fn ref_node_output_emits_node_and_field() {
        let s = emit_ref(&Ref::NodeOutput {
            node: "Triage".into(),
            field: "summary".into(),
        });
        assert_eq!(
            s,
            "RefSpec(kind=\"node_output\", node=\"Triage\", field=\"summary\")"
        );
    }

    #[test]
    fn ref_bound_emits_name() {
        let s = emit_ref(&Ref::Bound {
            name: "path".into(),
        });
        assert_eq!(s, "RefSpec(kind=\"bound\", name=\"path\")");
    }

    #[test]
    fn expr_literal_bool_true() {
        let expr = Expr::Literal {
            ty: "bool".into(),
            value: serde_yaml::Value::Bool(true),
        };
        let s = emit_expr(&expr).unwrap();
        assert_eq!(s, "ExprSpec(kind=\"literal\", type=\"bool\", value=True)");
    }

    #[test]
    fn expr_literal_bool_false() {
        let expr = Expr::Literal {
            ty: "bool".into(),
            value: serde_yaml::Value::Bool(false),
        };
        let s = emit_expr(&expr).unwrap();
        assert_eq!(s, "ExprSpec(kind=\"literal\", type=\"bool\", value=False)");
    }

    #[test]
    fn expr_literal_string() {
        let expr = Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("hi".into()),
        };
        let s = emit_expr(&expr).unwrap();
        assert_eq!(
            s,
            "ExprSpec(kind=\"literal\", type=\"string\", value=\"hi\")"
        );
    }

    #[test]
    fn expr_literal_int() {
        let expr = Expr::Literal {
            ty: "int".into(),
            value: serde_yaml::Value::Number(serde_yaml::Number::from(42i64)),
        };
        let s = emit_expr(&expr).unwrap();
        assert!(s.contains("value=42"));
    }

    #[test]
    fn expr_literal_unsupported_for_sequence() {
        let expr = Expr::Literal {
            ty: "weird".into(),
            value: serde_yaml::Value::Sequence(vec![]),
        };
        let result = emit_expr(&expr);
        assert!(matches!(
            result,
            Err(PythonBackendError::UnsupportedLiteral(_))
        ));
    }

    #[test]
    fn expr_not_wraps_inner() {
        let inner = Expr::Ref {
            r#ref: ref_input("task"),
        };
        let expr = Expr::Not {
            expr: Box::new(inner),
        };
        let s = emit_expr(&expr).unwrap();
        assert!(s.starts_with("ExprSpec(kind=\"not\", expr="));
        assert!(s.contains("ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"input\", name=\"task\")"));
    }

    #[test]
    fn expr_ref_emits_inner_ref() {
        let expr = Expr::Ref {
            r#ref: ref_input("task"),
        };
        let s = emit_expr(&expr).unwrap();
        assert_eq!(
            s,
            "ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"input\", name=\"task\"))"
        );
    }

    #[test]
    fn expr_method_call_args_emitted_as_tuple() {
        let receiver = Expr::Ref {
            r#ref: ref_input("cwd"),
        };
        let arg = Expr::Ref {
            r#ref: ref_input("path"),
        };
        let mc = Expr::MethodCall {
            receiver: Box::new(receiver),
            method: "contains".into(),
            args: vec![arg],
        };
        let s = emit_expr(&mc).unwrap();
        assert!(s.starts_with("ExprSpec(kind=\"method_call\""));
        assert!(s.contains("method=\"contains\""));
        // Single-arg method call MUST emit a trailing comma so the result is a
        // 1-tuple, not a parenthesized ExprSpec.
        assert!(s.contains(
            "args=(ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"input\", name=\"path\")),)"
        ));
    }

    #[test]
    fn expr_method_call_zero_args_emits_empty_tuple() {
        let receiver = Expr::Ref {
            r#ref: ref_input("cwd"),
        };
        let mc = Expr::MethodCall {
            receiver: Box::new(receiver),
            method: "contains".into(),
            args: vec![],
        };
        let s = emit_expr(&mc).unwrap();
        // Zero-arg method call should emit `args=()` (empty tuple).
        assert!(s.contains("args=()"));
    }

    #[test]
    fn expr_method_call_two_args_emits_tuple_without_trailing_comma() {
        let receiver = Expr::Ref {
            r#ref: ref_input("cwd"),
        };
        let mk_arg = |name: &str| Expr::Ref {
            r#ref: ref_input(name),
        };
        let mc = Expr::MethodCall {
            receiver: Box::new(receiver),
            method: "contains".into(),
            args: vec![mk_arg("a"), mk_arg("b")],
        };
        let s = emit_expr(&mc).unwrap();
        assert!(s.contains(
            "args=(ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"input\", name=\"a\")), "
                .to_string()
                .as_str()
        ));
        assert!(s.contains("name=\"b\")") && !s.contains("name=\"b\"),)"));
    }

    #[test]
    fn guard_always() {
        let s = emit_guard(&Guard::Always).unwrap();
        assert_eq!(s, "GuardSpec(kind=\"always\")");
    }

    #[test]
    fn guard_has_value() {
        let s = emit_guard(&Guard::HasValue {
            r#ref: Ref::NodeOutput {
                node: "Triage".into(),
                field: "unclear_points".into(),
            },
        })
        .unwrap();
        assert_eq!(
            s,
            "GuardSpec(kind=\"has_value\", ref=RefSpec(kind=\"node_output\", node=\"Triage\", field=\"unclear_points\"))"
        );
    }

    #[test]
    fn guard_missing() {
        let s = emit_guard(&Guard::Missing {
            r#ref: Ref::NodeOutput {
                node: "Triage".into(),
                field: "unclear_points".into(),
            },
        })
        .unwrap();
        assert_eq!(
            s,
            "GuardSpec(kind=\"missing\", ref=RefSpec(kind=\"node_output\", node=\"Triage\", field=\"unclear_points\"))"
        );
    }

    #[test]
    fn guard_eq_with_bool_literals() {
        let left = Expr::Ref {
            r#ref: Ref::NodeOutput {
                node: "Propose".into(),
                field: "ok".into(),
            },
        };
        let right = Expr::Literal {
            ty: "bool".into(),
            value: serde_yaml::Value::Bool(true),
        };
        let s = emit_guard(&Guard::Eq { left, right }).unwrap();
        assert!(s.starts_with("GuardSpec(kind=\"eq\""));
        assert!(s.contains("left=ExprSpec(kind=\"ref\""));
        assert!(s.contains("right=ExprSpec(kind=\"literal\", type=\"bool\", value=True)"));
    }

    #[test]
    fn read_with_input_ref() {
        let r = Read {
            ref_: ref_input("task"),
            optional: false,
            origin: "implicit_entry_input".into(),
        };
        let s = emit_read(&r);
        assert_eq!(
            s,
            "ReadSpec(ref=RefSpec(kind=\"input\", name=\"task\"), optional=False)"
        );
    }

    #[test]
    fn read_optional_true() {
        let r = Read {
            ref_: ref_input("unclear"),
            optional: true,
            origin: "dsl_stage_input".into(),
        };
        let s = emit_read(&r);
        assert!(s.contains("optional=True"));
    }

    #[test]
    fn read_drops_origin() {
        let r = Read {
            ref_: ref_input("task"),
            optional: false,
            origin: "implicit_entry_input".into(),
        };
        assert!(!emit_read(&r).contains("origin"));
    }

    #[test]
    fn write_required() {
        let w = Write {
            name: "summary".into(),
            ty: "string".into(),
            optional: false,
        };
        assert_eq!(
            emit_write(&w),
            "WriteSpec(name=\"summary\", type=\"string\", optional=False)"
        );
    }

    #[test]
    fn write_optional_string_array() {
        let w = Write {
            name: "unclear_points".into(),
            ty: "string[]".into(),
            optional: true,
        };
        assert_eq!(
            emit_write(&w),
            "WriteSpec(name=\"unclear_points\", type=\"string[]\", optional=True)"
        );
    }

    #[test]
    fn transition_to_with_priority_zero() {
        let t = Transition {
            to: "Clarify".into(),
            priority: 0,
            reason: "next_stage_required_input_available".into(),
            guard: Guard::Always,
        };
        let s = emit_transition(&t).unwrap();
        assert_eq!(
            s,
            "TransitionSpec(to=\"Clarify\", priority=0, reason=\"next_stage_required_input_available\", guard=GuardSpec(kind=\"always\"))"
        );
    }

    #[test]
    fn trigger_with_bind() {
        let mut bind = indexmap::IndexMap::new();
        bind.insert(
            "path".to_string(),
            BindArg {
                kind: "arg".into(),
                name: "path".into(),
            },
        );
        let t = nemoir_ir::Trigger {
            capability: "fs.write".into(),
            bind,
        };
        let s = super::emit_trigger(&t);
        assert_eq!(
            s,
            "TriggerSpec(capability=\"fs.write\", bind={\"path\": \"path\"})"
        );
    }

    #[test]
    fn trigger_with_empty_bind() {
        let t = nemoir_ir::Trigger {
            capability: "user.confirm".into(),
            bind: indexmap::IndexMap::new(),
        };
        let s = super::emit_trigger(&t);
        assert_eq!(s, "TriggerSpec(capability=\"user.confirm\", bind={})");
    }

    #[test]
    fn required_capability_empty_args() {
        let r = RequiredCapability {
            capability: "user.confirm".into(),
            args: indexmap::IndexMap::new(),
        };
        let s = super::emit_required_capability(&r);
        assert_eq!(
            s,
            "RequiredCapabilitySpec(capability=\"user.confirm\", args={})"
        );
    }

    #[test]
    fn required_capability_with_bound_ref_arg() {
        let mut args = indexmap::IndexMap::new();
        args.insert(
            "path".to_string(),
            ArgValue::Ref {
                r#ref: Ref::Bound {
                    name: "path".into(),
                },
            },
        );
        let r = RequiredCapability {
            capability: "fs.read".into(),
            args,
        };
        let s = super::emit_required_capability(&r);
        assert_eq!(
            s,
            "RequiredCapabilitySpec(capability=\"fs.read\", args={\"path\": RefSpec(kind=\"bound\", name=\"path\")})"
        );
    }

    #[test]
    fn policy_before_fs_write() {
        // before fs.write(path) requires fs.read(path), user.confirm
        let mut trigger_bind = indexmap::IndexMap::new();
        trigger_bind.insert(
            "path".to_string(),
            BindArg {
                kind: "arg".into(),
                name: "path".into(),
            },
        );
        let mut fs_read_args = indexmap::IndexMap::new();
        fs_read_args.insert(
            "path".to_string(),
            ArgValue::Ref {
                r#ref: Ref::Bound {
                    name: "path".into(),
                },
            },
        );
        let policy = Policy {
            id: "before fs.write(path) requires fs.read(path), user.confirm".into(),
            kind: "before".into(),
            trigger: nemoir_ir::Trigger {
                capability: "fs.write".into(),
                bind: trigger_bind,
            },
            requires: Some(vec![
                RequiredCapability {
                    capability: "fs.read".into(),
                    args: fs_read_args,
                },
                RequiredCapability {
                    capability: "user.confirm".into(),
                    args: indexmap::IndexMap::new(),
                },
            ]),
            condition: None,
        };
        let s = super::emit_policy(&policy).unwrap();
        assert!(s.starts_with(
            "PolicySpec(id=\"before fs.write(path) requires fs.read(path), user.confirm\""
        ));
        assert!(s.contains("kind=\"before\""));
        assert!(s.contains("TriggerSpec(capability=\"fs.write\""));
        assert!(s.contains("requires=("));
        assert!(s.contains("RequiredCapabilitySpec(capability=\"fs.read\""));
        assert!(s.contains("RequiredCapabilitySpec(capability=\"user.confirm\""));
    }

    #[test]
    fn policy_deny_with_condition_not() {
        let mut bind = indexmap::IndexMap::new();
        bind.insert(
            "path".to_string(),
            BindArg {
                kind: "arg".into(),
                name: "path".into(),
            },
        );
        let inner = Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Input { name: "cwd".into() },
            }),
            method: "contains".into(),
            args: vec![Expr::Ref {
                r#ref: Ref::Bound {
                    name: "path".into(),
                },
            }],
        };
        let policy = Policy {
            id: "deny fs.read(path) if not cwd.contains(path)".into(),
            kind: "deny".into(),
            trigger: nemoir_ir::Trigger {
                capability: "fs.read".into(),
                bind,
            },
            requires: None,
            condition: Some(Expr::Not {
                expr: Box::new(inner),
            }),
        };
        let s = super::emit_policy(&policy).unwrap();
        assert!(s.contains("kind=\"deny\""));
        assert!(s.contains("requires=()"));
        assert!(s.contains("condition=ExprSpec(kind=\"not\""));
        assert!(s.contains("method=\"contains\""));
    }

    #[test]
    fn stage_triage_minimal_shape() {
        let node = Node {
            id: "Triage".into(),
            annotations: vec!["entry".into()],
            prompt: "Understand user request. Ground your understanding to the codebase.".into(),
            reads: vec![Read {
                ref_: ref_input("task"),
                optional: false,
                origin: "implicit_entry_input".into(),
            }],
            writes: vec![Write {
                name: "summary".into(),
                ty: "string".into(),
                optional: false,
            }],
            requires: vec![StageCapability {
                capability: "fs.read".into(),
            }],
            transitions: vec![Transition {
                to: "Clarify".into(),
                priority: 0,
                reason: "next_stage_required_input_available".into(),
                guard: Guard::Always,
            }],
            execution: StageExecution::Model,
        };
        let s = super::emit_stage(&node).unwrap();
        assert!(s.starts_with("StageSpec(id=\"Triage\""));
        assert!(s.contains(
            "prompt=\"Understand user request. Ground your understanding to the codebase.\""
        ));
        assert!(s.contains(
            "reads=(ReadSpec(ref=RefSpec(kind=\"input\", name=\"task\"), optional=False),)"
        ));
        assert!(
            s.contains("writes=(WriteSpec(name=\"summary\", type=\"string\", optional=False),)")
        );
        assert!(s.contains("requires=frozenset({\"fs.read\"})"));
        assert!(s.contains("transitions=(TransitionSpec(to=\"Clarify\""));
    }

    #[test]
    fn node_annotations_are_dropped() {
        let node = Node {
            id: "X".into(),
            annotations: vec!["entry".into(), "extra".into()],
            prompt: "p".into(),
            reads: vec![],
            writes: vec![],
            requires: vec![],
            transitions: vec![],
            execution: StageExecution::Model,
        };
        let s = emit_stage(&node).unwrap();
        assert!(!s.contains("annotations"));
        assert!(!s.contains("entry"));
        assert!(!s.contains("extra"));
    }

    #[test]
    fn emit_expr_and_or_emits_exprs_tuple() {
        // Expr::And with 2 operands
        let and_expr = Expr::And {
            exprs: vec![
                Expr::Ref {
                    r#ref: ref_input("a"),
                },
                Expr::Literal {
                    ty: "string".into(),
                    value: serde_yaml::Value::String("x".into()),
                },
            ],
        };
        let s = emit_expr(&and_expr).unwrap();
        assert!(s.starts_with("ExprSpec(kind=\"and\", exprs=("));
        assert!(s.contains("kind=\"ref\""));
        assert!(s.contains("kind=\"literal\""));
        assert!(s.ends_with("))"));

        // Expr::Or with 1 operand — must have trailing comma for 1-tuple
        let or_expr = Expr::Or {
            exprs: vec![Expr::Ref {
                r#ref: ref_input("b"),
            }],
        };
        let s = emit_expr(&or_expr).unwrap();
        assert!(s.starts_with("ExprSpec(kind=\"or\", exprs=("));
        // Single-element exprs must be a 1-tuple: (e,)
        assert!(
            s.contains("exprs=(ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"input\", name=\"b\")),)"),
            "single-element exprs must have trailing comma for 1-tuple: {}",
            s
        );
    }

    #[test]
    fn deterministic_tool_stage_emits_execution() {
        let mut args = indexmap::IndexMap::new();
        args.insert(
            "command".to_string(),
            Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("echo hi".into()),
            },
        );
        let node = Node {
            id: "Run".into(),
            annotations: vec![],
            prompt: "".into(),
            reads: vec![],
            writes: vec![],
            requires: vec![StageCapability {
                capability: "os.shell".into(),
            }],
            transitions: vec![],
            execution: StageExecution::Tool {
                capability: "os.shell".into(),
                args,
            },
        };
        let s = emit_stage(&node).unwrap();
        assert!(s.contains("execution=StageExecutionSpec(kind=\"tool\""));
        assert!(s.contains("capability=\"os.shell\""));
        assert!(s.contains("args={\"command\": ExprSpec(kind=\"literal\", type=\"string\", value=\"echo hi\")}"));
    }

    #[test]
    fn model_stage_omits_execution() {
        let node = Node {
            id: "X".into(),
            annotations: vec![],
            prompt: "p".into(),
            reads: vec![],
            writes: vec![],
            requires: vec![],
            transitions: vec![],
            execution: StageExecution::Model,
        };
        let s = emit_stage(&node).unwrap();
        assert!(!s.contains("execution="));
    }
}
