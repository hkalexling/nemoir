use nemoir_dsl_fe::{check, lower};

#[test]
fn infer_entry_and_exit_no_annotations() {
    let source = r#"workflow Test {
  stage First {
    prompt: "first"
    output: { x: string }
  }
  stage Last {
    prompt: "last"
  }
}"#;
    check(source, "test.nemo").expect("should pass with inferred entry/exit");
}

#[test]
fn infer_entry_only_exit_explicit() {
    let source = r#"workflow Test {
  stage First {
    prompt: "first"
    output: { x: string }
  }
  stage@exit Last {
    prompt: "last"
  }
}"#;
    check(source, "test.nemo").expect("should pass with inferred entry");
}

#[test]
fn infer_exit_only_entry_explicit() {
    let source = r#"workflow Test {
  stage@entry First {
    prompt: "first"
    output: { x: string }
  }
  stage Last {
    prompt: "last"
  }
}"#;
    check(source, "test.nemo").expect("should pass with inferred exit");
}

#[test]
fn lower_inferred_entry_exit() {
    let source = r#"workflow Test {
  stage First {
    prompt: "first"
    output: { x: string }
  }
  stage Last {
    prompt: "last"
  }
}"#;
    let ir = lower(source, "test.nemo").expect("lowering should succeed");
    assert_eq!(ir.workflow.entry, "First");
    assert_eq!(ir.workflow.exits, vec!["Last"]);

    let first_node = ir.nodes.iter().find(|n| n.id == "First").unwrap();
    assert!(first_node.annotations.contains(&"entry".to_string()));

    let last_node = ir.nodes.iter().find(|n| n.id == "Last").unwrap();
    assert!(last_node.annotations.contains(&"exit".to_string()));
}

#[test]
fn infer_single_stage() {
    let source = r#"workflow Test {
  stage Only {
    prompt: "only stage"
  }
}"#;
    let ir = lower(source, "test.nemo").expect("lowering should succeed");
    assert_eq!(ir.workflow.entry, "Only");
    assert_eq!(ir.workflow.exits, vec!["Only"]);
    let node = &ir.nodes[0];
    assert!(node.annotations.contains(&"entry".to_string()));
    assert!(node.annotations.contains(&"exit".to_string()));
}
