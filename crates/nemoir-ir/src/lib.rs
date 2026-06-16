use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowIr {
    pub ir_version: String,
    pub kind: String,
    pub source: Source,
    pub workflow: Workflow,
    pub inputs: Vec<Input>,
    pub capabilities: Vec<String>,
    pub policies: Vec<Policy>,
    pub nodes: Vec<Node>,
}

impl WorkflowIr {
    pub fn new(file: &str, id: &str, entry: &str, exits: Vec<String>) -> Self {
        Self {
            ir_version: "0.1".into(),
            kind: "workflow_ir".into(),
            source: Source {
                frontend: "nemo_dsl".into(),
                file: file.into(),
            },
            workflow: Workflow {
                id: id.into(),
                entry: entry.into(),
                exits,
                transition_semantics: TransitionSemantics {
                    selection: "first_match_by_priority".into(),
                    no_match: "error_unless_exit".into(),
                },
            },
            inputs: vec![],
            capabilities: vec![],
            policies: vec![],
            nodes: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub frontend: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub id: String,
    pub entry: String,
    pub exits: Vec<String>,
    pub transition_semantics: TransitionSemantics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionSemantics {
    pub selection: String,
    pub no_match: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Input {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    pub id: String,
    pub kind: String,
    pub trigger: Trigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<RequiredCapability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trigger {
    pub capability: String,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub bind: IndexMap<String, BindArg>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BindArg {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequiredCapability {
    pub capability: String,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub args: IndexMap<String, ArgValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum ArgValue {
    #[serde(rename = "ref")]
    Ref { r#ref: Ref },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub annotations: Vec<String>,
    pub prompt: String,
    pub reads: Vec<Read>,
    pub writes: Vec<Write>,
    pub requires: Vec<StageCapability>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Read {
    #[serde(rename = "ref")]
    pub ref_: Ref,
    pub optional: bool,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Write {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageCapability {
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transition {
    pub to: String,
    pub priority: u32,
    pub reason: String,
    pub guard: Guard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Guard {
    #[serde(rename = "always")]
    Always,
    #[serde(rename = "has_value")]
    HasValue { r#ref: Ref },
    #[serde(rename = "missing")]
    Missing { r#ref: Ref },
    #[serde(rename = "eq")]
    Eq { left: Expr, right: Expr },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Ref {
    #[serde(rename = "input")]
    Input { name: String },
    #[serde(rename = "node_output")]
    NodeOutput { node: String, field: String },
    #[serde(rename = "bound")]
    Bound { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Expr {
    #[serde(rename = "not")]
    Not { expr: Box<Expr> },
    #[serde(rename = "method_call")]
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<Expr>,
    },
    #[serde(rename = "ref")]
    Ref { r#ref: Ref },
    #[serde(rename = "literal")]
    Literal {
        #[serde(rename = "type")]
        ty: String,
        value: serde_yaml::Value,
    },
}
