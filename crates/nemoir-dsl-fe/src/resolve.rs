use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, NameError, ShapeError, TransitionError, TypeError};

#[derive(Debug, Clone)]
pub struct ResolvedStage {
    pub index: usize,
    pub name: Ident,
    pub annotations: Vec<StageAnnotation>,
    pub prompt: Spanned<String>,
    pub inputs: Vec<StageInputRef>,
    pub outputs: Vec<OutputField>,
    pub requires: Vec<Ident>,
    pub exec: Option<ExecDecl>,
    pub transitions: Vec<ExplicitTransition>,
}

#[derive(Debug, Clone)]
pub struct ResolvedWorkflow {
    pub name: Ident,
    pub inputs: Vec<InputDecl>,
    pub policies: Vec<PolicyDecl>,
    pub stages: Vec<ResolvedStage>,
    pub stage_map: HashMap<String, usize>,
}

pub fn resolve(ast: WorkflowAst, filename: &str) -> Result<ResolvedWorkflow, Diagnostic> {
    let mut stage_map = HashMap::new();
    let mut resolved_stages = Vec::new();

    for (i, stage) in ast.stages.iter().enumerate() {
        if stage_map.contains_key(&stage.name.text) {
            return Err(Diagnostic::NameError(NameError {
                message: format!("duplicate stage name `{}`", stage.name.text),
                filename: filename.to_string(),
                label: Some((
                    stage.name.span.start,
                    stage.name.span.end,
                    format!("stage `{}` already defined", stage.name.text),
                )),
                help: None,
            }));
        }
        stage_map.insert(stage.name.text.clone(), i);
    }

    // Check duplicate workflow inputs
    let mut input_names = HashMap::new();
    for input in &ast.inputs {
        if input_names.contains_key(&input.name.text) {
            return Err(Diagnostic::NameError(NameError {
                message: format!("duplicate input name `{}`", input.name.text),
                filename: filename.to_string(),
                label: Some((
                    input.name.span.start,
                    input.name.span.end,
                    format!("input `{}` already defined", input.name.text),
                )),
                help: None,
            }));
        }
        input_names.insert(input.name.text.clone(), ());

        if input.ty.optional {
            return Err(Diagnostic::TypeError(TypeError {
                message: format!("workflow input `{}` cannot be optional", input.name.text),
                filename: filename.to_string(),
                label: Some((
                    input.ty.span.start,
                    input.ty.span.end,
                    format!(
                        "`{}` marked with `?`; workflow-level inputs must be required",
                        input.ty.to_ir_string()
                    ),
                )),
            }));
        }

        if matches!(input.ty.base, BaseType::Unknown) {
            return Err(Diagnostic::TypeError(TypeError {
                message: format!(
                    "unknown type `{}` for input `{}`",
                    input.ty.raw_name, input.name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    input.ty.span.start,
                    input.ty.span.end,
                    format!("unknown type `{}`", input.ty.raw_name),
                )),
            }));
        }
    }

    for stage in &ast.stages {
        let mut seen_keys = HashMap::new();

        for item in &stage.items {
            let key = match item {
                StageBodyItem::Prompt(_) => "prompt",
                StageBodyItem::Input(_) => "input",
                StageBodyItem::Output(_) => "output",
                StageBodyItem::Requires(_) => "requires",
                StageBodyItem::Exec(_) => "exec",
                StageBodyItem::Transition(_) => "transition",
            };
            if seen_keys.contains_key(key) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!("duplicate `{}:` in stage `{}`", key, stage.name.text),
                    filename: filename.to_string(),
                    label: Some((
                        stage.name.span.start,
                        stage.name.span.end,
                        format!("duplicate {} key", key),
                    )),
                    help: None,
                }));
            }
            seen_keys.insert(key, ());
        }

        let mut prompt: Option<Spanned<String>> = None;
        let mut inputs: Vec<StageInputRef> = Vec::new();
        let mut outputs: Vec<OutputField> = Vec::new();
        let mut requires: Vec<Ident> = Vec::new();
        let mut exec: Option<ExecDecl> = None;
        let mut explicit_transitions: Vec<ExplicitTransition> = Vec::new();

        for item in &stage.items {
            match item {
                StageBodyItem::Prompt(p) => {
                    prompt = Some(p.clone());
                }
                StageBodyItem::Exec(e) => {
                    // Resolve exec arg refs: Stage.field must exist
                    for arg in &e.args {
                        if let ExecValue::Ref(ref r) = &arg.value {
                            if !stage_map.contains_key(&r.stage.text) {
                                return Err(Diagnostic::NameError(NameError {
                                    message: format!(
                                        "unknown stage `{}` in exec arg",
                                        r.stage.text
                                    ),
                                    filename: filename.to_string(),
                                    label: Some((
                                        r.stage.span.start,
                                        r.stage.span.end,
                                        format!("stage `{}` not found", r.stage.text),
                                    )),
                                    help: None,
                                }));
                            }
                            let ref_stage_idx = stage_map[&r.stage.text];
                            let ref_stage = &ast.stages[ref_stage_idx];
                            let field_exists = ref_stage.items.iter().any(|item| {
                                if let StageBodyItem::Output(fields) = item {
                                    fields.iter().any(|f| f.name.text == r.field.text)
                                } else {
                                    false
                                }
                            });
                            if !field_exists {
                                return Err(Diagnostic::NameError(NameError {
                                    message: format!(
                                        "unknown output field `{}` on stage `{}` in exec arg",
                                        r.field.text, r.stage.text
                                    ),
                                    filename: filename.to_string(),
                                    label: Some((
                                        r.field.span.start,
                                        r.field.span.end,
                                        format!(
                                            "`{}` has no output named `{}`",
                                            r.stage.text, r.field.text
                                        ),
                                    )),
                                    help: None,
                                }));
                            }
                        }
                    }
                    exec = Some(e.clone());
                }
                StageBodyItem::Input(refs) => {
                    // Resolve each input ref: stage must exist, field must exist on that stage
                    for input_ref in refs {
                        if !stage_map.contains_key(&input_ref.stage.text) {
                            return Err(Diagnostic::NameError(NameError {
                                message: format!(
                                    "unknown stage `{}` in input reference",
                                    input_ref.stage.text
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    input_ref.stage.span.start,
                                    input_ref.stage.span.end,
                                    format!("stage `{}` not found", input_ref.stage.text),
                                )),
                                help: None,
                            }));
                        }
                        let ref_stage_idx = stage_map[&input_ref.stage.text];
                        let ref_stage = &ast.stages[ref_stage_idx];

                        let field_exists = ref_stage.items.iter().any(|item| {
                            if let StageBodyItem::Output(fields) = item {
                                fields.iter().any(|f| f.name.text == input_ref.field.text)
                            } else {
                                false
                            }
                        });

                        if !field_exists {
                            // Try to find a similar field name for help message
                            let similar = ref_stage
                                .items
                                .iter()
                                .filter_map(|item| {
                                    if let StageBodyItem::Output(fields) = item {
                                        Some(
                                            fields
                                                .iter()
                                                .filter_map(|f| {
                                                    if strsim::levenshtein(
                                                        &f.name.text,
                                                        &input_ref.field.text,
                                                    ) <= 2
                                                    {
                                                        Some(f.name.text.clone())
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect::<Vec<_>>(),
                                        )
                                    } else {
                                        None
                                    }
                                })
                                .flatten()
                                .next();

                            let help = similar.map(|s| format!("did you mean `{}`?", s));

                            return Err(Diagnostic::NameError(NameError {
                                message: format!(
                                    "unknown output field `{}` on stage `{}`",
                                    input_ref.field.text, input_ref.stage.text
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    input_ref.field.span.start,
                                    input_ref.field.span.end,
                                    format!(
                                        "`{}` has no output named `{}`",
                                        input_ref.stage.text, input_ref.field.text
                                    ),
                                )),
                                help,
                            }));
                        }
                        inputs.push(input_ref.clone());
                    }
                }
                StageBodyItem::Output(fields) => {
                    let mut field_names = HashMap::new();
                    for field in fields {
                        if field_names.contains_key(&field.name.text) {
                            return Err(Diagnostic::NameError(NameError {
                                message: format!(
                                    "duplicate output field `{}` in stage `{}`",
                                    field.name.text, stage.name.text
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    field.name.span.start,
                                    field.name.span.end,
                                    format!("field `{}` already defined", field.name.text),
                                )),
                                help: None,
                            }));
                        }
                        field_names.insert(field.name.text.clone(), ());

                        if matches!(field.ty.base, BaseType::Unknown) {
                            return Err(Diagnostic::TypeError(TypeError {
                                message: format!(
                                    "unknown type `{}` for field `{}`",
                                    field.ty.raw_name, field.name.text
                                ),
                                filename: filename.to_string(),
                                label: Some((
                                    field.ty.span.start,
                                    field.ty.span.end,
                                    format!("unknown type `{}`", field.ty.raw_name),
                                )),
                            }));
                        }

                        // Validate bool branches
                        if let Some(branches) = &field.branches {
                            if !matches!(field.ty.base, BaseType::Bool) || field.ty.is_array {
                                return Err(Diagnostic::TypeError(TypeError {
                                    message: format!(
                                        "bool branch block on non-bool field `{}`",
                                        field.name.text
                                    ),
                                    filename: filename.to_string(),
                                    label: Some((
                                        field.ty.span.start,
                                        field.ty.span.end,
                                        format!(
                                            "type is `{}`, expected `bool`",
                                            field.ty.to_ir_string()
                                        ),
                                    )),
                                }));
                            }
                            if field.ty.optional {
                                return Err(Diagnostic::TypeError(TypeError {
                                    message: format!(
                                        "bool branch block on optional bool field `{}`",
                                        field.name.text
                                    ),
                                    filename: filename.to_string(),
                                    label: Some((
                                        field.ty.span.start,
                                        field.ty.span.end,
                                        format!(
                                            "`{}` is `bool?`; branching on optional output is not supported",
                                            field.name.text
                                        ),
                                    )),
                                }));
                            }
                            // Validate branch targets exist
                            for target_name in
                                [&branches.true_target.text, &branches.false_target.text]
                            {
                                if !stage_map.contains_key(target_name) {
                                    return Err(Diagnostic::NameError(NameError {
                                        message: format!(
                                            "bool branch target `{}` does not exist",
                                            target_name
                                        ),
                                        filename: filename.to_string(),
                                        label: Some((
                                            branches.true_target.span.start,
                                            branches.true_target.span.end,
                                            format!("unknown stage `{}`", target_name),
                                        )),
                                        help: None,
                                    }));
                                }
                            }
                        }

                        outputs.push(field.clone());
                    }

                    let branch_count = outputs.iter().filter(|f| f.branches.is_some()).count();
                    if branch_count > 1 {
                        return Err(Diagnostic::TransitionError(TransitionError {
                            message: format!(
                                "stage `{}` has {} bool branch output fields; at most one is supported",
                                stage.name.text, branch_count
                            ),
                            filename: filename.to_string(),
                            label: Some((
                                stage.name.span.start,
                                stage.name.span.end,
                                "too many bool branch fields".into(),
                            )),
                            help: Some(
                                "only one output field per stage can have bool branches in this prototype"
                                    .into(),
                            ),
                        }));
                    }
                }
                StageBodyItem::Requires(caps) => {
                    requires = caps.clone();
                }
                StageBodyItem::Transition(trans) => {
                    explicit_transitions.extend(trans.clone());
                }
            }
        }

        let prompt = match prompt {
            Some(p) => p,
            None => {
                if exec.is_some() {
                    // Deterministic stages may omit prompt.
                    Spanned::new(String::new(), stage.name.span.clone())
                } else {
                    return Err(Diagnostic::ShapeError(ShapeError {
                        message: format!(
                            "stage `{}` is missing required `prompt:`",
                            stage.name.text
                        ),
                        filename: filename.to_string(),
                        label: Some((
                            stage.name.span.start,
                            stage.name.span.end,
                            format!("stage `{}` has no prompt", stage.name.text),
                        )),
                        help: Some(
                            "every stage must have a `prompt:` field with a string value".into(),
                        ),
                    }));
                }
            }
        };

        // Either/or check (§3.2/§3.5): bool-branches and explicit transitions cannot coexist.
        let has_bool_branches = outputs.iter().any(|f| f.branches.is_some());
        let has_explicit_transitions = !explicit_transitions.is_empty();
        if has_bool_branches && has_explicit_transitions {
            return Err(Diagnostic::TransitionError(TransitionError {
                message: format!(
                    "stage `{}` has both `bool_branches` and `transition` statements; choose one or the other",
                    stage.name.text
                ),
                filename: filename.to_string(),
                label: Some((
                    stage.name.span.start,
                    stage.name.span.end,
                    "cannot mix bool_branches and explicit transitions".into(),
                )),
                help: None,
            }));
        }

        // Desugar bool-branches (§3.5) into explicit transitions.
        for output in &mut outputs {
            if let Some(branches) = output.branches.take() {
                // output.branches is now None (consumed via .take())
                let not_cond = PolicyExpr::Not {
                    expr: Box::new(PolicyExpr::Ref(output.name.clone())),
                };
                explicit_transitions.push(ExplicitTransition {
                    cond: Some(PolicyExpr::Ref(output.name.clone())),
                    target: branches.true_target,
                    span: stage.name.span.clone(),
                });
                explicit_transitions.push(ExplicitTransition {
                    cond: Some(not_cond),
                    target: branches.false_target,
                    span: stage.name.span.clone(),
                });
            }
        }

        // Resolve explicit transition targets.
        for t in &explicit_transitions {
            if !stage_map.contains_key(&t.target.text) {
                return Err(Diagnostic::NameError(NameError {
                    message: format!("transition target `{}` does not exist", t.target.text),
                    filename: filename.to_string(),
                    label: Some((
                        t.target.span.start,
                        t.target.span.end,
                        format!("unknown stage `{}`", t.target.text),
                    )),
                    help: None,
                }));
            }
        }

        resolved_stages.push(ResolvedStage {
            index: resolved_stages.len(),
            name: stage.name.clone(),
            annotations: stage.annotations.clone(),
            prompt,
            inputs,
            outputs,
            requires,
            exec,
            transitions: explicit_transitions,
        });
    }

    let has_entry = resolved_stages.iter().any(|s| {
        s.annotations
            .iter()
            .any(|a| matches!(a, StageAnnotation::Entry))
    });
    if !has_entry && !resolved_stages.is_empty() {
        resolved_stages[0].annotations.push(StageAnnotation::Entry);
    }

    let has_exit = resolved_stages.iter().any(|s| {
        s.annotations
            .iter()
            .any(|a| matches!(a, StageAnnotation::Exit))
    });
    if !has_exit && !resolved_stages.is_empty() {
        let last = resolved_stages.len() - 1;
        resolved_stages[last]
            .annotations
            .push(StageAnnotation::Exit);
    }

    Ok(ResolvedWorkflow {
        name: ast.name,
        inputs: ast.inputs,
        policies: ast.policies,
        stages: resolved_stages,
        stage_map,
    })
}
