use nemoir_ir::*;
use serde::Serialize;

use crate::VisualizerError;

#[derive(Debug, Serialize)]
struct GraphData {
    nodes: Vec<CyNode>,
    edges: Vec<CyEdge>,
}

#[derive(Debug, Serialize)]
struct CyNode {
    data: CyNodeData,
}

#[derive(Debug, Serialize)]
struct CyNodeData {
    id: String,
    label: String,
    kind: String,
    #[serde(rename = "isEntry")]
    is_entry: bool,
    #[serde(rename = "isExit")]
    is_exit: bool,
    #[serde(rename = "isTool")]
    is_tool: bool,
    annotations: Vec<String>,
    prompt: String,
    reads: Vec<Read>,
    writes: Vec<Write>,
    requires: Vec<StageCapability>,
    transitions: Vec<TransitionSummary>,
    execution: ExecutionSummary,
}

#[derive(Debug, Serialize)]
struct ExecutionSummary {
    kind: String,
    capability: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Serialize)]
struct TransitionSummary {
    to: String,
    priority: u32,
    reason: String,
    guard_summary: String,
}

#[derive(Debug, Serialize)]
struct CyEdge {
    data: CyEdgeData,
}

#[derive(Debug, Serialize)]
struct CyEdgeData {
    id: String,
    source: String,
    target: String,
    label: String,
    priority: u32,
    reason: String,
    #[serde(rename = "guardSummary")]
    guard_summary: String,
    guard: Guard,
}

pub fn build_graph_data(ir: &WorkflowIr) -> Result<serde_json::Value, VisualizerError> {
    let exit_set: std::collections::HashSet<&str> =
        ir.workflow.exits.iter().map(|s| s.as_str()).collect();
    let entry_id = ir.workflow.entry.as_str();

    let mut data = GraphData {
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    for node in &ir.nodes {
        let is_entry = node.id == entry_id;
        let is_exit = exit_set.contains(node.id.as_str());

        let tsummaries: Vec<TransitionSummary> = node
            .transitions
            .iter()
            .map(|t| TransitionSummary {
                to: t.to.clone(),
                priority: t.priority,
                reason: t.reason.clone(),
                guard_summary: guard_summary(&t.guard),
            })
            .collect();

        data.nodes.push(CyNode {
            data: CyNodeData {
                id: node.id.clone(),
                label: node.id.clone(),
                kind: "state".to_string(),
                is_entry,
                is_exit,
                is_tool: !node.execution.is_model(),
                annotations: node.annotations.clone(),
                prompt: node.prompt.clone(),
                reads: node.reads.clone(),
                writes: node.writes.clone(),
                requires: node.requires.clone(),
                transitions: tsummaries,
                execution: execution_summary(&node.execution),
            },
        });
    }

    for node in &ir.nodes {
        for (j, t) in node.transitions.iter().enumerate() {
            let edge_id = format!("{}__{}__{}", node.id, j, t.to);
            let gs = guard_summary(&t.guard);

            data.edges.push(CyEdge {
                data: CyEdgeData {
                    id: edge_id,
                    source: node.id.clone(),
                    target: t.to.clone(),
                    label: format!("p{} {}", t.priority, gs),
                    priority: t.priority,
                    reason: t.reason.clone(),
                    guard_summary: gs,
                    guard: t.guard.clone(),
                },
            });
        }
    }

    let json = serde_json::to_value(&data)?;
    Ok(json)
}

fn guard_summary(g: &Guard) -> String {
    match g {
        Guard::Always => "always".to_string(),
        Guard::HasValue {
            r#ref: Ref::NodeOutput { node, field },
        } => {
            format!("has_value({}.{})", node, field)
        }
        Guard::HasValue { .. } => "has_value(?)".to_string(),
        Guard::Missing {
            r#ref: Ref::NodeOutput { node, field },
        } => {
            format!("missing({}.{})", node, field)
        }
        Guard::Missing { .. } => "missing(?)".to_string(),
        Guard::Eq { left, right } => {
            let l = expr_summary(left);
            let r = expr_summary(right);
            format!("{} == {}", l, r)
        }
    }
}

fn execution_summary(e: &StageExecution) -> ExecutionSummary {
    match e {
        StageExecution::Model => ExecutionSummary {
            kind: "model".to_string(),
            capability: None,
            summary: None,
        },
        StageExecution::Tool { capability, args } => {
            let arg_strs: Vec<String> = args
                .iter()
                .map(|(name, expr)| format!("{}: {}", name, expr_summary(expr)))
                .collect();
            ExecutionSummary {
                kind: "tool".to_string(),
                capability: Some(capability.clone()),
                summary: Some(format!("{}({})", capability, arg_strs.join(", "))),
            }
        }
    }
}

fn expr_summary(e: &Expr) -> String {
    match e {
        Expr::Not { expr } => format!("!({})", expr_summary(expr)),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let recv = expr_summary(receiver);
            let arg_strs: Vec<String> = args.iter().map(expr_summary).collect();
            format!("{}.{}({})", recv, method, arg_strs.join(", "))
        }
        Expr::Ref {
            r#ref: Ref::NodeOutput { node, field },
        } => format!("{}.{}", node, field),
        Expr::Ref {
            r#ref: Ref::Input { name },
        } => name.clone(),
        Expr::Ref {
            r#ref: Ref::Bound { name },
        } => name.clone(),
        Expr::Literal { value, .. } => match value {
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::String(s) => format!("\"{}\"", s),
            _ => "?".to_string(),
        },
        Expr::And { exprs } => {
            let parts: Vec<String> = exprs.iter().map(expr_summary).collect();
            format!("({})", parts.join(" and "))
        }
        Expr::Or { exprs } => {
            let parts: Vec<String> = exprs.iter().map(expr_summary).collect();
            format!("({})", parts.join(" or "))
        }
    }
}
