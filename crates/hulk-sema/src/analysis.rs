use crate::context::TypeRegistry;
use crate::error::SemanticError;
use crate::hir::SemanticProgram;
use crate::hir_builder::HirBuilder;
use crate::resolver::resolve_program;
use hulk_frontend::ast::Program;

pub fn analyze_program(program: &Program) -> Result<SemanticProgram, Vec<SemanticError>> {
    resolve_program(program).map_err(|e| vec![e])?;

    let registry = TypeRegistry::build(program).map_err(|e| vec![e])?;
    HirBuilder::new(registry).analyze_program(program)
}
