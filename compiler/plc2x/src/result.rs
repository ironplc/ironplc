use ironplc_dsl::diagnostic::Diagnostic;

pub(crate) type SemanticResult = Result<(), Vec<Diagnostic>>;
