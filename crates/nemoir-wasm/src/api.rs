//! Public request / response types and the top-level `wasm-bindgen` exports.
//!
//! The three entry points are:
//!
//! - [`analyze`] — parses, lowers, validates; returns diagnostics ± IR (debounced while editing).
//! - [`generate`] — repeats analysis then invokes the selected backend; returns artifact files.
//! - [`metadata`] — returns compiler / IR version information.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::artifacts::Artifact;
use crate::diagnostics::CompilerDiagnostic;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Backend target for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// Validated IR JSON export (no generated source ZIP).
    None,
    /// Standalone workflow visualization HTML.
    Visualizer,
    /// Installable Python package source tree.
    Python,
    /// Vite / TypeScript browser-app source tree.
    Web,
}

/// Payload for [`analyze`] and [`generate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileRequest {
    /// LF-normalized `.nemo` source text.
    pub source: String,

    /// Display name only — never a host filesystem path.
    #[serde(default = "default_filename")]
    pub filename: String,

    /// Target backend (defaults to `none` for analysis).
    #[serde(default)]
    pub target: Option<Target>,

    /// When `true`, include the lowered [`WorkflowIr`] JSON in the response.
    #[serde(default)]
    pub include_ir: bool,

    /// Reserved for documented backend options; default values apply initially.
    #[serde(default)]
    pub options: serde_json::Value,
}

fn default_filename() -> String {
    "workflow.nemo".to_owned()
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Common response fields returned by both [`analyze`] and [`generate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    /// `true` when the workflow is valid and (for generation) the backend
    /// produced an artifact.
    pub ok: bool,

    /// Compiler crate version, e.g. `"0.1.0"`.
    pub compiler_version: String,

    /// IR schema version string when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir_version: Option<String>,

    /// Serialised [`WorkflowIr`] when requested and lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<serde_json::Value>,

    /// Structured diagnostics collected during parsing, validation, and
    /// backend processing.
    pub diagnostics: Vec<CompilerDiagnostic>,
}

/// Generation response extends [`AnalyzeResponse`] with an optional artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResponse {
    #[serde(flatten)]
    pub analysis: AnalyzeResponse,

    /// Generated artifact (present only when `ok` and target ≠ `none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
}

/// Read-only compiler metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerMetadata {
    pub compiler_version: String,
    pub ir_version: String,
    pub supported_targets: Vec<String>,
}

// ---------------------------------------------------------------------------
// Serialization helper
// ---------------------------------------------------------------------------

/// Serialize `value` to a [`JsValue`] using a JSON-compatible serializer
/// that emits plain JavaScript objects instead of ES2015 `Map`s.
///
/// The default `serde_wasm_bindgen::to_value` serialises `#[serde(flatten)]`
/// struct fields and `serde_json::Value` as `Map` instances, which break
/// `structuredClone`, `JSON.stringify`, and normal property access.  This
/// helper configures the serializer to always produce plain objects.
fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, serde_wasm_bindgen::Error> {
    let ser = serde_wasm_bindgen::Serializer::json_compatible();
    value.serialize(&ser)
}

// ---------------------------------------------------------------------------
// wasm-bindgen exports
// ---------------------------------------------------------------------------

/// Analyse `.nemo` source: parse, lower, validate, and optionally return IR.
///
/// Called on a debounce while the user edits. Returns structured diagnostics
/// the editor renders as markers and panel entries.
#[wasm_bindgen]
pub fn analyze(request: JsValue) -> Result<JsValue, JsValue> {
    let req: CompileRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|e| JsValue::from_str(&format!("invalid request: {}", e)))?;
    let resp = crate::pipeline::analyze(&req.source, &req.filename, req.include_ir);
    to_js_value(&resp).map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Generate a download artifact from valid `.nemo` source.
///
/// Called only on a deliberate user action (e.g. "Download ZIP").
/// Re-runs the full analysis pipeline, then dispatches to the selected
/// backend.
#[wasm_bindgen]
pub fn generate(request: JsValue) -> Result<JsValue, JsValue> {
    let req: CompileRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|e| JsValue::from_str(&format!("invalid request: {}", e)))?;
    let target = req.target.unwrap_or(Target::None);
    let resp = crate::pipeline::generate(&req.source, &req.filename, target, req.include_ir);
    to_js_value(&resp).map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Return compiler and IR version metadata for the about / debug view.
#[wasm_bindgen]
pub fn metadata() -> Result<JsValue, JsValue> {
    let resp = crate::pipeline::metadata();
    to_js_value(&resp).map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}
