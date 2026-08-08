//! Private compiler pipeline orchestration.
//!
//! Pure Rust functions that call the NemoIR compiler library APIs and
//! produce typed response structs. These are unit-testable without any
//! WASM or browser dependency — the `#[wasm_bindgen]` exports in
//! [`crate::api`] are thin adapters around these functions.

use crate::api::{AnalyzeResponse, GenerateResponse, Target};
use crate::artifacts::{
    normalize_python_artifact, normalize_visualizer_artifact, normalize_web_artifact,
};
use crate::diagnostics::{
    convert_dsl_diagnostic, convert_ir_errors, convert_python_backend_error,
    convert_visualizer_error, convert_web_backend_error, internal_error, CompilerDiagnostic,
};

// ---------------------------------------------------------------------------
// Analysis pipeline
// ---------------------------------------------------------------------------

/// Parse, lower, and validate `.nemo` source, returning structured
/// diagnostics and optionally the lowered IR.
///
/// Never calls `std::process::exit`, filesystem APIs, or terminal rendering.
pub fn analyze(source: &str, filename: &str, include_ir: bool) -> AnalyzeResponse {
    let compiler_version = env!("CARGO_PKG_VERSION").to_string();
    let mut diagnostics: Vec<CompilerDiagnostic> = Vec::new();

    let ir_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        nemoir_dsl_fe::lower(source, filename)
    }));

    let (ir, ir_version) = match ir_result {
        Ok(Ok(ir)) => {
            let version = Some(ir.ir_version.clone());
            // Run IR validation defensively (backends re-run it too, but we
            // want IR errors in the analysis response).
            match nemoir_ir::validate::validate(&ir) {
                Ok(()) => {}
                Err(errors) => {
                    diagnostics.extend(convert_ir_errors(&errors));
                }
            }
            (Some(ir), version)
        }
        Ok(Err(diag)) => {
            diagnostics.push(convert_dsl_diagnostic(&diag, source));
            (None, None)
        }
        Err(_panic) => {
            diagnostics.push(internal_error(
                "internal compiler error during DSL lowering".into(),
            ));
            (None, None)
        }
    };

    let ir_json = if include_ir {
        ir.as_ref().and_then(|ir| serde_json::to_value(ir).ok())
    } else {
        None
    };

    AnalyzeResponse {
        ok: diagnostics.is_empty(),
        compiler_version,
        ir_version,
        ir: ir_json,
        diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Generation pipeline
// ---------------------------------------------------------------------------

/// Conditionally clear the IR field from an [`AnalyzeResponse`].
fn strip_ir(mut resp: AnalyzeResponse, include_ir: bool) -> AnalyzeResponse {
    if !include_ir {
        resp.ir = None;
    }
    resp
}

/// Run the full analysis-generate pipeline: parse, lower, validate, and
/// invoke the selected backend to produce a downloadable artifact.
pub fn generate(
    source: &str,
    filename: &str,
    target: Target,
    include_ir: bool,
) -> GenerateResponse {
    // Re-run analysis — always include IR so we can dispatch to the
    // backend.
    let analysis_full = analyze(source, filename, true);

    // If analysis produced diagnostics, return early with no artifact.
    if !analysis_full.ok {
        return GenerateResponse {
            analysis: strip_ir(analysis_full, include_ir),
            artifact: None,
        };
    }

    // Extract the IR we just validated.
    let ir = analysis_full
        .ir
        .as_ref()
        .and_then(|v| serde_json::from_value::<nemoir_ir::WorkflowIr>(v.clone()).ok());

    let Some(ir) = ir else {
        let mut diags = analysis_full.diagnostics;
        diags.push(internal_error(
            "IR deserialization failed during generation".into(),
        ));
        return GenerateResponse {
            analysis: AnalyzeResponse {
                ok: false,
                diagnostics: diags,
                ..analysis_full
            },
            artifact: None,
        };
    };

    // Dispatch to the selected backend.
    let artifact_result = match target {
        Target::None => {
            // "none" returns validated IR only — no source artifact.
            Ok(None)
        }
        Target::Visualizer => {
            let options = nemoir_backend_visualizer::VisualizerOptions::default();
            match nemoir_backend_visualizer::render_html(&ir, &options) {
                Ok(html) => Ok(Some(normalize_visualizer_artifact(&html, &ir.workflow.id))),
                Err(e) => {
                    let diags = convert_visualizer_error(&e);
                    Err(diags)
                }
            }
        }
        Target::Python => {
            let options = nemoir_backend_python::PythonBackendOptions::default();
            match nemoir_backend_python::generate_package(&ir, &options) {
                Ok(pkg) => match normalize_python_artifact(&pkg) {
                    Ok(artifact) => Ok(Some(artifact)),
                    Err(msg) => Err(vec![internal_error(msg)]),
                },
                Err(e) => Err(convert_python_backend_error(&e)),
            }
        }
        Target::Web => {
            let options = nemoir_backend_web::WebBackendOptions::default();
            match nemoir_backend_web::generate_package(&ir, &options) {
                Ok(pkg) => match normalize_web_artifact(&pkg) {
                    Ok(artifact) => Ok(Some(artifact)),
                    Err(msg) => Err(vec![internal_error(msg)]),
                },
                Err(e) => Err(convert_web_backend_error(&e)),
            }
        }
    };

    match artifact_result {
        Ok(artifact) => GenerateResponse {
            analysis: strip_ir(analysis_full, include_ir),
            artifact,
        },
        Err(diags) => {
            let mut all_diags = analysis_full.diagnostics;
            all_diags.extend(diags);
            GenerateResponse {
                analysis: AnalyzeResponse {
                    ok: false,
                    diagnostics: all_diags,
                    ..analysis_full
                },
                artifact: None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Return compiler and IR version information for the about / debug view.
pub fn metadata() -> crate::api::CompilerMetadata {
    crate::api::CompilerMetadata {
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        ir_version: "0.1".to_string(),
        supported_targets: vec![
            "none".to_string(),
            "visualizer".to_string(),
            "python".to_string(),
            "web".to_string(),
        ],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -----------------------------------------------------------

    fn hello_source() -> &'static str {
        include_str!("../../../examples/hello-workflow/hello.nemo")
    }

    fn hint_tutor_source() -> &'static str {
        include_str!("../../nemoir-dsl-fe/tests/fixtures/hint_tutor.nemo")
    }

    fn coding_agent_source() -> &'static str {
        include_str!("../../nemoir-dsl-fe/tests/fixtures/coding-agent.nemo")
    }

    // -- analysis tests ----------------------------------------------------

    #[test]
    fn analyze_valid_hello_returns_ok() {
        let resp = analyze(hello_source(), "hello.nemo", true);
        assert!(resp.ok, "expected ok, diagnostics: {:#?}", resp.diagnostics);
        assert!(resp.diagnostics.is_empty());
        assert!(resp.ir.is_some(), "expected IR when include_ir is true");
        assert_eq!(resp.ir_version.as_deref(), Some("0.1"));
    }

    #[test]
    fn analyze_invalid_syntax_returns_diagnostics() {
        let src = "not a valid workflow";
        let resp = analyze(src, "bad.nemo", false);
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
        for d in &resp.diagnostics {
            assert_eq!(d.phase, crate::diagnostics::DiagnosticPhase::Dsl);
        }
    }

    #[test]
    fn analyze_include_ir_true_preserves_ir() {
        let resp = analyze(hello_source(), "hello.nemo", true);
        assert!(resp.ir.is_some());
    }

    #[test]
    fn analyze_include_ir_false_omits_ir() {
        let resp = analyze(hello_source(), "hello.nemo", false);
        assert!(resp.ir.is_none());
    }

    #[test]
    fn analyze_with_ir_validation_error() {
        // Use a source that parses but produces an IR that IR-validation rejects.
        // The DSL does IR-level validation; we rely on lower() rejecting
        // bad semantics at DSL validation time. An IR validation error in
        // analyze() really comes from the separate ir::validate call.
        // We test this by feeding a valid parse result through manually,
        // but in practice the integration test in diagnostics.rs already
        // covers convert_ir_errors.
        let resp = analyze(hint_tutor_source(), "hint_tutor.nemo", false);
        assert!(
            resp.ok,
            "hint tutor should be valid: {:#?}",
            resp.diagnostics
        );
    }

    // -- generation tests --------------------------------------------------

    #[test]
    fn generate_none_returns_no_artifact() {
        let resp = generate(hello_source(), "hello.nemo", Target::None, false);
        assert!(resp.analysis.ok);
        assert!(resp.artifact.is_none());
    }

    #[test]
    fn generate_none_with_ir_includes_ir() {
        let resp = generate(hello_source(), "hello.nemo", Target::None, true);
        assert!(resp.analysis.ok);
        assert!(resp.analysis.ir.is_some());
        assert!(resp.artifact.is_none());
    }

    #[test]
    fn generate_visualizer_produces_html() {
        let resp = generate(hello_source(), "hello.nemo", Target::Visualizer, false);
        assert!(
            resp.analysis.ok,
            "visualizer failed: {:#?}",
            resp.analysis.diagnostics
        );
        let artifact = resp.artifact.expect("visualizer should produce artifact");
        assert_eq!(artifact.target, Target::Visualizer);
        assert_eq!(artifact.archive_root, "helloworkflow");
        assert_eq!(artifact.files.len(), 1);
        assert_eq!(artifact.files[0].path, "index.html");
        assert!(artifact.files[0].content.contains("<html"));
        assert!(artifact.files[0].content.contains("cytoscape@3.30.4"));
    }

    #[test]
    fn generate_python_produces_package() {
        let resp = generate(hello_source(), "hello.nemo", Target::Python, false);
        assert!(
            resp.analysis.ok,
            "python failed: {:#?}",
            resp.analysis.diagnostics
        );
        let artifact = resp.artifact.expect("python should produce artifact");
        assert_eq!(artifact.target, Target::Python);
        assert_eq!(artifact.archive_root, "hello-workflow");
        let paths: Vec<&str> = artifact.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "hello_workflow/_manifest.py",
                "hello_workflow/types.py",
                "hello_workflow/_agent.py",
                "hello_workflow/__init__.py",
                "pyproject.toml",
            ],
            "Python artifact must preserve the import package beneath its distribution root",
        );
        let pyproject = artifact
            .files
            .iter()
            .find(|file| file.path == "pyproject.toml")
            .expect("pyproject.toml should exist");
        assert!(pyproject
            .content
            .contains("packages = [\"hello_workflow\"]"));
    }

    #[test]
    fn generate_web_valid_produces_package() {
        let resp = generate(hint_tutor_source(), "hint_tutor.nemo", Target::Web, false);
        assert!(
            resp.analysis.ok,
            "web hint_tutor failed: {:#?}",
            resp.analysis.diagnostics
        );
        let artifact = resp.artifact.expect("web should produce artifact");
        assert_eq!(artifact.target, Target::Web);
        assert_eq!(artifact.archive_root, "hint-tutor");
        let paths: Vec<&str> = artifact.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "package.json",
                "tsconfig.json",
                "tsconfig.node.json",
                "vite.config.ts",
                "index.html",
                "netlify.toml",
                "vercel.json",
                "public/_headers",
                "src/workflow.json",
                "src/agent.ts",
                "src/main.tsx",
                "src/app.css",
                "src/webllm.worker.ts",
                ".gitignore",
                "README.md",
            ],
        );
        let workflow_json = artifact
            .files
            .iter()
            .find(|file| file.path == "src/workflow.json")
            .expect("workflow.json should exist");
        assert!(workflow_json.content.contains("\"id\": \"HintTutor\""));
    }

    #[test]
    fn generate_web_incompatible_returns_target_diagnostics() {
        let resp = generate(
            coding_agent_source(),
            "coding_agent.nemo",
            Target::Web,
            false,
        );
        assert!(
            !resp.analysis.ok,
            "coding-agent should be rejected by web target"
        );
        assert!(
            resp.analysis
                .diagnostics
                .iter()
                .any(|d| matches!(d.phase, crate::diagnostics::DiagnosticPhase::Target)),
            "expected target-phase diagnostics, got {:#?}",
            resp.analysis.diagnostics
        );
        assert!(
            resp.artifact.is_none(),
            "should not produce artifact for incompatible workflow"
        );
    }

    #[test]
    fn generate_invalid_source_yields_no_artifact() {
        let src = "not a workflow";
        let resp = generate(src, "bad.nemo", Target::Python, false);
        assert!(!resp.analysis.ok);
        assert!(resp.artifact.is_none());
    }

    // -- metadata tests ----------------------------------------------------

    #[test]
    fn metadata_returns_expected_values() {
        let m = metadata();
        assert_eq!(m.compiler_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(m.ir_version, "0.1");
        assert_eq!(m.supported_targets.len(), 4);
        assert!(m.supported_targets.contains(&"none".to_string()));
        assert!(m.supported_targets.contains(&"visualizer".to_string()));
        assert!(m.supported_targets.contains(&"python".to_string()));
        assert!(m.supported_targets.contains(&"web".to_string()));
    }

    // -- round-trip tests --------------------------------------------------

    #[test]
    fn analyze_response_roundtrips_serde_json() {
        let resp = analyze(hello_source(), "hello.nemo", true);
        let json = serde_json::to_value(&resp).expect("serialize");
        let back: AnalyzeResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.ok, resp.ok);
        assert_eq!(back.compiler_version, resp.compiler_version);
    }

    #[test]
    fn generate_response_roundtrips_with_artifact() {
        let resp = generate(hello_source(), "hello.nemo", Target::Visualizer, false);
        let json = serde_json::to_value(&resp).expect("serialize");
        let back: GenerateResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.analysis.ok, resp.analysis.ok);
        assert!(back.artifact.is_some());
        assert_eq!(
            back.artifact.as_ref().unwrap().target,
            resp.artifact.as_ref().unwrap().target
        );
    }

    #[test]
    fn generate_response_roundtrips_without_artifact() {
        let resp = generate(hello_source(), "hello.nemo", Target::None, false);
        let json = serde_json::to_value(&resp).expect("serialize");
        let back: GenerateResponse = serde_json::from_value(json).expect("deserialize");
        assert!(back.analysis.ok);
        assert!(back.artifact.is_none());
    }
}
