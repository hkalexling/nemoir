use nemoir_dsl_fe::check;

macro_rules! test_invalid {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            let source = include_str!(concat!("fixtures/invalid/", $file));
            let result = check(source, $file);
            assert!(result.is_err(), "expected error for {}, but got Ok", $file);
        }
    };
}

test_invalid!(no_stages, "no_stages.nemo");
test_invalid!(multiple_entry, "multiple_entry.nemo");
test_invalid!(duplicate_stage, "duplicate_stage.nemo");
test_invalid!(unknown_type, "unknown_type.nemo");
test_invalid!(bool_branch_on_nonbool, "bool_branch_on_nonbool.nemo");
test_invalid!(unknown_stage_ref, "unknown_stage_ref.nemo");
test_invalid!(unknown_output_field, "unknown_output_field.nemo");
test_invalid!(duplicate_output_field, "duplicate_output_field.nemo");
test_invalid!(
    bool_branch_target_missing,
    "bool_branch_target_missing.nemo"
);
test_invalid!(
    optional_skip_compound_guard,
    "optional_skip_compound_guard.nemo"
);
test_invalid!(unreachable_stage, "unreachable_stage.nemo");
test_invalid!(ambiguous_backward_ref, "ambiguous_backward_ref.nemo");
test_invalid!(no_fallthrough_target, "no_fallthrough_target.nemo");
test_invalid!(policy_unknown_input, "policy_unknown_input.nemo");
test_invalid!(policy_unknown_bound, "policy_unknown_bound.nemo");
test_invalid!(read_future_stage_branch, "read_future_stage_branch.nemo");
test_invalid!(policy_not_on_nonbool, "policy_not_on_nonbool.nemo");
test_invalid!(
    policy_deny_nonbool_condition,
    "policy_deny_nonbool_condition.nemo"
);
test_invalid!(policy_unsupported_method, "policy_unsupported_method.nemo");
test_invalid!(policy_string_contains, "policy_string_contains.nemo");
test_invalid!(policy_empty_in, "policy_empty_in.nemo");
test_invalid!(
    policy_starts_with_path_receiver,
    "policy_starts_with_path_receiver.nemo"
);
test_invalid!(policy_eq_type_mismatch, "policy_eq_type_mismatch.nemo");
test_invalid!(
    policy_starts_with_path_arg,
    "policy_starts_with_path_arg.nemo"
);
test_invalid!(
    policy_in_option_type_mismatch,
    "policy_in_option_type_mismatch.nemo"
);
test_invalid!(
    policy_and_nonbool_operand,
    "policy_and_nonbool_operand.nemo"
);
test_invalid!(policy_or_nonbool_operand, "policy_or_nonbool_operand.nemo");
test_invalid!(
    policy_contains_bool_receiver,
    "policy_contains_bool_receiver.nemo"
);
test_invalid!(policy_eq_bool_receiver, "policy_eq_bool_receiver.nemo");
test_invalid!(
    policy_contains_extra_args,
    "policy_contains_extra_args.nemo"
);
test_invalid!(optional_workflow_input, "optional_workflow_input.nemo");
test_invalid!(optional_bool_branch, "optional_bool_branch.nemo");
test_invalid!(multiple_bool_branches, "multiple_bool_branches.nemo");
test_invalid!(unknown_capability_stage, "unknown_capability_stage.nemo");
test_invalid!(unknown_capability_policy, "unknown_capability_policy.nemo");
test_invalid!(exec_unknown_capability, "exec_unknown_capability.nemo");
test_invalid!(exec_missing_param, "exec_missing_param.nemo");
test_invalid!(exec_unknown_param, "exec_unknown_param.nemo");
test_invalid!(exec_unknown_input_ref, "exec_unknown_input_ref.nemo");
test_invalid!(exec_unknown_output_ref, "exec_unknown_output_ref.nemo");
test_invalid!(
    policy_unknown_trigger_param,
    "policy_unknown_trigger_param.nemo"
);
test_invalid!(
    numeric_compare_nonnumeric,
    "numeric_compare_nonnumeric.nemo"
);
test_invalid!(
    bool_branches_and_transition_mixed,
    "bool_branches_and_transition_mixed.nemo"
);
test_invalid!(
    transition_unreachable_graph,
    "transition_unreachable_graph.nemo"
);
test_invalid!(numeric_eq_operator, "numeric_eq_operator.nemo");
test_invalid!(
    transition_and_nonbool_operand,
    "transition_and_nonbool_operand.nemo"
);
test_invalid!(transition_not_nonbool, "transition_not_nonbool.nemo");
test_invalid!(transition_numeric_eq, "transition_numeric_eq.nemo");
test_invalid!(transition_numeric_in, "transition_numeric_in.nemo");

test_invalid!(transition_multiple_else, "transition_multiple_else.nemo");

test_invalid!(
    policy_unknown_required_param,
    "policy_unknown_required_param.nemo"
);

// Targeted diagnostic quality tests: assert error message content for key cases.

#[test]
fn unknown_output_field_message() {
    let source = include_str!("fixtures/invalid/unknown_output_field.nemo");
    let err = check(source, "unknown_output_field.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown output field"),
        "expected 'unknown output field' in error, got: {}",
        msg
    );
}

#[test]
fn bool_branch_on_nonbool_message() {
    let source = include_str!("fixtures/invalid/bool_branch_on_nonbool.nemo");
    let err = check(source, "bool_branch_on_nonbool.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("non-bool"),
        "expected 'non-bool' in error, got: {}",
        msg
    );
}

#[test]
fn unreachable_stage_message() {
    let source = include_str!("fixtures/invalid/unreachable_stage.nemo");
    let err = check(source, "unreachable_stage.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unreachable"),
        "expected 'unreachable' in error, got: {}",
        msg
    );
}

#[test]
fn unknown_type_message() {
    let source = include_str!("fixtures/invalid/unknown_type.nemo");
    let err = check(source, "unknown_type.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown type"),
        "expected 'unknown type' in error, got: {}",
        msg
    );
}

#[test]
fn duplicate_stage_message() {
    let source = include_str!("fixtures/invalid/duplicate_stage.nemo");
    let err = check(source, "duplicate_stage.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate stage name"),
        "expected 'duplicate stage name' in error, got: {}",
        msg
    );
}

#[test]
fn ambiguous_backward_ref_message() {
    let source = include_str!("fixtures/invalid/ambiguous_backward_ref.nemo");
    let err = check(source, "ambiguous_backward_ref.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("ambiguous backward-reference"),
        "expected 'ambiguous backward-reference' in error, got: {}",
        msg
    );
}

#[test]
fn policy_unknown_input_message() {
    let source = include_str!("fixtures/invalid/policy_unknown_input.nemo");
    let err = check(source, "policy_unknown_input.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown ref") && msg.contains("policy"),
        "expected 'unknown ref' and 'policy' in error, got: {}",
        msg
    );
}

#[test]
fn optional_workflow_input_message() {
    let source = include_str!("fixtures/invalid/optional_workflow_input.nemo");
    let err = check(source, "optional_workflow_input.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be optional"),
        "expected 'cannot be optional' in error, got: {}",
        msg
    );
}

#[test]
fn optional_bool_branch_message() {
    let source = include_str!("fixtures/invalid/optional_bool_branch.nemo");
    let err = check(source, "optional_bool_branch.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("optional bool field"),
        "expected 'optional bool field' in error, got: {}",
        msg
    );
}

#[test]
fn multiple_bool_branches_message() {
    let source = include_str!("fixtures/invalid/multiple_bool_branches.nemo");
    let err = check(source, "multiple_bool_branches.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("bool branch output fields"),
        "expected 'bool branch output fields' in error, got: {}",
        msg
    );
}

#[test]
fn unknown_capability_stage_message() {
    let source = include_str!("fixtures/invalid/unknown_capability_stage.nemo");
    let err = check(source, "unknown_capability_stage.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown capability"),
        "expected 'unknown capability' in error, got: {}",
        msg
    );
}

#[test]
fn unknown_capability_policy_message() {
    let source = include_str!("fixtures/invalid/unknown_capability_policy.nemo");
    let err = check(source, "unknown_capability_policy.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown capability"),
        "expected 'unknown capability' in error, got: {}",
        msg
    );
}

#[test]
fn numeric_eq_operator_message() {
    let source = include_str!("fixtures/invalid/numeric_eq_operator.nemo");
    let err = check(source, "numeric_eq_operator.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("numeric equality") && msg.contains("ordering predicates"),
        "expected 'numeric equality' and 'ordering predicates' in error, got: {}",
        msg
    );
}

#[test]
fn transition_and_nonbool_operand_message() {
    let source = include_str!("fixtures/invalid/transition_and_nonbool_operand.nemo");
    let err = check(source, "transition_and_nonbool_operand.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("bool") || msg.contains("bool operands"),
        "expected 'bool' in error, got: {}",
        msg
    );
}

#[test]
fn transition_not_nonbool_message() {
    let source = include_str!("fixtures/invalid/transition_not_nonbool.nemo");
    let err = check(source, "transition_not_nonbool.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not") && msg.contains("bool"),
        "expected 'not' and 'bool' in error, got: {}",
        msg
    );
}

#[test]
fn transition_numeric_eq_message() {
    let source = include_str!("fixtures/invalid/transition_numeric_eq.nemo");
    let err = check(source, "transition_numeric_eq.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("number") && msg.contains("compare predicate"),
        "expected 'number' and 'compare predicate' in error, got: {}",
        msg
    );
}

#[test]
fn transition_numeric_in_message() {
    let source = include_str!("fixtures/invalid/transition_numeric_in.nemo");
    let err = check(source, "transition_numeric_in.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("number"),
        "expected 'number' in error, got: {}",
        msg
    );
}

#[test]
fn transition_multiple_else_message() {
    let source = include_str!("fixtures/invalid/transition_multiple_else.nemo");
    let err = check(source, "transition_multiple_else.nemo").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("at most one") && msg.contains("else"),
        "expected 'at most one' and 'else' in error, got: {}",
        msg
    );
}
