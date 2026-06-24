use indexmap::IndexMap;
use nemoir_ir::*;

#[test]
fn catalog_get_capability_returns_specs() {
    use nemoir_ir::capabilities::*;

    let spec = get_capability("fs.read").expect("fs.read should be known");
    assert_eq!(spec.name, "fs.read");
    assert_eq!(spec.required_params.len(), 1);
    assert_eq!(spec.required_params[0].name, "path");
    assert_eq!(spec.required_params[0].ty, CapabilityParamType::Path);

    let spec = get_capability("fs.write").expect("fs.write should be known");
    assert_eq!(spec.name, "fs.write");
    assert_eq!(spec.required_params.len(), 2);
    assert_eq!(spec.required_params[0].name, "path");
    assert_eq!(spec.required_params[0].ty, CapabilityParamType::Path);
    assert_eq!(spec.required_params[1].name, "content");
    assert_eq!(spec.required_params[1].ty, CapabilityParamType::String);

    let spec = get_capability("os.shell").expect("os.shell should be known");
    assert_eq!(spec.name, "os.shell");
    assert_eq!(spec.required_params.len(), 1);
    assert_eq!(spec.required_params[0].name, "command");
    assert_eq!(spec.required_params[0].ty, CapabilityParamType::String);

    assert!(get_capability("made.up").is_none());
    assert!(is_known_capability("fs.read"));
    assert!(!is_known_capability("made.up"));
}

#[test]
fn catalog_fs_write_content_is_string() {
    use nemoir_ir::capabilities::get_capability;
    let spec = get_capability("fs.write").unwrap();
    let content = spec
        .required_params
        .iter()
        .find(|p| p.name == "content")
        .unwrap();
    assert_eq!(
        content.ty,
        nemoir_ir::capabilities::CapabilityParamType::String
    );
}

#[test]
fn catalog_has_exactly_five_entries() {
    use nemoir_ir::capabilities::get_capability;
    let names = [
        "fs.read",
        "fs.write",
        "os.shell",
        "user.elicit",
        "user.confirm",
    ];
    for name in &names {
        assert!(
            get_capability(name).is_some(),
            "expected catalog to contain '{}'",
            name
        );
    }
    assert!(get_capability("fs.delete").is_none());
}

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
            entry: "A".into(),
            exits: vec!["B".into()],
            transition_semantics: TransitionSemantics {
                selection: "first_match_by_priority".into(),
                no_match: "error_unless_exit".into(),
            },
        },
        inputs: vec![Input {
            id: "in1".into(),
            ty: "string".into(),
        }],
        capabilities: vec!["fs.read".into()],
        policies: vec![],
        nodes: vec![
            Node {
                id: "A".into(),
                annotations: vec!["entry".into()],
                prompt: "node A".into(),
                reads: vec![Read {
                    ref_: Ref::Input { name: "in1".into() },
                    optional: false,
                    origin: "test".into(),
                }],
                writes: vec![Write {
                    name: "out_a".into(),
                    ty: "string".into(),
                    optional: false,
                }],
                requires: vec![StageCapability {
                    capability: "fs.read".into(),
                }],
                transitions: vec![Transition {
                    to: "B".into(),
                    priority: 0,
                    reason: "fallthrough".into(),
                    guard: Guard::Always,
                }],
                execution: StageExecution::Model,
            },
            Node {
                id: "B".into(),
                annotations: vec!["exit".into()],
                prompt: "node B".into(),
                reads: vec![],
                writes: vec![Write {
                    name: "out_b".into(),
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

fn assert_valid(ir: &WorkflowIr) {
    match nemoir_ir::validate::validate(ir) {
        Ok(()) => {}
        Err(errors) => {
            for e in &errors.errors {
                eprintln!("  validation error: {}", e);
            }
            panic!("expected valid IR but got {} errors", errors.errors.len());
        }
    }
}

fn assert_invalid(ir: &WorkflowIr, expected_substr: &str) {
    match nemoir_ir::validate::validate(ir) {
        Ok(()) => panic!("expected invalid IR but validation passed"),
        Err(errors) => {
            let combined: String = errors
                .errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join(" | ");
            if !combined.contains(expected_substr) {
                for e in &errors.errors {
                    eprintln!("  validation error: {}", e);
                }
                panic!(
                    "expected error containing '{}' but got: {}",
                    expected_substr, combined
                );
            }
        }
    }
}

#[test]
fn valid_minimal_one_node_workflow_passes() {
    let ir = valid_minimal_ir();
    assert_valid(&ir);
}

fn make_valid_coding_agent_ir() -> WorkflowIr {
    let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml");
    serde_yaml::from_str(source).expect("should parse coding-agent-ir.yml")
}

#[test]
fn valid_coding_agent_ir_passes() {
    let ir = make_valid_coding_agent_ir();
    assert_valid(&ir);
}

#[test]
fn duplicate_node_ids_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes.push(Node {
        id: "A".into(),
        annotations: vec![],
        prompt: "duplicate".into(),
        reads: vec![],
        writes: vec![],
        requires: vec![],
        transitions: vec![],
        execution: StageExecution::Model,
    });
    assert_invalid(&ir, "duplicate node id");
}

#[test]
fn unknown_entry_rejected() {
    let mut ir = valid_minimal_ir();
    ir.workflow.entry = "Z".into();
    assert_invalid(&ir, "entry node 'Z' does not exist");
}

#[test]
fn unknown_exit_rejected() {
    let mut ir = valid_minimal_ir();
    ir.workflow.exits = vec!["Z".into()];
    assert_invalid(&ir, "exit node 'Z' does not exist");
}

#[test]
fn unknown_transition_target_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].transitions[0].to = "Z".into();
    assert_invalid(&ir, "transition target 'Z' does not exist");
}

#[test]
fn unreachable_node_rejected() {
    let mut ir = valid_minimal_ir();
    // Add a node with no incoming edges
    ir.nodes.push(Node {
        id: "C".into(),
        annotations: vec![],
        prompt: "orphan".into(),
        reads: vec![],
        writes: vec![],
        requires: vec![],
        transitions: vec![],
        execution: StageExecution::Model,
    });
    assert_invalid(&ir, "unreachable from entry");
}

#[test]
fn non_exit_with_no_outgoing_transition_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].annotations.retain(|a| a != "entry");
    ir.nodes[0].transitions = vec![];
    ir.workflow.entry = "A".into();
    assert_invalid(&ir, "no outgoing transitions");
}

#[test]
fn exit_with_outgoing_transition_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[1].transitions = vec![Transition {
        to: "A".into(),
        priority: 0,
        reason: "back".into(),
        guard: Guard::Always,
    }];
    assert_invalid(&ir, "exit node 'B' must have no outgoing transitions");
}

#[test]
fn duplicate_transition_priority_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].transitions.push(Transition {
        to: "B".into(),
        priority: 0,
        reason: "dup".into(),
        guard: Guard::Always,
    });
    assert_invalid(&ir, "duplicate priority");
}

#[test]
fn read_of_unknown_workflow_input_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].reads[0] = Read {
        ref_: Ref::Input {
            name: "unknown".into(),
        },
        optional: false,
        origin: "test".into(),
    };
    assert_invalid(&ir, "unknown workflow input");
}

#[test]
fn read_of_unknown_node_output_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].reads[0] = Read {
        ref_: Ref::NodeOutput {
            node: "B".into(),
            field: "nope".into(),
        },
        optional: false,
        origin: "test".into(),
    };
    assert_invalid(&ir, "unknown output");
}

#[test]
fn guard_ref_to_unknown_node_output_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].transitions[0].guard = Guard::HasValue {
        r#ref: Ref::NodeOutput {
            node: "A".into(),
            field: "no_such_field".into(),
        },
    };
    assert_invalid(&ir, "unknown output");
}

#[test]
fn has_value_on_non_optional_output_rejected() {
    // Make A have an optional output and use has_value on B's non-optional output
    let mut ir = valid_minimal_ir();
    ir.nodes[0].writes.push(Write {
        name: "opt_out".into(),
        ty: "string".into(),
        optional: true,
    });
    ir.nodes[0].transitions[0].guard = Guard::HasValue {
        r#ref: Ref::NodeOutput {
            node: "A".into(),
            field: "out_a".into(),
        },
    };
    assert_invalid(&ir, "non-optional output");
}

#[test]
fn node_required_capability_missing_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities = vec![];
    assert_invalid(&ir, "not declared in top-level capabilities");
}

#[test]
fn policy_trigger_capability_missing_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities = vec![];
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "missing.cap".into(),
            bind: Default::default(),
        },
        requires: None,
        condition: None,
    }];
    assert_invalid(&ir, "not declared in top-level capabilities");
}

#[test]
fn unknown_top_level_capability_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("made.up".into());
    assert_invalid(&ir, "unknown capability");
}

#[test]
fn unknown_stage_capability_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].requires[0].capability = "made.up".into();
    assert_invalid(&ir, "unknown capability");
}

#[test]
fn unknown_policy_trigger_capability_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("made.up".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "made.up".into(),
            bind: Default::default(),
        },
        requires: None,
        condition: None,
    }];
    assert_invalid(&ir, "unknown capability");
}

#[test]
fn unknown_policy_required_capability_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("fs.write".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "fs.write".into(),
            bind: Default::default(),
        },
        requires: Some(vec![RequiredCapability {
            capability: "made.up".into(),
            args: Default::default(),
        }]),
        condition: None,
    }];
    assert_invalid(&ir, "unknown capability");
}

#[test]
fn unknown_policy_trigger_bind_param_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("fs.write".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "fs.write".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "filename".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "filename".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: None,
    }];
    assert_invalid(&ir, "has no required parameter");
}

#[test]
fn unknown_policy_required_arg_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("fs.read".into());
    ir.capabilities.push("fs.write".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "fs.write".into(),
            bind: Default::default(),
        },
        requires: Some(vec![RequiredCapability {
            capability: "fs.read".into(),
            args: {
                let mut args = indexmap::IndexMap::new();
                args.insert(
                    "filename".into(),
                    ArgValue::Ref {
                        r#ref: Ref::Input { name: "in1".into() },
                    },
                );
                args
            },
        }]),
        condition: None,
    }];
    assert_invalid(&ir, "has no required parameter");
}

#[test]
fn user_confirm_without_forwarded_args_is_valid() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("fs.write".into());
    ir.capabilities.push("user.confirm".into());
    ir.policies = vec![Policy {
        id: "before_fs_write_requires_fs_read_and_user_confirm".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "fs.write".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "path".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "path".into(),
                    },
                );
                b
            },
        },
        requires: Some(vec![
            RequiredCapability {
                capability: "fs.read".into(),
                args: {
                    let mut args = indexmap::IndexMap::new();
                    args.insert(
                        "path".into(),
                        ArgValue::Ref {
                            r#ref: Ref::Bound {
                                name: "path".into(),
                            },
                        },
                    );
                    args
                },
            },
            RequiredCapability {
                capability: "user.confirm".into(),
                args: Default::default(),
            },
        ]),
        condition: None,
    }];
    assert_valid(&ir);
}

#[test]
fn policy_bound_ref_not_in_trigger_bind_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: Default::default(),
        },
        requires: Some(vec![RequiredCapability {
            capability: "os.shell".into(),
            args: {
                let mut args = indexmap::IndexMap::new();
                args.insert(
                    "p".into(),
                    ArgValue::Ref {
                        r#ref: Ref::Bound { name: "p".into() },
                    },
                );
                args
            },
        }]),
        condition: None,
    }];
    assert_invalid(&ir, "not declared in trigger bind");
}

#[test]
fn guard_eq_input_ref_passes() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].transitions[0].guard = Guard::Eq {
        left: Expr::Ref {
            r#ref: Ref::Input { name: "in1".into() },
        },
        right: Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("hello".into()),
        },
    };
    assert_valid(&ir);
}

#[test]
fn guard_eq_bound_ref_rejected() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].transitions[0].guard = Guard::Eq {
        left: Expr::Ref {
            r#ref: Ref::Bound { name: "x".into() },
        },
        right: Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("hello".into()),
        },
    };
    assert_invalid(&ir, "Ref::Bound");
}

#[test]
fn future_producer_read_rejected() {
    let mut ir = valid_minimal_ir();
    // B reads A's output — valid. Add a third node C and make A read C's output (future/unreachable).
    ir.workflow.exits = vec!["C".into()];
    ir.nodes.push(Node {
        id: "C".into(),
        annotations: vec!["exit".into()],
        prompt: "node C".into(),
        reads: vec![],
        writes: vec![Write {
            name: "out_c".into(),
            ty: "string".into(),
            optional: false,
        }],
        requires: vec![],
        transitions: vec![],
        execution: StageExecution::Model,
    });
    ir.nodes[1].transitions = vec![Transition {
        to: "C".into(),
        priority: 0,
        reason: "fallthrough".into(),
        guard: Guard::Always,
    }];
    // A tries to read C's output, but C cannot reach A (reverse direction)
    ir.nodes[0].reads[0] = Read {
        ref_: Ref::NodeOutput {
            node: "C".into(),
            field: "out_c".into(),
        },
        optional: false,
        origin: "test".into(),
    };
    assert_invalid(&ir, "cannot reach consumer");
}

#[test]
fn self_read_rejected() {
    let mut ir = valid_minimal_ir();
    // A reads its own output
    ir.nodes[0].reads = vec![Read {
        ref_: Ref::NodeOutput {
            node: "A".into(),
            field: "out_a".into(),
        },
        optional: false,
        origin: "test".into(),
    }];
    assert_invalid(&ir, "self-read");
}

#[test]
fn valid_loop_backref_read_passes() {
    // A -> B -> A is a valid loop. B reads A's output — valid because A can reach B's position.
    let mut ir = valid_minimal_ir();
    ir.nodes[1].annotations.retain(|a| a != "exit");
    ir.nodes[1].transitions = vec![Transition {
        to: "A".into(),
        priority: 0,
        reason: "backward_ref_loop".into(),
        guard: Guard::Always,
    }];
    // B reads A's out_a
    ir.nodes[1].reads = vec![Read {
        ref_: Ref::NodeOutput {
            node: "A".into(),
            field: "out_a".into(),
        },
        optional: false,
        origin: "test".into(),
    }];
    ir.nodes[0].reads = vec![]; // A no longer reads in1 (not needed for this test)
                                // Need an exit that is reachable from the loop
    ir.nodes.push(Node {
        id: "C".into(),
        annotations: vec!["exit".into()],
        prompt: "exit".into(),
        reads: vec![],
        writes: vec![],
        requires: vec![],
        transitions: vec![],
        execution: StageExecution::Model,
    });
    ir.workflow.exits = vec!["C".into()];
    ir.nodes[1].transitions[0] = Transition {
        to: "C".into(),
        priority: 0,
        reason: "fallthrough".into(),
        guard: Guard::Always,
    };
    assert_valid(&ir);
}

#[test]
fn guard_eq_incompatible_types_rejected() {
    let mut ir = valid_minimal_ir();
    // A's out_a is string. Guard::Eq compares it with bool literal.
    ir.nodes[0].transitions[0].guard = Guard::Eq {
        left: Expr::Ref {
            r#ref: Ref::NodeOutput {
                node: "A".into(),
                field: "out_a".into(),
            },
        },
        right: Expr::Literal {
            ty: "bool".into(),
            value: serde_yaml::Value::Bool(true),
        },
    };
    assert_invalid(&ir, "incompatible types");
}

#[test]
fn guard_eq_compatible_types_passes() {
    let mut ir = valid_minimal_ir();
    // A's out_a is string. Compare with another string literal. Compatible.
    ir.nodes[0].transitions[0].guard = Guard::Eq {
        left: Expr::Ref {
            r#ref: Ref::NodeOutput {
                node: "A".into(),
                field: "out_a".into(),
            },
        },
        right: Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("hello".into()),
        },
    };
    assert_valid(&ir);
}

#[test]
fn policy_require_arg_missing_input_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: Some(vec![RequiredCapability {
            capability: "os.shell".into(),
            args: {
                let mut args = indexmap::IndexMap::new();
                args.insert(
                    "command".into(),
                    ArgValue::Ref {
                        r#ref: Ref::Input {
                            name: "nonexistent".into(),
                        },
                    },
                );
                args
            },
        }]),
        condition: None,
    }];
    assert_invalid(&ir, "unknown workflow input");
}

#[test]
fn policy_require_arg_valid_input_passes() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: Some(vec![RequiredCapability {
            capability: "os.shell".into(),
            args: {
                let mut args = indexmap::IndexMap::new();
                args.insert(
                    "command".into(),
                    ArgValue::Ref {
                        r#ref: Ref::Input { name: "in1".into() },
                    },
                );
                args
            },
        }]),
        condition: None,
    }];
    assert_valid(&ir);
}

#[test]
fn policy_require_arg_valid_bound_passes() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: Some(vec![RequiredCapability {
            capability: "os.shell".into(),
            args: {
                let mut args = indexmap::IndexMap::new();
                args.insert(
                    "command".into(),
                    ArgValue::Ref {
                        r#ref: Ref::Bound {
                            name: "command".into(),
                        },
                    },
                );
                args
            },
        }]),
        condition: None,
    }];
    assert_valid(&ir);
}

// ---------------------------------------------------------------------------
// Policy expression predicate tests (Phase 1: And/Or, eq, starts_with)
// ---------------------------------------------------------------------------

#[test]
fn policy_condition_and_or_passes() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::Or {
            exprs: vec![
                Expr::MethodCall {
                    receiver: Box::new(Expr::Ref {
                        r#ref: Ref::Bound {
                            name: "command".into(),
                        },
                    }),
                    method: "eq".into(),
                    args: vec![Expr::Literal {
                        ty: "string".into(),
                        value: serde_yaml::Value::String("a".into()),
                    }],
                },
                Expr::MethodCall {
                    receiver: Box::new(Expr::Ref {
                        r#ref: Ref::Bound {
                            name: "command".into(),
                        },
                    }),
                    method: "starts_with".into(),
                    args: vec![Expr::Literal {
                        ty: "string".into(),
                        value: serde_yaml::Value::String("python".into()),
                    }],
                },
            ],
        }),
    }];
    assert_valid(&ir);
}

#[test]
fn policy_condition_in_lowered_to_or_passes() {
    // Direct IR test: hand-built Or of two eq method calls (what `in [...]` lowers to)
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::And {
            exprs: vec![Expr::Not {
                expr: Box::new(Expr::Or {
                    exprs: vec![
                        Expr::MethodCall {
                            receiver: Box::new(Expr::Ref {
                                r#ref: Ref::Bound {
                                    name: "command".into(),
                                },
                            }),
                            method: "eq".into(),
                            args: vec![Expr::Literal {
                                ty: "string".into(),
                                value: serde_yaml::Value::String("a".into()),
                            }],
                        },
                        Expr::MethodCall {
                            receiver: Box::new(Expr::Ref {
                                r#ref: Ref::Bound {
                                    name: "command".into(),
                                },
                            }),
                            method: "eq".into(),
                            args: vec![Expr::Literal {
                                ty: "string".into(),
                                value: serde_yaml::Value::String("b".into()),
                            }],
                        },
                    ],
                }),
            }],
        }),
    }];
    assert_valid(&ir);
}

#[test]
fn policy_condition_empty_and_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::And { exprs: vec![] }),
    }];
    assert_invalid(&ir, "requires at least 1 operand");
}

#[test]
fn policy_condition_empty_or_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::Or { exprs: vec![] }),
    }];
    assert_invalid(&ir, "requires at least 1 operand");
}

#[test]
fn policy_condition_unknown_method_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Bound {
                    name: "command".into(),
                },
            }),
            method: "bogus".into(),
            args: vec![Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("x".into()),
            }],
        }),
    }];
    assert_invalid(&ir, "unknown method");
}

#[test]
fn policy_deny_without_condition_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: Default::default(),
        },
        requires: None,
        condition: None,
    }];
    assert_invalid(&ir, "deny policy must have a condition");
}

#[test]
fn policy_before_with_condition_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "before".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: Default::default(),
        },
        requires: None,
        condition: Some(Expr::Literal {
            ty: "bool".into(),
            value: serde_yaml::Value::Bool(true),
        }),
    }];
    assert_invalid(&ir, "before policy must not have a condition");
}

#[test]
fn policy_eq_type_mismatch_rejected() {
    let mut ir = valid_minimal_ir();
    ir.inputs.push(Input {
        id: "cwd".into(),
        ty: "path".into(),
    });
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Bound {
                    name: "command".into(),
                },
            }),
            method: "eq".into(),
            args: vec![Expr::Ref {
                r#ref: Ref::Input { name: "cwd".into() },
            }],
        }),
    }];
    assert_invalid(&ir, "incompatible types");
}

#[test]
fn policy_starts_with_non_string_receiver_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("fs.write".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "fs.write".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "path".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "path".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Bound {
                    name: "path".into(),
                },
            }),
            method: "starts_with".into(),
            args: vec![Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("x".into()),
            }],
        }),
    }];
    assert_invalid(&ir, "string receiver");
}

#[test]
fn policy_eq_string_exact_passes() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::Not {
            expr: Box::new(Expr::MethodCall {
                receiver: Box::new(Expr::Ref {
                    r#ref: Ref::Bound {
                        name: "command".into(),
                    },
                }),
                method: "eq".into(),
                args: vec![Expr::Literal {
                    ty: "string".into(),
                    value: serde_yaml::Value::String("exact".into()),
                }],
            }),
        }),
    }];
    assert_valid(&ir);
}

#[test]
fn policy_and_nonbool_operand_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::And {
            exprs: vec![
                Expr::Ref {
                    r#ref: Ref::Bound {
                        name: "command".into(),
                    },
                },
                Expr::MethodCall {
                    receiver: Box::new(Expr::Ref {
                        r#ref: Ref::Bound {
                            name: "command".into(),
                        },
                    }),
                    method: "eq".into(),
                    args: vec![Expr::Literal {
                        ty: "string".into(),
                        value: serde_yaml::Value::String("x".into()),
                    }],
                },
            ],
        }),
    }];
    assert_invalid(&ir, "and operand must be bool");
}

#[test]
fn policy_or_nonbool_operand_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::Or {
            exprs: vec![
                Expr::Ref {
                    r#ref: Ref::Bound {
                        name: "command".into(),
                    },
                },
                Expr::MethodCall {
                    receiver: Box::new(Expr::Ref {
                        r#ref: Ref::Bound {
                            name: "command".into(),
                        },
                    }),
                    method: "starts_with".into(),
                    args: vec![Expr::Literal {
                        ty: "string".into(),
                        value: serde_yaml::Value::String("x".into()),
                    }],
                },
            ],
        }),
    }];
    assert_invalid(&ir, "or operand must be bool");
}

#[test]
fn policy_contains_bool_receiver_rejected() {
    let mut ir = valid_minimal_ir();
    ir.inputs.push(Input {
        id: "flag".into(),
        ty: "bool".into(),
    });
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Input {
                    name: "flag".into(),
                },
            }),
            method: "contains".into(),
            args: vec![Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("x".into()),
            }],
        }),
    }];
    assert_invalid(&ir, "is not supported");
}

#[test]
fn policy_eq_bool_receiver_rejected() {
    let mut ir = valid_minimal_ir();
    ir.inputs.push(Input {
        id: "flag".into(),
        ty: "bool".into(),
    });
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Input {
                    name: "flag".into(),
                },
            }),
            method: "eq".into(),
            args: vec![Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("true".into()),
            }],
        }),
    }];
    assert_invalid(&ir, "incompatible types");
}

#[test]
fn policy_contains_extra_args_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Bound {
                    name: "command".into(),
                },
            }),
            method: "contains".into(),
            args: vec![
                Expr::Literal {
                    ty: "string".into(),
                    value: serde_yaml::Value::String("x".into()),
                },
                Expr::Literal {
                    ty: "string".into(),
                    value: serde_yaml::Value::String("y".into()),
                },
            ],
        }),
    }];
    assert_invalid(&ir, "requires exactly 1 argument");
}

#[test]
fn policy_contains_number_receiver_rejected() {
    let mut ir = valid_minimal_ir();
    ir.inputs.push(Input {
        id: "score".into(),
        ty: "number".into(),
    });
    ir.capabilities.push("os.shell".into());
    ir.policies = vec![Policy {
        id: "p1".into(),
        kind: "deny".into(),
        trigger: Trigger {
            capability: "os.shell".into(),
            bind: {
                let mut b = indexmap::IndexMap::new();
                b.insert(
                    "command".into(),
                    BindArg {
                        kind: "arg".into(),
                        name: "command".into(),
                    },
                );
                b
            },
        },
        requires: None,
        condition: Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ref {
                r#ref: Ref::Input {
                    name: "score".into(),
                },
            }),
            method: "contains".into(),
            args: vec![Expr::Literal {
                ty: "string".into(),
                value: serde_yaml::Value::String("x".into()),
            }],
        }),
    }];
    assert_invalid(&ir, "is not supported on this receiver type");
}

// ---------------------------------------------------------------------------
// StageExecution validation tests
// ---------------------------------------------------------------------------

fn make_tool_stage_ir(capability: &str, args: IndexMap<String, Expr>) -> WorkflowIr {
    let mut ir = valid_minimal_ir();
    if !ir.capabilities.iter().any(|c| c == capability) {
        ir.capabilities.push(capability.to_string());
    }
    ir.nodes[0].execution = StageExecution::Tool {
        capability: capability.to_string(),
        args,
    };
    if !ir.nodes[0].requires.iter().any(|c| c.capability == capability) {
        ir.nodes[0].requires.push(StageCapability {
            capability: capability.to_string(),
        });
    }
    ir
}

#[test]
fn exec_unknown_capability_rejected() {
    let ir = make_tool_stage_ir("unknown.cap", IndexMap::new());
    assert_invalid(&ir, "unknown exec capability");
}

#[test]
fn exec_capability_not_in_top_level_rejected() {
    let mut ir = valid_minimal_ir();
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo hi".into()),
        },
    );
    ir.nodes[0].execution = StageExecution::Tool {
        capability: "os.shell".into(),
        args,
    };
    ir.nodes[0].requires.push(StageCapability {
        capability: "os.shell".into(),
    });
    // capability_set does not contain os.shell
    assert_invalid(&ir, "not declared in top-level capabilities");
}

#[test]
fn exec_capability_not_in_requires_rejected() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo hi".into()),
        },
    );
    ir.nodes[0].execution = StageExecution::Tool {
        capability: "os.shell".into(),
        args,
    };
    // node requires only has fs.read, not os.shell
    assert_invalid(&ir, "must be in node's requires");
}

#[test]
fn exec_missing_required_param_rejected() {
    let args = IndexMap::new();
    // os.shell requires 'command' — we don't add it
    let ir = make_tool_stage_ir("os.shell", args);
    assert_invalid(&ir, "missing required exec arg 'command'");
}

#[test]
fn exec_unknown_param_rejected() {
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo hi".into()),
        },
    );
    args.insert(
        "extra".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("x".into()),
        },
    );
    let ir = make_tool_stage_ir("os.shell", args);
    assert_invalid(&ir, "unknown exec arg 'extra'");
}

#[test]
fn exec_bound_ref_rejected() {
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Ref {
            r#ref: Ref::Bound {
                name: "cmd".into(),
            },
        },
    );
    let ir = make_tool_stage_ir("os.shell", args);
    assert_invalid(&ir, "Ref::Bound");
}

#[test]
fn exec_non_ref_literal_expr_rejected() {
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Not {
            expr: Box::new(Expr::Literal {
                ty: "bool".into(),
                value: serde_yaml::Value::Bool(true),
            }),
        },
    );
    let ir = make_tool_stage_ir("os.shell", args);
    assert_invalid(&ir, "only Ref and Literal expressions");
}

#[test]
fn exec_valid_tool_stage_accepted() {
    let mut args = IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo hi".into()),
        },
    );
    let ir = make_tool_stage_ir("os.shell", args);
    assert_valid(&ir);
}
