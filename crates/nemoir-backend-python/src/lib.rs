mod emit;
mod escape;
mod naming;
mod translate;

#[derive(Debug, Default)]
pub struct PythonBackendOptions {
    pub package_version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PythonBackendError {
    #[error("IR validation failed: {0}")]
    ValidationFailed(#[from] nemoir_ir::validate::ValidationErrors),

    #[error("workflow id '{0}' cannot be converted to a valid Python package name")]
    InvalidWorkflowId(String),

    #[error("unsupported literal value in IR: {0}")]
    UnsupportedLiteral(String),

    #[error("IR name '{0}' is not a valid Python identifier or is a reserved keyword")]
    InvalidPythonField(String),
}

#[derive(Debug)]
pub struct GeneratedFile {
    pub relative_path: std::path::PathBuf,
    pub content: String,
}

#[derive(Debug)]
pub struct GeneratedPackage {
    pub package_name: String,
    pub distribution_name: String,
    pub files: Vec<GeneratedFile>,
}

pub fn generate_package(
    ir: &nemoir_ir::WorkflowIr,
    options: &PythonBackendOptions,
) -> Result<GeneratedPackage, PythonBackendError> {
    nemoir_ir::validate::validate(ir)?;

    validate_python_field_names(ir)?;

    let package_name = naming::package_name(&ir.workflow.id)
        .ok_or_else(|| PythonBackendError::InvalidWorkflowId(ir.workflow.id.clone()))?;
    let distribution_name = naming::distribution_name(&ir.workflow.id);

    let manifest_source = translate::emit_manifest_module(ir)?;
    let types_source = translate::emit_types_module(ir)?;
    let agent_source = translate::emit_agent_module(&package_name, ir)?;
    let init_source = translate::emit_init_module(&package_name, ir)?;
    let pyproject_source = emit::emit_pyproject(
        &distribution_name,
        &package_name,
        options.package_version.as_deref().unwrap_or("0.1.0"),
    );

    let files = vec![
        GeneratedFile {
            relative_path: format!("{package_name}/_manifest.py").into(),
            content: manifest_source,
        },
        GeneratedFile {
            relative_path: format!("{package_name}/types.py").into(),
            content: types_source,
        },
        GeneratedFile {
            relative_path: format!("{package_name}/_agent.py").into(),
            content: agent_source,
        },
        GeneratedFile {
            relative_path: format!("{package_name}/__init__.py").into(),
            content: init_source,
        },
        GeneratedFile {
            relative_path: "pyproject.toml".into(),
            content: pyproject_source,
        },
    ];

    Ok(GeneratedPackage {
        package_name,
        distribution_name,
        files,
    })
}

/// Reject IR names that would be emitted as Python identifiers but are not
/// valid (or are Python reserved keywords). Scope: input ids and exit-stage
/// write names -- these are the only IR names that end up as Python dataclass
/// field names and attribute accesses in generated `_agent.py`/`types.py`.
///
/// Other IR names (node ids, policy ids, transition reasons, non-exit write
/// names) only appear in generated code as quoted string literals, so they do
/// not require Python-identifier validity.
fn validate_python_field_names(ir: &nemoir_ir::WorkflowIr) -> Result<(), PythonBackendError> {
    for inp in &ir.inputs {
        if !naming::is_valid_python_field_name(&inp.id) {
            return Err(PythonBackendError::InvalidPythonField(inp.id.clone()));
        }
    }
    let exit_set: std::collections::HashSet<&str> =
        ir.workflow.exits.iter().map(|s| s.as_str()).collect();
    for node in &ir.nodes {
        if !exit_set.contains(node.id.as_str()) {
            continue;
        }
        for w in &node.writes {
            if !naming::is_valid_python_field_name(&w.name) {
                return Err(PythonBackendError::InvalidPythonField(w.name.clone()));
            }
        }
    }
    Ok(())
}
