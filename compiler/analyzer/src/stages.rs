#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::debug;

use crate::{
    function_environment::FunctionEnvironmentBuilder,
    ironplc_dsl::common::Library,
    result::SemanticResult,
    rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_function_block_invocation, rule_function_call_declared,
    rule_pou_hierarchy, rule_program_task_definition_exists, rule_stdlib_type_redefinition,
    rule_unsupported_stdlib_type, rule_use_declared_enumerated_value,
    rule_use_declared_symbolic_var, rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const,
    semantic_context::SemanticContext,
    symbol_environment::SymbolEnvironment,
    type_environment::{TypeEnvironment, TypeEnvironmentBuilder},
    type_table, xform_resolve_late_bound_expr_kind, xform_resolve_late_bound_type_initializer,
    xform_resolve_symbol_and_function_environment, xform_resolve_type_aliases,
    xform_resolve_type_decl_environment, xform_toposort_declarations,
};

/// Analyze runs semantic analysis on the set of files as a self-contained and complete unit.
///
/// Returns `Ok(SemanticContext)` containing all type, function, and symbol information
/// gathered during analysis. If any analysis step found errors, they are stored in
/// `context.diagnostics()` rather than causing an `Err` return.
///
/// Returns `Err` only when no sources are provided or when foundational type resolution
/// fails (declaration sorting or type environment building).
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>> {
    if sources.is_empty() {
        let span = SourceSpan::range(0, 0).with_file_id(&FileId::default());
        return Err(vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(span, "First location"),
        )]);
    }
    let (library, mut context) = resolve_types(sources)?;

    if let Err(diagnostics) = semantic(&library, &context) {
        context.add_diagnostics(diagnostics);
    }

    // TODO this is currently in progress. It isn't clear to me yet how this will influence
    // semantic analysis, but it should because the type table should influence rule checking.
    // For now, this is just after the rules as they were originally written.
    match type_table::apply(&library) {
        Ok(type_table_result) => {
            debug!("{type_table_result:?}");
        }
        Err(diagnostics) => {
            context.add_diagnostics(diagnostics);
        }
    }

    Ok(context)
}

pub(crate) fn resolve_types(
    sources: &[&Library],
) -> Result<(Library, SemanticContext), Vec<Diagnostic>> {
    let mut diagnostics: Vec<Diagnostic> = vec![];

    // We want to analyze this as a complete set, so we need to join the items together
    // into a single library. Extend owns the item so after this we are free to modify
    let mut library = Library::new();
    for x in sources {
        library = library.extend((*x).clone());
    }

    // Hard failures: these are foundational and all subsequent steps depend on them.
    let mut type_environment = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .with_stdlib_function_blocks()
        .build()
        .map_err(|err| vec![err])?;

    let mut function_environment = FunctionEnvironmentBuilder::new()
        .with_stdlib_functions()
        .build();

    let mut symbol_environment = SymbolEnvironment::new();

    // Hard failure: declaration ordering is required for all subsequent transforms.
    let mut library = xform_toposort_declarations::apply(library)?;

    // Recoverable: Fold-based transforms consume the Library on error, so clone
    // before each one. On failure, fall back to the pre-transform clone.
    let recoverable_xforms: Vec<
        fn(Library, &mut TypeEnvironment) -> Result<Library, Vec<Diagnostic>>,
    > = vec![
        xform_resolve_type_decl_environment::apply,
        xform_resolve_late_bound_expr_kind::apply,
        xform_resolve_late_bound_type_initializer::apply,
    ];

    for xform in recoverable_xforms {
        let fallback = library.clone();
        match xform(library, &mut type_environment) {
            Ok(result) => library = result,
            Err(errs) => {
                diagnostics.extend(errs);
                library = fallback;
            }
        }
    }

    // Recoverable: takes Library by value; clone to recover on failure.
    let fallback = library.clone();
    match xform_resolve_symbol_and_function_environment::apply(
        library,
        &mut symbol_environment,
        &mut function_environment,
    ) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Recoverable: takes Library by value; clone to recover on failure.
    let fallback = library.clone();
    match xform_resolve_type_aliases::apply(library, &type_environment, &mut symbol_environment) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Generate and display useful symbol table information
    debug!("Type Environment:");
    debug!("{type_environment:?}");

    debug!("Symbol Environment:");
    debug!("{symbol_environment:?}");

    let mut context =
        SemanticContext::new(type_environment, function_environment, symbol_environment);
    context.add_diagnostics(diagnostics);

    Ok((library, context))
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub(crate) fn semantic(library: &Library, context: &SemanticContext) -> SemanticResult {
    let functions: Vec<fn(&Library, &SemanticContext) -> SemanticResult> = vec![
        rule_decl_struct_element_unique_names::apply,
        rule_decl_subrange_limits::apply,
        rule_enumeration_values_unique::apply,
        rule_function_block_invocation::apply,
        rule_function_call_declared::apply,
        rule_program_task_definition_exists::apply,
        rule_stdlib_type_redefinition::apply,
        rule_use_declared_enumerated_value::apply,
        rule_use_declared_symbolic_var::apply,
        rule_unsupported_stdlib_type::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_var_decl_global_const_requires_external_const::apply,
        rule_pou_hierarchy::apply,
    ];

    let mut all_diagnostics = vec![];
    for func in functions {
        match func(library, context) {
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
    use ironplc_dsl::common::Library;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::ParseOptions;
    use ironplc_parser::parse_program;
    use ironplc_test::read_shared_resource;

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let lib = parse_shared_library("first_steps.st");
        let res = analyze(&[&lib]);
        assert!(res.is_ok());
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_ok_with_diagnostics() {
        let lib = parse_shared_library("first_steps_semantic_error.st");
        let res = analyze(&[&lib]);
        let context = res.unwrap();
        assert!(context.has_diagnostics());
    }

    #[test]
    fn analyze_2() {
        let lib = parse_shared_library("main.st");
        let res = analyze(&[&lib]);
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

        let program1 =
            parse_program(program1, &FileId::default(), &ParseOptions::default()).unwrap();
        let program2 =
            parse_program(program2, &FileId::default(), &ParseOptions::default()).unwrap();

        let result = analyze(&[&program1, &program2]);
        assert!(result.is_ok())
    }

    fn parse_shared_library(name: &'static str) -> Library {
        let src = read_shared_resource(name);
        parse_program(&src, &FileId::default(), &ParseOptions::default()).unwrap()
    }
}
