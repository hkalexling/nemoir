use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::{
    Diagnostic, GraphError, NameError, ShapeError, TransitionError, TypeError,
};
use crate::resolve::ResolvedWorkflow;
use nemoir_ir::{Expr, Guard, Ref, Transition};
use serde_yaml;

pub fn validate(rw: &ResolvedWorkflow, filename: &str) -> Result<Vec<Vec<Transition>>, Diagnostic> {
    validate_shape(rw, filename)?;
    validate_policies(rw, filename)?;
    let transitions = infer_transitions(rw, filename)?;

    for (i, _node_transitions) in transitions.iter().enumerate() {
        if rw.stages[i]
            .annotations
            .iter()
            .any(|a| matches!(a, StageAnnotation::Exit))
            && !_node_transitions.is_empty()
        {
            return Err(Diagnostic::TransitionError(TransitionError {
                message: format!(
                    "exit stage `{}` has inferred outgoing transitions but should have none",
                    rw.stages[i].name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    rw.stages[i].name.span.start,
                    rw.stages[i].name.span.end,
                    "exit stage".into(),
                )),
                help: None,
            }));
        }
    }

    validate_graph(rw, &transitions, filename)?;
    Ok(transitions)
}

fn validate_policies(rw: &ResolvedWorkflow, filename: &str) -> Result<(), Diagnostic> {
    let input_names: HashSet<&str> = rw.inputs.iter().map(|i| i.name.text.as_str()).collect();
    let input_types: HashMap<&str, BaseType> = rw
        .inputs
        .iter()
        .map(|i| (i.name.text.as_str(), i.ty.base))
        .collect();

    for policy in &rw.policies {
        let bound_vars: HashSet<&str> = policy
            .trigger
            .args
            .iter()
            .map(|a| a.text.as_str())
            .collect();

        if let Some(ref requires) = policy.requires {
            for req in requires {
                for arg in &req.args {
                    if !bound_vars.contains(arg.text.as_str()) {
                        return Err(Diagnostic::NameError(NameError {
                            message: format!(
                                "unknown bound variable `{}` in policy requirement",
                                arg.text
                            ),
                            filename: filename.to_string(),
                            label: Some((
                                arg.span.start,
                                arg.span.end,
                                format!("`{}` is not bound by the trigger", arg.text),
                            )),
                            help: Some(
                                "bound variables come from the trigger capability arguments".into(),
                            ),
                        }));
                    }
                }
            }
        }

        if let Some(ref condition) = policy.condition {
            validate_policy_expr(condition, &input_names, &bound_vars, &input_types, filename)?;
            let cond_ty = type_of_policy_expr(condition, &input_names, &bound_vars, &input_types);
            if !matches!(cond_ty, Some(BaseType::Bool)) {
                let (span_start, span_end) = policy_expr_first_span(condition);
                return Err(Diagnostic::TypeError(TypeError {
                    message: format!(
                        "deny condition must be a bool expression, but is `{}`",
                        policy_expr_type_name(cond_ty)
                    ),
                    filename: filename.to_string(),
                    label: Some((span_start, span_end, "this condition".into())),
                }));
            }
        }
    }

    Ok(())
}

fn validate_policy_expr(
    expr: &PolicyExpr,
    input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    input_types: &HashMap<&str, BaseType>,
    filename: &str,
) -> Result<(), Diagnostic> {
    match expr {
        PolicyExpr::Not { expr } => {
            validate_policy_expr(expr, input_names, bound_vars, input_types, filename)?;
            let inner_ty = type_of_policy_expr(expr, input_names, bound_vars, input_types);
            if !matches!(inner_ty, Some(BaseType::Bool)) {
                let (span_start, span_end) = policy_expr_first_span(expr);
                return Err(Diagnostic::TypeError(TypeError {
                    message: format!(
                        "`not` requires a bool expression, but got `{}`",
                        policy_expr_type_name(inner_ty)
                    ),
                    filename: filename.to_string(),
                    label: Some((span_start, span_end, "this expression".into())),
                }));
            }
        }
        PolicyExpr::MethodCall {
            receiver,
            method: _,
            args,
        } => {
            if !input_names.contains(receiver.text.as_str())
                && !bound_vars.contains(receiver.text.as_str())
            {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "unknown ref `{}` in policy expression; expected a workflow input or bound variable",
                        receiver.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        receiver.span.start,
                        receiver.span.end,
                        format!("`{}` is not a workflow input or bound variable", receiver.text),
                    )),
                    help: None,
                }));
            }
            for arg in args {
                if !input_names.contains(arg.text.as_str())
                    && !bound_vars.contains(arg.text.as_str())
                {
                    return Err(Diagnostic::NameError(NameError {
                        message: format!(
                            "unknown ref `{}` in policy expression; expected a workflow input or bound variable",
                            arg.text
                        ),
                        filename: filename.to_string(),
                        label: Some((
                            arg.span.start,
                            arg.span.end,
                            format!("`{}` is not a workflow input or bound variable", arg.text),
                        )),
                        help: None,
                    }));
                }
            }
        }
        PolicyExpr::Ref(id) => {
            if !input_names.contains(id.text.as_str()) && !bound_vars.contains(id.text.as_str()) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "unknown ref `{}` in policy expression; expected a workflow input or bound variable",
                        id.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        id.span.start,
                        id.span.end,
                        format!(
                            "`{}` is not a workflow input or bound variable",
                            id.text
                        ),
                    )),
                    help: None,
                }));
            }
        }
    }
    Ok(())
}

fn type_of_policy_expr(
    expr: &PolicyExpr,
    _input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    input_types: &HashMap<&str, BaseType>,
) -> Option<BaseType> {
    match expr {
        PolicyExpr::Not { .. } => Some(BaseType::Bool),
        PolicyExpr::MethodCall {
            receiver,
            method,
            args: _,
        } => {
            if method.text == "contains" {
                let recv_is_bound = bound_vars.contains(receiver.text.as_str());
                if recv_is_bound {
                    // Bound variable: type is unknown, allow per plan's "path.contains(any)"
                    Some(BaseType::Bool)
                } else {
                    let recv_ty = input_types.get(receiver.text.as_str());
                    match recv_ty {
                        Some(BaseType::Path) => Some(BaseType::Bool),
                        _ => None,
                    }
                }
            } else {
                None
            }
        }
        PolicyExpr::Ref(id) => input_types.get(id.text.as_str()).copied(),
    }
}

fn policy_expr_type_name(ty: Option<BaseType>) -> &'static str {
    match ty {
        Some(BaseType::Bool) => "bool",
        Some(BaseType::String) => "string",
        Some(BaseType::Path) => "path",
        Some(BaseType::Unknown) => "unknown",
        None => "unknown",
    }
}

fn policy_expr_first_span(expr: &PolicyExpr) -> (usize, usize) {
    match expr {
        PolicyExpr::Not { expr } => policy_expr_first_span(expr),
        PolicyExpr::MethodCall { receiver, .. } => (receiver.span.start, receiver.span.end),
        PolicyExpr::Ref(id) => (id.span.start, id.span.end),
    }
}

fn validate_shape(rw: &ResolvedWorkflow, filename: &str) -> Result<(), Diagnostic> {
    if rw.stages.is_empty() {
        return Err(Diagnostic::ShapeError(ShapeError {
            message: "workflow has no stages".into(),
            filename: filename.to_string(),
            label: None,
            help: None,
        }));
    }

    let entry_stages: Vec<_> = rw
        .stages
        .iter()
        .filter(|s| {
            s.annotations
                .iter()
                .any(|a| matches!(a, StageAnnotation::Entry))
        })
        .collect();

    if entry_stages.len() > 1 {
        let names: Vec<_> = entry_stages.iter().map(|s| s.name.text.clone()).collect();
        return Err(Diagnostic::ShapeError(ShapeError {
            message: format!("multiple @entry stages: {}", names.join(", ")),
            filename: filename.to_string(),
            label: None,
            help: Some("only one stage can be annotated @entry".into()),
        }));
    }

    Ok(())
}

pub fn infer_transitions(
    rw: &ResolvedWorkflow,
    filename: &str,
) -> Result<Vec<Vec<Transition>>, Diagnostic> {
    let stage_names: Vec<&str> = rw.stages.iter().map(|s| s.name.text.as_str()).collect();
    let mut all_transitions: Vec<Vec<Transition>> = Vec::new();

    for (i, _stage) in rw.stages.iter().enumerate() {
        let transitions = infer_stage_transitions(rw, i, &stage_names, filename)?;
        all_transitions.push(transitions);
    }

    Ok(all_transitions)
}

fn infer_stage_transitions(
    rw: &ResolvedWorkflow,
    stage_idx: usize,
    _stage_names: &[&str],
    filename: &str,
) -> Result<Vec<Transition>, Diagnostic> {
    let stage = &rw.stages[stage_idx];

    if stage
        .annotations
        .iter()
        .any(|a| matches!(a, StageAnnotation::Exit))
    {
        return Ok(vec![]);
    }

    // Rule 1: Bool branch
    for output in &stage.outputs {
        if let Some(branches) = &output.branches {
            let (guard_node, guard_field) = (stage.name.text.clone(), output.name.text.clone());
            return Ok(vec![
                Transition {
                    to: branches.true_target.text.clone(),
                    priority: 0,
                    reason: "output_branch_true".into(),
                    guard: Guard::Eq {
                        left: Expr::Ref {
                            r#ref: Ref::NodeOutput {
                                node: guard_node.clone(),
                                field: guard_field.clone(),
                            },
                        },
                        right: Expr::Literal {
                            ty: "bool".to_string(),
                            value: serde_yaml::Value::Bool(true),
                        },
                    },
                },
                Transition {
                    to: branches.false_target.text.clone(),
                    priority: 1,
                    reason: "output_branch_false".into(),
                    guard: Guard::Eq {
                        left: Expr::Ref {
                            r#ref: Ref::NodeOutput {
                                node: guard_node.clone(),
                                field: guard_field.clone(),
                            },
                        },
                        right: Expr::Literal {
                            ty: "bool".to_string(),
                            value: serde_yaml::Value::Bool(false),
                        },
                    },
                },
            ]);
        }
    }

    // Rule 2: Backward-reference loop
    let backward_refs: Vec<usize> = rw
        .stages
        .iter()
        .take(stage_idx)
        .filter(|prior_stage| {
            prior_stage
                .inputs
                .iter()
                .any(|input_ref| input_ref.stage.text == stage.name.text)
        })
        .map(|s| s.index)
        .collect();

    if backward_refs.len() > 1 {
        return Err(Diagnostic::TransitionError(TransitionError {
            message: format!(
                "ambiguous backward-reference loop: multiple prior stages reference outputs from `{}`",
                stage.name.text
            ),
            filename: filename.to_string(),
            label: Some((
                stage.name.span.start,
                stage.name.span.end,
                "this stage's outputs are referenced by multiple prior stages".into(),
            )),
            help: Some(
                "future versions will support explicit transition syntax to resolve this".into(),
            ),
        }));
    }

    if backward_refs.len() == 1 {
        let target_name = rw.stages[backward_refs[0]].name.text.clone();
        return Ok(vec![Transition {
            to: target_name,
            priority: 0,
            reason: "backward_ref_loop".into(),
            guard: Guard::Always,
        }]);
    }

    // Rule 3: Optional-skip
    let next_idx = stage_idx + 1;

    if next_idx < rw.stages.len() {
        let next_stage = &rw.stages[next_idx];

        // Find non-optional inputs on next stage that reference optional outputs from any prior stage
        let mut optional_refs: Vec<(usize, String, String)> = Vec::new();

        for input_ref in &next_stage.inputs {
            if input_ref.optional {
                continue;
            }
            let source_stage_idx = rw.stage_map.get(&input_ref.stage.text).copied();
            if let Some(source_idx) = source_stage_idx {
                if source_idx > stage_idx {
                    continue;
                }
                let source_stage = &rw.stages[source_idx];
                for output in &source_stage.outputs {
                    if output.name.text == input_ref.field.text && output.ty.optional {
                        optional_refs.push((
                            source_idx,
                            source_stage.name.text.clone(),
                            output.name.text.clone(),
                        ));
                    }
                }
            }
        }

        if optional_refs.len() > 1 {
            return Err(Diagnostic::TransitionError(TransitionError {
                message: "optional-skip would need a compound guard: the next stage has multiple non-optional inputs that reference optional outputs".into(),
                filename: filename.to_string(),
                label: Some((
                    next_stage.name.span.start,
                    next_stage.name.span.end,
                    "next stage has multiple guards needed".into(),
                )),
                help: Some("compound guards are not yet supported in this prototype".into()),
            }));
        }

        if optional_refs.len() == 1 {
            let (_, node_name, field_name) = &optional_refs[0];
            let (guard_node, guard_field) = (node_name.clone(), field_name.clone());

            // Skip target: the stage after next, or exit
            let skip_idx = next_idx + 1;
            let skip_target = if skip_idx < rw.stages.len() {
                rw.stages[skip_idx].name.text.clone()
            } else {
                return Err(Diagnostic::TransitionError(TransitionError {
                    message: format!(
                        "optional-skip would skip past the last stage from `{}`",
                        stage.name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        stage.name.span.start,
                        stage.name.span.end,
                        "no valid skip target".into(),
                    )),
                    help: None,
                }));
            };

            return Ok(vec![
                Transition {
                    to: rw.stages[next_idx].name.text.clone(),
                    priority: 0,
                    reason: "next_stage_required_input_available".into(),
                    guard: Guard::HasValue {
                        r#ref: Ref::NodeOutput {
                            node: guard_node.clone(),
                            field: guard_field.clone(),
                        },
                    },
                },
                Transition {
                    to: skip_target,
                    priority: 1,
                    reason: "skip_next_stage_required_input_missing".into(),
                    guard: Guard::Missing {
                        r#ref: Ref::NodeOutput {
                            node: guard_node.clone(),
                            field: guard_field.clone(),
                        },
                    },
                },
            ]);
        }
    }

    // Rule 4: Fallthrough
    if next_idx < rw.stages.len() {
        Ok(vec![Transition {
            to: rw.stages[next_idx].name.text.clone(),
            priority: 0,
            reason: "fallthrough".into(),
            guard: Guard::Always,
        }])
    } else {
        Err(Diagnostic::TransitionError(TransitionError {
            message: format!(
                "non-exit final stage `{}` has no valid fallthrough target",
                stage.name.text
            ),
            filename: filename.to_string(),
            label: Some((
                stage.name.span.start,
                stage.name.span.end,
                "last non-exit stage must be able to reach an exit".into(),
            )),
            help: Some("add a bool branch output or make this an exit stage".into()),
        }))
    }
}

fn validate_graph(
    rw: &ResolvedWorkflow,
    transitions: &[Vec<Transition>],
    filename: &str,
) -> Result<(), Diagnostic> {
    let n = rw.stages.len();
    let stage_to_idx: HashMap<&str, usize> = rw
        .stages
        .iter()
        .map(|s| (s.name.text.as_str(), s.index))
        .collect();

    // Build adjacency from transitions
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, node_transitions) in transitions.iter().enumerate() {
        for t in node_transitions {
            if let Some(&target_idx) = stage_to_idx.get(t.to.as_str()) {
                adj[i].push(target_idx);
            } else {
                return Err(Diagnostic::GraphError(GraphError {
                    message: format!(
                        "transition to unknown stage `{}` from `{}`",
                        t.to, rw.stages[i].name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        rw.stages[i].name.span.start,
                        rw.stages[i].name.span.end,
                        format!("transition target `{}` not found", t.to),
                    )),
                    help: None,
                }));
            }
        }
    }

    // Find entry
    let entry_idx = rw
        .stages
        .iter()
        .find(|s| {
            s.annotations
                .iter()
                .any(|a| matches!(a, StageAnnotation::Entry))
        })
        .map(|s| s.index)
        .expect("entry stage must exist");

    // Find reachable nodes from entry
    let mut reachable = vec![false; n];
    let mut stack = vec![entry_idx];
    reachable[entry_idx] = true;

    while let Some(u) = stack.pop() {
        for &v in &adj[u] {
            if !reachable[v] {
                reachable[v] = true;
                stack.push(v);
            }
        }
    }

    for (i, reachable_item) in reachable.iter_mut().enumerate() {
        if !*reachable_item {
            return Err(Diagnostic::GraphError(GraphError {
                message: format!(
                    "stage `{}` is unreachable from entry",
                    rw.stages[i].name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    rw.stages[i].name.span.start,
                    rw.stages[i].name.span.end,
                    "unreachable stage".into(),
                )),
                help: Some("add a transition to this stage from another reachable stage".into()),
            }));
        }
    }

    // Check that non-exit stages can reach an exit
    let exit_indices: HashSet<usize> = rw
        .stages
        .iter()
        .filter(|s| {
            s.annotations
                .iter()
                .any(|a| matches!(a, StageAnnotation::Exit))
        })
        .map(|s| s.index)
        .collect();

    for i in 0..n {
        let is_exit = exit_indices.contains(&i);
        if is_exit {
            continue;
        }

        // Check reachability to any exit
        let mut visited = vec![false; n];
        let mut stack = vec![i];
        visited[i] = true;
        let mut can_reach_exit = false;

        while let Some(u) = stack.pop() {
            if exit_indices.contains(&u) {
                can_reach_exit = true;
                break;
            }
            for &v in &adj[u] {
                if !visited[v] {
                    visited[v] = true;
                    stack.push(v);
                }
            }
        }

        if !can_reach_exit {
            return Err(Diagnostic::GraphError(GraphError {
                message: format!(
                    "stage `{}` cannot reach any exit stage",
                    rw.stages[i].name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    rw.stages[i].name.span.start,
                    rw.stages[i].name.span.end,
                    "no path to exit".into(),
                )),
                help: Some(
                    "ensure there is a transition path from this stage to an @exit stage".into(),
                ),
            }));
        }

        // Non-exit stage must have at least one outgoing transition
        if adj[i].is_empty() {
            return Err(Diagnostic::GraphError(GraphError {
                message: format!(
                    "non-exit stage `{}` has no outgoing transitions",
                    rw.stages[i].name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    rw.stages[i].name.span.start,
                    rw.stages[i].name.span.end,
                    "no transitions".into(),
                )),
                help: Some("add a bool branch output or rely on inferred transition rules".into()),
            }));
        }
    }

    // Data availability validation (conservative)
    validate_data_availability(rw, transitions, &adj, filename)?;

    Ok(())
}

fn can_reach(adj: &[Vec<usize>], from: usize, to: usize, n: usize) -> bool {
    let mut visited = vec![false; n];
    let mut stack = vec![from];
    visited[from] = true;
    while let Some(u) = stack.pop() {
        if u == to {
            return true;
        }
        for &v in &adj[u] {
            if !visited[v] {
                visited[v] = true;
                stack.push(v);
            }
        }
    }
    false
}

fn validate_data_availability(
    rw: &ResolvedWorkflow,
    transitions: &[Vec<Transition>],
    adj: &[Vec<usize>],
    filename: &str,
) -> Result<(), Diagnostic> {
    let n = rw.stages.len();

    for (i, stage) in rw.stages.iter().enumerate() {
        for input_ref in &stage.inputs {
            if input_ref.optional {
                continue;
            }

            let source_idx = rw.stage_map.get(&input_ref.stage.text).copied();

            if let Some(si) = source_idx {
                let source_stage = &rw.stages[si];
                let output = source_stage
                    .outputs
                    .iter()
                    .find(|f| f.name.text == input_ref.field.text);

                if let Some(out) = output {
                    if out.ty.optional {
                        // Non-optional read of an optional output.
                        // All incoming paths must have a has_value guard.
                        let guarded = has_incoming_guard(
                            rw,
                            transitions,
                            i,
                            &input_ref.stage.text,
                            &input_ref.field.text,
                        );

                        if !guarded {
                            let source_out =
                                format!("{}.{}", input_ref.stage.text, input_ref.field.text);
                            return Err(Diagnostic::GraphError(GraphError {
                                message: format!(
                                    "stage `{}` has a required read of optional output `{}` that may not be available",
                                    stage.name.text, source_out
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    input_ref.span.start,
                                    input_ref.span.end,
                                    format!("`{}` is optional", source_out),
                                )),
                                help: Some(
                                    "this is valid only when the predecessor transition checks has_value for this output"
                                        .into(),
                                ),
                            }));
                        }
                    } else if si == i {
                        // Self-read: a stage reading its own output (rare; reject)
                        return Err(Diagnostic::GraphError(GraphError {
                            message: format!(
                                "stage `{}` cannot read its own output `{}`",
                                stage.name.text, input_ref.field.text
                            ),
                            filename: filename.to_string(),
                            label: Some((
                                input_ref.span.start,
                                input_ref.span.end,
                                format!("self-referential read of `{}`", input_ref.field.text),
                            )),
                            help: None,
                        }));
                    } else if !can_reach(adj, si, i, n) {
                        // Producer cannot reach consumer in the CFG
                        let source_out =
                            format!("{}.{}", input_ref.stage.text, input_ref.field.text);
                        return Err(Diagnostic::GraphError(GraphError {
                            message: format!(
                                "stage `{}` requires `{}` but `{}` cannot reach `{}` on any control-flow path",
                                stage.name.text, source_out, input_ref.stage.text, stage.name.text
                            ),
                            filename: filename.to_string(),
                            label: Some((
                                input_ref.span.start,
                                input_ref.span.end,
                                format!(
                                    "`{}` has not executed before `{}` on any path",
                                    input_ref.stage.text, stage.name.text
                                ),
                            )),
                            help: Some(
                                "a stage can only read outputs from producers that can reach it via control-flow edges"
                                    .into(),
                            ),
                        }));
                    }
                }
            }
        }
    }

    Ok(())
}

fn has_incoming_guard(
    rw: &ResolvedWorkflow,
    transitions: &[Vec<Transition>],
    target_idx: usize,
    guard_node: &str,
    guard_field: &str,
) -> bool {
    let mut predecessors = Vec::new();

    for (i, node_transitions) in transitions.iter().enumerate() {
        for t in node_transitions {
            let to_idx = rw.stage_map.get(t.to.as_str()).copied();
            if to_idx == Some(target_idx) {
                predecessors.push((i, t.clone()));
            }
        }
    }

    if predecessors.is_empty() {
        return false;
    }

    predecessors.iter().all(|(_, t)| {
        matches!(
            &t.guard,
            Guard::HasValue { r#ref: Ref::NodeOutput { node, field } }
                if node == guard_node && field == guard_field
        )
    })
}
