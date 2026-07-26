//! Web-target capability/type validation (compile-time contract).
//!
//! This is the core product guarantee: the compiler raises an error at
//! compile time if a workflow requires something the web target cannot
//! provide (`fs.*`, `os.shell`, `path` types, deterministic stages).

use nemoir_ir::{Expr, Policy, Ref, StageExecution, WorkflowIr, Write};

/// Capabilities the web target supports (including deterministic-only ones).
/// Everything else is a compile-time error for `--target web`.
const WEB_ALLOWED_CAPABILITIES: &[&str] = &[
    "user.elicit",
    "user.confirm",
    "http.fetch",
    "browser.storage.read",
    "browser.storage.write",
    "browser.js.run",
    "browser.js.sandbox",
];

/// Capabilities the web target supports for deterministic (`exec:`) stages.
const WEB_DETERMINISTIC_CAPABILITIES: &[&str] = &[
    "user.elicit",
    "user.confirm",
    "http.fetch",
    "browser.storage.read",
    "browser.storage.write",
    "browser.js.run",
    "browser.js.sandbox",
];

/// Capabilities that are ONLY allowed in deterministic stages, not as
/// model-stage `requires` or policy triggers/requires.
const WEB_DETERMINISTIC_ONLY_CAPABILITIES: &[&str] = &["browser.js.run", "browser.js.sandbox"];

/// Check a capability name against the web allowlist.
fn is_web_allowed(capability: &str) -> bool {
    WEB_ALLOWED_CAPABILITIES.contains(&capability)
}

/// Check whether a capability is deterministic-only (not usable in
/// model stages or policies).
fn is_deterministic_only(capability: &str) -> bool {
    WEB_DETERMINISTIC_ONLY_CAPABILITIES.contains(&capability)
}

/// Dynamic sandbox execution is only allowed when the workflow makes the
/// approval step explicit and inspectable in IR. The policy engine supplies
/// the source preview to `user.confirm` at runtime.
fn is_sandbox_approval_policy(policy: &Policy) -> bool {
    if policy.kind != "before" || policy.trigger.capability != "browser.js.sandbox" {
        return false;
    }

    let Some(code_binding) = policy.trigger.bind.get("code") else {
        return false;
    };
    if code_binding.kind != "arg" || code_binding.name != "code" {
        return false;
    }

    policy.requires.as_ref().is_some_and(|requires| {
        requires
            .iter()
            .any(|req| req.capability == "user.confirm" && req.args.is_empty())
    })
}

/// Resolve a `browser.js.sandbox` `code` ref to its IR type and reject anything
/// that is not a non-optional `string`. The IR validator already ensures the
/// referenced input/output exists; here we additionally enforce the catalog
/// type contract (`code: String`) at the web compile-time boundary.
fn validate_sandbox_code_ref(
    r#ref: &Ref,
    node: &nemoir_ir::Node,
    input_types: &std::collections::HashMap<&str, &str>,
    writes_per_node: &std::collections::HashMap<&str, std::collections::HashMap<&str, &Write>>,
    errors: &mut Vec<String>,
) {
    let (where_desc, found_type, is_optional) = match r#ref {
        Ref::Input { name } => match input_types.get(name.as_str()) {
            Some(ty) => (format!("input '{}'", name), Some(*ty), false),
            None => return, // existence is owned by the IR validator
        },
        Ref::NodeOutput {
            node: ref_node,
            field,
        } => {
            match writes_per_node
                .get(ref_node.as_str())
                .and_then(|m| m.get(field.as_str()))
            {
                Some(w) => (
                    format!("output '{}.{}'", ref_node, field),
                    Some(w.ty.as_str()),
                    w.optional,
                ),
                None => return,
            }
        }
        Ref::Bound { .. } => return, // policy-local only; rejected by the IR validator
    };
    match found_type {
        Some("string") if !is_optional => {}
        Some(ty) => errors.push(format!(
            "web-target-error: deterministic stage \"{}\" (capability browser.js.sandbox) 'code' must resolve to a non-optional string, but {} has type '{}{}'",
            node.id, where_desc, ty, if is_optional { "?" } else { "" }
        )),
        None => {}
    }
}

/// Check whether an IR type string references the `path` type.
///
/// The DSL grammar allows any ident as a base type and array/optional
/// markers, so we do a substring check to catch `path`, `path[]`, and
/// `path?`. Only the four canonical base types (`string`, `bool`,
/// `path`, `number`) plus `string[]` variants exist in practice;
/// `string` does not contain the substring `path`.
fn type_uses_path(ty: &str) -> bool {
    ty.contains("path")
}

/// Validate an already-structurally-valid IR for the web target.
///
/// Walks every usage site (not just `ir.capabilities`) and aggregates
/// all violations into a single error so the user sees every problem
/// at once. Mirrors how `nemoir-backend-python::validate_python_field_names`
/// layers a backend-specific check on top of IR validation.
///
/// Returns `Ok(())` if the workflow is web-compatible.
pub fn validate_for_web(ir: &WorkflowIr) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    // Resolve input types by name (inputs are never optional in the IR).
    let input_types: std::collections::HashMap<&str, &str> = ir
        .inputs
        .iter()
        .map(|inp| (inp.id.as_str(), inp.ty.as_str()))
        .collect();
    // Resolve node output writes as node -> (field -> write).
    let writes_per_node: std::collections::HashMap<&str, std::collections::HashMap<&str, &Write>> =
        ir.nodes
            .iter()
            .map(|n| {
                (
                    n.id.as_str(),
                    n.writes.iter().map(|w| (w.name.as_str(), w)).collect(),
                )
            })
            .collect();

    // Top-level capabilities
    for cap in &ir.capabilities {
        if !is_web_allowed(cap) {
            errors.push(format!(
                "web-target-error: top-level capability \"{cap}\" is not supported on the web target"
            ));
        }
    }

    // Inputs: reject `path` (and `path[]`/`path?` defensively)
    for inp in &ir.inputs {
        if type_uses_path(&inp.ty) {
            errors.push(format!(
                "web-target-error: input \"{}\" has unsupported type \"{}\" on the web target",
                inp.id, inp.ty
            ));
        }
    }

    // Nodes
    for node in &ir.nodes {
        // Writes: reject path-typed outputs
        for w in &node.writes {
            if type_uses_path(&w.ty) {
                errors.push(format!(
                    "web-target-error: stage \"{}\" write \"{}\" has unsupported type \"{}\" on the web target",
                    node.id, w.name, w.ty
                ));
            }
        }

        // Requires: only web-allowed capabilities
        for cap in &node.requires {
            if !is_web_allowed(&cap.capability) {
                errors.push(format!(
                    "web-target-error: stage \"{}\" requires unsupported capability \"{}\"",
                    node.id, cap.capability
                ));
            }
            // browser.js.run is deterministic-only — reject in model stage requires
            if WEB_DETERMINISTIC_ONLY_CAPABILITIES.contains(&cap.capability.as_str())
                && matches!(node.execution, StageExecution::Model)
            {
                errors.push(format!(
                    "web-target-error: stage \"{}\" requires capability \"{}\" which is only allowed in deterministic (exec:) stages",
                    node.id, cap.capability
                ));
            }
        }

        // Deterministic (exec:) stages: allowed only for browser-supported capabilities
        if let StageExecution::Tool { capability, args } = &node.execution {
            if !WEB_DETERMINISTIC_CAPABILITIES.contains(&capability.as_str()) {
                errors.push(format!(
                    "web-target-error: deterministic stage \"{}\" uses capability \"{}\" which is not supported on the web target",
                    node.id, capability
                ));
            }
            // browser.js.run executes trusted workflow-author code, never
            // model- or input-derived JavaScript.
            if capability == "browser.js.run" {
                if let Some(code_expr) = args.get("code") {
                    if !matches!(code_expr, Expr::Literal { ty, .. } if ty == "string") {
                        errors.push(format!(
                            "web-target-error: deterministic stage \"{}\" (capability browser.js.run) requires a literal string for the 'code' argument; input/output refs are not allowed",
                            node.id
                        ));
                    }
                } else {
                    errors.push(format!(
                        "web-target-error: deterministic stage \"{}\" (capability browser.js.run) is missing the required 'code' argument",
                        node.id
                    ));
                }
            }

            // browser.js.sandbox is the intentionally dynamic path. Source may
            // be a string literal, workflow input, or prior stage output, but
            // never an arbitrary expression. Catalog declares `code: String`,
            // so the referenced value must resolve to a non-optional string;
            // its mandatory user-confirm policy is checked after this loop.
            if capability == "browser.js.sandbox" {
                match args.get("code") {
                    Some(Expr::Literal { ty, .. }) if ty == "string" => {}
                    Some(Expr::Ref { r#ref }) => {
                        validate_sandbox_code_ref(
                            r#ref,
                            node,
                            &input_types,
                            &writes_per_node,
                            &mut errors,
                        );
                    }
                    Some(_) => errors.push(format!(
                        "web-target-error: deterministic stage \"{}\" (capability browser.js.sandbox) requires 'code' to be a string literal or input/output ref",
                        node.id
                    )),
                    None => errors.push(format!(
                        "web-target-error: deterministic stage \"{}\" (capability browser.js.sandbox) is missing the required 'code' argument",
                        node.id
                    )),
                }
            }
        }
    }

    // Policies
    for policy in &ir.policies {
        // Trigger capability
        if !is_web_allowed(&policy.trigger.capability) {
            errors.push(format!(
                "web-target-error: policy \"{}\" triggers unsupported capability \"{}\"",
                policy.id, policy.trigger.capability
            ));
        }
        // `browser.js.sandbox` is the one deterministic-only capability
        // permitted as a policy trigger: it must use the required explicit
        // before/user.confirm approval form. Other deterministic-only tools
        // remain unavailable to policies.
        if is_deterministic_only(&policy.trigger.capability)
            && policy.trigger.capability != "browser.js.sandbox"
        {
            errors.push(format!(
                "web-target-error: policy \"{}\" triggers capability \"{}\" which is deterministic-stage-only and cannot be used in policies",
                policy.id, policy.trigger.capability
            ));
        }
        if policy.trigger.capability == "browser.js.sandbox" && !is_sandbox_approval_policy(policy)
        {
            errors.push(format!(
                "web-target-error: policy \"{}\" must approve browser.js.sandbox with `before browser.js.sandbox(code) requires user.confirm`",
                policy.id
            ));
        }
        // Before-policy required capabilities
        if let Some(requires) = &policy.requires {
            for req in requires {
                if !is_web_allowed(&req.capability) {
                    errors.push(format!(
                        "web-target-error: policy \"{}\" requires unsupported capability \"{}\"",
                        policy.id, req.capability
                    ));
                }
                if is_deterministic_only(&req.capability) {
                    errors.push(format!(
                        "web-target-error: policy \"{}\" requires capability \"{}\" which is deterministic-stage-only and cannot be used in policies",
                        policy.id, req.capability
                    ));
                }
            }
        }
    }

    let sandbox_is_used = ir.nodes.iter().any(|node| {
        matches!(
            &node.execution,
            StageExecution::Tool { capability, .. } if capability == "browser.js.sandbox"
        )
    });
    if sandbox_is_used && !ir.policies.iter().any(is_sandbox_approval_policy) {
        errors.push(
            "web-target-error: browser.js.sandbox requires an explicit approval policy: `before browser.js.sandbox(code) requires user.confirm`".to_string(),
        );
    }
    if sandbox_is_used && !ir.capabilities.iter().any(|cap| cap == "user.confirm") {
        errors.push(
            "web-target-error: browser.js.sandbox requires user.confirm to be declared in top-level capabilities".to_string(),
        );
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemoir_ir::*;

    /// Build a minimal valid IR with the given capabilities and node requires.
    fn make_ir(
        capabilities: Vec<String>,
        node_requires: Vec<&str>,
        execution: StageExecution,
    ) -> WorkflowIr {
        let requires = node_requires
            .iter()
            .map(|c| StageCapability {
                capability: (*c).to_string(),
            })
            .collect();
        WorkflowIr {
            ir_version: "0.1".into(),
            kind: "workflow_ir".into(),
            source: Source {
                frontend: "test".into(),
                file: "test.nemo".into(),
            },
            workflow: Workflow {
                id: "WebTest".into(),
                entry: "Start".into(),
                exits: vec!["Done".into()],
                transition_semantics: TransitionSemantics {
                    selection: "first_match_by_priority".into(),
                    no_match: "error_unless_exit".into(),
                },
            },
            inputs: vec![],
            capabilities,
            policies: vec![],
            nodes: vec![
                Node {
                    id: "Start".into(),
                    annotations: vec!["entry".into()],
                    prompt: "start".into(),
                    reads: vec![],
                    writes: vec![Write {
                        name: "out".into(),
                        ty: "string".into(),
                        optional: false,
                    }],
                    requires,
                    transitions: vec![Transition {
                        to: "Done".into(),
                        priority: 0,
                        reason: "fallthrough".into(),
                        guard: Guard::Always,
                    }],
                    execution,
                },
                Node {
                    id: "Done".into(),
                    annotations: vec!["exit".into()],
                    prompt: "done".into(),
                    reads: vec![],
                    writes: vec![],
                    requires: vec![],
                    transitions: vec![],
                    execution: StageExecution::Model,
                },
            ],
        }
    }

    fn sandbox_approval_policy() -> Policy {
        let mut bind = indexmap::IndexMap::new();
        bind.insert(
            "code".into(),
            BindArg {
                kind: "arg".into(),
                name: "code".into(),
            },
        );
        Policy {
            id: "before browser.js.sandbox(code) requires user.confirm".into(),
            kind: "before".into(),
            trigger: Trigger {
                capability: "browser.js.sandbox".into(),
                bind,
            },
            requires: Some(vec![RequiredCapability {
                capability: "user.confirm".into(),
                args: indexmap::IndexMap::new(),
            }]),
            condition: None,
        }
    }

    #[test]
    fn allows_model_only_workflow_with_user_capabilities() {
        let ir = make_ir(
            vec!["user.elicit".into(), "user.confirm".into()],
            vec!["user.elicit"],
            StageExecution::Model,
        );
        nemoir_ir::validate::validate(&ir).expect("IR must be structurally valid");
        validate_for_web(&ir).expect("model-only workflow with user.* should pass");
    }

    #[test]
    fn rejects_fs_read_capability() {
        let ir = make_ir(
            vec!["fs.read".into()],
            vec!["fs.read"],
            StageExecution::Model,
        );
        let err = validate_for_web(&ir).expect_err("fs.read should be rejected");
        assert!(err.iter().any(|e| e.contains("fs.read")), "{err:?}");
    }

    #[test]
    fn rejects_fs_write_capability() {
        let ir = make_ir(
            vec!["fs.write".into()],
            vec!["fs.write"],
            StageExecution::Model,
        );
        let err = validate_for_web(&ir).expect_err("fs.write should be rejected");
        assert!(err.iter().any(|e| e.contains("fs.write")), "{err:?}");
    }

    #[test]
    fn rejects_os_shell_capability() {
        let ir = make_ir(
            vec!["os.shell".into()],
            vec!["os.shell"],
            StageExecution::Model,
        );
        let err = validate_for_web(&ir).expect_err("os.shell should be rejected");
        assert!(err.iter().any(|e| e.contains("os.shell")), "{err:?}");
    }

    #[test]
    fn allows_deterministic_tool_stage_with_user_confirm() {
        let ir = make_ir(
            vec!["user.confirm".into()],
            vec!["user.confirm"],
            StageExecution::Tool {
                capability: "user.confirm".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "message".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String("ok?".into()),
                        },
                    );
                    m
                },
            },
        );
        validate_for_web(&ir).expect("deterministic user.confirm should be allowed on web");
    }

    #[test]
    fn rejects_deterministic_tool_stage_with_unsupported_capability() {
        let ir = make_ir(
            vec!["fs.read".into()],
            vec!["fs.read"],
            StageExecution::Tool {
                capability: "fs.read".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "path".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String("test.txt".into()),
                        },
                    );
                    m
                },
            },
        );
        let err = validate_for_web(&ir).expect_err("fs.read tool stage should be rejected");
        assert!(err.iter().any(|e| e.contains("fs.read")), "{err:?}");
    }

    #[test]
    fn rejects_path_input_type() {
        let mut ir = make_ir(vec![], vec![], StageExecution::Model);
        ir.inputs = vec![nemoir_ir::Input {
            id: "cwd".into(),
            ty: "path".into(),
        }];
        let err = validate_for_web(&ir).expect_err("path input should be rejected");
        assert!(
            err.iter().any(|e| e.contains("cwd") && e.contains("path")),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_path_write_type() {
        let mut ir = make_ir(vec![], vec![], StageExecution::Model);
        ir.nodes[0].writes = vec![Write {
            name: "file".into(),
            ty: "path".into(),
            optional: false,
        }];
        let err = validate_for_web(&ir).expect_err("path write should be rejected");
        assert!(
            err.iter().any(|e| e.contains("file") && e.contains("path")),
            "{err:?}"
        );
    }

    #[test]
    fn aggregates_multiple_errors() {
        let ir = make_ir(
            vec!["fs.read".into(), "os.shell".into()],
            vec!["fs.read", "os.shell"],
            StageExecution::Model,
        );
        let err = validate_for_web(&ir).expect_err("should produce multiple errors");
        assert!(err.len() >= 2, "expected aggregated errors, got {err:?}");
        assert!(err.iter().any(|e| e.contains("fs.read")));
        assert!(err.iter().any(|e| e.contains("os.shell")));
    }

    #[test]
    fn allows_number_input_and_write() {
        let mut ir = make_ir(vec![], vec![], StageExecution::Model);
        ir.inputs = vec![nemoir_ir::Input {
            id: "eps".into(),
            ty: "number".into(),
        }];
        ir.nodes[0].writes = vec![Write {
            name: "score".into(),
            ty: "number".into(),
            optional: false,
        }];
        validate_for_web(&ir).expect("number types should be allowed on web");
    }

    #[test]
    fn rejects_browser_js_run_with_ref_code() {
        // browser.js.run requires code to be a compile-time string literal —
        // input/output refs are not allowed for code.
        let ir = make_ir(
            vec!["browser.js.run".into()],
            vec!["browser.js.run"],
            StageExecution::Tool {
                capability: "browser.js.run".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Ref {
                            r#ref: nemoir_ir::Ref::NodeOutput {
                                node: "SomeStage".into(),
                                field: "generated".into(),
                            },
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String("{}".into()),
                        },
                    );
                    m
                },
            },
        );
        let err = validate_for_web(&ir).expect_err("ref code should be rejected");
        assert!(err.iter().any(|e| e.contains("literal string")), "{err:?}");
    }

    #[test]
    fn allows_browser_js_sandbox_with_dynamic_ref_and_approval_policy() {
        let mut ir = make_ir(
            vec!["browser.js.sandbox".into(), "user.confirm".into()],
            vec!["browser.js.sandbox"],
            StageExecution::Tool {
                capability: "browser.js.sandbox".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Ref {
                            r#ref: Ref::Input {
                                name: "user_code".into(),
                            },
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "json".into(),
                            value: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        },
                    );
                    m
                },
            },
        );
        ir.inputs.push(Input {
            id: "user_code".into(),
            ty: "string".into(),
        });
        ir.policies.push(sandbox_approval_policy());

        validate_for_web(&ir)
            .expect("dynamic sandbox source with explicit user confirmation should pass");
    }

    fn sandbox_with_code_input_ref(input_type: &str) -> WorkflowIr {
        let mut ir = make_ir(
            vec!["browser.js.sandbox".into(), "user.confirm".into()],
            vec!["browser.js.sandbox"],
            StageExecution::Tool {
                capability: "browser.js.sandbox".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Ref {
                            r#ref: Ref::Input {
                                name: "user_code".into(),
                            },
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "json".into(),
                            value: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        },
                    );
                    m
                },
            },
        );
        ir.inputs.push(Input {
            id: "user_code".into(),
            ty: input_type.into(),
        });
        ir.policies.push(sandbox_approval_policy());
        ir
    }

    #[test]
    fn rejects_browser_js_sandbox_with_json_typed_code_input() {
        let ir = sandbox_with_code_input_ref("json");
        let err = validate_for_web(&ir).expect_err("json code input should be rejected");
        assert!(
            err.iter().any(|e| e.contains("non-optional string")),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_browser_js_sandbox_with_string_array_typed_code_input() {
        let ir = sandbox_with_code_input_ref("string[]");
        let err = validate_for_web(&ir).expect_err("string[] code input should be rejected");
        assert!(
            err.iter().any(|e| e.contains("non-optional string")),
            "{err:?}"
        );
    }

    fn sandbox_with_code_node_output_ref(write_type: &str, optional: bool) -> WorkflowIr {
        // The exit node ("Done") produces a write consumed as the sandbox
        // stage's `code` ref. Graph reachability is owned by the IR validator;
        // validate_for_web only inspects the declared types.
        let mut ir = make_ir(
            vec!["browser.js.sandbox".into(), "user.confirm".into()],
            vec!["browser.js.sandbox"],
            StageExecution::Tool {
                capability: "browser.js.sandbox".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Ref {
                            r#ref: Ref::NodeOutput {
                                node: "Done".into(),
                                field: "code".into(),
                            },
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "json".into(),
                            value: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        },
                    );
                    m
                },
            },
        );
        ir.nodes[1].writes = vec![Write {
            name: "code".into(),
            ty: write_type.into(),
            optional,
        }];
        ir.policies.push(sandbox_approval_policy());
        ir
    }

    #[test]
    fn rejects_browser_js_sandbox_with_optional_code_node_output() {
        let ir = sandbox_with_code_node_output_ref("string", true);
        let err = validate_for_web(&ir).expect_err("optional code output should be rejected");
        assert!(
            err.iter().any(|e| e.contains("non-optional string")),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_browser_js_sandbox_with_json_typed_code_node_output() {
        let ir = sandbox_with_code_node_output_ref("json", false);
        let err = validate_for_web(&ir).expect_err("json code output should be rejected");
        assert!(
            err.iter().any(|e| e.contains("non-optional string")),
            "{err:?}"
        );
    }

    #[test]
    fn allows_browser_js_sandbox_with_non_optional_string_code_node_output() {
        let ir = sandbox_with_code_node_output_ref("string", false);
        validate_for_web(&ir).expect("non-optional string code output should pass");
    }

    #[test]
    fn rejects_browser_js_sandbox_without_approval_policy() {
        let ir = make_ir(
            vec!["browser.js.sandbox".into()],
            vec!["browser.js.sandbox"],
            StageExecution::Tool {
                capability: "browser.js.sandbox".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String("return { ok: true };".into()),
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "json".into(),
                            value: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        },
                    );
                    m
                },
            },
        );

        let err = validate_for_web(&ir).expect_err("sandbox must require approval policy");
        assert!(
            err.iter().any(|e| e.contains("explicit approval policy")),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_browser_js_sandbox_in_model_stage() {
        let ir = make_ir(
            vec!["browser.js.sandbox".into()],
            vec!["browser.js.sandbox"],
            StageExecution::Model,
        );

        let err = validate_for_web(&ir).expect_err("sandbox must be deterministic-only");
        assert!(
            err.iter()
                .any(|e| e.contains("only allowed in deterministic")),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_malformed_browser_js_sandbox_approval_policy() {
        let mut ir = make_ir(vec![], vec![], StageExecution::Model);
        ir.policies.push(Policy {
            id: "bad sandbox policy".into(),
            kind: "before".into(),
            trigger: Trigger {
                capability: "browser.js.sandbox".into(),
                bind: indexmap::IndexMap::new(),
            },
            requires: Some(vec![]),
            condition: None,
        });

        let err = validate_for_web(&ir).expect_err("malformed sandbox policy must be rejected");
        assert!(
            err.iter()
                .any(|e| e.contains("must approve browser.js.sandbox")),
            "{err:?}"
        );
    }

    #[test]
    fn allows_browser_js_run_with_literal_code() {
        let ir = make_ir(
            vec!["browser.js.run".into()],
            vec!["browser.js.run"],
            StageExecution::Tool {
                capability: "browser.js.run".into(),
                args: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert(
                        "code".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String(
                                "return { result: input.x + input.y };".into(),
                            ),
                        },
                    );
                    m.insert(
                        "input".into(),
                        Expr::Literal {
                            ty: "string".into(),
                            value: serde_yaml::Value::String("{}".into()),
                        },
                    );
                    m
                },
            },
        );
        validate_for_web(&ir).expect("literal code should be allowed");
    }
}
