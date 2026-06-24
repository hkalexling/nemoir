use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::{
    Diagnostic, GraphError, NameError, ShapeError, TransitionError, TypeError,
};
use crate::resolve::ResolvedWorkflow;
use nemoir_ir::capabilities;
use nemoir_ir::{Expr, Guard, Ref, Transition};
use serde_yaml;

pub fn validate(rw: &ResolvedWorkflow, filename: &str) -> Result<Vec<Vec<Transition>>, Diagnostic> {
    validate_shape(rw, filename)?;
    validate_stage_capabilities(rw, filename)?;
    validate_exec(rw, filename)?;
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

fn validate_stage_capabilities(rw: &ResolvedWorkflow, filename: &str) -> Result<(), Diagnostic> {
    for stage in &rw.stages {
        for cap in &stage.requires {
            if !capabilities::is_known_capability(&cap.text) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "unknown capability `{}` in stage `{}`",
                        cap.text, stage.name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        cap.span.start,
                        cap.span.end,
                        format!("`{}` is not a known NemoIR capability", cap.text),
                    )),
                    help: None,
                }));
            }
        }
    }
    Ok(())
}

fn validate_exec(rw: &ResolvedWorkflow, filename: &str) -> Result<(), Diagnostic> {
    let input_names: HashSet<&str> = rw.inputs.iter().map(|i| i.name.text.as_str()).collect();
    for stage in &rw.stages {
        let Some(ref exec) = stage.exec else {
            continue;
        };
        // Exec capability must be known.
        if !capabilities::is_known_capability(&exec.capability.text) {
            return Err(Diagnostic::NameError(NameError {
                message: format!(
                    "unknown exec capability `{}` in stage `{}`",
                    exec.capability.text, stage.name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    exec.capability.span.start,
                    exec.capability.span.end,
                    format!(
                        "`{}` is not a known NemoIR capability",
                        exec.capability.text
                    ),
                )),
                help: None,
            }));
        }
        // Validate args
        let spec = capabilities::get_capability(&exec.capability.text).unwrap();
        let mut seen_params: HashSet<&str> = HashSet::new();
        for arg in &exec.args {
            if seen_params.contains(arg.name.text.as_str()) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "duplicate exec arg `{}` in stage `{}`",
                        arg.name.text, stage.name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        arg.name.span.start,
                        arg.name.span.end,
                        format!("arg `{}` appears more than once", arg.name.text),
                    )),
                    help: None,
                }));
            }
            seen_params.insert(arg.name.text.as_str());
            // Must be a required param of the capability
            if !spec.has_required_param(&arg.name.text) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "unknown exec arg `{}` for capability `{}` in stage `{}`",
                        arg.name.text, exec.capability.text, stage.name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        arg.name.span.start,
                        arg.name.span.end,
                        format!(
                            "`{}` is not a required parameter of `{}`",
                            arg.name.text, exec.capability.text
                        ),
                    )),
                    help: None,
                }));
            }
            // Validate arg value refs
            match &arg.value {
                ExecValue::Ref(_r) => {
                    // Stage.field ref — validated in resolve.rs already
                    // (field existence checked there)
                }
                ExecValue::InputRef(id) => {
                    if !input_names.contains(id.text.as_str()) {
                        return Err(Diagnostic::NameError(NameError {
                            message: format!(
                                "unknown workflow input `{}` in exec arg in stage `{}`",
                                id.text, stage.name.text
                            ),
                            filename: filename.to_string(),
                            label: Some((
                                id.span.start,
                                id.span.end,
                                format!("`{}` is not a workflow input", id.text),
                            )),
                            help: None,
                        }));
                    }
                }
                ExecValue::String(_) => {
                    // String literal — always valid
                }
            }
        }
        // All required catalog params must be provided
        for param in spec.required_params {
            if !seen_params.contains(param.name) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!(
                        "missing required exec arg `{}` for capability `{}` in stage `{}`",
                        param.name, exec.capability.text, stage.name.text
                    ),
                    filename: filename.to_string(),
                    label: Some((
                        exec.capability.span.start,
                        exec.capability.span.end,
                        format!(
                            "`{}` requires parameter `{}`",
                            exec.capability.text, param.name
                        ),
                    )),
                    help: None,
                }));
            }
        }
    }
    Ok(())
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

        if !capabilities::is_known_capability(&policy.trigger.capability.text) {
            return Err(Diagnostic::NameError(NameError {
                message: format!(
                    "unknown capability `{}` in policy trigger",
                    policy.trigger.capability.text
                ),
                filename: filename.to_string(),
                label: Some((
                    policy.trigger.capability.span.start,
                    policy.trigger.capability.span.end,
                    format!(
                        "`{}` is not a known NemoIR capability",
                        policy.trigger.capability.text
                    ),
                )),
                help: None,
            }));
        }

        if let Some(spec) = capabilities::get_capability(&policy.trigger.capability.text) {
            for arg in &policy.trigger.args {
                if !spec.has_required_param(&arg.text) {
                    return Err(Diagnostic::NameError(NameError {
                        message: format!(
                            "capability `{}` has no required parameter `{}`",
                            policy.trigger.capability.text, arg.text
                        ),
                        filename: filename.to_string(),
                        label: Some((
                            arg.span.start,
                            arg.span.end,
                            format!(
                                "`{}` is not a required parameter of `{}`",
                                arg.text, policy.trigger.capability.text
                            ),
                        )),
                        help: None,
                    }));
                }
            }
        }

        if let Some(ref requires) = policy.requires {
            for req in requires {
                if !capabilities::is_known_capability(&req.capability.text) {
                    return Err(Diagnostic::NameError(NameError {
                        message: format!(
                            "unknown capability `{}` in policy requirement",
                            req.capability.text
                        ),
                        filename: filename.to_string(),
                        label: Some((
                            req.capability.span.start,
                            req.capability.span.end,
                            format!("`{}` is not a known NemoIR capability", req.capability.text),
                        )),
                        help: None,
                    }));
                }

                if let Some(spec) = capabilities::get_capability(&req.capability.text) {
                    for arg in &req.args {
                        if !spec.has_required_param(&arg.text) {
                            return Err(Diagnostic::NameError(NameError {
                                message: format!(
                                    "capability `{}` has no required parameter `{}`",
                                    req.capability.text, arg.text
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    arg.span.start,
                                    arg.span.end,
                                    format!(
                                        "`{}` is not a required parameter of `{}`",
                                        arg.text, req.capability.text
                                    ),
                                )),
                                help: None,
                            }));
                        }
                    }
                }

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
            validate_policy_expr(
                condition,
                &input_names,
                &bound_vars,
                &input_types,
                &policy.trigger.capability.text,
                filename,
            )?;
            let cond_ty = type_of_policy_expr(
                condition,
                &input_names,
                &bound_vars,
                &input_types,
                &policy.trigger.capability.text,
            );
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
    trigger_capability: &str,
    filename: &str,
) -> Result<(), Diagnostic> {
    match expr {
        PolicyExpr::Not { expr } => {
            validate_policy_expr(
                expr,
                input_names,
                bound_vars,
                input_types,
                trigger_capability,
                filename,
            )?;
            let inner_ty = type_of_policy_expr(
                expr,
                input_names,
                bound_vars,
                input_types,
                trigger_capability,
            );
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
        PolicyExpr::Or { exprs } | PolicyExpr::And { exprs } => {
            if exprs.is_empty() {
                return Err(Diagnostic::TypeError(TypeError {
                    message: "and/or requires at least 1 operand".into(),
                    filename: filename.to_string(),
                    label: None,
                }));
            }
            for e in exprs {
                validate_policy_expr(
                    e,
                    input_names,
                    bound_vars,
                    input_types,
                    trigger_capability,
                    filename,
                )?;
                // Plan §5: and/or are boolean combinators; every operand must be bool.
                let op_ty = type_of_policy_expr(
                    e,
                    input_names,
                    bound_vars,
                    input_types,
                    trigger_capability,
                );
                if !matches!(op_ty, Some(BaseType::Bool)) {
                    let (span_start, span_end) = policy_expr_first_span(e);
                    return Err(Diagnostic::TypeError(TypeError {
                        message: format!(
                            "and/or requires bool operands, but got `{}`",
                            policy_expr_type_name(op_ty)
                        ),
                        filename: filename.to_string(),
                        label: Some((span_start, span_end, "this operand".into())),
                    }));
                }
            }
        }
        PolicyExpr::MethodCall {
            receiver,
            method,
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
                validate_policy_expr_value(arg, input_names, bound_vars, filename)?;
            }
            // Method and type validation
            let recv_ty = infer_receiver_type(
                receiver,
                input_names,
                bound_vars,
                input_types,
                trigger_capability,
            );
            match method.text.as_str() {
                "contains" => {
                    // Plan §5: contains accepts exactly 1 argument.
                    if args.len() != 1 {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: "contains() requires exactly 1 argument".into(),
                            filename: filename.to_string(),
                            label: Some((method.span.start, method.span.end, "this method".into())),
                        }));
                    }
                    let arg_ty = policy_expr_value_type(
                        &args[0],
                        input_names,
                        bound_vars,
                        input_types,
                        trigger_capability,
                    );
                    // Plan §5: only Path.contains(Path|string) and string.contains(string) are supported.
                    match recv_ty {
                        Some(BaseType::Path) => {
                            if arg_ty != Some(BaseType::Path) && arg_ty != Some(BaseType::String) {
                                return Err(Diagnostic::TypeError(TypeError {
                                    message: "path.contains() argument must be path or string"
                                        .into(),
                                    filename: filename.to_string(),
                                    label: None,
                                }));
                            }
                        }
                        Some(BaseType::String) => {
                            if arg_ty != Some(BaseType::String) {
                                return Err(Diagnostic::TypeError(TypeError {
                                    message: "string.contains() argument must be string".into(),
                                    filename: filename.to_string(),
                                    label: None,
                                }));
                            }
                        }
                        // Plan §5: bool and unknown receiver types are unsupported.
                        Some(BaseType::Bool) | Some(BaseType::Unknown) | None => {
                            return Err(Diagnostic::TypeError(TypeError {
                                message: format!(
                                    "contains() is not supported on receiver type {:?}",
                                    recv_ty
                                ),
                                filename: filename.to_string(),
                                label: None,
                            }));
                        }
                    }
                }
                "eq" => {
                    if args.len() != 1 {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: "eq() requires exactly 1 argument".into(),
                            filename: filename.to_string(),
                            label: Some((method.span.start, method.span.end, "this method".into())),
                        }));
                    }
                    let arg_ty = policy_expr_value_type(
                        &args[0],
                        input_names,
                        bound_vars,
                        input_types,
                        trigger_capability,
                    );
                    // Plan §5: Path.eq(Path/string) allowed; string.eq(string) allowed;
                    // string.eq(Path) and all other combinations (bool, unknown, etc.) rejected.
                    let compatible = match (recv_ty, arg_ty) {
                        (Some(BaseType::Path), Some(BaseType::Path))
                        | (Some(BaseType::Path), Some(BaseType::String)) => true,
                        (Some(BaseType::String), Some(BaseType::String)) => true,
                        (Some(BaseType::String), Some(BaseType::Path)) => false,
                        _ => false, // Plan §5: no other type combinations are supported for eq
                    };
                    if !compatible {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: format!(
                                "eq() compares incompatible types: {:?} vs {:?}",
                                recv_ty, arg_ty
                            ),
                            filename: filename.to_string(),
                            label: None,
                        }));
                    }
                }
                "starts_with" => {
                    if args.len() != 1 {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: "starts_with() requires exactly 1 argument".into(),
                            filename: filename.to_string(),
                            label: Some((method.span.start, method.span.end, "this method".into())),
                        }));
                    }
                    if recv_ty != Some(BaseType::String) && recv_ty.is_some() {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: "starts_with() requires a string receiver".into(),
                            filename: filename.to_string(),
                            label: None,
                        }));
                    }
                    let arg_ty = policy_expr_value_type(
                        &args[0],
                        input_names,
                        bound_vars,
                        input_types,
                        trigger_capability,
                    );
                    if arg_ty != Some(BaseType::String) && arg_ty.is_some() {
                        return Err(Diagnostic::TypeError(TypeError {
                            message: "starts_with() requires a string argument".into(),
                            filename: filename.to_string(),
                            label: None,
                        }));
                    }
                }
                _ => {
                    return Err(Diagnostic::NameError(NameError {
                        message: format!(
                            "unknown method `{}`; expected contains, eq, or starts_with",
                            method.text
                        ),
                        filename: filename.to_string(),
                        label: Some((method.span.start, method.span.end, "this method".into())),
                        help: None,
                    }));
                }
            }
        }
        PolicyExpr::In { value, options } => {
            if !input_names.contains(value.text.as_str())
                && !bound_vars.contains(value.text.as_str())
            {
                return Err(Diagnostic::NameError(NameError {
                    message: format!("unknown ref `{}` in `in [...]` expression", value.text),
                    filename: filename.to_string(),
                    label: Some((
                        value.span.start,
                        value.span.end,
                        format!("`{}` is not a workflow input or bound variable", value.text),
                    )),
                    help: None,
                }));
            }
            if options.is_empty() {
                return Err(Diagnostic::TypeError(TypeError {
                    message: "`in []` is empty; must have at least one option".into(),
                    filename: filename.to_string(),
                    label: None,
                }));
            }
            // Check each option's type against the LHS value ({eq} compatibility).
            let val_ty = infer_receiver_type(
                value,
                input_names,
                bound_vars,
                input_types,
                trigger_capability,
            );
            for opt in options {
                validate_policy_expr_value(opt, input_names, bound_vars, filename)?;
                let opt_ty = policy_expr_value_type(
                    opt,
                    input_names,
                    bound_vars,
                    input_types,
                    trigger_capability,
                );
                // Apply the same compatibility rules as eq():
                // path value accepts path/string; string value accepts only string
                let compatible = match (val_ty, opt_ty) {
                    (Some(BaseType::Path), Some(BaseType::Path))
                    | (Some(BaseType::Path), Some(BaseType::String)) => true,
                    (Some(BaseType::String), Some(BaseType::String)) => true,
                    (Some(BaseType::String), Some(BaseType::Path)) => false,
                    _ => false, // Plan §5: no other type combinations for eq
                };
                if !compatible {
                    return Err(Diagnostic::TypeError(TypeError {
                        message: format!(
                            "`in [...]` option type incompatible with `{}`: {:?} vs {:?}",
                            value.text, val_ty, opt_ty
                        ),
                        filename: filename.to_string(),
                        label: None,
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

fn validate_policy_expr_value(
    val: &PolicyExprValue,
    input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    filename: &str,
) -> Result<(), Diagnostic> {
    if let PolicyExprValue::Ref(id) = val {
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
                    format!("`{}` is not a workflow input or bound variable", id.text),
                )),
                help: None,
            }));
        }
    }
    Ok(())
}

fn policy_expr_value_type(
    val: &PolicyExprValue,
    _input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    input_types: &HashMap<&str, BaseType>,
    trigger_capability: &str,
) -> Option<BaseType> {
    match val {
        PolicyExprValue::Ref(id) => {
            if bound_vars.contains(id.text.as_str()) {
                // Bound variables: look up type from the capability catalog
                nemoir_ir::capabilities::bound_var_type(trigger_capability, &id.text).map(|t| {
                    match t {
                        nemoir_ir::capabilities::CapabilityParamType::String => BaseType::String,
                        nemoir_ir::capabilities::CapabilityParamType::Path => BaseType::Path,
                        nemoir_ir::capabilities::CapabilityParamType::Bool => BaseType::Bool,
                    }
                })
            } else {
                input_types.get(id.text.as_str()).copied()
            }
        }
        PolicyExprValue::String(_) => Some(BaseType::String),
    }
}

fn infer_receiver_type(
    receiver: &Ident,
    input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    input_types: &HashMap<&str, BaseType>,
    trigger_capability: &str,
) -> Option<BaseType> {
    if input_names.contains(receiver.text.as_str()) {
        input_types.get(receiver.text.as_str()).copied()
    } else if bound_vars.contains(receiver.text.as_str()) {
        // Bound variable: look up type from the capability catalog
        nemoir_ir::capabilities::bound_var_type(trigger_capability, &receiver.text).map(|t| match t
        {
            nemoir_ir::capabilities::CapabilityParamType::String => BaseType::String,
            nemoir_ir::capabilities::CapabilityParamType::Path => BaseType::Path,
            nemoir_ir::capabilities::CapabilityParamType::Bool => BaseType::Bool,
        })
    } else {
        None
    }
}

fn type_of_policy_expr(
    expr: &PolicyExpr,
    _input_names: &HashSet<&str>,
    bound_vars: &HashSet<&str>,
    input_types: &HashMap<&str, BaseType>,
    trigger_capability: &str,
) -> Option<BaseType> {
    match expr {
        PolicyExpr::Not { .. } | PolicyExpr::Or { .. } | PolicyExpr::And { .. } => {
            Some(BaseType::Bool)
        }
        PolicyExpr::In { .. } => Some(BaseType::Bool),
        PolicyExpr::MethodCall { method, .. } => match method.text.as_str() {
            "contains" | "eq" | "starts_with" => Some(BaseType::Bool),
            _ => None,
        },
        PolicyExpr::Ref(id) => {
            // Check workflow inputs first, then bound trigger variables.
            input_types.get(id.text.as_str()).copied().or_else(|| {
                if bound_vars.contains(id.text.as_str()) {
                    nemoir_ir::capabilities::bound_var_type(trigger_capability, &id.text).map(|t| {
                        match t {
                            nemoir_ir::capabilities::CapabilityParamType::String => {
                                BaseType::String
                            }
                            nemoir_ir::capabilities::CapabilityParamType::Path => BaseType::Path,
                            nemoir_ir::capabilities::CapabilityParamType::Bool => BaseType::Bool,
                        }
                    })
                } else {
                    None
                }
            })
        }
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
        PolicyExpr::Or { exprs } | PolicyExpr::And { exprs } => policy_expr_first_span(&exprs[0]),
        PolicyExpr::MethodCall { receiver, .. } => (receiver.span.start, receiver.span.end),
        PolicyExpr::In { value, .. } => (value.span.start, value.span.end),
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
