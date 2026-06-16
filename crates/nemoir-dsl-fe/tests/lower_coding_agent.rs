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
