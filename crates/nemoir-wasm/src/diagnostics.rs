//! Diagnostic conversion: Rust byte-offset errors → editor-ready structures.
//!
//! The compiler core produces errors with byte-offset source labels. Monaco
//! expects 1-based line numbers and UTF-16 column offsets. This module maps
//! between the two and returns stable [`CompilerDiagnostic`] values for every
//! phase of the pipeline.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Source positions
// ---------------------------------------------------------------------------

/// 1-based source position with a UTF-16 column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
    /// 1-based line number.
    pub line: u32,

    /// 1-based UTF-16 column offset.
    pub utf16_column: u32,
}

/// Start → end range in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Classification of which compiler phase produced the diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticPhase {
    /// DSL parser / resolver / type / shape / transition / graph errors.
    Dsl,

    /// IR structural validation errors.
    Ir,

    /// Backend-specific validation or code-generation errors.
    Target,

    /// Unrecoverable bridge / serialisation failure.
    Internal,
}

/// A single structured diagnostic the editor can render as a marker, panel
/// entry, or both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerDiagnostic {
    /// Which compiler phase produced this diagnostic.
    pub phase: DiagnosticPhase,

    /// Always `"error"` in the initial product.
    pub severity: String,

    /// Human-readable error message.
    pub message: String,

    /// Optional help / hint text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,

    /// Machine-readable error code when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Source range for editor squiggles (present for DSL diagnostics with a
    /// valid span; absent for IR / target / internal errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<SourceRange>,
}

// ---------------------------------------------------------------------------
// Byte → UTF-16 conversion
// ---------------------------------------------------------------------------

/// Convert a byte offset in LF-normalised UTF-8 source to a 1-based
/// [`SourcePosition`].
///
/// Walks valid UTF-8 characters from the start through `byte_offset`,
/// incrementing `line` at `\n` and `utf16_column` by `char.len_utf16()`.
/// Clamps malformed or out-of-bounds offsets safely to the nearest valid
/// position rather than panicking.
pub fn byte_to_position(source: &str, byte_offset: usize) -> SourcePosition {
    let clamped = byte_offset.min(source.len());
    // Walk to `clamped` using char boundaries; if `clamped` lands inside a
    // multi-byte char, walk to the previous char boundary.
    let safe_prefix = {
        let mut boundary = clamped;
        while boundary > 0 && !source.is_char_boundary(boundary) {
            boundary -= 1;
        }
        &source[..boundary]
    };

    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for ch in safe_prefix.chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += ch.len_utf16() as u32;
        }
    }

    SourcePosition {
        line,
        utf16_column: col,
    }
}

/// Convert a byte range `[start, end)` in LF-normalised UTF-8 source to a
/// [`SourceRange`].
///
/// Handles reversed ranges by swapping `start`/`end`, and clamps both
/// offsets before conversion.
pub fn byte_range_to_range(source: &str, start: usize, end: usize) -> SourceRange {
    let s = start.min(end);
    let e = start.max(end);
    SourceRange {
        start: byte_to_position(source, s),
        end: byte_to_position(source, e),
    }
}

// ---------------------------------------------------------------------------
// Diagnostic conversion
// ---------------------------------------------------------------------------

/// Convert a DSL frontend [`nemoir_dsl_fe::Diagnostic`] into a
/// [`CompilerDiagnostic`] with a source range when the error carries a
/// byte-offset label.
pub fn convert_dsl_diagnostic(
    diag: &nemoir_dsl_fe::diagnostics::Diagnostic,
    source: &str,
) -> CompilerDiagnostic {
    use nemoir_dsl_fe::diagnostics::Diagnostic;

    match diag {
        Diagnostic::ParseError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: e.help.clone(),
            code: Some("parse".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
        Diagnostic::NameError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: e.help.clone(),
            code: Some("name".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
        Diagnostic::TypeError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: None,
            code: Some("type".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
        Diagnostic::ShapeError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: e.help.clone(),
            code: Some("shape".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
        Diagnostic::TransitionError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: e.help.clone(),
            code: Some("transition".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
        Diagnostic::GraphError(e) => CompilerDiagnostic {
            phase: DiagnosticPhase::Dsl,
            severity: "error".into(),
            message: e.message.clone(),
            help: e.help.clone(),
            code: Some("graph".into()),
            range: e
                .label
                .as_ref()
                .map(|(s, e, _)| byte_range_to_range(source, *s, *e)),
        },
    }
}

/// Convert IR validation errors into a list of unstructured [`CompilerDiagnostic`]
/// values (no source ranges).
pub fn convert_ir_errors(
    errors: &nemoir_ir::validate::ValidationErrors,
) -> Vec<CompilerDiagnostic> {
    errors
        .errors
        .iter()
        .map(|e| CompilerDiagnostic {
            phase: DiagnosticPhase::Ir,
            severity: "error".into(),
            message: format!("{}: {}", e.path, e.message),
            help: None,
            code: None,
            range: None,
        })
        .collect()
}

/// Build a single [`CompilerDiagnostic`] for an internal / unrecoverable
/// bridge failure.
pub fn internal_error(message: String) -> CompilerDiagnostic {
    CompilerDiagnostic {
        phase: DiagnosticPhase::Internal,
        severity: "error".into(),
        message,
        help: None,
        code: None,
        range: None,
    }
}

/// Convert a Python backend error into one or more [`CompilerDiagnostic`]
/// values.
pub fn convert_python_backend_error(
    err: &nemoir_backend_python::PythonBackendError,
) -> Vec<CompilerDiagnostic> {
    match err {
        nemoir_backend_python::PythonBackendError::ValidationFailed(e) => convert_ir_errors(e)
            .into_iter()
            .map(|mut d| {
                d.phase = DiagnosticPhase::Target;
                d
            })
            .collect(),
        other => vec![CompilerDiagnostic {
            phase: DiagnosticPhase::Target,
            severity: "error".into(),
            message: other.to_string(),
            help: None,
            code: None,
            range: None,
        }],
    }
}

/// Convert a web backend error into one or more [`CompilerDiagnostic`]
/// values. Aggregated `UnsupportedForWebTarget` errors are split into one
/// diagnostic per violation line.
pub fn convert_web_backend_error(
    err: &nemoir_backend_web::WebBackendError,
) -> Vec<CompilerDiagnostic> {
    match err {
        nemoir_backend_web::WebBackendError::ValidationFailed(e) => convert_ir_errors(e)
            .into_iter()
            .map(|mut d| {
                d.phase = DiagnosticPhase::Target;
                d
            })
            .collect(),
        nemoir_backend_web::WebBackendError::UnsupportedForWebTarget(msg) => msg
            .lines()
            .map(|line| {
                // Strip the "web-target-error: " prefix the backend prepends.
                let cleaned = line.strip_prefix("web-target-error: ").unwrap_or(line);
                CompilerDiagnostic {
                    phase: DiagnosticPhase::Target,
                    severity: "error".into(),
                    message: cleaned.to_string(),
                    help: None,
                    code: None,
                    range: None,
                }
            })
            .collect(),
        other => vec![CompilerDiagnostic {
            phase: DiagnosticPhase::Target,
            severity: "error".into(),
            message: other.to_string(),
            help: None,
            code: None,
            range: None,
        }],
    }
}

/// Convert a visualizer backend error into one or more
/// [`CompilerDiagnostic`] values.
pub fn convert_visualizer_error(
    err: &nemoir_backend_visualizer::VisualizerError,
) -> Vec<CompilerDiagnostic> {
    match err {
        nemoir_backend_visualizer::VisualizerError::ValidationFailed(e) => convert_ir_errors(e)
            .into_iter()
            .map(|mut d| {
                d.phase = DiagnosticPhase::Target;
                d
            })
            .collect(),
        other => vec![CompilerDiagnostic {
            phase: DiagnosticPhase::Target,
            severity: "error".into(),
            message: other.to_string(),
            help: None,
            code: None,
            range: None,
        }],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_single_line() {
        let src = "hello world";
        let pos = byte_to_position(src, 6); // 'w'
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 7); // 1-based
    }

    #[test]
    fn ascii_multi_line() {
        let src = "line one\nline two\nline three";
        let pos = byte_to_position(src, 14); // 'w' in "line two"
        assert_eq!(pos.line, 2);
        assert_eq!(pos.utf16_column, 6);
    }

    #[test]
    fn emoji_bmp_supplementary() {
        // '🔥' is U+1F525 (4 bytes UTF-8, 2 UTF-16 code units)
        let src = "a🔥b";
        let pos = byte_to_position(src, 5); // 'b' at byte offset 5
        assert_eq!(pos.line, 1);
        // 'a' = 1 col, '🔥' = 2 cols → column 4 (1-based)
        assert_eq!(pos.utf16_column, 4);
    }

    #[test]
    fn clamped_out_of_bounds() {
        let src = "abc";
        let pos = byte_to_position(src, 999);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 4); // end of "abc"
    }

    #[test]
    fn range_basic() {
        let src = "hello\nworld";
        let r = byte_range_to_range(src, 0, 5);
        assert_eq!(
            r.start,
            SourcePosition {
                line: 1,
                utf16_column: 1
            }
        );
        assert_eq!(
            r.end,
            SourcePosition {
                line: 1,
                utf16_column: 6
            }
        );
    }

    #[test]
    fn empty_source() {
        let pos = byte_to_position("", 0);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 1);
    }

    #[test]
    fn reversed_range_swapped() {
        let src = "abc";
        let r = byte_range_to_range(src, 3, 0);
        assert_eq!(
            r.start,
            SourcePosition {
                line: 1,
                utf16_column: 1
            }
        );
        assert_eq!(
            r.end,
            SourcePosition {
                line: 1,
                utf16_column: 4
            }
        );
    }

    #[test]
    fn newline_boundary_position_is_after_newline() {
        // Position at the \n byte itself: line should still be 1 (column after 'e')
        let src = "hello\nworld";
        let pos = byte_to_position(src, 5);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 6);
        // byte 6 is 'w' — line 2 col 1
        let pos2 = byte_to_position(src, 6);
        assert_eq!(pos2.line, 2);
        assert_eq!(pos2.utf16_column, 1);
    }

    #[test]
    fn multi_byte_boundary_clamps() {
        // '🔥' is bytes 1-4, offset 3 is inside the char
        let src = "a🔥b";
        let pos = byte_to_position(src, 3);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 2);
    }

    #[test]
    fn supplementary_at_start() {
        let src = "🔥ab";
        let pos = byte_to_position(src, 4); // 'a' at byte 4
        assert_eq!(pos.line, 1);
        assert_eq!(pos.utf16_column, 3);
    }

    // -- conversion tests --------------------------------------------------

    #[test]
    fn convert_dsl_diagnostic_with_range() {
        let src = "workflow Bad {\n  stage @entry Foo { prompt: \"hi\" }\n";
        let result = nemoir_dsl_fe::lower(src, "test.nemo");
        let err = result.unwrap_err();
        let diag = convert_dsl_diagnostic(&err, src);
        assert_eq!(diag.phase, DiagnosticPhase::Dsl);
        assert!(!diag.message.is_empty(), "message should be non-empty");
        assert!(diag.range.is_some(), "expected a source range");
        let range = diag.range.unwrap();
        assert!(range.start.line >= 1);
        assert!(range.start.utf16_column >= 1);
    }

    #[test]
    fn convert_ir_errors_no_range() {
        let mut ir = nemoir_ir::WorkflowIr::new("test.nemo", "Test", "Entry", vec!["Exit".into()]);
        ir.nodes = vec![
            nemoir_ir::Node {
                id: "Entry".into(),
                annotations: vec!["entry".into()],
                prompt: "hi".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![nemoir_ir::Transition {
                    to: "Exit".into(),
                    priority: 0,
                    reason: "fallthrough".into(),
                    guard: nemoir_ir::Guard::Always,
                }],
                execution: nemoir_ir::StageExecution::Model,
            },
            nemoir_ir::Node {
                id: "Exit".into(),
                annotations: vec!["exit".into()],
                prompt: "done".into(),
                reads: vec![],
                writes: vec![],
                requires: vec![],
                transitions: vec![],
                execution: nemoir_ir::StageExecution::Model,
            },
        ];
        assert!(nemoir_ir::validate::validate(&ir).is_ok());

        // Create a deliberately invalid IR
        ir.ir_version = "999".into();
        let err = nemoir_ir::validate::validate(&ir).unwrap_err();
        let diags = convert_ir_errors(&err);
        assert!(!diags.is_empty());
        for d in &diags {
            assert_eq!(d.phase, DiagnosticPhase::Ir);
            assert!(d.range.is_none());
        }
    }

    #[test]
    fn internal_error_no_range() {
        let diag = internal_error("something broke".into());
        assert_eq!(diag.phase, DiagnosticPhase::Internal);
        assert_eq!(diag.severity, "error");
        assert!(diag.range.is_none());
    }
}
