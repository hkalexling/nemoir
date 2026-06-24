use nemoir_dsl_fe::lower;

#[test]
fn lower_coding_agent_golden() {
    let source = include_str!("fixtures/coding-agent.nemo");
    let ir = lower(source, "coding-agent.nemo").expect("lowering should succeed");

    let generated_yaml = serde_yaml::to_string(&ir).expect("YAML serialization should succeed");

    let expected_yaml = include_str!("fixtures/coding-agent-ir.yml");

    // Parse both as generic YAML values and compare structurally
    let generated: serde_yaml::Value =
        serde_yaml::from_str(&generated_yaml).expect("generated YAML should be valid");
    let expected: serde_yaml::Value =
        serde_yaml::from_str(expected_yaml).expect("expected YAML should be valid");

    if generated != expected {
        // Print diff for debugging
        eprintln!("=== GENERATED YAML ===");
        eprintln!("{}", generated_yaml);
        eprintln!("=== EXPECTED YAML ===");
        eprintln!("{}", expected_yaml);
        panic!("generated YAML does not match expected YAML");
    }
}

#[test]
fn lower_policy_command_allowlist() {
    // Verify the target syntax from docs/dsl-and-ir.md §8 lowers and produces And/Or IR nodes.
    let source = include_str!("fixtures/policy_command_allowlist.nemo");
    let ir = lower(source, "policy_command_allowlist.nemo").expect("lowering should succeed");
    assert_eq!(ir.policies.len(), 6, "should have 6 policies");

    // Policy 4 (zero-indexed): the shell allowlist with Or
    // deny os.shell(command) if not (command.eq(...) or command.starts_with(...) or ...)
    let shell_policy = &ir.policies[3];
    assert_eq!(shell_policy.kind, "deny");
    let cond = shell_policy
        .condition
        .as_ref()
        .expect("should have condition");
    // Should be Not(Or(eq, starts_with, starts_with))
    if let nemoir_ir::Expr::Not { expr } = cond {
        if let nemoir_ir::Expr::Or { exprs } = expr.as_ref() {
            assert_eq!(exprs.len(), 3, "shell allowlist should have 3 disjuncts");
        } else {
            panic!("expected Or, got {:?}", expr);
        }
    } else {
        panic!("expected Not, got {:?}", cond);
    }

    // Policy 5: the `in [...]` sugar lowered to Or of eq
    let in_policy = &ir.policies[5];
    assert_eq!(in_policy.kind, "deny");
    let cond_in = in_policy.condition.as_ref().expect("should have condition");
    // Should be Not(Or(eq(candidate_path), eq("candidate_bak.py")))
    if let nemoir_ir::Expr::Not { expr } = cond_in {
        if let nemoir_ir::Expr::Or { exprs } = expr.as_ref() {
            assert_eq!(exprs.len(), 2, "in [...] should lower to 2 eq disjuncts");
            // Both should be MethodCall(eq)
            for e in exprs {
                if let nemoir_ir::Expr::MethodCall { method, .. } = e {
                    assert_eq!(method, "eq");
                } else {
                    panic!("expected MethodCall in in-lowered Or, got {:?}", e);
                }
            }
        } else {
            panic!("expected Or from in-lowering, got {:?}", expr);
        }
    } else {
        panic!("expected Not from in-lowering, got {:?}", cond_in);
    }
}
