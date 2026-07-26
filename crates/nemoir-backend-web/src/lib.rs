//! Web backend for NemoIR — compiles a validated `WorkflowIr` into a
//! static Vite/TypeScript browser application.
//!
//! Mirrors `nemoir-backend-python` in API shape and pipeline:
//!   `validate(ir)?` → `validate_for_web(ir)?` → derive package dir → emit files.
//!
//! The generated app loads WebLLM in a Web Worker and runs the workflow
//! capability contract (`validate_for_web`) is the core product guarantee:
//! the compiler rejects workflows that require capabilities the browser
//! cannot provide (`fs.*`, `os.shell`, `path` types, deterministic stages).

pub mod emit;
pub mod escape;
pub mod naming;
pub mod options;
pub mod translate;
pub mod validate_web;

pub use options::WebBackendOptions;

use std::path::PathBuf;

/// Errors raised by the web backend.
#[derive(Debug, thiserror::Error)]
pub enum WebBackendError {
    /// Structural IR validation failed (defensive re-run of
    /// `nemoir_ir::validate::validate`).
    #[error("IR validation failed:\n{0}")]
    ValidationFailed(#[from] nemoir_ir::validate::ValidationErrors),

    /// The workflow uses capabilities or types unsupported on the web target.
    /// The string contains all aggregated violations (one per line).
    #[error("workflow is not compatible with the web target:\n{0}")]
    UnsupportedForWebTarget(String),

    /// The workflow id cannot be converted to a valid kebab-case package
    /// directory name.
    #[error("workflow id '{0}' cannot be converted to a valid web package directory name")]
    InvalidWorkflowId(String),

    /// An IR name that would be emitted as a TypeScript identifier is not
    /// a valid TS identifier.
    #[error("IR name '{0}' is not a valid TypeScript identifier")]
    InvalidWebField(String),

    /// JSON serialization of the IR failed.
    #[error("JSON serialization failed: {0}")]
    JsonSerialization(String),
}

/// A single generated file with its path relative to the output directory.
#[derive(Debug)]
pub struct GeneratedFile {
    pub relative_path: PathBuf,
    pub content: String,
}

/// The complete set of generated files for a web package.
#[derive(Debug)]
pub struct GeneratedPackage {
    /// Kebab-case package directory name (e.g. `judge-candidate`).
    pub package_name: String,
    pub files: Vec<GeneratedFile>,
}

/// Generate a complete web app package from a validated `WorkflowIr`.
///
/// Pipeline:
/// 1. Re-run structural IR validation (defensive backstop).
/// 2. Run web-target validation (`validate_for_web`).
/// 3. Derive the kebab-case package directory name.
/// 4. Emit all files (templates + workflow.json + agent.ts).
///
/// Returns the package name and all files with paths relative to the
/// caller's output directory. The caller is responsible for writing them
/// to disk (the CLI does this; tests can inspect the struct directly).
pub fn generate_package(
    ir: &nemoir_ir::WorkflowIr,
    options: &WebBackendOptions,
) -> Result<GeneratedPackage, WebBackendError> {
    // 1. Structural IR validation (defensive backstop — the CLI already
    //    ran this, but backends re-run it so they are safe as libraries).
    nemoir_ir::validate::validate(ir).map_err(WebBackendError::from)?;

    // 2. Web-target capability/type validation (the core compile-time contract).
    validate_web::validate_for_web(ir)
        .map_err(|errors| WebBackendError::UnsupportedForWebTarget(errors.join("\n")))?;

    // 3. Derive the kebab-case package directory name.
    let package_name = naming::package_dir(&ir.workflow.id)
        .ok_or_else(|| WebBackendError::InvalidWorkflowId(ir.workflow.id.clone()))?;

    // 4. Emit files.
    let version = options.package_version.as_deref().unwrap_or("0.1.0");
    let runtime_dep = options.runtime_dependency.as_deref().unwrap_or("^0.3.1");
    let files = translate::build_files(ir, &package_name, version, runtime_dep)?;

    Ok(GeneratedPackage {
        package_name,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemoir_ir::*;

    fn judge_candidate_ir() -> WorkflowIr {
        let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/judge_candidate-ir.yml");
        serde_yaml::from_str(source).expect("should parse judge_candidate-ir.yml")
    }

    fn coding_agent_ir() -> WorkflowIr {
        let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml");
        serde_yaml::from_str(source).expect("should parse coding-agent-ir.yml")
    }

    fn hint_tutor_ir() -> WorkflowIr {
        let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/hint_tutor-ir.yml");
        serde_yaml::from_str(source).expect("should parse hint_tutor-ir.yml")
    }

    fn minimal_web_ir() -> WorkflowIr {
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
            inputs: vec![nemoir_ir::Input {
                id: "task".into(),
                ty: "string".into(),
            }],
            capabilities: vec![],
            policies: vec![],
            nodes: vec![
                Node {
                    id: "Start".into(),
                    annotations: vec!["entry".into()],
                    prompt: "start".into(),
                    reads: vec![],
                    writes: vec![Write {
                        name: "summary".into(),
                        ty: "string".into(),
                        optional: false,
                    }],
                    requires: vec![],
                    transitions: vec![Transition {
                        to: "Done".into(),
                        priority: 0,
                        reason: "fallthrough".into(),
                        guard: Guard::Always,
                    }],
                    execution: StageExecution::Model,
                },
                Node {
                    id: "Done".into(),
                    annotations: vec!["exit".into()],
                    prompt: "done".into(),
                    reads: vec![],
                    writes: vec![Write {
                        name: "result".into(),
                        ty: "string".into(),
                        optional: false,
                    }],
                    requires: vec![],
                    transitions: vec![],
                    execution: StageExecution::Model,
                },
            ],
        }
    }

    #[test]
    fn generate_package_judge_candidate_succeeds() {
        let ir = judge_candidate_ir();
        let pkg = generate_package(&ir, &WebBackendOptions::default()).expect("should generate");
        assert_eq!(pkg.package_name, "judge-candidate");
        assert!(!pkg.files.is_empty());
    }

    #[test]
    fn generate_package_coding_agent_fails_with_capability_errors() {
        let ir = coding_agent_ir();
        let result = generate_package(&ir, &WebBackendOptions::default());
        assert!(
            matches!(result, Err(WebBackendError::UnsupportedForWebTarget(_))),
            "expected UnsupportedForWebTarget, got {result:?}"
        );
        if let Err(WebBackendError::UnsupportedForWebTarget(msg)) = result {
            assert!(msg.contains("fs.read"), "should mention fs.read: {msg}");
            assert!(msg.contains("fs.write"), "should mention fs.write: {msg}");
            assert!(msg.contains("os.shell"), "should mention os.shell: {msg}");
            assert!(msg.contains("path"), "should mention path type: {msg}");
        }
    }

    #[test]
    fn generate_package_rejects_invalid_workflow_id() {
        let mut ir = minimal_web_ir();
        ir.workflow.id = "123abc".into();
        let result = generate_package(&ir, &WebBackendOptions::default());
        assert!(
            matches!(result, Err(WebBackendError::InvalidWorkflowId(_))),
            "expected InvalidWorkflowId, got {result:?}"
        );
    }

    #[test]
    fn generate_package_rejects_invalid_input_id() {
        let mut ir = minimal_web_ir();
        ir.inputs = vec![nemoir_ir::Input {
            id: "task-class".into(),
            ty: "string".into(),
        }];
        let result = generate_package(&ir, &WebBackendOptions::default());
        assert!(
            matches!(result, Err(WebBackendError::InvalidWebField(_))),
            "expected InvalidWebField, got {result:?}"
        );
    }

    #[test]
    fn minimal_web_package_has_agent_with_typed_io() {
        let ir = minimal_web_ir();
        let pkg = generate_package(&ir, &WebBackendOptions::default()).unwrap();
        let agent = pkg
            .files
            .iter()
            .find(|f| f.relative_path.to_string_lossy().ends_with("agent.ts"))
            .expect("agent.ts should exist");
        assert!(agent.content.contains("task: string"));
        assert!(agent.content.contains("result: string"));
        assert!(agent.content.contains("export class Agent"));
    }

    #[test]
    fn generate_package_hint_tutor_succeeds_with_typed_io() {
        let ir = hint_tutor_ir();
        let pkg = generate_package(&ir, &WebBackendOptions::default()).expect("should generate");
        assert_eq!(pkg.package_name, "hint-tutor");

        let agent = pkg
            .files
            .iter()
            .find(|f| f.relative_path.to_string_lossy().ends_with("agent.ts"))
            .expect("agent.ts should exist");
        // Input types
        assert!(
            agent.content.contains("task: string"),
            "AgentInput should have task: string"
        );
        assert!(
            agent.content.contains("learner_code: string"),
            "AgentInput should have learner_code: string"
        );
        assert!(
            agent.content.contains("failure_report: string"),
            "AgentInput should have failure_report: string"
        );
        // Output types — single-exit stage so fields are non-optional
        assert!(
            agent.content.contains("hint: string"),
            "AgentOutput should have hint: string"
        );
        assert!(
            agent.content.contains("key_points: string[]"),
            "AgentOutput should have key_points: string[]"
        );
        // Required capabilities include user.elicit (used by AskClarify)
        assert!(
            agent.content.contains("\"user.elicit\""),
            "REQUIRED_CAPABILITIES should include user.elicit"
        );
        assert!(
            agent.content.contains("HintTutor"),
            "agent.ts should carry the workflow id"
        );
    }
}
