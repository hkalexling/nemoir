use std::collections::HashSet;

use indexmap::IndexMap;
use nemoir_ir::*;

use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolvedWorkflow;

pub fn lower(
    rw: &ResolvedWorkflow,
    transitions: &[Vec<Transition>],
    filename: &str,
) -> Result<WorkflowIr, Diagnostic> {
    let entry_stage = rw
        .stages
        .iter()
        .find(|s| {
            s.annotations
                .iter()
                .any(|a| matches!(a, StageAnnotation::Entry))
        })
        .expect("entry stage must exist");

    let exit_stages: Vec<String> = rw
        .stages
        .iter()
        .filter(|s| {
            s.annotations
                .iter()
                .any(|a| matches!(a, StageAnnotation::Exit))
        })
        .map(|s| s.name.text.clone())
        .collect();

    let mut ir = WorkflowIr::new(filename, &rw.name.text, &entry_stage.name.text, exit_stages);

    ir.inputs = rw
        .inputs
        .iter()
        .map(|inp| Input {
            id: inp.name.text.clone(),
            ty: inp.ty.to_ir_string(),
        })
        .collect();

    let mut cap_set: HashSet<String> = HashSet::new();
    let mut cap_first_seen: Vec<String> = Vec::new();

    for stage in &rw.stages {
        for cap in &stage.requires {
            let c = cap.text.clone();
            if cap_set.insert(c.clone()) {
                cap_first_seen.push(c);
            }
        }
        if let Some(ref exec) = stage.exec {
            let c = exec.capability.text.clone();
            if cap_set.insert(c.clone()) {
                cap_first_seen.push(c);
            }
        }
    }
    for policy in &rw.policies {
        let c = policy.trigger.capability.text.clone();
        if cap_set.insert(c.clone()) {
            cap_first_seen.push(c);
        }
        if let Some(ref requires) = policy.requires {
            for req in requires {
                let c = req.capability.text.clone();
                if cap_set.insert(c.clone()) {
                    cap_first_seen.push(c);
                }
            }
        }
    }
    ir.capabilities = cap_first_seen;

    ir.policies = rw.policies.iter().map(lower_policy).collect();

    for (i, stage) in rw.stages.iter().enumerate() {
        let node = lower_node(rw, stage, i, &transitions[i]);
        ir.nodes.push(node);
    }

    Ok(ir)
}

fn lower_policy(p: &PolicyDecl) -> Policy {
    let source_text = policy_source_text(p);
    let kind = match p.kind {
        PolicyKind::Before => "before".to_string(),
        PolicyKind::Deny => "deny".to_string(),
    };

    let mut bind: IndexMap<String, BindArg> = IndexMap::new();
    for arg in &p.trigger.args {
        bind.insert(
            arg.text.clone(),
            BindArg {
                kind: "arg".to_string(),
                name: arg.text.clone(),
            },
        );
    }

    let trigger = Trigger {
        capability: p.trigger.capability.text.clone(),
        bind,
    };

    let requires = p.requires.as_ref().map(|reqs| {
        reqs.iter()
            .map(|r| {
                let mut args: IndexMap<String, ArgValue> = IndexMap::new();
                for arg in &r.args {
                    args.insert(
                        arg.text.clone(),
                        ArgValue::Ref {
                            r#ref: Ref::Bound {
                                name: arg.text.clone(),
                            },
                        },
                    );
                }
                RequiredCapability {
                    capability: r.capability.text.clone(),
                    args,
                }
            })
            .collect()
    });

    let bound_names: HashSet<String> = p.trigger.args.iter().map(|a| a.text.clone()).collect();
    let condition = p
        .condition
        .as_ref()
        .map(|c| lower_policy_expr(c, &bound_names));

    Policy {
        id: source_text,
        kind,
        trigger,
        requires,
        condition,
    }
}

fn policy_source_text(p: &PolicyDecl) -> String {
    let trigger_args: Vec<String> = p.trigger.args.iter().map(|a| a.text.clone()).collect();
    let trigger_str = format!("{}({})", p.trigger.capability.text, trigger_args.join(", "));

    match p.kind {
        PolicyKind::Before => {
            let req_strs: Vec<String> = p
                .requires
                .as_ref()
                .map(|reqs| {
                    reqs.iter()
                        .map(|r| {
                            if r.args.is_empty() {
                                r.capability.text.clone()
                            } else {
                                let args: Vec<String> =
                                    r.args.iter().map(|a| a.text.clone()).collect();
                                format!("{}({})", r.capability.text, args.join(", "))
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            format!("before {} requires {}", trigger_str, req_strs.join(", "))
        }
        PolicyKind::Deny => {
            let cond = p
                .condition
                .as_ref()
                .map(policy_expr_to_string)
                .unwrap_or_default();
            format!("deny {} if {}", trigger_str, cond)
        }
    }
}

fn policy_expr_to_string(expr: &PolicyExpr) -> String {
    match expr {
        PolicyExpr::Not { expr } => format!("not {}", policy_expr_to_string(expr)),
        PolicyExpr::Or { exprs } => {
            let parts: Vec<String> = exprs.iter().map(policy_expr_to_string).collect();
            format!("({})", parts.join(" or "))
        }
        PolicyExpr::And { exprs } => {
            let parts: Vec<String> = exprs.iter().map(policy_expr_to_string).collect();
            format!("({})", parts.join(" and "))
        }
        PolicyExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let arg_strs: Vec<String> = args.iter().map(policy_expr_value_to_string).collect();
            format!("{}.{}({})", receiver.text, method.text, arg_strs.join(", "))
        }
        PolicyExpr::In { value, options } => {
            let opt_strs: Vec<String> = options.iter().map(policy_expr_value_to_string).collect();
            format!("{} in [{}]", value.text, opt_strs.join(", "))
        }
        PolicyExpr::Ref(id) => id.text.clone(),
        PolicyExpr::Number(n) => format!("{}", n.value),
        PolicyExpr::Compare { op, left, right } => {
            format!(
                "{} {} {}",
                policy_expr_to_string(left),
                op,
                policy_expr_to_string(right)
            )
        }
        PolicyExpr::BinOp { op, left, right } => {
            format!(
                "{} {} {}",
                policy_expr_to_string(left),
                op,
                policy_expr_to_string(right)
            )
        }
        PolicyExpr::NodeRef { stage, field, .. } => {
            format!("{}.{}", stage.text, field.text)
        }
    }
}

fn policy_expr_value_to_string(val: &PolicyExprValue) -> String {
    match val {
        PolicyExprValue::Ref(id) => id.text.clone(),
        PolicyExprValue::String(s) => format!("\"{}\"", s.value),
        PolicyExprValue::Number(n) => format!("{}", n.value),
    }
}

/// Convert an f64 number literal to a serde_yaml::Value::Number.
/// Handles the serde_yaml 0.9 limitation (no from_f64) by round-tripping
/// floating-point values through YAML text parsing.
///
/// Non-finite values (NaN, infinity) are unreachable from the DSL grammar
/// ([`number_literal`] only accepts finite decimal strings).  A
/// `debug_assert!` documents this invariant; a non-finite input indicates
/// a programming error.
pub fn number_literal_value(n: f64) -> serde_yaml::Value {
    debug_assert!(
        n.is_finite(),
        "number_literal_value received non-finite value {n}; \
         the DSL grammar cannot author non-finite number literals — \
         this indicates a programming error"
    );
    if n == (n as i64 as f64)
        && n.fract() == 0.0
        && n >= (i64::MIN as f64)
        && n <= (i64::MAX as f64)
    {
        serde_yaml::Value::Number(serde_yaml::Number::from(n as i64))
    } else {
        let yaml_str = format!("{}", n);
        serde_yaml::from_str(&yaml_str).unwrap_or(serde_yaml::Value::Number(
            serde_yaml::Number::from(n as i64),
        ))
    }
}

fn lower_policy_expr(expr: &PolicyExpr, bound_names: &HashSet<String>) -> Expr {
    match expr {
        PolicyExpr::Not { expr } => Expr::Not {
            expr: Box::new(lower_policy_expr(expr, bound_names)),
        },
        PolicyExpr::Or { exprs } => Expr::Or {
            exprs: exprs
                .iter()
                .map(|e| lower_policy_expr(e, bound_names))
                .collect(),
        },
        PolicyExpr::And { exprs } => Expr::And {
            exprs: exprs
                .iter()
                .map(|e| lower_policy_expr(e, bound_names))
                .collect(),
        },
        PolicyExpr::In { value, options } => {
            let lhs = Expr::Ref {
                r#ref: classify_ref_name(&value.text, bound_names),
            };
            let disjuncts: Vec<Expr> = options
                .iter()
                .map(|opt| Expr::MethodCall {
                    receiver: Box::new(lhs.clone()),
                    method: "eq".to_string(),
                    args: vec![lower_value(opt, bound_names)],
                })
                .collect();
            if disjuncts.len() == 1 {
                disjuncts.into_iter().next().unwrap()
            } else {
                Expr::Or { exprs: disjuncts }
            }
        }
        PolicyExpr::MethodCall {
            receiver,
            method,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: classify_ref_name(&receiver.text, bound_names),
            }),
            method: method.text.clone(),
            args: args.iter().map(|a| lower_value(a, bound_names)).collect(),
        },
        PolicyExpr::Ref(id) => Expr::Ref {
            r#ref: classify_ref_name(&id.text, bound_names),
        },
        PolicyExpr::Number(n) => Expr::Literal {
            ty: "number".to_string(),
            value: number_literal_value(n.value),
        },
        PolicyExpr::Compare { op, left, right } => Expr::Compare {
            op: op_symbol_to_name(op).to_string(),
            left: Box::new(lower_policy_expr(left, bound_names)),
            right: Box::new(lower_policy_expr(right, bound_names)),
        },
        PolicyExpr::BinOp { op, left, right } => Expr::BinOp {
            op: op_symbol_to_name(op).to_string(),
            left: Box::new(lower_policy_expr(left, bound_names)),
            right: Box::new(lower_policy_expr(right, bound_names)),
        },
        PolicyExpr::NodeRef { stage, field, .. } => Expr::Ref {
            r#ref: Ref::NodeOutput {
                node: stage.text.clone(),
                field: field.text.clone(),
            },
        },
    }
}

fn lower_value(val: &PolicyExprValue, bound_names: &HashSet<String>) -> Expr {
    match val {
        PolicyExprValue::Ref(id) => Expr::Ref {
            r#ref: classify_ref_name(&id.text, bound_names),
        },
        PolicyExprValue::String(s) => Expr::Literal {
            ty: "string".to_string(),
            value: serde_yaml::Value::String(s.value.clone()),
        },
        PolicyExprValue::Number(n) => Expr::Literal {
            ty: "number".to_string(),
            value: number_literal_value(n.value),
        },
    }
}

fn classify_ref_name(name: &str, bound_names: &HashSet<String>) -> Ref {
    if bound_names.contains(name) {
        Ref::Bound {
            name: name.to_string(),
        }
    } else {
        Ref::Input {
            name: name.to_string(),
        }
    }
}

fn lower_node(
    rw: &ResolvedWorkflow,
    stage: &crate::resolve::ResolvedStage,
    _stage_idx: usize,
    transitions: &[Transition],
) -> Node {
    let is_entry = stage
        .annotations
        .iter()
        .any(|a| matches!(a, StageAnnotation::Entry));
    let is_exit = stage
        .annotations
        .iter()
        .any(|a| matches!(a, StageAnnotation::Exit));

    let mut annotations: Vec<String> = Vec::new();
    if is_entry {
        annotations.push("entry".to_string());
    }
    if is_exit {
        annotations.push("exit".to_string());
    }

    let mut reads: Vec<Read> = Vec::new();

    // Entry stage implicitly reads all workflow inputs
    if is_entry {
        for inp in &rw.inputs {
            reads.push(Read {
                ref_: Ref::Input {
                    name: inp.name.text.clone(),
                },
                optional: false,
                origin: "implicit_entry_input".to_string(),
            });
        }
    }

    // DSL stage inputs
    for input_ref in &stage.inputs {
        reads.push(Read {
            ref_: Ref::NodeOutput {
                node: input_ref.stage.text.clone(),
                field: input_ref.field.text.clone(),
            },
            optional: input_ref.optional,
            origin: "dsl_stage_input".to_string(),
        });
    }

    // Auto-add reads from exec arg refs (Stage.field)
    let mut stage_field_reads: HashSet<String> = HashSet::new();
    for r in &reads {
        if let Ref::NodeOutput { node, field } = &r.ref_ {
            stage_field_reads.insert(format!("{}:{}", node, field));
        }
    }
    if let Some(ref exec) = stage.exec {
        for arg in &exec.args {
            if let ExecValue::Ref(r) = &arg.value {
                let key = format!("{}:{}", r.stage.text, r.field.text);
                if !stage_field_reads.contains(&key) {
                    stage_field_reads.insert(key);
                    reads.push(Read {
                        ref_: Ref::NodeOutput {
                            node: r.stage.text.clone(),
                            field: r.field.text.clone(),
                        },
                        optional: false,
                        origin: "exec_arg".to_string(),
                    });
                }
            }
        }
    }

    let writes: Vec<Write> = stage
        .outputs
        .iter()
        .map(|f| Write {
            name: f.name.text.clone(),
            ty: f.ty.to_ir_string(),
            optional: f.ty.optional,
        })
        .collect();

    let mut requires: Vec<StageCapability> = stage
        .requires
        .iter()
        .map(|c| StageCapability {
            capability: c.text.clone(),
        })
        .collect();

    // Auto-add exec capability to requires
    if let Some(ref exec) = stage.exec {
        let exec_cap = exec.capability.text.clone();
        if !requires.iter().any(|c| c.capability == exec_cap) {
            requires.push(StageCapability {
                capability: exec_cap,
            });
        }
    }

    // Build execution
    let execution = if let Some(ref exec) = stage.exec {
        let mut args: IndexMap<String, Expr> = IndexMap::new();
        for arg in &exec.args {
            let value = match &arg.value {
                ExecValue::Ref(r) => Expr::Ref {
                    r#ref: Ref::NodeOutput {
                        node: r.stage.text.clone(),
                        field: r.field.text.clone(),
                    },
                },
                ExecValue::InputRef(id) => Expr::Ref {
                    r#ref: Ref::Input {
                        name: id.text.clone(),
                    },
                },
                ExecValue::String(s) => Expr::Literal {
                    ty: "string".to_string(),
                    value: serde_yaml::Value::String(s.value.clone()),
                },
            };
            args.insert(arg.name.text.clone(), value);
        }
        StageExecution::Tool {
            capability: exec.capability.text.clone(),
            args,
        }
    } else {
        StageExecution::Model
    };

    Node {
        id: stage.name.text.clone(),
        annotations,
        prompt: stage.prompt.value.clone(),
        reads,
        writes,
        requires,
        transitions: transitions.to_vec(),
        execution,
    }
}
