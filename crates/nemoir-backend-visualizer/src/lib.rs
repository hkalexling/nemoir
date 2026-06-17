mod graph;
mod html;

#[derive(Debug, Default)]
pub struct VisualizerOptions {
    pub title: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum VisualizerError {
    #[error("IR validation failed: {0}")]
    ValidationFailed(#[from] nemoir_ir::validate::ValidationErrors),

    #[error("JSON serialization failed: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub fn render_html(
    ir: &nemoir_ir::WorkflowIr,
    options: &VisualizerOptions,
) -> Result<String, VisualizerError> {
    nemoir_ir::validate::validate(ir)?;

    let graph_data = graph::build_graph_data(ir)?;
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| ir.workflow.id.clone());

    Ok(html::generate_html(ir, &title, &graph_data))
}
