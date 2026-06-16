pub mod ast;
pub mod diagnostics;
pub mod lower;
pub mod parse;
pub mod resolve;
pub mod validate;

#[cfg(test)]
mod test_parse;

pub use diagnostics::Diagnostic;
use nemoir_ir::WorkflowIr;

pub fn check(source: &str, filename: &str) -> Result<(), Diagnostic> {
    let ast = parse::parse_source(source, filename)?;
    let resolved = resolve::resolve(ast, filename)?;
    let _ = validate::validate(&resolved, filename)?;
    Ok(())
}

pub fn lower(source: &str, filename: &str) -> Result<WorkflowIr, Diagnostic> {
    let ast = parse::parse_source(source, filename)?;
    let resolved = resolve::resolve(ast, filename)?;
    let transitions = validate::validate(&resolved, filename)?;
    let ir = lower::lower(&resolved, &transitions, filename)?;
    Ok(ir)
}
