#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::debug;

use crate::{
    ironplc_dsl::common::Library,
    result::SemanticResult,
    rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_function_block_invocation, rule_pou_hierarchy,
    rule_program_task_definition_exists, rule_unsupported_stdlib_type,
    rule_use_declared_enumerated_value, rule_use_declared_symbolic_var,
    rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const,
    symbol_environment::SymbolEnvironment,
    type_environment::{TypeEnvironment, TypeEnvironmentBuilder},
    type_table, xform_resolve_late_bound_expr_kind, xform_resolve_late_bound_type_initializer,
    xform_resolve_symbol_environment, xform_resolve_type_aliases,
    xform_resolve_type_decl_environment, xform_toposort_declarations,
};

/// Analyze runs semantic analysis on the set of files as a self-contained and complete unit.
///
/// Returns `Ok(Library)` if analysis succeeded (containing a possibly new library) that is
/// the merge of the inputs.
/// Returns `Err(Diagnostic)` if analysis did not succeed.
pub fn analyze(sources: &[&Library]) -> Result<(), Vec<Diagnostic>> {
    if sources.is_empty() {
        let span = SourceSpan::range(0, 0).with_file_id(&FileId::default());
        return Err(vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(span, "First location"),
        )]);
    }
    let (library, type_environment, symbol_environment) = resolve_types(sources)?;
    let result = semantic(&library, &type_environment, &symbol_environment);

    // TODO this is currently in progress. It isn't clear to me yet how this will influence
    // semantic analysis, but it should because the type table should influence rule checking.
    // For now, this is just after the rules as they were originally written.
    let type_table_result = type_table::apply(&library)?;
    debug!("{type_table_result:?}");

    result
}

pub(crate) fn resolve_types(
    sources: &[&Library],
) -> Result<(Library, TypeEnvironment, SymbolEnvironment), Vec<Diagnostic>> {
    // We want to analyze this as a complete set, so we need to join the items together
    // into a single library. Extend owns the item so after this we are free to modify
    let mut library = Library::new();
    for x in sources {
        library = library.extend((*x).clone());
    }

    let mut type_environment = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .with_stdlib_function_blocks()
        .build()
        .map_err(|err| vec![err])?;

    let mut symbol_environment = SymbolEnvironment::new();

    let mut library = xform_toposort_declarations::apply(library)?;

    let xforms: Vec<fn(Library, &mut TypeEnvironment) -> Result<Library, Vec<Diagnostic>>> = vec![
        xform_resolve_type_decl_environment::apply,
        xform_resolve_late_bound_expr_kind::apply,
        xform_resolve_late_bound_type_initializer::apply,
    ];

    for xform in xforms {
        library = xform(library, &mut type_environment)?
    }

    // Resolve symbols after types are resolved
    library = xform_resolve_symbol_environment::apply(
        library,
        &type_environment,
        &mut symbol_environment,
    )?;

    // Resolve type aliases by duplicating values
    library =
        xform_resolve_type_aliases::apply(library, &type_environment, &mut symbol_environment)?;

    // Generate and display useful symbol table information
    debug!("Type Environment:");
    debug!("{type_environment:?}");

    debug!("Symbol Environment:");
    debug!("{symbol_environment:?}");

    Ok((library, type_environment, symbol_environment))
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub(crate) fn semantic(
    library: &Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let functions: Vec<fn(&Library, &TypeEnvironment, &SymbolEnvironment) -> SemanticResult> = vec![
        rule_decl_struct_element_unique_names::apply,
        rule_decl_subrange_limits::apply,
        rule_enumeration_values_unique::apply,
        rule_function_block_invocation::apply,
        rule_program_task_definition_exists::apply,
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
        match func(library, type_environment, symbol_environment) {
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
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let lib = parse_shared_library("first_steps_semantic_error.st");
        let res = analyze(&[&lib]);
        assert!(res.is_err())
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
