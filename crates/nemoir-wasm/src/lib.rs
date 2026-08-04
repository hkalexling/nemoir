//! Browser-callable compiler API.
//!
//! This crate is a thin WASM facade around the existing NemoIR compiler
//! libraries. It exposes typed `analyze` / `generate` / `metadata` entry
//! points that the browser Worker calls. The facade owns:
//!
//! - diagnostic conversion (Rust byte-offsets → 1-based UTF-16 columns),
//! - backend dispatch,
//! - artifact path normalization for ZIP consumption, and
//! - metadata reporting.
//!
//! It does **not** contain parsing, validation, IR semantics, or code
//! generation — those stay in `nemoir-dsl-fe`, `nemoir-ir`, and the backend
//! crates.

pub mod api;
pub mod artifacts;
pub mod diagnostics;
pub mod pipeline;

pub use api::*;
pub use artifacts::*;
pub use diagnostics::*;
