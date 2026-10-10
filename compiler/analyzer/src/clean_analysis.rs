//! The gate between semantic analysis and code generation.

use ironplc_dsl::common::{Library, ProgramDeclaration, VarDecl};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::execution::{Execution, GlobalId, ProgramId};

use crate::execution_model::{self, Declarations};
use crate::semantic_context::SemanticContext;

/// An analyzed library paired with a semantic context that holds no
/// diagnostics: what code generation takes as input.
///
/// [`CleanAnalysis::new`] is the only way to make one, and it refuses a
/// context that holds a diagnostic. A backend that takes a `CleanAnalysis`
/// cannot be reached by a caller that forgot to check for diagnostics first.
///
/// # What it guarantees
///
/// The context held no diagnostics when the pair was made. The context is
/// held by shared borrow for as long as the `CleanAnalysis` exists, so it
/// cannot gain a diagnostic in the meantime.
///
/// # What it does not guarantee
///
/// - That the semantic rules ran. A context built by
///   [`SemanticContextBuilder`](crate::SemanticContextBuilder), or returned by
///   [`stages::resolve_types`](crate::stages::resolve_types), has not been
///   through the rules, so it holds no rule diagnostics and passes the gate
///   even for a library the rules would reject.
/// - That the context was built from the library. Nothing ties the two
///   together, so a context can be paired with any library. The execution
///   model is the exception: `new` resolves it from `library` itself, so the
///   model and the declarations it reaches always belong to that library.
///
/// The gate stops a caller from forgetting the check, not one that sets out
/// to skip it.
#[derive(Clone, Debug)]
pub struct CleanAnalysis<'a> {
    library: &'a Library,
    context: &'a SemanticContext,
    /// The execution model resolved from `library`.
    execution: Execution,
    /// The declaration behind each id of `execution`, borrowed from
    /// `library` by the walk that allocated the ids.
    declarations: Declarations<'a>,
}

impl<'a> CleanAnalysis<'a> {
    /// Pairs `library` with `context` when the context holds no diagnostics,
    /// and resolves the execution model of `library`.
    ///
    /// Returns the context's diagnostics otherwise. Returning them, rather
    /// than `None`, means a caller that unwraps the result with `.expect()`
    /// prints what analysis reported.
    pub fn new(
        library: &'a Library,
        context: &'a SemanticContext,
    ) -> Result<Self, &'a [Diagnostic]> {
        if context.has_diagnostics() {
            return Err(context.diagnostics());
        }
        let (execution, declarations) =
            execution_model::resolve_with_declarations(library, context.compiler_options());
        Ok(Self {
            library,
            context,
            execution,
            declarations,
        })
    }

    /// The analyzed library.
    pub fn library(&self) -> &'a Library {
        self.library
    }

    /// The semantic context, which holds no diagnostics.
    pub fn context(&self) -> &'a SemanticContext {
        self.context
    }

    /// What runs, and when: the execution model of the library, or why
    /// nothing can run.
    pub fn execution(&self) -> &Execution {
        &self.execution
    }

    /// The `PROGRAM` declaration whose body the program `id` of the
    /// execution model runs.
    ///
    /// Answered from the walk of the library that allocated `id`. `None`
    /// means a compiler defect, which the caller reports as an internal
    /// error: `id` is another model's.
    pub fn program_declaration(&self, id: ProgramId) -> Option<&'a ProgramDeclaration> {
        self.declarations.program(id)
    }

    /// The declaration of the declared global `id` of the execution model,
    /// which holds its type and initial value.
    ///
    /// Answered from the walk of the library that allocated `id`. `None`
    /// means a compiler defect, which the caller reports as an internal
    /// error: `id` names a system global, which has no declaration, or is
    /// another model's.
    pub fn global_declaration(&self, id: GlobalId) -> Option<&'a VarDecl> {
        self.declarations.global(id)
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::Library;
    use ironplc_dsl::core::SourceSpan;
    use ironplc_dsl::diagnostic::{Diagnostic, Label};
    use ironplc_problems::Problem;

    use super::CleanAnalysis;
    use crate::semantic_context::SemanticContextBuilder;

    #[test]
    fn new_when_context_has_diagnostic_then_err() {
        let library = Library::new();
        let mut context = SemanticContextBuilder::new().build().unwrap();
        context.add_diagnostics(vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(SourceSpan::default(), "test diagnostic"),
        )]);

        let result = CleanAnalysis::new(&library, &context);

        assert!(std::ptr::eq(result.unwrap_err(), context.diagnostics()));
    }

    #[test]
    fn new_when_context_has_no_diagnostics_then_returns_library_and_context() {
        let library = Library::new();
        let context = SemanticContextBuilder::new().build().unwrap();

        let analysis = CleanAnalysis::new(&library, &context).unwrap();

        assert!(std::ptr::eq(analysis.library(), &library));
        assert!(std::ptr::eq(analysis.context(), &context));
    }
}
