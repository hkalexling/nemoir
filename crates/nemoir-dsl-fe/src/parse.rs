use pest::Parser;
use pest_derive::Parser;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, ParseError};

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct NemoParser;

pub fn parse_source(source: &str, filename: &str) -> Result<WorkflowAst, Diagnostic> {
    let pairs = match NemoParser::parse(Rule::workflow, source) {
        Ok(pairs) => pairs,
        Err(err) => {
            let (start, end) = match err.location {
                pest::error::InputLocation::Pos(p) => (p, p + 1),
                pest::error::InputLocation::Span((s, e)) => (s, e),
            };
            // §5.2: produce a clear diagnostic for == / != (numeric equality not supported)
            let check_start = start.saturating_sub(1);
            let check_end = (start + 3).min(source.len());
            if check_start < check_end
                && (source[check_start..check_end].contains("==")
                    || source[check_start..check_end].contains("!="))
            {
                return Err(Diagnostic::ParseError(ParseError {
                    message: "numeric equality (`==`/`!=`) is not supported; use ordering predicates (>, >=, <, <=) or `score - x > eps` for near-equality".into(),
                    filename: filename.to_string(),
                    label: Some((start, end, "`==` or `!=` not allowed in expressions".into())),
                    help: Some(
                        "see docs/dsl-and-ir.md §6 for the ordering-only numeric rule".into(),
                    ),
                }));
            }
            let msg = match err.variant {
                pest::error::ErrorVariant::ParsingError {
                    positives,
                    negatives: _,
                } => {
                    let expected: Vec<&str> = positives
                        .iter()
                        .map(|r| match r {
                            Rule::string => "a quoted string",
                            Rule::ident => "an identifier",
                            Rule::dotted_ident => "a capability name",
                            Rule::type_base => "a type (string, bool, path)",
                            Rule::input_block => "input { ... }",
                            Rule::input_field => "an input field (name: type)",
                            Rule::policy_block => "policy { ... }",
                            Rule::policy_expr => "a policy condition",
                            Rule::policy_value => "an identifier or string literal",
                            Rule::stage => "stage declaration",
                            Rule::stage_body_item => {
                                "prompt:, input:, output:, requires:, or exec:"
                            }
                            Rule::prompt_decl => "prompt:",
                            Rule::stage_input => "input:",
                            Rule::input_ref => "stage field reference like Stage.field",
                            Rule::output_block => "output: { ... }",
                            Rule::output_field => "an output field (name: type)",
                            Rule::requires_block => "requires:",
                            Rule::exec_decl => "exec: capability(args)",
                            Rule::exec_arg => "exec arg name: value",
                            Rule::exec_value => "a string literal or Stage.field reference",
                            Rule::bool_branches => "{ true => X false => Y }",
                            Rule::before_policy => "before policy",
                            Rule::deny_policy => "deny policy",
                            Rule::cap_call => "a capability call",
                            Rule::require_item => "a required capability",
                            Rule::annotation => "@entry or @exit annotation",
                            Rule::workflow => "workflow",
                            _ => "unknown token",
                        })
                        .collect();
                    if expected.is_empty() {
                        "unexpected token".to_string()
                    } else {
                        format!("expected {}", expected.join(" or "))
                    }
                }
                pest::error::ErrorVariant::CustomError { message } => message,
            };
            let label_msg = format!("here {msg}");
            return Err(Diagnostic::ParseError(ParseError {
                message: format!("parse error: {}", msg),
                filename: filename.to_string(),
                label: Some((start, end, label_msg)),
                help: None,
            }));
        }
    };

    let workflow_pair = pairs.into_iter().next().unwrap();
    parse_workflow(workflow_pair)
}

fn parse_workflow(pair: pest::iterators::Pair<Rule>) -> Result<WorkflowAst, Diagnostic> {
    let mut inner = pair.into_inner();

    let name = parse_ident(inner.next().expect("workflow name"));

    let mut inputs = Vec::new();
    let mut policies = Vec::new();
    let mut stages = Vec::new();

    for item in inner {
        match item.as_rule() {
            Rule::input_block => {
                inputs = parse_input_block(item)?;
            }
            Rule::policy_block => {
                policies = parse_policy_block(item)?;
            }
            Rule::stage => {
                stages.push(parse_stage(item)?);
            }
            _ => {}
        }
    }

    Ok(WorkflowAst {
        name,
        inputs,
        policies,
        stages,
    })
}

fn parse_ident(pair: pest::iterators::Pair<Rule>) -> Ident {
    let span = pair.as_span();
    Ident {
        text: pair.as_str().to_string(),
        span: Span::new(span.start(), span.end()),
    }
}

fn span_from_pair(pair: &pest::iterators::Pair<Rule>) -> Span {
    let s = pair.as_span();
    Span::new(s.start(), s.end())
}

fn parse_type_ref(pair: pest::iterators::Pair<Rule>) -> TypeRef {
    let span = span_from_pair(&pair);
    let mut inner = pair.into_inner();

    let base_pair = inner.next().expect("type base");
    let raw_name = base_pair.as_str().to_string();
    let base = BaseType::from_name(&raw_name);

    let mut is_array = false;
    let mut optional = false;

    for p in inner {
        match p.as_rule() {
            Rule::array_marker => is_array = true,
            Rule::optional_marker => optional = true,
            _ => {}
        }
    }

    TypeRef {
        base,
        is_array,
        optional,
        span,
        raw_name,
    }
}

fn parse_input_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<InputDecl>, Diagnostic> {
    let mut inputs = Vec::new();
    for field in pair.into_inner() {
        if field.as_rule() == Rule::input_field {
            let mut inner = field.into_inner();
            let name = parse_ident(inner.next().expect("input field name"));
            let ty = parse_type_ref(inner.next().expect("input field type"));
            inputs.push(InputDecl { name, ty });
        }
    }
    Ok(inputs)
}

fn parse_policy_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<PolicyDecl>, Diagnostic> {
    let mut policies = Vec::new();
    for item in pair.into_inner() {
        match item.as_rule() {
            Rule::before_policy => {
                policies.push(parse_before_policy(item));
            }
            Rule::deny_policy => {
                policies.push(parse_deny_policy(item));
            }
            _ => {}
        }
    }
    Ok(policies)
}

fn parse_cap_call(pair: pest::iterators::Pair<Rule>) -> CapCall {
    let mut inner = pair.into_inner();
    let capability = parse_ident(inner.next().expect("capability name"));
    let mut args = Vec::new();
    for arg in inner {
        args.push(parse_ident(arg));
    }
    CapCall { capability, args }
}

fn parse_require_item(pair: pest::iterators::Pair<Rule>) -> RequireItem {
    let mut inner = pair.into_inner();
    let capability = parse_ident(inner.next().expect("require capability"));
    let mut args = Vec::new();
    for arg in inner {
        args.push(parse_ident(arg));
    }
    RequireItem { capability, args }
}

fn parse_before_policy(pair: pest::iterators::Pair<Rule>) -> PolicyDecl {
    let mut inner = pair.into_inner();
    let trigger = parse_cap_call(inner.next().expect("trigger cap_call"));

    let mut requires = Vec::new();
    for item in inner {
        if item.as_rule() == Rule::require_list {
            for req in item.into_inner() {
                requires.push(parse_require_item(req));
            }
        }
    }

    PolicyDecl {
        kind: PolicyKind::Before,
        trigger,
        requires: Some(requires),
        condition: None,
    }
}

fn parse_policy_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    // policy_expr -> or_expr
    let inner = pair.into_inner().next().expect("or_expr");
    parse_or_expr(inner)
}

fn parse_or_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    if items.is_empty() {
        return PolicyExpr::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        });
    }
    // First is always and_expr, then pairs of (or_kw, and_expr)
    let mut exprs = vec![parse_and_expr(items[0].clone())];
    let mut idx = 1;
    while idx < items.len() {
        // skip or_kw, parse next and_expr
        idx += 1;
        exprs.push(parse_and_expr(items[idx].clone()));
        idx += 1;
    }
    if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
    } else {
        PolicyExpr::Or { exprs }
    }
}

fn parse_and_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    if items.is_empty() {
        return PolicyExpr::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        });
    }
    // First is always not_expr, then pairs of (and_kw, not_expr)
    let mut exprs = vec![parse_not_expr(items[0].clone())];
    let mut idx = 1;
    while idx < items.len() {
        // skip and_kw, parse next not_expr
        idx += 1;
        exprs.push(parse_not_expr(items[idx].clone()));
        idx += 1;
    }
    if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
    } else {
        PolicyExpr::And { exprs }
    }
}

fn parse_not_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    let mut not_count = 0usize;
    let mut idx = 0;
    while idx < items.len() && items[idx].as_rule() == Rule::not_kw {
        not_count += 1;
        idx += 1;
    }
    let compare = parse_compare_expr(items[idx].clone());
    (0..not_count).fold(compare, |acc, _| PolicyExpr::Not {
        expr: Box::new(acc),
    })
}

fn parse_compare_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    if items.is_empty() {
        return PolicyExpr::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        });
    }
    let left = parse_add_expr(items[0].clone());
    // Optional compare op + right operand
    if items.len() >= 3 {
        // items: [add_expr, compare_op, add_expr]
        let op_text = items[1].as_str().to_string();
        let right = parse_add_expr(items[2].clone());
        PolicyExpr::Compare {
            op: op_text,
            left: Box::new(left),
            right: Box::new(right),
        }
    } else {
        left
    }
}

fn parse_add_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    if items.is_empty() {
        return PolicyExpr::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        });
    }
    // First is always mul_expr, then pairs of (add_op, mul_expr)
    let mut expr = parse_mul_expr(items[0].clone());
    let mut idx = 1;
    while idx + 1 < items.len() {
        let op_text = items[idx].as_str().to_string();
        idx += 1;
        let right = parse_mul_expr(items[idx].clone());
        idx += 1;
        expr = PolicyExpr::BinOp {
            op: op_text,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    expr
}

fn parse_mul_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    if items.is_empty() {
        return PolicyExpr::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        });
    }
    // First is always unary_expr, then pairs of (mul_op, unary_expr)
    let mut expr = parse_unary_expr(items[0].clone());
    let mut idx = 1;
    while idx < items.len() {
        let op_text = items[idx].as_str().to_string();
        idx += 1;
        let right = parse_unary_expr(items[idx].clone());
        idx += 1;
        expr = PolicyExpr::BinOp {
            op: op_text,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    expr
}

fn parse_unary_expr(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let items: Vec<_> = pair.into_inner().collect();
    let mut idx = 0usize;
    // Count leading unary minus signs (now a named rule — pest captures them as pairs)
    let mut minus_count = 0usize;
    while idx < items.len() && items[idx].as_rule() == Rule::unary_minus {
        minus_count += 1;
        idx += 1;
    }
    let primary = parse_primary(items[idx].clone());
    if minus_count.is_multiple_of(2) {
        // Even number of minuses cancel out
        primary
    } else {
        // Odd => negate: 0 - expr
        let zero = PolicyExpr::Number(Spanned::new(0.0, Span::new(0, 0)));
        PolicyExpr::BinOp {
            op: "-".to_string(),
            left: Box::new(zero),
            right: Box::new(primary),
        }
    }
}

fn parse_primary(pair: pest::iterators::Pair<Rule>) -> PolicyExpr {
    let inner = pair.into_inner().next().expect("primary inner");
    match inner.as_rule() {
        Rule::policy_ref => PolicyExpr::Ref(parse_ident(inner.into_inner().next().expect("ident"))),
        Rule::node_ref => {
            let mut ci = inner.into_inner();
            let stage = parse_ident(ci.next().expect("stage"));
            let field = parse_ident(ci.next().expect("field"));
            let span = Span::new(stage.span.start, field.span.end);
            PolicyExpr::NodeRef { stage, field, span }
        }
        Rule::number_literal => {
            let text = inner.as_str();
            let value: f64 = text.parse().unwrap_or(0.0);
            let span = span_from_pair(&inner);
            PolicyExpr::Number(Spanned::new(value, span))
        }
        Rule::call_or_in => {
            let mut ci = inner.into_inner();
            let ident_pair = ci.next().expect("ident");
            let ident = parse_ident(ident_pair);
            let next = ci.next().expect("dot/in");
            match next.as_rule() {
                Rule::dot => {
                    let method = parse_ident(ci.next().expect("method"));
                    let args = if let Some(arg_list) = ci.next() {
                        parse_policy_arg_list(arg_list)
                    } else {
                        Vec::new()
                    };
                    PolicyExpr::MethodCall {
                        receiver: ident,
                        method,
                        args,
                    }
                }
                Rule::policy_array => {
                    // "in" branch: next IS the policy_array (the "in" literal is silent)
                    let options = parse_policy_array(next);
                    PolicyExpr::In {
                        value: ident,
                        options,
                    }
                }
                _ => PolicyExpr::Ref(ident),
            }
        }
        _ => {
            // Parenthesized: "(" ~ or_expr ~ ")"
            parse_or_expr(inner)
        }
    }
}

fn parse_policy_arg_list(pair: pest::iterators::Pair<Rule>) -> Vec<PolicyExprValue> {
    pair.into_inner().map(|p| parse_policy_value(p)).collect()
}

fn parse_policy_array(pair: pest::iterators::Pair<Rule>) -> Vec<PolicyExprValue> {
    pair.into_inner().map(|p| parse_policy_value(p)).collect()
}

fn parse_policy_value(pair: pest::iterators::Pair<Rule>) -> PolicyExprValue {
    let inner = pair.into_inner().next();
    match inner {
        Some(p) if p.as_rule() == Rule::ident => PolicyExprValue::Ref(parse_ident(p)),
        Some(p) if p.as_rule() == Rule::number_literal => {
            let text = p.as_str();
            let value: f64 = text.parse().unwrap_or(0.0);
            let span = span_from_pair(&p);
            PolicyExprValue::Number(Spanned::new(value, span))
        }
        Some(p) if p.as_rule() == Rule::single_line_string => {
            let raw = p.as_str();
            let processed = process_policy_string_literal(raw);
            let span = span_from_pair(&p);
            PolicyExprValue::String(Spanned::new(processed, span))
        }
        _ => PolicyExprValue::Ref(Ident {
            text: String::new(),
            span: Span::new(0, 0),
        }),
    }
}

fn parse_deny_policy(pair: pest::iterators::Pair<Rule>) -> PolicyDecl {
    let mut inner = pair.into_inner();
    let trigger = parse_cap_call(inner.next().expect("trigger cap_call"));
    let condition = parse_policy_expr(inner.next().expect("condition"));

    PolicyDecl {
        kind: PolicyKind::Deny,
        trigger,
        requires: None,
        condition: Some(condition),
    }
}

fn parse_stage(pair: pest::iterators::Pair<Rule>) -> Result<StageDecl, Diagnostic> {
    let mut inner = pair.into_inner();

    let mut annotations = Vec::new();

    let first = inner.next().expect("stage name or annotation");

    let name = if first.as_rule() == Rule::annotation {
        match first.as_str() {
            "@entry" => annotations.push(StageAnnotation::Entry),
            "@exit" => annotations.push(StageAnnotation::Exit),
            _other => {
                let s = span_from_pair(&first);
                return Err(Diagnostic::ParseError(ParseError {
                    message: format!("unknown stage annotation: {}", _other),
                    filename: String::new(),
                    label: Some((s.start, s.end, format!("unknown annotation `{}`", _other))),
                    help: None,
                }));
            }
        }
        parse_ident(inner.next().expect("stage name after annotation"))
    } else {
        parse_ident(first)
    };

    let mut items = Vec::new();

    for item in inner {
        if item.as_rule() != Rule::stage_body_item {
            continue;
        }
        let body_item = item.into_inner().next().unwrap();
        match body_item.as_rule() {
            Rule::prompt_decl => {
                let mut p_inner = body_item.into_inner();
                let s = p_inner.next().expect("prompt string");
                let raw = s.as_str();
                let processed = process_string(raw);
                let span = span_from_pair(&s);
                items.push(StageBodyItem::Prompt(Spanned::new(processed, span)));
            }
            Rule::stage_input => {
                let inputs = parse_stage_input(body_item);
                items.push(StageBodyItem::Input(inputs));
            }
            Rule::output_block => {
                let outputs = parse_output_block(body_item)?;
                items.push(StageBodyItem::Output(outputs));
            }
            Rule::requires_block => {
                let requires = parse_requires_block(body_item);
                items.push(StageBodyItem::Requires(requires));
            }
            Rule::exec_decl => {
                items.push(parse_exec_decl(body_item)?);
            }
            Rule::transition_block => {
                items.push(parse_transition_block(body_item)?);
            }
            _ => {}
        }
    }

    Ok(StageDecl {
        name,
        annotations,
        items,
    })
}

fn parse_stage_input(pair: pest::iterators::Pair<Rule>) -> Vec<StageInputRef> {
    let mut refs = Vec::new();
    for item in pair.into_inner() {
        if item.as_rule() == Rule::input_ref {
            let span = span_from_pair(&item);
            let mut inner = item.into_inner();
            let stage = parse_ident(inner.next().expect("stage in input ref"));
            let field = parse_ident(inner.next().expect("field in input ref"));
            let optional = inner
                .next()
                .is_some_and(|p| p.as_rule() == Rule::optional_marker);
            refs.push(StageInputRef {
                stage,
                field,
                optional,
                span,
            });
        }
    }
    refs
}

fn parse_output_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<OutputField>, Diagnostic> {
    let mut fields = Vec::new();
    for item in pair.into_inner() {
        if item.as_rule() == Rule::output_field {
            let mut inner = item.into_inner();
            let name = parse_ident(inner.next().expect("output field name"));
            let ty = parse_type_ref(inner.next().expect("output field type"));

            let branches = inner
                .next()
                .map(|branch_pair| parse_bool_branches(branch_pair));

            fields.push(OutputField { name, ty, branches });
        }
    }
    Ok(fields)
}

fn parse_bool_branches(pair: pest::iterators::Pair<Rule>) -> BoolBranches {
    let inner = pair.into_inner();

    let mut true_target = None;
    let mut false_target = None;

    for item in inner {
        match item.as_str() {
            "true" | "false" | "=>" => {}
            _other => {
                if true_target.is_none() {
                    true_target = Some(parse_ident(item));
                } else {
                    false_target = Some(parse_ident(item));
                }
            }
        }
    }

    BoolBranches {
        true_target: true_target.expect("true target"),
        false_target: false_target.expect("false target"),
    }
}

fn parse_requires_block(pair: pest::iterators::Pair<Rule>) -> Vec<Ident> {
    pair.into_inner().map(parse_ident).collect()
}

fn parse_transition_block(pair: pest::iterators::Pair<Rule>) -> Result<StageBodyItem, Diagnostic> {
    let mut transitions = Vec::new();
    for decl in pair.into_inner() {
        if decl.as_rule() == Rule::transition_decl {
            transitions.push(parse_transition_decl(decl));
        }
    }
    Ok(StageBodyItem::Transition(transitions))
}

fn parse_transition_decl(pair: pest::iterators::Pair<Rule>) -> ExplicitTransition {
    let span = span_from_pair(&pair);
    let mut inner = pair.into_inner();
    let first = inner.next().expect("transition cond or else");
    match first.as_rule() {
        Rule::transition_cond => {
            // "if" ~ or_expr
            let mut cond_inner = first.into_inner();
            let expr_pair = cond_inner.next().expect("or_expr in transition_cond");
            let cond = parse_or_expr(expr_pair);
            let target = parse_ident(inner.next().expect("target stage"));
            ExplicitTransition {
                cond: Some(cond),
                target,
                span,
            }
        }
        Rule::transition_else => {
            // "else" — no condition
            let target = parse_ident(inner.next().expect("target stage"));
            ExplicitTransition {
                cond: None,
                target,
                span,
            }
        }
        _ => {
            let fallback = parse_ident(inner.next().expect("fallback target"));
            ExplicitTransition {
                cond: None,
                target: fallback,
                span,
            }
        }
    }
}

fn parse_exec_decl(pair: pest::iterators::Pair<Rule>) -> Result<StageBodyItem, Diagnostic> {
    let span = span_from_pair(&pair);
    let mut inner = pair.into_inner();
    let capability = parse_ident(inner.next().expect("exec capability"));
    let mut args = Vec::new();
    // The children are: exec_arg_list (containing exec_arg items)
    for child in inner {
        if child.as_rule() == Rule::exec_arg_list {
            for arg_pair in child.into_inner() {
                args.push(parse_exec_arg(arg_pair));
            }
        }
    }
    Ok(StageBodyItem::Exec(ExecDecl {
        capability,
        args,
        span,
    }))
}

fn parse_exec_arg(pair: pest::iterators::Pair<Rule>) -> ExecArg {
    let span = span_from_pair(&pair);
    let mut inner = pair.into_inner();
    let name = parse_ident(inner.next().expect("exec arg name"));
    let value = parse_exec_value(inner.next().expect("exec arg value"));
    ExecArg { name, value, span }
}

fn parse_exec_value(pair: pest::iterators::Pair<Rule>) -> ExecValue {
    // exec_value = { multiline_string | single_line_string | json_value | ident ~ ("." ~ ident)? }
    let mut inner = pair.into_inner();
    let first = inner.next().expect("exec value first token");
    match first.as_rule() {
        Rule::multiline_string => {
            let raw = first.as_str();
            let processed = process_exec_multiline_string(raw);
            let span = span_from_pair(&first);
            ExecValue::MultilineString(Spanned::new(processed, span))
        }
        Rule::single_line_string => {
            let raw = first.as_str();
            let processed = process_policy_string_literal(raw);
            let span = span_from_pair(&first);
            ExecValue::String(Spanned::new(processed, span))
        }
        Rule::json_value => {
            let span = span_from_pair(&first);
            // Parse the literal's source text into a serde_json::Value. The
            // grammar guarantees this is well-formed JSON, so a parse error
            // is an internal-consistency bug rather than a user error.
            let value: serde_json::Value = serde_json::from_str(first.as_str())
                .expect("json_value rule produced unparseable JSON");
            ExecValue::Json(Spanned::new(value, span))
        }
        Rule::ident => {
            let first_ident = parse_ident(first);
            // Silent "." was consumed; check for optional second ident (field name)
            match inner.next() {
                Some(second) if second.as_rule() == Rule::ident => {
                    let field = parse_ident(second);
                    let span = Span::new(first_ident.span.start, field.span.end);
                    ExecValue::Ref(StageInputRef {
                        stage: first_ident,
                        field,
                        optional: false,
                        span,
                    })
                }
                Some(_) | None => ExecValue::InputRef(first_ident),
            }
        }
        _ => {
            // Should be unreachable — grammar guarantees one of the above
            let span = span_from_pair(&first);
            ExecValue::InputRef(Ident {
                text: String::new(),
                span,
            })
        }
    }
}

pub fn process_string(raw: &str) -> String {
    if raw.starts_with("\"\"\"") {
        let inner = &raw[3..raw.len() - 3];
        let trimmed = inner.trim();
        if trimmed.contains('\n') {
            trimmed
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        let inner = &raw[1..raw.len() - 1];
        let unescaped = inner.replace("\\\"", "\"");
        let trimmed = unescaped.trim();
        if trimmed.contains('\n') {
            trimmed
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// Unescape a policy string literal: strip surrounding `"` quotes and
/// unescape `\"` sequences.  Unlike `process_string`, this does **not**
/// trim the content — significant leading/trailing whitespace is preserved
/// for shell-command prefix and metacharacter predicates.
pub fn process_policy_string_literal(raw: &str) -> String {
    debug_assert!(raw.starts_with('"') && raw.ends_with('"'));
    let inner = &raw[1..raw.len() - 1];
    inner.replace("\\\"", "\"")
}

/// Process a multi-line string literal for exec contexts.
///
/// Unlike `process_string`, this preserves the content **verbatim** —
/// no dedent, no trim. The `"""` delimiters are stripped; interior
/// content (including leading/trailing whitespace on each line) is
/// passed through unchanged. This is the right behavior for code
/// blocks (`browser.js.run(code: """...""")`) where formatting matters.
pub fn process_exec_multiline_string(raw: &str) -> String {
    debug_assert!(raw.starts_with("\"\"\"") && raw.ends_with("\"\"\""));
    let inner = &raw[3..raw.len() - 3];
    // Preserve content verbatim — just strip the delimiters.
    inner.to_string()
}
