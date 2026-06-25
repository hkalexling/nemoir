#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl From<std::ops::Range<usize>> for Span {
    fn from(r: std::ops::Range<usize>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BaseType {
    String,
    Bool,
    Path,
    Number,
    Unknown,
}

impl BaseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BaseType::String => "string",
            BaseType::Bool => "bool",
            BaseType::Path => "path",
            BaseType::Number => "number",
            BaseType::Unknown => "unknown",
        }
    }

    pub fn from_name(s: &str) -> BaseType {
        match s {
            "string" => BaseType::String,
            "bool" => BaseType::Bool,
            "path" => BaseType::Path,
            "number" => BaseType::Number,
            _ => BaseType::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub base: BaseType,
    pub is_array: bool,
    pub optional: bool,
    pub span: Span,
    pub raw_name: String,
}

impl TypeRef {
    pub fn to_ir_string(&self) -> String {
        let mut s = match self.base {
            BaseType::Unknown => self.raw_name.clone(),
            _ => self.base.as_str().to_string(),
        };
        if self.is_array {
            s.push_str("[]");
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct InputDecl {
    pub name: Ident,
    pub ty: TypeRef,
}

#[derive(Debug, Clone)]
pub struct CapCall {
    pub capability: Ident,
    pub args: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct RequireItem {
    pub capability: Ident,
    pub args: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub enum PolicyKind {
    Before,
    Deny,
}

#[derive(Debug, Clone)]
pub enum PolicyExpr {
    Or {
        exprs: Vec<PolicyExpr>,
    },
    And {
        exprs: Vec<PolicyExpr>,
    },
    Not {
        expr: Box<PolicyExpr>,
    },
    Compare {
        op: String,
        left: Box<PolicyExpr>,
        right: Box<PolicyExpr>,
    },
    BinOp {
        op: String,
        left: Box<PolicyExpr>,
        right: Box<PolicyExpr>,
    },
    MethodCall {
        receiver: Ident,
        method: Ident,
        args: Vec<PolicyExprValue>,
    },
    In {
        value: Ident,
        options: Vec<PolicyExprValue>,
    },
    Ref(Ident),
    Number(Spanned<f64>),
    NodeRef {
        stage: Ident,
        field: Ident,
        span: Span,
    },
}

/// A value in a policy expression: either a variable reference, a string literal, or a number literal.
#[derive(Debug, Clone)]
pub enum PolicyExprValue {
    Ref(Ident),
    String(Spanned<String>),
    Number(Spanned<f64>),
}

#[derive(Debug, Clone)]
pub struct PolicyDecl {
    pub kind: PolicyKind,
    pub trigger: CapCall,
    pub requires: Option<Vec<RequireItem>>,
    pub condition: Option<PolicyExpr>,
}

#[derive(Debug, Clone)]
pub enum StageAnnotation {
    Entry,
    Exit,
}

impl StageAnnotation {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageAnnotation::Entry => "entry",
            StageAnnotation::Exit => "exit",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StageInputRef {
    pub stage: Ident,
    pub field: Ident,
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BoolBranches {
    pub true_target: Ident,
    pub false_target: Ident,
}

#[derive(Debug, Clone)]
pub struct OutputField {
    pub name: Ident,
    pub ty: TypeRef,
    pub branches: Option<BoolBranches>,
}

#[derive(Debug, Clone)]
pub struct ExplicitTransition {
    pub cond: Option<PolicyExpr>,
    pub target: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StageBodyItem {
    Prompt(Spanned<String>),
    Input(Vec<StageInputRef>),
    Output(Vec<OutputField>),
    Requires(Vec<Ident>),
    Exec(ExecDecl),
    Transition(Vec<ExplicitTransition>),
}

#[derive(Debug, Clone)]
pub enum ExecValue {
    Ref(StageInputRef),
    InputRef(Ident),
    String(Spanned<String>),
}

#[derive(Debug, Clone)]
pub struct ExecArg {
    pub name: Ident,
    pub value: ExecValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExecDecl {
    pub capability: Ident,
    pub args: Vec<ExecArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StageDecl {
    pub name: Ident,
    pub annotations: Vec<StageAnnotation>,
    pub items: Vec<StageBodyItem>,
}

#[derive(Debug, Clone)]
pub struct WorkflowAst {
    pub name: Ident,
    pub inputs: Vec<InputDecl>,
    pub policies: Vec<PolicyDecl>,
    pub stages: Vec<StageDecl>,
}

/// Translate a raw symbol operator from the DSL grammar into the canonical
/// named operator used in IR `Expr::Compare` / `Expr::BinOp`.
///
/// Plan §4.1: IR `op ∈ gt|gte|lt|lte` and `op ∈ add|sub|mul|div`.
/// The parser emits raw symbols (`>`, `>=`, `+`, `-`, ...); this function
/// translates them at the lowering boundary so the IR is always canonical.
pub fn op_symbol_to_name(sym: &str) -> &str {
    match sym {
        ">" => "gt",
        ">=" => "gte",
        "<" => "lt",
        "<=" => "lte",
        "+" => "add",
        "-" => "sub",
        "*" => "mul",
        "/" => "div",
        other => other, // pass through already-named ops
    }
}
