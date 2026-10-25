//! Compiles an analyzed library in codegen's own tests: lowers it as the
//! project does (`ironplc_lowering::lower`), then compiles the lowered
//! program.

use ironplc_analyzer::{CleanAnalysis, SemanticContext};
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_ir::Diagnostic;

/// The container of `library`, analyzed with `context`; otherwise the first
/// problem lowering or codegen reports.
pub(crate) fn compile_analyzed(
    library: &Library,
    context: &SemanticContext,
) -> Result<Container, Diagnostic> {
    let analysis =
        CleanAnalysis::new(library, context).map_err(|_| Diagnostic::internal_error())?;
    let program = ironplc_lowering::lower(&analysis).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(Diagnostic::internal_error)
    })?;
    crate::compile(&program, &crate::EmptyLookup)
}
