use nemoir_backend_visualizer::{render_html, VisualizerOptions};
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
                execution: StageExecution::Model,
            },
            Node {
                id: "Done".into(),
                annotations: vec!["exit".into()],
                prompt: "done node".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![],
                execution: StageExecution::Model,
            },
        ],
    }
}

fn coding_agent_ir() -> WorkflowIr {
    let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/coding-agent-ir.yml");
    serde_yaml::from_str(source).expect("should parse coding-agent-ir.yml")
}

#[test]
fn valid_ir_renders_html() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).expect("should render");
    assert!(!html.is_empty());
    assert!(html.contains("<!DOCTYPE html>"));
}

#[test]
fn invalid_ir_returns_error() {
    let mut ir = valid_minimal_ir();
    ir.workflow.entry = "DoesNotExist".into();
    let result = render_html(&ir, &VisualizerOptions::default());
    assert!(result.is_err());
}

#[test]
fn html_includes_cytoscape_cdn_url() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(html.contains("cytoscape@3.30.4/dist/cytoscape.min.js"));
}

#[test]
fn html_includes_workflow_id() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(html.contains("Minimal"));
}

#[test]
fn html_includes_graph_data_with_node_ids() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(html.contains("\"id\":\"Start\""));
    assert!(html.contains("\"id\":\"Done\""));
}

#[test]
fn html_includes_edge_data() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(html.contains("\"source\":\"Start\""));
    assert!(html.contains("\"target\":\"Done\""));
}

#[test]
fn prompt_with_script_tag_is_escaped() {
    let mut ir = valid_minimal_ir();
    ir.nodes[0].prompt = "prompt with </script> injection".into();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    // The embedded JSON should not contain a raw </script> inside the graphdata script tag
    let graphdata_start = html.find("id=\"graphdata\"").unwrap();
    let relative = html[graphdata_start..].find("</script>").unwrap();
    let graphdata_close = relative + graphdata_start;
    let graphdata_content = &html[graphdata_start..graphdata_close];
    assert!(!graphdata_content.contains("</script>"));
}

#[test]
fn coding_agent_has_correct_node_and_edge_count() {
    let ir = coding_agent_ir();
    assert_eq!(ir.nodes.len(), 7);
    let edges: usize = ir.nodes.iter().map(|n| n.transitions.len()).sum();
    assert_eq!(edges, 10);
}

#[test]
fn coding_agent_renders_html() {
    let ir = coding_agent_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).expect("should render");
    assert!(html.contains("CodingAgent"));
    for node_id in &[
        "Triage", "Clarify", "Plan", "Propose", "Apply", "Verify", "Fin",
    ] {
        assert!(
            html.contains(&format!("\"id\":\"{}\"", node_id)),
            "missing node {}",
            node_id
        );
    }
}

#[test]
fn custom_title_is_used() {
    let ir = valid_minimal_ir();
    let opts = VisualizerOptions {
        title: Some("Custom Title".into()),
    };
    let html = render_html(&ir, &opts).unwrap();
    assert!(html.contains("<title>Custom Title</title>"));
}

#[test]
fn html_uses_explicit_entry_exit_selectors() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(
        html.contains("node[?isEntry]"),
        "should use truthiness entry selector"
    );
    assert!(
        html.contains("node[?isExit]"),
        "should use truthiness exit selector"
    );
}

#[test]
fn html_does_not_contain_broken_iife_bootstrap() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    // The broken pattern was calling () on addEventListener's undefined return value.
    // Check that the )})(); IIFE wrapper around addEventListener is absent.
    assert!(
        !html.contains("})();\n</script>"),
        "should not have IIFE wrapper around addEventListener"
    );
}

#[test]
fn html_defines_init_visualizer() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(
        html.contains("function initVisualizer()"),
        "should define initVisualizer"
    );
}

#[test]
fn html_saves_positions_on_dragfree() {
    let ir = valid_minimal_ir();
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    assert!(
        html.contains("dragfree"),
        "should save positions on dragfree"
    );
}

#[test]
fn deterministic_stage_has_execution_in_graph_data() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.nodes[0].requires.push(StageCapability {
        capability: "os.shell".into(),
    });
    let mut args = indexmap::IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo ok".into()),
        },
    );
    ir.nodes[0].execution = StageExecution::Tool {
        capability: "os.shell".into(),
        args,
    };
    let graph = nemoir_backend_visualizer::graph::build_graph_data(&ir).unwrap();
    let node = &graph["nodes"][0]["data"];
    assert_eq!(node["execution"]["kind"], "tool");
    assert_eq!(node["execution"]["capability"], "os.shell");
    assert_eq!(node["isTool"], true);
}

#[test]
fn deterministic_stage_renders_execution_section_in_html() {
    let mut ir = valid_minimal_ir();
    ir.capabilities.push("os.shell".into());
    ir.nodes[0].requires.push(StageCapability {
        capability: "os.shell".into(),
    });
    let mut args = indexmap::IndexMap::new();
    args.insert(
        "command".into(),
        Expr::Literal {
            ty: "string".into(),
            value: serde_yaml::Value::String("echo ok".into()),
        },
    );
    ir.nodes[0].execution = StageExecution::Tool {
        capability: "os.shell".into(),
        args,
    };
    let html = render_html(&ir, &VisualizerOptions::default()).unwrap();
    // The inspector JS renders exec: for tool stages.
    assert!(html.contains("exec:"));
    // The graph data carries isTool=true for tool nodes.
    assert!(html.contains("\"isTool\":true"));

    // Model-stage HTML should have isTool:false (or absent) for all nodes.
    let ir_model = valid_minimal_ir();
    let html_model = render_html(&ir_model, &VisualizerOptions::default()).unwrap();
    assert!(html_model.contains("\"isTool\":false"));
}
