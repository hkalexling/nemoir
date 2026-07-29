//! Web backend codegen options.

/// Options for the web backend codegen.
///
/// Mirrors `nemoir-backend-python::PythonBackendOptions`.
/// All fields are optional with sensible defaults.
#[derive(Debug, Default)]
pub struct WebBackendOptions {
    /// Package version string emitted into `package.json`.
    /// Defaults to `"0.1.0"`.
    pub package_version: Option<String>,
    /// Dependency spec for `@nemoir/web-runtime` in the generated
    /// `package.json`. Defaults to `"^0.3.1"`. For local development
    /// point this at a `file:` path to an in-repo runtime checkout.
    pub runtime_dependency: Option<String>,
    /// Dependency spec for `@nemoir/web-ui` in the generated
    /// `package.json`. Defaults to `"^0.1.0"`. For local development
    /// point this at a `file:` path to an in-repo web-ui checkout.
    pub ui_dependency: Option<String>,
}
