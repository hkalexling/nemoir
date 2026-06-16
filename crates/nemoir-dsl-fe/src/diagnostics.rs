use miette::{Diagnostic as MietteDiagnostic, LabeledSpan};

#[derive(Debug, thiserror::Error, MietteDiagnostic)]
pub enum Diagnostic {
    #[error(transparent)]
    #[diagnostic(transparent)]
    ParseError(ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NameError(NameError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeError(TypeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ShapeError(ShapeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TransitionError(TransitionError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    GraphError(GraphError),
}

fn make_label(
    label: &Option<(usize, usize, String)>,
) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
    label.as_ref().map(|(start, end, msg)| {
        let labeled = LabeledSpan::new(Some(msg.clone()), *start, end - start);
        Box::new(std::iter::once(labeled)) as Box<dyn Iterator<Item = LabeledSpan>>
    })
}

fn make_help(help: &Option<String>) -> Option<Box<dyn std::fmt::Display + '_>> {
    help.as_ref()
        .map(|h| Box::new(h.as_str()) as Box<dyn std::fmt::Display>)
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
    pub help: Option<String>,
}

impl MietteDiagnostic for ParseError {
    fn code(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new("parse"))
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        make_help(&self.help)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct NameError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
    pub help: Option<String>,
}

impl MietteDiagnostic for NameError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        make_help(&self.help)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct TypeError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
}

impl MietteDiagnostic for TypeError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ShapeError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
    pub help: Option<String>,
}

impl MietteDiagnostic for ShapeError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        make_help(&self.help)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct TransitionError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
    pub help: Option<String>,
}

impl MietteDiagnostic for TransitionError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        make_help(&self.help)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GraphError {
    pub message: String,
    pub filename: String,
    pub label: Option<(usize, usize, String)>,
    pub help: Option<String>,
}

impl MietteDiagnostic for GraphError {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        make_label(&self.label)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        make_help(&self.help)
    }
}
