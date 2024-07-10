use ironplc_dsl::diagnostic::Diagnostic;

/// Defines a result type for semantic analysis.
///
/// Semantic analysis either returns nothing or
/// a list of diagnostic errors.
pub(crate) type SemanticResult = Result<(), Vec<Diagnostic>>;
