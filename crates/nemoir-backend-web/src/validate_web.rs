//! Web-target capability/type validation (compile-time contract).
//!
//! This is the core product guarantee: the compiler raises an error at
//! compile time if a workflow requires something the web target cannot
//! provide (`fs.*`, `os.shell`, `path` types, deterministic stages).

use nemoir_ir::{StageExecution, WorkflowIr};

/// Capabilities the web MVP supports. Everything else is a compile-time
/// error for `--target web`.
const WEB_ALLOWED_CAPABILITIES: &[&str] = &["user.elicit", "user.confirm"];

/// Check a capability name against the web allowlist.
fn is_web_allowed(capability: &str) -> bool {
    WEB_ALLOWED_CAPABILITIES.contains(&capability)
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

        // Requires: only user.elicit / user.confirm
        for cap in &node.requires {
            if !is_web_allowed(&cap.capability) {
                errors.push(format!(
                    "web-target-error: stage \"{}\" requires unsupported capability \"{}\"",
                    node.id, cap.capability
                ));
            }
        }

        // Deterministic (exec:) stages: unsupported in the MVP
        if !matches!(node.execution, StageExecution::Model) {
            errors.push(format!(
                "web-target-error: deterministic stage \"{}\" cannot run on the web target (exec stages are unsupported in the MVP)",
                node.id
            ));
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
        // Before-policy required capabilities
        if let Some(requires) = &policy.requires {
            for req in requires {
                if !is_web_allowed(&req.capability) {
                    errors.push(format!(
                        "web-target-error: policy \"{}\" requires unsupported capability \"{}\"",
                        policy.id, req.capability
                    ));
                }
            }
        }
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
    fn rejects_deterministic_tool_stage() {
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
        let err = validate_for_web(&ir).expect_err("tool stage should be rejected");
        assert!(
            err.iter().any(|e| e.contains("deterministic stage")),
            "{err:?}"
        );
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
}
