use std::collections::{HashMap, HashSet};

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for err in &self.errors {
            writeln!(f, "{}", err)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

impl ValidationErrors {
    fn new() -> Self {
        Self { errors: vec![] }
    }

    fn push(&mut self, path: String, message: String) {
        self.errors.push(ValidationError { path, message });
    }
}

pub fn validate(ir: &WorkflowIr) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();

    if ir.ir_version != "0.1" {
        errors.push(
            "ir_version".into(),
            format!("expected '0.1', got '{}'", ir.ir_version),
        );
    }
    if ir.kind != "workflow_ir" {
        errors.push(
            "kind".into(),
            format!("expected 'workflow_ir', got '{}'", ir.kind),
        );
    }
    if ir.workflow.transition_semantics.selection != "first_match_by_priority" {
        errors.push(
            "workflow.transition_semantics.selection".into(),
            format!(
                "expected 'first_match_by_priority', got '{}'",
                ir.workflow.transition_semantics.selection
            ),
        );
    }
    if ir.workflow.transition_semantics.no_match != "error_unless_exit" {
        errors.push(
            "workflow.transition_semantics.no_match".into(),
            format!(
                "expected 'error_unless_exit', got '{}'",
                ir.workflow.transition_semantics.no_match
            ),
        );
    }
    if ir.workflow.id.is_empty() {
        errors.push("workflow.id".into(), "must be non-empty".into());
    }

    let node_map = build_node_map(&ir.nodes, &mut errors);
    let input_map = build_input_map(&ir.inputs, &mut errors);
    let capability_set = build_capability_set(&ir.capabilities, &mut errors);
    let writes_per_node = build_writes_per_node(&ir.nodes, &mut errors);

    if !ir.workflow.entry.is_empty() && !node_map.contains_key(&ir.workflow.entry) {
        errors.push(
            "workflow.entry".into(),
            format!("entry node '{}' does not exist", ir.workflow.entry),
        );
    }

    if ir.workflow.exits.is_empty() {
        errors.push("workflow.exits".into(), "must be non-empty".into());
    }
    for exit_id in &ir.workflow.exits {
        if !node_map.contains_key(exit_id) {
            errors.push(
                format!("workflow.exits[{}]", exit_id),
                format!("exit node '{}' does not exist", exit_id),
            );
        }
    }

    let adj = build_adjacency(&ir.nodes, &node_map);

    validate_transitions(
        &ir.nodes,
        &node_map,
        &writes_per_node,
        &input_map,
        &mut errors,
    );

    if !ir.workflow.entry.is_empty() && node_map.contains_key(&ir.workflow.entry) {
        let entry_idx = node_map[&ir.workflow.entry];
        validate_reachability(&ir.nodes, &adj, entry_idx, &ir.workflow.exits, &mut errors);
    }

    validate_node_refs(
        &ir.nodes,
        &node_map,
        &input_map,
        &writes_per_node,
        &adj,
        &mut errors,
    );

    for node in &ir.nodes {
        for cap in &node.requires {
            if !crate::capabilities::is_known_capability(&cap.capability) {
                errors.push(
                    format!("nodes.{}.requires", node.id),
                    format!("unknown capability '{}'", cap.capability),
                );
            }
            if !capability_set.contains(&cap.capability) {
                errors.push(
                    format!("nodes.{}.requires", node.id),
                    format!(
                        "required capability '{}' not declared in top-level capabilities",
                        cap.capability
                    ),
                );
            }
        }
    }

    for node in &ir.nodes {
        validate_stage_execution(
            node,
            &capability_set,
            &input_map,
            &writes_per_node,
            &mut errors,
        );
    }

    validate_policy_refs(&ir.policies, &input_map, &capability_set, &mut errors);

    if errors.errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn build_node_map(nodes: &[Node], errors: &mut ValidationErrors) -> HashMap<String, usize> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.id.is_empty() {
            errors.push(
                format!("nodes[{}].id", i),
                "node id must be non-empty".into(),
            );
        } else if map.contains_key(&node.id) {
            errors.push(
                format!("nodes[{}].id", i),
                format!("duplicate node id '{}'", node.id),
            );
        } else {
            map.insert(node.id.clone(), i);
        }
    }
    map
}

fn build_input_map<'a>(
    inputs: &'a [Input],
    errors: &mut ValidationErrors,
) -> HashMap<String, &'a str> {
    let mut map: HashMap<String, &str> = HashMap::new();
    for (i, inp) in inputs.iter().enumerate() {
        if inp.id.is_empty() {
            errors.push(
                format!("inputs[{}].id", i),
                "input id must be non-empty".into(),
            );
        } else if map.contains_key(&inp.id) {
            errors.push(
                format!("inputs[{}].id", i),
                format!("duplicate input id '{}'", inp.id),
            );
        } else {
            map.insert(inp.id.clone(), &inp.ty);
        }
    }
    map
}

fn build_capability_set(capabilities: &[String], errors: &mut ValidationErrors) -> HashSet<String> {
    let mut set: HashSet<String> = HashSet::new();
    for (i, cap) in capabilities.iter().enumerate() {
        if cap.is_empty() {
            errors.push(
                format!("capabilities[{}]", i),
                "capability name must be non-empty".into(),
            );
        } else if !set.insert(cap.clone()) {
            errors.push(
                format!("capabilities[{}]", i),
                format!("duplicate capability '{}'", cap),
            );
        }
        if !crate::capabilities::is_known_capability(cap) {
            errors.push(
                format!("capabilities[{}]", i),
                format!("unknown capability '{}'", cap),
            );
        }
    }
    set
}

fn build_writes_per_node<'a>(
    nodes: &'a [Node],
    errors: &mut ValidationErrors,
) -> HashMap<String, HashMap<String, &'a Write>> {
    let mut map: HashMap<String, HashMap<String, &Write>> = HashMap::new();
    for node in nodes {
        let mut writes: HashMap<String, &Write> = HashMap::new();
        for (j, w) in node.writes.iter().enumerate() {
            if w.name.is_empty() {
                errors.push(
                    format!("nodes.{}.writes[{}].name", node.id, j),
                    "write name must be non-empty".into(),
                );
            } else if writes.contains_key(&w.name) {
                errors.push(
                    format!("nodes.{}.writes[{}]", node.id, j),
                    format!("duplicate write name '{}'", w.name),
                );
            } else {
                writes.insert(w.name.clone(), w);
            }
        }
        map.insert(node.id.clone(), writes);
    }
    map
}

fn validate_transitions(
    nodes: &[Node],
    node_map: &HashMap<String, usize>,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    for node in nodes {
        let mut seen_priorities: HashSet<u32> = HashSet::new();
        for (j, t) in node.transitions.iter().enumerate() {
            if !node_map.contains_key(&t.to) {
                errors.push(
                    format!("nodes.{}.transitions[{}].to", node.id, j),
                    format!("transition target '{}' does not exist", t.to),
                );
            }
            if !seen_priorities.insert(t.priority) {
                errors.push(
                    format!("nodes.{}.transitions[{}].priority", node.id, j),
                    format!("duplicate priority {}", t.priority),
                );
            }
            validate_guard_refs(
                &t.guard,
                node.id.clone(),
                j,
                writes_per_node,
                input_map,
                errors,
            );
        }
    }
}

fn expr_type<'a>(
    expr: &'a Expr,
    writes_per_node: &'a HashMap<String, HashMap<String, &'a Write>>,
    input_map: &'a HashMap<String, &'a str>,
) -> Option<&'a str> {
    match expr {
        Expr::Not { .. } => Some("bool"),
        Expr::MethodCall { .. } => None,
        Expr::And { .. } => Some("bool"),
        Expr::Or { .. } => Some("bool"),
        Expr::Compare { .. } => Some("bool"),
        Expr::BinOp { .. } => Some("number"),
        Expr::Ref { r#ref } => resolve_ref_type(r#ref, writes_per_node, input_map),
        Expr::Literal { ty, .. } => Some(ty.as_str()),
    }
}

fn resolve_ref_type<'a>(
    r: &Ref,
    writes_per_node: &'a HashMap<String, HashMap<String, &'a Write>>,
    input_map: &'a HashMap<String, &'a str>,
) -> Option<&'a str> {
    match r {
        Ref::NodeOutput { node, field } => writes_per_node
            .get(node)
            .and_then(|writes| writes.get(field))
            .map(|w| w.ty.as_str()),
        Ref::Input { name } => input_map.get(name.as_str()).copied(),
        Ref::Bound { .. } => None,
    }
}

fn validate_guard_refs(
    guard: &Guard,
    node_id: String,
    transition_idx: usize,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    match guard {
        Guard::Always => {}
        Guard::HasValue { r#ref } | Guard::Missing { r#ref } => {
            validate_guard_output_ref(r#ref, &node_id, transition_idx, writes_per_node, errors);
        }
        Guard::Eq { left, right } => {
            validate_expr_refs(
                left,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_expr_refs(
                right,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            // Recurse into nested Compare/BinOp to validate their operands
            // (same surface as Guard::If — plan §4.2).
            validate_guard_expr_semantics(
                left,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_guard_expr_semantics(
                right,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );

            let left_ty = expr_type(left, writes_per_node, input_map);
            let right_ty = expr_type(right, writes_per_node, input_map);
            if let (Some(lt), Some(rt)) = (left_ty, right_ty) {
                // §3.4: numeric equality is forbidden — use ordering predicates instead.
                if lt == "number" || rt == "number" {
                    errors.push(
                        format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                        "Guard::Eq does not support number operands; use a compare predicate (>, >=, <, <=) or `score - x > eps` for near-equality".into(),
                    );
                } else {
                    // Plan §4.2: path≡path, path≡string, string≡string, bool≡bool are valid.
                    // Symmetric for Guard::Eq (equality is commutative).
                    let compatible = matches!(
                        (lt, rt),
                        ("path", "path")
                            | ("path", "string")
                            | ("string", "path")
                            | ("string", "string")
                            | ("bool", "bool")
                    );
                    if !compatible {
                        errors.push(
                            format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                            format!(
                                "Guard::Eq compares incompatible types: '{}' vs '{}'",
                                lt, rt
                            ),
                        );
                    }
                }
            }
        }
        Guard::If { cond } => {
            validate_expr_refs(
                cond,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            let cond_ty = expr_type(cond, writes_per_node, input_map);
            if cond_ty != Some("bool") {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("Guard::If condition must be bool, got {:?}", cond_ty),
                );
            }
            validate_guard_expr_semantics(
                cond,
                &node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
        }
    }
}

/// Semantic validation for expressions used in `Guard::If` conditions.
/// Ensures Compare/BinOp operands are number-typed and ops are valid.
fn validate_guard_expr_semantics(
    expr: &Expr,
    node_id: &str,
    transition_idx: usize,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    match expr {
        Expr::Compare { op, left, right } => {
            let valid_ops = ["gt", "gte", "lt", "lte"];
            if !valid_ops.contains(&op.as_str()) {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!(
                        "unknown compare op '{}'; expected one of gt, gte, lt, lte",
                        op
                    ),
                );
            }
            validate_guard_expr_semantics(
                left,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_guard_expr_semantics(
                right,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            // Both operands must be number (plan §4.2)
            let left_ty = expr_type(left, writes_per_node, input_map);
            let right_ty = expr_type(right, writes_per_node, input_map);
            if left_ty != Some("number") || right_ty != Some("number") {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    "compare requires number operands".into(),
                );
            }
        }
        Expr::BinOp { op, left, right } => {
            let valid_ops = ["add", "sub", "mul", "div"];
            if !valid_ops.contains(&op.as_str()) {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("unknown binop '{}'; expected one of add, sub, mul, div", op),
                );
            }
            validate_guard_expr_semantics(
                left,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_guard_expr_semantics(
                right,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            let left_ty = expr_type(left, writes_per_node, input_map);
            let right_ty = expr_type(right, writes_per_node, input_map);
            if left_ty != Some("number") || right_ty != Some("number") {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    "binop requires number operands".into(),
                );
            }
        }
        Expr::Not { expr } => {
            validate_guard_expr_semantics(
                expr,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            // Plan §5: not requires a bool operand (mirrors and/or arms).
            let inner_ty = expr_type(expr, writes_per_node, input_map);
            if inner_ty != Some("bool") {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("not operand must be bool, got {:?}", inner_ty),
                );
            }
        }
        Expr::And { exprs } => {
            for e in exprs {
                validate_guard_expr_semantics(
                    e,
                    node_id,
                    transition_idx,
                    writes_per_node,
                    input_map,
                    errors,
                );
                // Plan §5: and/or are boolean combinators; every operand must be bool.
                let op_ty = expr_type(e, writes_per_node, input_map);
                if op_ty != Some("bool") {
                    errors.push(
                        format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                        format!("and operand must be bool, got {:?}", op_ty),
                    );
                }
            }
        }
        Expr::Or { exprs } => {
            for e in exprs {
                validate_guard_expr_semantics(
                    e,
                    node_id,
                    transition_idx,
                    writes_per_node,
                    input_map,
                    errors,
                );
                // Plan §5: and/or are boolean combinators; every operand must be bool.
                let op_ty = expr_type(e, writes_per_node, input_map);
                if op_ty != Some("bool") {
                    errors.push(
                        format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                        format!("or operand must be bool, got {:?}", op_ty),
                    );
                }
            }
        }
        Expr::MethodCall { .. } => {
            errors.push(
                format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                "method calls are not supported in guard conditions".into(),
            );
        }
        Expr::Ref { .. } | Expr::Literal { .. } => {}
    }
}
fn validate_guard_output_ref(
    r: &Ref,
    node_id: &str,
    transition_idx: usize,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    errors: &mut ValidationErrors,
) {
    if let Ref::NodeOutput { node, field } = r {
        if let Some(fields) = writes_per_node.get(node) {
            if !fields.contains_key(field) {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("guard refs unknown output '{}.{}'", node, field),
                );
            }
        } else {
            errors.push(
                format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                format!("guard refs unknown node '{}'", node),
            );
        }
    } else {
        errors.push(
            format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
            "guard ref must be a node_output, not input or bound".into(),
        );
    }
}

fn validate_expr_refs(
    expr: &Expr,
    node_id: &str,
    transition_idx: usize,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    match expr {
        Expr::Not { expr } => validate_expr_refs(
            expr,
            node_id,
            transition_idx,
            writes_per_node,
            input_map,
            errors,
        ),
        Expr::MethodCall { receiver, args, .. } => {
            validate_expr_refs(
                receiver,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            for arg in args {
                validate_expr_refs(
                    arg,
                    node_id,
                    transition_idx,
                    writes_per_node,
                    input_map,
                    errors,
                );
            }
        }
        Expr::Ref { r#ref } => {
            validate_expr_ref(
                r#ref,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
        }
        Expr::Literal { .. } => {}
        Expr::And { exprs } => {
            for e in exprs {
                validate_expr_refs(
                    e,
                    node_id,
                    transition_idx,
                    writes_per_node,
                    input_map,
                    errors,
                );
            }
        }
        Expr::Or { exprs } => {
            for e in exprs {
                validate_expr_refs(
                    e,
                    node_id,
                    transition_idx,
                    writes_per_node,
                    input_map,
                    errors,
                );
            }
        }
        Expr::Compare { left, right, .. } => {
            validate_expr_refs(
                left,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_expr_refs(
                right,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
        }
        Expr::BinOp { left, right, .. } => {
            validate_expr_refs(
                left,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
            validate_expr_refs(
                right,
                node_id,
                transition_idx,
                writes_per_node,
                input_map,
                errors,
            );
        }
    }
}

fn validate_expr_ref(
    r: &Ref,
    node_id: &str,
    transition_idx: usize,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    match r {
        Ref::NodeOutput { node, field } => {
            if let Some(fields) = writes_per_node.get(node) {
                if !fields.contains_key(field) {
                    errors.push(
                        format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                        format!("guard refs unknown output '{}.{}'", node, field),
                    );
                }
            } else {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("guard refs unknown node '{}'", node),
                );
            }
        }
        Ref::Input { name } => {
            if !input_map.contains_key(name) {
                errors.push(
                    format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                    format!("guard refs unknown workflow input '{}'", name),
                );
            }
        }
        Ref::Bound { name } => {
            errors.push(
                format!("nodes.{}.transitions[{}].guard", node_id, transition_idx),
                format!(
                    "guard uses Ref::Bound('{}') which is policy-local only",
                    name
                ),
            );
        }
    }
}

fn build_adjacency(nodes: &[Node], node_map: &HashMap<String, usize>) -> Vec<Vec<usize>> {
    let n = nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, node) in nodes.iter().enumerate() {
        for t in &node.transitions {
            if let Some(&target_idx) = node_map.get(&t.to) {
                adj[i].push(target_idx);
            }
        }
    }
    adj
}

fn can_reach(adj: &[Vec<usize>], from: usize, to: usize) -> bool {
    let n = adj.len();
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

fn validate_reachability(
    nodes: &[Node],
    adj: &[Vec<usize>],
    entry_idx: usize,
    exit_ids: &[String],
    errors: &mut ValidationErrors,
) {
    let n = nodes.len();

    let exit_indices: HashSet<usize> = exit_ids
        .iter()
        .filter_map(|eid| nodes.iter().position(|nd| &nd.id == eid))
        .collect();

    let mut reachable_from_entry = vec![false; n];
    let mut stack = vec![entry_idx];
    reachable_from_entry[entry_idx] = true;
    while let Some(u) = stack.pop() {
        for &v in &adj[u] {
            if !reachable_from_entry[v] {
                reachable_from_entry[v] = true;
                stack.push(v);
            }
        }
    }

    for (i, reachable_item) in reachable_from_entry.iter().enumerate() {
        if !reachable_item {
            errors.push(
                format!("nodes.{}.id", nodes[i].id),
                format!("node '{}' is unreachable from entry", nodes[i].id),
            );
        }
    }

    for node in nodes {
        let idx = nodes.iter().position(|nd| nd.id == node.id).unwrap();
        let is_exit = exit_indices.contains(&idx);

        if is_exit {
            if !node.transitions.is_empty() {
                errors.push(
                    format!("nodes.{}.transitions", node.id),
                    format!("exit node '{}' must have no outgoing transitions", node.id),
                );
            }
        } else {
            if adj[idx].is_empty() {
                errors.push(
                    format!("nodes.{}.transitions", node.id),
                    format!("non-exit node '{}' has no outgoing transitions", node.id),
                );
            }

            let mut visited = vec![false; n];
            let mut stack = vec![idx];
            visited[idx] = true;
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
                errors.push(
                    format!("nodes.{}.id", node.id),
                    format!("node '{}' cannot reach any exit", node.id),
                );
            }
        }
    }
}

fn validate_node_refs(
    nodes: &[Node],
    node_map: &HashMap<String, usize>,
    input_map: &HashMap<String, &str>,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    adj: &[Vec<usize>],
    errors: &mut ValidationErrors,
) {
    for node in nodes {
        let consumer_idx = node_map.get(&node.id).copied().unwrap_or(usize::MAX);

        for (j, read) in node.reads.iter().enumerate() {
            match &read.ref_ {
                Ref::Input { name } => {
                    if !input_map.contains_key(name) {
                        errors.push(
                            format!("nodes.{}.reads[{}]", node.id, j),
                            format!("read refs unknown workflow input '{}'", name),
                        );
                    }
                }
                Ref::NodeOutput {
                    node: ref_node,
                    field,
                } => {
                    if let Some(source_writes) = writes_per_node.get(ref_node) {
                        if !source_writes.contains_key(field) {
                            errors.push(
                                format!("nodes.{}.reads[{}]", node.id, j),
                                format!("read refs unknown output '{}.{}'", ref_node, field),
                            );
                        }
                    } else {
                        errors.push(
                            format!("nodes.{}.reads[{}]", node.id, j),
                            format!("read refs unknown node '{}'", ref_node),
                        );
                    }

                    if let Some(producer_idx) = node_map.get(ref_node) {
                        let pi = *producer_idx;
                        if pi == consumer_idx {
                            errors.push(
                                format!("nodes.{}.reads[{}]", node.id, j),
                                format!(
                                    "self-read: '{}' reads its own output '{}.{}'",
                                    node.id, ref_node, field
                                ),
                            );
                        } else if !can_reach(adj, pi, consumer_idx) {
                            errors.push(
                                format!("nodes.{}.reads[{}]", node.id, j),
                                format!(
                                    "producer '{}' cannot reach consumer '{}' on any control-flow path",
                                    ref_node, node.id
                                ),
                            );
                        }
                    }
                }
                Ref::Bound { name } => {
                    errors.push(
                        format!("nodes.{}.reads[{}]", node.id, j),
                        format!(
                            "read uses Ref::Bound('{}') which is policy-local only",
                            name
                        ),
                    );
                }
            }
        }
        validate_has_value_guard_requirement(node, nodes, node_map, writes_per_node, errors);
        validate_optional_guard_refs(node, writes_per_node, errors);
    }
}

fn validate_optional_guard_refs(
    node: &Node,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    errors: &mut ValidationErrors,
) {
    for (j, t) in node.transitions.iter().enumerate() {
        match &t.guard {
            Guard::HasValue { r#ref } | Guard::Missing { r#ref } => {
                if let Ref::NodeOutput {
                    node: ref_node,
                    field,
                } = r#ref
                {
                    if let Some(src_writes) = writes_per_node.get(ref_node) {
                        if let Some(w) = src_writes.get(field) {
                            if !w.optional {
                                errors.push(
                                    format!("nodes.{}.transitions[{}].guard", node.id, j),
                                    format!(
                                        "guard uses has_value/missing on non-optional output '{}.{}'",
                                        ref_node, field
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn validate_stage_execution(
    node: &Node,
    capability_set: &HashSet<String>,
    input_map: &HashMap<String, &str>,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    errors: &mut ValidationErrors,
) {
    let StageExecution::Tool { capability, args } = &node.execution else {
        return;
    };

    if !crate::capabilities::is_known_capability(capability) {
        errors.push(
            format!("nodes.{}.execution", node.id),
            format!("unknown exec capability '{}'", capability),
        );
        return;
    }

    if !capability_set.contains(capability) {
        errors.push(
            format!("nodes.{}.execution", node.id),
            format!(
                "exec capability '{}' not declared in top-level capabilities",
                capability
            ),
        );
    }

    if !node.requires.iter().any(|c| c.capability == *capability) {
        errors.push(
            format!("nodes.{}.execution", node.id),
            format!(
                "exec capability '{}' must be in node's requires",
                capability
            ),
        );
    }

    let spec = crate::capabilities::get_capability(capability).unwrap();
    let mut seen_params: HashSet<&str> = HashSet::new();
    let reads_set: HashSet<String> = node
        .reads
        .iter()
        .filter_map(|r| match &r.ref_ {
            Ref::NodeOutput { node, field } => Some(format!("{}:{}", node, field)),
            _ => None,
        })
        .collect();

    for (arg_name, arg_expr) in args {
        seen_params.insert(arg_name.as_str());

        if !spec.has_param(arg_name) {
            errors.push(
                format!("nodes.{}.execution.args.{}", node.id, arg_name),
                format!(
                    "unknown exec arg '{}' for capability '{}'",
                    arg_name, capability
                ),
            );
        }

        validate_exec_arg_expr(
            arg_expr,
            &node.id,
            arg_name,
            input_map,
            writes_per_node,
            &reads_set,
            errors,
        );
    }

    for param in spec.required_params {
        if param.required && !seen_params.contains(param.name) {
            errors.push(
                format!("nodes.{}.execution", node.id),
                format!(
                    "missing required exec arg '{}' for capability '{}'",
                    param.name, capability
                ),
            );
        }
    }
}

fn validate_exec_arg_expr(
    expr: &Expr,
    node_id: &str,
    arg_name: &str,
    input_map: &HashMap<String, &str>,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    reads_set: &HashSet<String>,
    errors: &mut ValidationErrors,
) {
    let path = format!("nodes.{}.execution.args.{}", node_id, arg_name);
    match expr {
        Expr::Ref { r#ref } => match r#ref {
            Ref::Input { name } => {
                if !input_map.contains_key(name) {
                    errors.push(
                        path,
                        format!("exec arg refs unknown workflow input '{}'", name),
                    );
                }
            }
            Ref::NodeOutput { node, field } => {
                if let Some(writes) = writes_per_node.get(node) {
                    if !writes.contains_key(field) {
                        errors.push(
                            path.clone(),
                            format!("exec arg refs unknown output '{}.{}'", node, field),
                        );
                    }
                } else {
                    errors.push(
                        path.clone(),
                        format!("exec arg refs unknown node '{}'", node),
                    );
                }
                let key = format!("{}:{}", node, field);
                if !reads_set.contains(&key) {
                    errors.push(
                        path,
                        format!("exec arg refs '{}.{}' but not in node's reads", node, field),
                    );
                }
            }
            Ref::Bound { name } => {
                errors.push(
                    path,
                    format!(
                        "exec arg uses Ref::Bound('{}') which is policy-local only",
                        name
                    ),
                );
            }
        },
        Expr::Literal { .. } => {}
        Expr::Not { .. }
        | Expr::MethodCall { .. }
        | Expr::And { .. }
        | Expr::Or { .. }
        | Expr::Compare { .. }
        | Expr::BinOp { .. } => {
            errors.push(
                path,
                "only Ref and Literal expressions are allowed in exec args (MVP)".into(),
            );
        }
    }
}

fn validate_has_value_guard_requirement(
    consumer: &Node,
    nodes: &[Node],
    node_map: &HashMap<String, usize>,
    writes_per_node: &HashMap<String, HashMap<String, &Write>>,
    errors: &mut ValidationErrors,
) {
    let consumer_idx = node_map.get(&consumer.id).copied();
    if consumer_idx.is_none() {
        return;
    }
    let target_idx = consumer_idx.unwrap();

    for (j, read) in consumer.reads.iter().enumerate() {
        if read.optional {
            continue;
        }
        if let Ref::NodeOutput {
            node: ref_node,
            field,
        } = &read.ref_
        {
            if let Some(src_writes) = writes_per_node.get(ref_node) {
                if let Some(w) = src_writes.get(field) {
                    if w.optional {
                        let guarded = all_incoming_transitions_guard(
                            target_idx, ref_node, field, nodes, node_map,
                        );
                        if !guarded {
                            errors.push(
                                format!("nodes.{}.reads[{}]", consumer.id, j),
                                format!(
                                    "required read of optional output '{}.{}' without incoming has_value guard",
                                    ref_node, field
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn all_incoming_transitions_guard(
    target_idx: usize,
    guard_node: &str,
    guard_field: &str,
    nodes: &[Node],
    node_map: &HashMap<String, usize>,
) -> bool {
    for node in nodes {
        for t in &node.transitions {
            if let Some(&to_idx) = node_map.get(&t.to) {
                if to_idx == target_idx
                    && !matches!(
                        &t.guard,
                        Guard::HasValue {
                            r#ref: Ref::NodeOutput { node, field }
                        } if node == guard_node && field == guard_field
                    )
                {
                    return false;
                }
            }
        }
    }

    true
}

fn validate_policy_refs(
    policies: &[Policy],
    input_map: &HashMap<String, &str>,
    capability_set: &HashSet<String>,
    errors: &mut ValidationErrors,
) {
    for (i, policy) in policies.iter().enumerate() {
        let trigger_spec = crate::capabilities::get_capability(&policy.trigger.capability);

        if trigger_spec.is_none() {
            errors.push(
                format!("policies[{}].trigger.capability", i),
                format!("unknown capability '{}'", policy.trigger.capability),
            );
        }

        if !capability_set.contains(&policy.trigger.capability) {
            errors.push(
                format!("policies[{}].trigger.capability", i),
                format!(
                    "trigger capability '{}' not declared in top-level capabilities",
                    policy.trigger.capability
                ),
            );
        }

        if let Some(spec) = trigger_spec {
            for (bind_name, _) in &policy.trigger.bind {
                if !spec.has_required_param(bind_name) {
                    errors.push(
                        format!("policies[{}].trigger.bind.{}", i, bind_name),
                        format!(
                            "capability '{}' has no required parameter '{}'",
                            policy.trigger.capability, bind_name
                        ),
                    );
                }
            }
        }

        let bound_names: HashSet<&str> = policy.trigger.bind.keys().map(|s| s.as_str()).collect();

        if let Some(ref requires) = policy.requires {
            for (j, req) in requires.iter().enumerate() {
                if !crate::capabilities::is_known_capability(&req.capability) {
                    errors.push(
                        format!("policies[{}].requires[{}].capability", i, j),
                        format!("unknown capability '{}'", req.capability),
                    );
                }

                if !capability_set.contains(&req.capability) {
                    errors.push(
                        format!("policies[{}].requires[{}].capability", i, j),
                        format!(
                            "required capability '{}' not declared in top-level capabilities",
                            req.capability
                        ),
                    );
                }

                if let Some(spec) = crate::capabilities::get_capability(&req.capability) {
                    for (arg_name, _) in &req.args {
                        if !spec.has_required_param(arg_name) {
                            errors.push(
                                format!("policies[{}].requires[{}].args.{}", i, j, arg_name),
                                format!(
                                    "capability '{}' has no required parameter '{}'",
                                    req.capability, arg_name
                                ),
                            );
                        }
                    }
                }

                for (arg_name, arg_val) in &req.args {
                    let ArgValue::Ref { r#ref } = arg_val;
                    validate_policy_ref(
                        r#ref,
                        i,
                        &format!("requires[{}].args.{}", j, arg_name),
                        input_map,
                        &bound_names,
                        errors,
                    );
                }
            }
        }

        // --- policy shape checks ---
        if policy.kind == "deny" && policy.condition.is_none() {
            errors.push(
                format!("policies[{}].condition", i),
                "deny policy must have a condition".into(),
            );
        }
        if policy.kind == "before" && policy.condition.is_some() {
            errors.push(
                format!("policies[{}].condition", i),
                "before policy must not have a condition".into(),
            );
        }

        if let Some(ref condition) = policy.condition {
            validate_policy_expr_refs(condition, i, "condition", input_map, &bound_names, errors);
            validate_policy_expr_semantics(
                condition,
                i,
                "condition",
                &policy.trigger.capability,
                &bound_names,
                input_map,
                errors,
            );
            // Top-level deny condition must be bool (non-DSL frontends
            // may not enforce this; IR is the authoritative layer).
            if policy.kind == "deny" {
                let cond_ty = policy_expr_type(
                    condition,
                    &policy.trigger.capability,
                    &bound_names,
                    input_map,
                );
                if cond_ty.as_deref() != Some("bool") {
                    errors.push(
                        format!("policies[{}].condition", i),
                        format!("deny condition must be bool, got {:?}", cond_ty),
                    );
                }
            }
        }
    }
}

fn validate_policy_ref(
    r: &Ref,
    policy_idx: usize,
    path: &str,
    input_map: &HashMap<String, &str>,
    bound_names: &HashSet<&str>,
    errors: &mut ValidationErrors,
) {
    match r {
        Ref::Input { name } => {
            if !input_map.contains_key(name) {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    format!("refs unknown workflow input '{}'", name),
                );
            }
        }
        Ref::Bound { name } => {
            if !bound_names.contains(name.as_str()) {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    format!("bound ref '{}' not declared in trigger bind", name),
                );
            }
        }
        Ref::NodeOutput { node, field } => {
            errors.push(
                format!("policies[{}].{}", policy_idx, path),
                format!(
                    "policy refs '{}.{}' should not reference node outputs",
                    node, field
                ),
            );
        }
    }
}

fn validate_policy_expr_refs(
    expr: &Expr,
    policy_idx: usize,
    path: &str,
    input_map: &HashMap<String, &str>,
    bound_names: &HashSet<&str>,
    errors: &mut ValidationErrors,
) {
    match expr {
        Expr::Not { expr } => {
            validate_policy_expr_refs(expr, policy_idx, path, input_map, bound_names, errors);
        }
        Expr::MethodCall { receiver, args, .. } => {
            validate_policy_expr_refs(receiver, policy_idx, path, input_map, bound_names, errors);
            for (k, arg) in args.iter().enumerate() {
                validate_policy_expr_refs(
                    arg,
                    policy_idx,
                    &format!("{}.args[{}]", path, k),
                    input_map,
                    bound_names,
                    errors,
                );
            }
        }
        Expr::Ref { r#ref } => {
            validate_policy_ref(r#ref, policy_idx, path, input_map, bound_names, errors);
        }
        Expr::Literal { .. } => {}
        Expr::And { exprs } => {
            for (k, e) in exprs.iter().enumerate() {
                validate_policy_expr_refs(
                    e,
                    policy_idx,
                    &format!("{}.and[{}]", path, k),
                    input_map,
                    bound_names,
                    errors,
                );
            }
        }
        Expr::Or { exprs } => {
            for (k, e) in exprs.iter().enumerate() {
                validate_policy_expr_refs(
                    e,
                    policy_idx,
                    &format!("{}.or[{}]", path, k),
                    input_map,
                    bound_names,
                    errors,
                );
            }
        }
        Expr::Compare { left, right, .. } => {
            validate_policy_expr_refs(left, policy_idx, path, input_map, bound_names, errors);
            validate_policy_expr_refs(right, policy_idx, path, input_map, bound_names, errors);
        }
        Expr::BinOp { left, right, .. } => {
            validate_policy_expr_refs(left, policy_idx, path, input_map, bound_names, errors);
            validate_policy_expr_refs(right, policy_idx, path, input_map, bound_names, errors);
        }
    }
}

/// Validate policy expression semantics: method names, arity, type compatibility,
/// boolean operand requirements, and non-empty and/or.
fn validate_policy_expr_semantics(
    expr: &Expr,
    policy_idx: usize,
    path: &str,
    trigger_capability: &str,
    bound_names: &HashSet<&str>,
    input_map: &HashMap<String, &str>,
    errors: &mut ValidationErrors,
) {
    match expr {
        Expr::Not { expr } => {
            validate_policy_expr_semantics(
                expr,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            let inner_ty = policy_expr_type(expr, trigger_capability, bound_names, input_map);
            if inner_ty.as_deref() != Some("bool") {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    "`not` requires a bool expression".into(),
                );
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            validate_policy_expr_semantics(
                receiver,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            for (k, arg) in args.iter().enumerate() {
                validate_policy_expr_semantics(
                    arg,
                    policy_idx,
                    &format!("{}.args[{}]", path, k),
                    trigger_capability,
                    bound_names,
                    input_map,
                    errors,
                );
            }

            let known_methods = ["contains", "eq", "starts_with"];
            if !known_methods.contains(&method.as_str()) {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    format!(
                        "unknown method '{}'; expected one of contains, eq, starts_with",
                        method
                    ),
                );
                return;
            }

            let receiver_ty =
                policy_expr_type(receiver, trigger_capability, bound_names, input_map);

            match method.as_str() {
                "eq" => {
                    if args.len() != 1 {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "eq() requires exactly 1 argument".into(),
                        );
                        return;
                    }
                    let arg_ty =
                        policy_expr_type(&args[0], trigger_capability, bound_names, input_map);
                    // §3.4: numeric equality is forbidden — use ordering predicates instead.
                    if receiver_ty.as_deref() == Some("number")
                        || arg_ty.as_deref() == Some("number")
                    {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "eq() does not support number operands; use a compare predicate (>, >=, <, <=) or `score - x > eps` for near-equality".into(),
                        );
                        return;
                    }
                    // Plan §5: Path.eq(Path/string) allowed; string.eq(string) allowed;
                    // string.eq(Path) and all other combinations (bool, unknown, etc.) rejected.
                    let compatible = match (receiver_ty.as_deref(), arg_ty.as_deref()) {
                        (Some("path"), Some("path")) | (Some("path"), Some("string")) => true,
                        (Some("string"), Some("string")) => true,
                        (Some("string"), Some("path")) => false,
                        _ => false, // Plan §5: no other type combinations are supported for eq
                    };
                    if !compatible {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            format!(
                                "eq() compares incompatible types: {:?} vs {:?}",
                                receiver_ty, arg_ty
                            ),
                        );
                    }
                }
                "starts_with" => {
                    if args.len() != 1 {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "starts_with() requires exactly 1 argument".into(),
                        );
                        return;
                    }
                    if receiver_ty.as_deref() != Some("string") {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "starts_with() requires a string receiver".into(),
                        );
                    }
                    let arg_ty =
                        policy_expr_type(&args[0], trigger_capability, bound_names, input_map);
                    if arg_ty.as_deref() != Some("string") {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "starts_with() argument must be string".into(),
                        );
                    }
                }
                "contains" => {
                    // Plan §5: contains accepts exactly 1 argument.
                    if args.len() != 1 {
                        errors.push(
                            format!("policies[{}].{}", policy_idx, path),
                            "contains() requires exactly 1 argument".into(),
                        );
                        return;
                    }
                    match receiver_ty.as_deref() {
                        Some("path") => {
                            let arg_ty = policy_expr_type(
                                &args[0],
                                trigger_capability,
                                bound_names,
                                input_map,
                            );
                            // plan: Path.contains(Path/string) — allow both
                            if arg_ty.as_deref() != Some("path")
                                && arg_ty.as_deref() != Some("string")
                            {
                                errors.push(
                                    format!("policies[{}].{}", policy_idx, path),
                                    "path.contains() argument must be path or string".into(),
                                );
                            }
                        }
                        Some("string") => {
                            let arg_ty = policy_expr_type(
                                &args[0],
                                trigger_capability,
                                bound_names,
                                input_map,
                            );
                            if arg_ty.as_deref() != Some("string") {
                                errors.push(
                                    format!("policies[{}].{}", policy_idx, path),
                                    "string.contains() argument must be string".into(),
                                );
                            }
                        }
                        // Plan §5: bool and unknown receiver types are unsupported.
                        Some("bool") | None => {
                            errors.push(
                                format!("policies[{}].{}", policy_idx, path),
                                "contains() is not supported on this receiver type".into(),
                            );
                        }
                        Some(_) => {
                            errors.push(
                                format!("policies[{}].{}", policy_idx, path),
                                "contains() is not supported on this receiver type".into(),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        Expr::And { exprs } => {
            if exprs.is_empty() {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    "and() requires at least 1 operand".into(),
                );
            }
            for (k, e) in exprs.iter().enumerate() {
                validate_policy_expr_semantics(
                    e,
                    policy_idx,
                    &format!("{}.and[{}]", path, k),
                    trigger_capability,
                    bound_names,
                    input_map,
                    errors,
                );
                // Plan §5: and/or are boolean combinators; every operand must be bool.
                let op_ty = policy_expr_type(e, trigger_capability, bound_names, input_map);
                if op_ty.as_deref() != Some("bool") {
                    errors.push(
                        format!("policies[{}].{}.and[{}]", policy_idx, path, k),
                        format!("and operand must be bool, got {:?}", op_ty),
                    );
                }
            }
        }
        Expr::Or { exprs } => {
            if exprs.is_empty() {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    "or() requires at least 1 operand".into(),
                );
            }
            for (k, e) in exprs.iter().enumerate() {
                validate_policy_expr_semantics(
                    e,
                    policy_idx,
                    &format!("{}.or[{}]", path, k),
                    trigger_capability,
                    bound_names,
                    input_map,
                    errors,
                );
                // Plan §5: and/or are boolean combinators; every operand must be bool.
                let op_ty = policy_expr_type(e, trigger_capability, bound_names, input_map);
                if op_ty.as_deref() != Some("bool") {
                    errors.push(
                        format!("policies[{}].{}.or[{}]", policy_idx, path, k),
                        format!("or operand must be bool, got {:?}", op_ty),
                    );
                }
            }
        }
        Expr::Ref { .. } | Expr::Literal { .. } => {}
        Expr::Compare { op, left, right } => {
            validate_policy_expr_semantics(
                left,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            validate_policy_expr_semantics(
                right,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            let valid_ops = ["gt", "gte", "lt", "lte"];
            if !valid_ops.contains(&op.as_str()) {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    format!(
                        "unknown compare op '{}'; expected one of gt, gte, lt, lte",
                        op
                    ),
                );
            }
            // Both operands must be number
            let left_ty = policy_expr_type(left, trigger_capability, bound_names, input_map);
            let right_ty = policy_expr_type(right, trigger_capability, bound_names, input_map);
            if left_ty.as_deref() != Some("number") || right_ty.as_deref() != Some("number") {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    "compare requires number operands".into(),
                );
            }
        }
        Expr::BinOp { op, left, right } => {
            validate_policy_expr_semantics(
                left,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            validate_policy_expr_semantics(
                right,
                policy_idx,
                path,
                trigger_capability,
                bound_names,
                input_map,
                errors,
            );
            let valid_ops = ["add", "sub", "mul", "div"];
            if !valid_ops.contains(&op.as_str()) {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    format!(
                        "unknown binop op '{}'; expected one of add, sub, mul, div",
                        op
                    ),
                );
            }
            // Both operands must be number
            let left_ty = policy_expr_type(left, trigger_capability, bound_names, input_map);
            let right_ty = policy_expr_type(right, trigger_capability, bound_names, input_map);
            if left_ty.as_deref() != Some("number") || right_ty.as_deref() != Some("number") {
                errors.push(
                    format!("policies[{}].{}", policy_idx, path),
                    "binop requires number operands".into(),
                );
            }
        }
    }
}

/// Infer the type of a policy expression from trigger bindings and input types.
fn policy_expr_type(
    expr: &Expr,
    trigger_capability: &str,
    bound_names: &HashSet<&str>,
    input_map: &HashMap<String, &str>,
) -> Option<String> {
    match expr {
        Expr::Not { .. } | Expr::And { .. } | Expr::Or { .. } => Some("bool".to_string()),
        Expr::Compare { .. } => Some("bool".to_string()),
        Expr::BinOp { .. } => Some("number".to_string()),
        Expr::MethodCall { .. } => Some("bool".to_string()),
        Expr::Ref { r#ref } => match r#ref {
            Ref::Input { name } => input_map.get(name).map(|s| s.to_string()),
            Ref::Bound { name } => {
                if bound_names.contains(name.as_str()) {
                    crate::capabilities::bound_var_type(trigger_capability, name).map(|t| match t {
                        crate::capabilities::CapabilityParamType::String => "string".to_string(),
                        crate::capabilities::CapabilityParamType::Path => "path".to_string(),
                        crate::capabilities::CapabilityParamType::Bool => "bool".to_string(),
                        crate::capabilities::CapabilityParamType::Json => "json".to_string(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        },
        Expr::Literal { ty, .. } => Some(ty.clone()),
    }
}
