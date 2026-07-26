//! Positive DSL tests for deterministic (`exec:`) stages — lowering and IR-shape verification.
//!
//! Mirrors the pattern in `lower_coding_agent.rs` and ensures the ref-exec path
//! (Critical #1) has a regression guard at the DSL-to-IR level.

use nemoir_dsl_fe::lower;
use nemoir_ir::{Expr, Ref, StageExecution};

fn find_node<'a>(ir: &'a nemoir_ir::WorkflowIr, id: &str) -> &'a nemoir_ir::Node {
    ir.nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("node '{}' not found", id))
}

#[test]
fn lower_exec_shell_fixture() {
    let source = include_str!("fixtures/exec_shell.nemo");
    let ir = lower(source, "exec_shell.nemo").expect("lowering should succeed");

    assert_eq!(ir.capabilities, vec!["os.shell"]);

    let run = find_node(&ir, "Run");
    assert_eq!(run.prompt, ""); // deterministic, prompt omitted
    assert!(run.requires.iter().any(|c| c.capability == "os.shell"));

    match &run.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "os.shell");
            assert_eq!(args.len(), 1);
            let cmd_value = args.get("command").expect("command arg");
            match cmd_value {
                Expr::Literal { ty, value } => {
                    assert_eq!(ty, "string");
                    assert_eq!(value.as_str(), Some("echo hello"));
                }
                other => panic!("expected Literal for command, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_exec_fs_read_fixture() {
    let source = include_str!("fixtures/exec_fs_read.nemo");
    let ir = lower(source, "exec_fs_read.nemo").expect("lowering should succeed");

    assert_eq!(ir.capabilities, vec!["fs.read"]);

    let read = find_node(&ir, "Read");
    assert_eq!(read.prompt, ""); // deterministic, prompt omitted
    assert!(read.requires.iter().any(|c| c.capability == "fs.read"));

    match &read.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "fs.read");
            assert_eq!(args.len(), 1);
            let path_value = args.get("path").expect("path arg");
            match path_value {
                Expr::Ref { r#ref } => match r#ref {
                    Ref::Input { name } => assert_eq!(name, "config_path"),
                    other => panic!("expected Input ref for path, got {:?}", other),
                },
                other => panic!("expected Ref for path, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_exec_fs_write_fixture() {
    let source = include_str!("fixtures/exec_fs_write.nemo");
    let ir = lower(source, "exec_fs_write.nemo").expect("lowering should succeed");

    assert!(ir.capabilities.contains(&"fs.write".to_string()));

    let write = find_node(&ir, "Write");
    assert_eq!(write.prompt, ""); // deterministic, prompt omitted
    assert!(write.requires.iter().any(|c| c.capability == "fs.write"));

    // Write should auto-read Produce.content
    assert!(
        write.reads.iter().any(|r| matches!(
            &r.ref_,
            Ref::NodeOutput { node, field } if node == "Produce" && field == "content"
        )),
        "Write should auto-read Produce.content from exec arg"
    );

    match &write.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "fs.write");
            assert_eq!(args.len(), 2);

            // path: Ref::Input { name: "out_path" }
            let path_value = args.get("path").expect("path arg");
            match path_value {
                Expr::Ref { r#ref } => match r#ref {
                    Ref::Input { name } => assert_eq!(name, "out_path"),
                    other => panic!("expected Input ref for path, got {:?}", other),
                },
                other => panic!("expected Ref for path, got {:?}", other),
            }

            // content: Ref::NodeOutput { node: "Produce", field: "content" }
            let content_value = args.get("content").expect("content arg");
            match content_value {
                Expr::Ref { r#ref } => match r#ref {
                    Ref::NodeOutput { node, field } => {
                        assert_eq!(node, "Produce");
                        assert_eq!(field, "content");
                    }
                    other => panic!("expected NodeOutput ref for content, got {:?}", other),
                },
                other => panic!("expected Ref for content, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_exec_user_confirm_fixture() {
    let source = include_str!("fixtures/exec_user_confirm.nemo");
    let ir = lower(source, "exec_user_confirm.nemo").expect("lowering should succeed");

    assert!(ir.capabilities.contains(&"user.confirm".to_string()));

    let confirm = find_node(&ir, "Confirm");
    assert_eq!(confirm.prompt, ""); // deterministic, prompt omitted
    assert!(confirm
        .requires
        .iter()
        .any(|c| c.capability == "user.confirm"));

    match &confirm.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "user.confirm");
            assert_eq!(args.len(), 1);
            let msg_value = args.get("message").expect("message arg");
            match msg_value {
                Expr::Literal { ty, value } => {
                    assert_eq!(ty, "string");
                    assert_eq!(value.as_str(), Some("Proceed?"));
                }
                other => panic!("expected Literal for message, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_exec_with_prompt_fixture() {
    let source = include_str!("fixtures/exec_with_prompt.nemo");
    let ir = lower(source, "exec_with_prompt.nemo").expect("lowering should succeed");

    let run = find_node(&ir, "Run");
    // Deterministic stage with a prompt — prompt is documentation-only, not empty.
    assert_eq!(
        run.prompt,
        "This prompt is documentation-only for a deterministic stage"
    );
    assert!(run.requires.iter().any(|c| c.capability == "os.shell"));

    match &run.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "os.shell");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_js_run_positive_fixture() {
    let source = include_str!("fixtures/js_run_positive.nemo");
    let ir = lower(source, "js_run_positive.nemo").expect("lowering should succeed");

    assert!(ir.capabilities.contains(&"browser.js.run".to_string()));

    let compute = find_node(&ir, "Compute");
    assert_eq!(compute.prompt, ""); // deterministic, prompt omitted
    assert!(compute
        .requires
        .iter()
        .any(|c| c.capability == "browser.js.run"));

    match &compute.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "browser.js.run");
            assert_eq!(args.len(), 2);

            // code: a multiline-string-literal lowered to a string Literal.
            let code_value = args.get("code").expect("code arg");
            match code_value {
                Expr::Literal { ty, value } => {
                    assert_eq!(ty, "string");
                    // The multiline body is preserved verbatim (trimmed).
                    assert!(value
                        .as_str()
                        .unwrap_or_default()
                        .contains("return { result: input.x + 1 }"));
                }
                other => panic!("expected Literal for code, got {:?}", other),
            }

            // input: a Ref::Input { name: "x" }.
            let input_value = args.get("input").expect("input arg");
            match input_value {
                Expr::Ref { r#ref } => match r#ref {
                    Ref::Input { name } => assert_eq!(name, "x"),
                    other => panic!("expected Input ref for input, got {:?}", other),
                },
                other => panic!("expected Ref for input, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_js_sandbox_positive_fixture() {
    let source = include_str!("fixtures/js_sandbox_positive.nemo");
    let ir = lower(source, "js_sandbox_positive.nemo").expect("lowering should succeed");

    assert!(ir.capabilities.contains(&"browser.js.sandbox".to_string()));
    assert!(ir.capabilities.contains(&"user.confirm".to_string()));
    assert!(ir.policies.iter().any(|policy| {
        policy.kind == "before"
            && policy.trigger.capability == "browser.js.sandbox"
            && policy.trigger.bind.contains_key("code")
            && policy
                .requires
                .as_ref()
                .is_some_and(|requires| requires.iter().any(|req| req.capability == "user.confirm"))
    }));

    let run = find_node(&ir, "RunSandbox");
    assert!(run
        .requires
        .iter()
        .any(|c| c.capability == "browser.js.sandbox"));
    assert!(run.reads.iter().any(|r| matches!(
        &r.ref_,
        Ref::NodeOutput { node, field } if node == "GenerateCode" && field == "code"
    )));

    match &run.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "browser.js.sandbox");
            match args.get("code").expect("code arg") {
                Expr::Ref {
                    r#ref: Ref::NodeOutput { node, field },
                } => {
                    assert_eq!(node, "GenerateCode");
                    assert_eq!(field, "code");
                }
                other => panic!("expected model-output code Ref, got {:?}", other),
            }
            match args.get("input").expect("input arg") {
                Expr::Ref {
                    r#ref: Ref::Input { name },
                } => assert_eq!(name, "payload"),
                other => panic!("expected workflow-input JSON Ref, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_js_sandbox_user_code_fixture() {
    let source = include_str!("fixtures/js_sandbox_user_code.nemo");
    let ir = lower(source, "js_sandbox_user_code.nemo").expect("lowering should succeed");

    assert_eq!(ir.workflow.entry, "RunSandbox");
    assert_eq!(ir.workflow.exits, vec!["RunSandbox"]);
    let run = find_node(&ir, "RunSandbox");
    match &run.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "browser.js.sandbox");
            match args.get("code").expect("code arg") {
                Expr::Ref {
                    r#ref: Ref::Input { name },
                } => assert_eq!(name, "code"),
                other => panic!("expected user-code input Ref, got {:?}", other),
            }
            match args.get("input").expect("input arg") {
                Expr::Ref {
                    r#ref: Ref::Input { name },
                } => assert_eq!(name, "payload"),
                other => panic!("expected payload input Ref, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}

#[test]
fn lower_http_fetch_fixture() {
    let source = include_str!("fixtures/http_fetch.nemo");
    let ir = lower(source, "http_fetch.nemo").expect("lowering should succeed");

    assert!(ir.capabilities.contains(&"http.fetch".to_string()));

    let fetch_node = find_node(&ir, "Fetch");
    assert_eq!(fetch_node.prompt, ""); // deterministic, prompt omitted
    assert!(fetch_node
        .requires
        .iter()
        .any(|c| c.capability == "http.fetch"));

    match &fetch_node.execution {
        StageExecution::Tool { capability, args } => {
            assert_eq!(capability, "http.fetch");
            assert_eq!(args.len(), 2);

            let url_value = args.get("url").expect("url arg");
            match url_value {
                Expr::Literal { ty, value } => {
                    assert_eq!(ty, "string");
                    assert_eq!(value.as_str(), Some("https://example.com"));
                }
                other => panic!("expected Literal for url, got {:?}", other),
            }

            let method_value = args.get("method").expect("method arg");
            match method_value {
                Expr::Literal { ty, value } => {
                    assert_eq!(ty, "string");
                    assert_eq!(value.as_str(), Some("GET"));
                }
                other => panic!("expected Literal for method, got {:?}", other),
            }
        }
        other => panic!("expected Tool execution, got {:?}", other),
    }
}
