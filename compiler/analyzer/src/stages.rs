#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::{token::Token, tokenize_program};
use ironplc_problems::Problem;

use crate::{
    compilation_set::{CompilationSet, CompilationSource},
    ironplc_dsl::common::Library,
    result::SemanticResult,
    rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_function_block_invocation, rule_pous_no_cycles,
    rule_program_task_definition_exists, rule_unsupported_stdlib_type,
    rule_use_declared_enumerated_value, rule_use_declared_symbolic_var,
    rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const, xform_assign_file_id,
    xform_resolve_late_bound_data_decl, xform_resolve_late_bound_expr_kind,
    xform_resolve_late_bound_type_initializer,
};

pub fn tokenize(source: &str, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
    tokenize_program(source, file_id)
}

/// Parse create a library (set of elements) if the text is valid.
///
/// Returns `Ok(Library)` if parsing succeeded.
/// Returns `Err(Diagnostic)` if parsing did not succeed.
pub fn parse(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let library = ironplc_parser::parse_program(source, file_id)?;

    // The parser does not know about the concept of files, so we apply the file
    // ID as a post transformation.
    xform_assign_file_id::apply(library, file_id)
}

/// Analyze runs semantic analysis on the set of files as a self-contained and complete unit.
///
/// Returns `Ok(Library)` if analysis succeeded (containing a possibly new library) that is
/// the merge of the inputs.
/// Returns `Err(Diagnostic)` if analysis did not succeed.
pub fn analyze(compilation_set: &CompilationSet) -> Result<(), Vec<Diagnostic>> {
    if compilation_set.sources.is_empty() {
        let span = SourceSpan::range(0, 0).with_file_id(&FileId::default());
        return Err(vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(span, "First location"),
        )]);
    }
    let library = resolve_types(compilation_set)?;
    semantic(&library)
}

pub(crate) fn resolve_types(compilation_set: &CompilationSet) -> Result<Library, Vec<Diagnostic>> {
    // We want to analyze this as a complete set, so we need to join the items together
    // into a single library. Extend owns the item so after this we are free to modify
    let mut library = Library::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for x in &compilation_set.sources {
        match x {
            CompilationSource::Library(lib) => {
                library = library.extend(lib.clone());
            }
            CompilationSource::Text(txt) => match parse(&txt.0, &txt.1) {
                Ok(lib) => library = library.extend(lib),
                Err(err) => diagnostics.push(err),
            },
            CompilationSource::TextRef(txt) => match parse(txt.0, &txt.1) {
                Ok(lib) => library = library.extend(lib),
                Err(err) => diagnostics.push(err),
            },
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let xforms: Vec<fn(Library) -> Result<Library, Vec<Diagnostic>>> = vec![
        xform_resolve_late_bound_data_decl::apply,
        xform_resolve_late_bound_expr_kind::apply,
        xform_resolve_late_bound_type_initializer::apply,
    ];

    for xform in xforms {
        library = xform(library)?
    }

    Ok(library)
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub(crate) fn semantic(library: &Library) -> SemanticResult {
    let functions: Vec<fn(&Library) -> SemanticResult> = vec![
        rule_decl_struct_element_unique_names::apply,
        rule_decl_subrange_limits::apply,
        rule_enumeration_values_unique::apply,
        rule_function_block_invocation::apply,
        rule_pous_no_cycles::apply,
        rule_program_task_definition_exists::apply,
        rule_use_declared_enumerated_value::apply,
        rule_use_declared_symbolic_var::apply,
        rule_unsupported_stdlib_type::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_var_decl_global_const_requires_external_const::apply,
    ];

    let mut all_diagnostics = vec![];
    for func in functions {
        match func(library) {
            Ok(_) => {
                // Nothing to do here
            }
            Err(diagnostics) => {
                all_diagnostics.extend(diagnostics);
            }
        }
    }

    if !all_diagnostics.is_empty() {
        return Err(all_diagnostics);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use crate::stages::CompilationSet;
    use crate::stages::CompilationSource;
    use ironplc_dsl::core::FileId;
    use ironplc_test::read_shared_resource;

    impl<'a> CompilationSet<'a> {
        fn of_source(str: &String) -> Self {
            Self {
                sources: vec![CompilationSource::Text((
                    str.to_string(),
                    FileId::default(),
                ))],
                references: vec![],
            }
        }
    }

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let src = read_shared_resource("first_steps.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_ok());
    }

    #[test]
    fn analyze_when_first_steps_syntax_error_then_result_is_err() {
        let src = read_shared_resource("first_steps_syntax_error.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_err())
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let src = read_shared_resource("first_steps_semantic_error.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_err())
    }

    #[test]
    fn analyze_2() {
        let src = read_shared_resource("main.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_ok());
    }

    #[test]
    fn analyze_when_split_across_multiple_files_then_ok() {
        let program1 = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE";

        let program2 = "
FUNCTION_BLOCK LOGGER
VAR_EXTERNAL CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

        let mut compilation_set = CompilationSet::new();
        compilation_set.push(CompilationSource::Text((
            program1.to_owned(),
            FileId::default(),
        )));
        compilation_set.push(CompilationSource::Text((
            program2.to_owned(),
            FileId::default(),
        )));

        let result = analyze(&compilation_set);
        assert!(result.is_ok())
    }
}
