use nemoir_backend_python::{
    generate_package, GeneratedPackage, PythonBackendError, PythonBackendOptions,
};
use nemoir_ir::*;

fn valid_minimal_ir() -> WorkflowIr {
    WorkflowIr {
        ir_version: "0.1".into(),
        kind: "workflow_ir".into(),
        source: Source {
            frontend: "test".into(),
            file: "test.nemo".into(),
        },
        workflow: Workflow {
            id: "Minimal".into(),
            entry: "Start".into(),
            exits: vec!["Done".into()],
            transition_semantics: TransitionSemantics {
                selection: "first_match_by_priority".into(),
                no_match: "error_unless_exit".into(),
            },
        },
        inputs: vec![],
        capabilities: vec![],
        policies: vec![],
        nodes: vec![
            Node {
                id: "Start".into(),
                annotations: vec!["entry".into()],
                prompt: "start node".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![Transition {
                    to: "Done".into(),
                    priority: 0,
                    reason: "fallthrough".into(),
                    guard: Guard::Always,
                }],
            },
            Node {
                id: "Done".into(),
                annotations: vec!["exit".into()],
                prompt: "done node".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![],
            },
        ],
    }
}

fn coding_agent_ir() -> WorkflowIr {
    let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml");
    serde_yaml::from_str(source).expect("should parse coding-agent-ir.yml")
}

fn file_content<'a>(pkg: &'a GeneratedPackage, name: &str) -> &'a str {
    pkg.files
        .iter()
        .find(|f| f.relative_path.to_string_lossy() == name)
        .map(|f| f.content.as_str())
        .unwrap_or_else(|| panic!("missing generated file: {}", name))
}

// ---------------------------------------------------------------------------
// Package structure: minimal IR + coding-agent fixture
// ---------------------------------------------------------------------------

#[test]
fn generate_minimal_package_has_all_expected_files() {
    let ir = valid_minimal_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).expect("should generate");

    assert_eq!(pkg.package_name, "minimal");
    assert_eq!(pkg.distribution_name, "minimal");

    for path in &[
        "pyproject.toml",
        "minimal/__init__.py",
        "minimal/_manifest.py",
        "minimal/_agent.py",
        "minimal/types.py",
    ] {
        assert!(
            pkg.files
                .iter()
                .any(|f| f.relative_path.to_string_lossy() == *path),
            "expected file path {} in generated package",
            path
        );
    }
}

#[test]
fn generate_coding_agent_package_surface() {
    let ir = coding_agent_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).expect("should generate");

    assert_eq!(pkg.package_name, "coding_agent");
    assert_eq!(pkg.distribution_name, "coding-agent");

    // pyproject surface
    let pyproject = file_content(&pkg, "pyproject.toml");
    assert!(pyproject.contains("name = \"coding-agent\""));
    assert!(pyproject.contains("\"nemoir-runtime>=0.1.0\""));
    assert!(pyproject.contains("packages = [\"coding_agent\"]"));
    assert!(pyproject.contains("version = \"0.1.0\""));

    // __init__.py re-exports
    let init_src = file_content(&pkg, "coding_agent/__init__.py");
    assert!(init_src
        .contains("from nemoir_runtime import RunOptions, Tool, ToolContext, ToolRegistry, tool"));
    assert!(init_src.contains("from coding_agent._agent import Agent"));
    assert!(init_src.contains("from coding_agent._manifest import WORKFLOW_MANIFEST"));
    assert!(
        init_src.contains("from coding_agent.types import AgentInput, AgentOutput, AgentResult")
    );
    for sym in &[
        "Agent",
        "AgentInput",
        "AgentOutput",
        "AgentResult",
        "RunOptions",
        "Tool",
        "ToolRegistry",
        "tool",
    ] {
        assert!(
            init_src.contains(&format!("\"{}\"", sym)),
            "init should re-export {}",
            sym
        );
    }

    // _manifest.py: 7 stages + correct workflow header
    let manifest = file_content(&pkg, "coding_agent/_manifest.py");
    assert_eq!(manifest.matches("StageSpec(").count(), 7);
    assert!(manifest.contains("workflow_id=\"CodingAgent\""));
    assert!(manifest.contains("entry_stage_id=\"Triage\""));
    assert!(manifest.contains("exit_stage_ids=frozenset({\"Fin\"})"));
    assert!(manifest.contains("REQUIRED_CAPABILITIES = frozenset({"));
    assert!(manifest.contains("\"fs.read\""));
    assert!(manifest.contains("\"user.elicit\""));
    assert!(manifest.contains("\"user.confirm\""));
    assert!(manifest.contains("\"fs.write\""));
    assert!(manifest.contains("\"os.shell\""));
    for stage_id in &[
        "Triage", "Clarify", "Plan", "Propose", "Apply", "Verify", "Fin",
    ] {
        assert!(
            manifest.contains(&format!("StageSpec(id=\"{}\"", stage_id)),
            "manifest should declare StageSpec(id=\"{}\")",
            stage_id
        );
    }
    // The before-fs.write policy should appear with its full id verbatim.
    assert!(manifest
        .contains("PolicySpec(id=\"before fs.write(path) requires fs.read(path), user.confirm\""));
    assert!(manifest.contains("TriggerSpec(capability=\"fs.write\""));
    assert!(manifest.contains("TriggerSpec(capability=\"fs.read\""));
    assert!(manifest.contains("RequiredCapabilitySpec(capability=\"fs.read\""));
    assert!(manifest.contains("RequiredCapabilitySpec(capability=\"user.confirm\""));
    // Both deny policies + their condition bodies uses ExprSpec(kind="not")
    // wrapping a method_call (regression guard for F1).
    assert!(manifest.contains("PolicySpec(id=\"deny fs.read(path) if not cwd.contains(path)\""));
    assert!(manifest.contains("PolicySpec(id=\"deny fs.write(path) if not cwd.contains(path)\""));
    assert!(manifest.contains("ExprSpec(kind=\"not\""));
    assert!(manifest.contains("method=\"contains\""));
    // F1 regression: the deny policies' single-arg method_call MUST emit a
    // trailing comma so args= is a 1-tuple, not a parenthesized ExprSpec.
    assert!(
        manifest
            .contains("args=(ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"bound\", name=\"path\")),)"),
        "single-arg method_call must emit a 1-tuple with trailing comma"
    );

    // types.py: required AgentInput fields and a non-optional AgentOutput.summary
    let types = file_content(&pkg, "coding_agent/types.py");
    assert!(types.contains("class AgentInput:"));
    assert!(types.contains("    task: str"));
    assert!(types.contains("    cwd: Path"));
    assert!(types.contains("class AgentOutput:"));
    assert!(types.contains("    summary: str"));
    assert!(types.contains("class AgentResult:"));
    assert!(types.contains("    output: AgentOutput"));

    // _agent.py: package-private references and generated converters
    let agent = file_content(&pkg, "coding_agent/_agent.py");
    assert!(agent.contains("from coding_agent._manifest import ("));
    assert!(agent.contains("from coding_agent.types import AgentInput, AgentOutput, AgentResult"));
    assert!(agent.contains("\"task\": inputs.task,"));
    assert!(agent.contains("\"cwd\": inputs.cwd,"));
    assert!(agent.contains("summary=output[\"summary\"],"));
    assert!(agent.contains("raise NotImplementedError(msg)"));
    assert!(agent.contains("Agent.run() requires the Phase 4 model adapter"));
}

#[test]
fn generate_minimal_manifest_round_trip() {
    let ir = valid_minimal_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let src = file_content(&pkg, "minimal/_manifest.py");

    assert!(src.contains("WORKFLOW_ID = \"Minimal\""));
    assert!(src.contains("ENTRY_STAGE_ID = \"Start\""));
    assert!(src.contains("EXIT_STAGE_IDS = frozenset({\"Done\"})"));
    assert!(src.contains("REQUIRED_CAPABILITIES = frozenset({})"));
    assert!(src.contains("workflow_id=\"Minimal\""));
    assert!(src.contains("entry_stage_id=\"Start\""));
    assert!(src.contains("exit_stage_ids=frozenset({\"Done\"})"));
    assert!(src.contains("inputs=()"));
    assert!(src.contains("capabilities=frozenset({})"));
    assert!(src.contains("policies=()"));
    assert!(src.contains("StageSpec(id=\"Start\""));
    assert!(src.contains("StageSpec(id=\"Done\""));
}

#[test]
fn generate_minimal_types_round_trip() {
    let ir = valid_minimal_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let src = file_content(&pkg, "minimal/types.py");

    // Minimal IR has no inputs and an exit (Done) with no writes.
    assert!(src.contains("class AgentInput:\n    pass"));
    assert!(src.contains("class AgentOutput:\n    pass"));
    assert!(src.contains("class AgentResult:\n    output: AgentOutput"));
}

// ---------------------------------------------------------------------------
// F1 regression: single-arg method-call emits a 1-tuple
// ---------------------------------------------------------------------------

#[test]
fn coding_agent_deny_policies_emit_method_call_args_as_tuple() {
    // This is the F1 regression guard. The coding-agent fixture has two
    // `deny ... cwd.contains(path)` policies; both have a single-arg
    // method_call that the runtime evaluates as `expr.args` (an iterable).
    let ir = coding_agent_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let manifest = file_content(&pkg, "coding_agent/_manifest.py");

    // The 1-tuple form `args=(ExprSpec(...),)` MUST be present at least twice
    // (one per deny policy).
    let one_tuple_count = manifest
        .matches("args=(ExprSpec(kind=\"ref\", ref=RefSpec(kind=\"bound\", name=\"path\")),)")
        .count();
    assert_eq!(
        one_tuple_count, 2,
        "expected 2 single-arg 1-tuple method_call sites (one per deny policy)"
    );
}

// ---------------------------------------------------------------------------
// F2 regression: multi-exit _output_from_mapping uses output.get everywhere
// ---------------------------------------------------------------------------

fn multi_exit_ir() -> WorkflowIr {
    WorkflowIr {
        ir_version: "0.1".into(),
        kind: "workflow_ir".into(),
        source: Source {
            frontend: "test".into(),
            file: "test.nemo".into(),
        },
        workflow: Workflow {
            id: "MultiExit".into(),
            entry: "Start".into(),
            exits: vec!["Done1".into(), "Done2".into()],
            transition_semantics: TransitionSemantics {
                selection: "first_match_by_priority".into(),
                no_match: "error_unless_exit".into(),
            },
        },
        inputs: vec![],
        capabilities: vec![],
        policies: vec![],
        nodes: vec![
            Node {
                id: "Start".into(),
                annotations: vec!["entry".into()],
                prompt: "start".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![
                    Transition {
                        to: "Done1".into(),
                        priority: 0,
                        reason: "r0".into(),
                        guard: Guard::Always,
                    },
                    Transition {
                        to: "Done2".into(),
                        priority: 1,
                        reason: "r1".into(),
                        guard: Guard::Always,
                    },
                ],
            },
            Node {
                id: "Done1".into(),
                annotations: vec!["exit".into()],
                prompt: "d1".into(),
                reads: vec![],
                writes: vec![Write {
                    name: "summary_a".into(),
                    ty: "string".into(),
                    optional: false,
                }],
                requires: vec![],
                transitions: vec![],
            },
            Node {
                id: "Done2".into(),
                annotations: vec!["exit".into()],
                prompt: "d2".into(),
                reads: vec![],
                writes: vec![Write {
                    name: "summary_b".into(),
                    ty: "string".into(),
                    optional: false,
                }],
                requires: vec![],
                transitions: vec![],
            },
        ],
    }
}

#[test]
fn multi_exit_types_make_every_field_optional() {
    let ir = multi_exit_ir();
    nemoir_ir::validate::validate(&ir).expect("multi-exit fixture must validate");
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let types = file_content(&pkg, "multi_exit/types.py");
    assert!(types.contains("    summary_a: Optional[str] = None\n"));
    assert!(types.contains("    summary_b: Optional[str] = None\n"));
}

#[test]
fn multi_exit_output_converter_uses_get_for_every_field() {
    let ir = multi_exit_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let agent = file_content(&pkg, "multi_exit/_agent.py");

    // Every multi-exit output field MUST use output.get(...) to avoid KeyError
    // when only one of the two exits was taken (its fields are absent from the
    // runtime output mapping).
    assert!(
        agent.contains("summary_a=output.get(\"summary_a\"),"),
        "expected summary_a=output.get(...), got:\n{}",
        agent
    );
    assert!(
        agent.contains("summary_b=output.get(\"summary_b\"),"),
        "expected summary_b=output.get(...), got:\n{}",
        agent
    );

    // And MUST NOT contain the indexing form for either field.
    assert!(
        !agent.contains("summary_a=output[\"summary_a\"]"),
        "multi-exit converter must not use indexing"
    );
    assert!(
        !agent.contains("summary_b=output[\"summary_b\"]"),
        "multi-exit converter must not use indexing"
    );
}

#[test]
fn single_exit_optional_write_still_uses_get() {
    // A single-exit workflow with an optional write should still use output.get
    // for the optional field (the existing behavior).
    let mut ir = valid_minimal_ir();
    ir.nodes[1].writes = vec![
        Write {
            name: "summary".into(),
            ty: "string".into(),
            optional: false,
        },
        Write {
            name: "extra".into(),
            ty: "string".into(),
            optional: true,
        },
    ];
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();
    let agent = file_content(&pkg, "minimal/_agent.py");
    assert!(agent.contains("summary=output[\"summary\"],"));
    assert!(agent.contains("extra=output.get(\"extra\"),"));
}

// ---------------------------------------------------------------------------
// Validation: invalid IR / invalid workflow id / invalid Python field names
// ---------------------------------------------------------------------------

#[test]
fn generate_package_rejects_invalid_ir() {
    let mut ir = valid_minimal_ir();
    ir.workflow.entry = "DoesNotExist".into();
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::ValidationFailed(_))),
        "expected ValidationFailed, got {:?}",
        result
    );
}

#[test]
fn generate_package_rejects_invalid_workflow_id() {
    let mut ir = valid_minimal_ir();
    ir.workflow.id = "123abc".into();
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::InvalidWorkflowId(_))),
        "expected InvalidWorkflowId, got {:?}",
        result
    );
}

#[test]
fn generate_package_rejects_workflow_id_that_becomes_python_keyword() {
    // F6: `Class` camelCase-converts to `class`, which is a Python keyword.
    // Codegen must reject it rather than producing `from class._agent import Agent`.
    let mut ir = valid_minimal_ir();
    ir.workflow.id = "Class".into();
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::InvalidWorkflowId(_))),
        "expected InvalidWorkflowId for keyword package name, got {:?}",
        result
    );
}

#[test]
fn generate_package_rejects_keyword_input_id() {
    let mut ir = valid_minimal_ir();
    ir.inputs = vec![nemoir_ir::Input {
        id: "class".into(),
        ty: "string".into(),
    }];
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::InvalidPythonField(ref s)) if s == "class"),
        "expected InvalidPythonField(\"class\"), got {:?}",
        result
    );
}

#[test]
fn generate_package_rejects_hyphenated_input_id() {
    let mut ir = valid_minimal_ir();
    ir.inputs = vec![nemoir_ir::Input {
        id: "task-class".into(),
        ty: "string".into(),
    }];
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::InvalidPythonField(ref s)) if s == "task-class"),
        "expected InvalidPythonField(\"task-class\"), got {:?}",
        result
    );
}

#[test]
fn generate_package_rejects_invalid_exit_write_name() {
    let mut ir = valid_minimal_ir();
    ir.nodes[1].writes = vec![nemoir_ir::Write {
        name: "1bad".into(),
        ty: "string".into(),
        optional: false,
    }];
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        matches!(result, Err(PythonBackendError::InvalidPythonField(ref s)) if s == "1bad"),
        "expected InvalidPythonField(\"1bad\"), got {:?}",
        result
    );
}

#[test]
fn generate_package_accepts_non_exit_invalid_write_names() {
    // A write name that is not a valid Python identifier on a NON-EXIT stage
    // should be accepted -- those names only appear as quoted string literals
    // in the manifest, never as Python identifiers.
    let mut ir = valid_minimal_ir();
    ir.nodes[0].writes = vec![nemoir_ir::Write {
        name: "task-output".into(),
        ty: "string".into(),
        optional: false,
    }];
    let result = generate_package(&ir, &PythonBackendOptions::default());
    assert!(
        result.is_ok(),
        "non-exit write name should not be validated: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Syntactic validity: every .py file must parse via python3's AST
// ---------------------------------------------------------------------------

#[test]
fn generated_files_are_syntactically_valid_python() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping AST parse test: python3 not on PATH");
        return;
    }

    let ir = coding_agent_ir();
    let pkg = generate_package(&ir, &PythonBackendOptions::default()).unwrap();

    for file in &pkg.files {
        let path_str = file.relative_path.to_string_lossy().to_string();
        if !path_str.ends_with(".py") {
            continue;
        }
        let result = std::process::Command::new("python3")
            .arg("-c")
            .arg("import ast, sys; ast.parse(sys.stdin.read())")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(file.content.as_bytes())?;
                }
                child.wait_with_output()
            });

        match result {
            Ok(output) => assert!(
                output.status.success(),
                "python AST parse failed for {}: {}",
                path_str,
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(e) => {
                eprintln!("skipping AST parse test: failed to invoke python3: {}", e);
                return;
            }
        }
    }
}
