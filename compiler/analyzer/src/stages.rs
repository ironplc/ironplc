#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{
    core::{FileId, Id, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use log::debug;

use crate::{
    function_environment::FunctionEnvironmentBuilder,
    ironplc_dsl::common::Library,
    result::SemanticResult,
    rule_abstract_not_instantiated, rule_assignment_aggregate_type_compat, rule_bit_access_range,
    rule_case_bit_string_label, rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_extends_field_duplicated,
    rule_function_block_call_unsupported, rule_function_block_invocation,
    rule_function_call_declared, rule_function_call_type_check, rule_method_call_declared,
    rule_mixed_located_var_declarations, rule_no_top_level_var_global, rule_pou_hierarchy,
    rule_program_task_definition_exists, rule_ref_to, rule_stdlib_type_redefinition,
    rule_string_encoding_compat, rule_struct_initializer_expression_allowed,
    rule_task_names_unique, rule_unsupported_extension, rule_unsupported_stdlib_type,
    rule_use_declared_enumerated_value, rule_use_declared_symbolic_var,
    rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const, rule_var_decl_initializer_type_compat,
    semantic_context::SemanticContext,
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
    type_environment::{TypeEnvironment, TypeEnvironmentBuilder},
    type_table, xform_fold_constant_expressions, xform_fold_initializer_expressions,
    xform_insert_implicit_deref, xform_int_to_bool_initializer, xform_named_to_positional_args,
    xform_resolve_adr, xform_resolve_constant_expressions, xform_resolve_expr_types,
    xform_resolve_late_bound_expr_kind, xform_resolve_late_bound_type_initializer,
    xform_resolve_symbol_and_function_environment, xform_resolve_type_aliases,
    xform_resolve_type_decl_environment, xform_toposort_declarations,
};

/// Analyze runs semantic analysis on the set of files as a self-contained and complete unit.
///
/// Returns `Ok((Library, SemanticContext))` containing the type-resolved AST and all type,
/// function, and symbol information gathered during analysis. If any analysis step found
/// errors, they are stored in `context.diagnostics()` rather than causing an `Err` return.
///
/// Returns `Err` only when no sources are provided or when foundational type resolution
/// fails (declaration sorting or type environment building).
pub fn analyze(
    sources: &[&Library],
    options: &CompilerOptions,
) -> Result<(Library, SemanticContext), Vec<Diagnostic>> {
    if sources.is_empty() {
        let span = SourceSpan::range(0, 0).with_file_id(&FileId::default());
        return Err(vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(span, "First location"),
        )]);
    }
    let (library, mut context) = resolve_types(sources, options)?;

    if let Err(diagnostics) = semantic(&library, &context, options) {
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

    Ok((library, context))
}

pub fn resolve_types(
    sources: &[&Library],
    options: &CompilerOptions,
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

    // Conditionally register dialect-extension functions gated by allow flags.
    if options.allow_sizeof {
        use crate::intermediates::stdlib_function::get_sizeof_function;
        function_environment
            .insert(get_sizeof_function())
            .expect("SIZEOF should not conflict with stdlib");
    }

    let mut symbol_environment = SymbolEnvironment::new();

    // Register implicit system globals when the uptime feature is enabled.
    if options.allow_system_uptime_global {
        symbol_environment
            .insert(
                &Id::from("__SYSTEM_UP_TIME"),
                SymbolKind::Variable,
                &ScopeKind::Global,
            )
            .map_err(|e| vec![e])?;
        symbol_environment
            .insert(
                &Id::from("__SYSTEM_UP_LTIME"),
                SymbolKind::Variable,
                &ScopeKind::Global,
            )
            .map_err(|e| vec![e])?;
    }

    // Resolve constant references in type parameters (STRING lengths, array bounds).
    // Must run before toposort so that concrete integer values are available.
    let fallback = library.clone();
    match xform_resolve_constant_expressions::apply(library, options) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Hard failure: declaration ordering is required for all subsequent transforms.
    // Also computes the set of declarations reachable from PROGRAM roots,
    // which codegen uses to skip unused functions.
    let (mut library, reachable) = xform_toposort_declarations::apply(library)?;

    // Hard failure: declaration ordering and type-environment population is
    // required for all subsequent transforms, and a failure here reflects a
    // fundamentally broken declaration (not an unrelated one), so reverting
    // the whole library on error is correct.
    let fallback = library.clone();
    match xform_resolve_type_decl_environment::apply(library, &mut type_environment) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Recoverable: an unresolvable declaration is diagnosed but does not
    // discard the rest of the library's successfully resolved declarations.
    // See specs/plans/2026-08-02-partial-resolution-revert-on-unrelated-error.md.
    let recoverable_xforms: Vec<
        fn(Library, &mut TypeEnvironment) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>,
    > = vec![
        xform_resolve_late_bound_expr_kind::apply,
        xform_resolve_late_bound_type_initializer::apply,
    ];

    for xform in recoverable_xforms {
        let fallback = library.clone();
        match xform(library, &mut type_environment) {
            Ok((result, errs)) => {
                library = result;
                diagnostics.extend(errs);
            }
            Err(errs) => {
                diagnostics.extend(errs);
                library = fallback;
            }
        }
    }

    // Give TwinCAT `REFERENCE TO` variables their auto-dereferencing semantics
    // (bare reads/writes go through the reference) and lower `__ISVALIDREF`.
    // Runs after late-bound expression resolution (so bare identifiers are
    // already `ExprKind::Variable`) but before symbol/function resolution (so
    // `__ISVALIDREF` is lowered before it would be flagged as undeclared) and
    // before the reference semantic rules. See
    // specs/design/reference-to-twincat.md (PR 2).
    let fallback = library.clone();
    match xform_insert_implicit_deref::apply(library, options) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Rewrite the `ADR(x)` address-of operator into `ExprKind::Ref` when
    // `allow_adr` is set. Runs after implicit-deref (so a `REFERENCE TO`
    // operand is not mis-addressed) and before symbol/function resolution
    // (so a recognized `ADR` is not reported as an undeclared function).
    // Recoverable: a diagnosed call is lowered to a placeholder, so the
    // transformed library is kept even when diagnostics are present.
    let fallback = library.clone();
    match xform_resolve_adr::apply(library, options) {
        Ok((result, errs)) => {
            library = result;
            diagnostics.extend(errs);
        }
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Fold constant-expression VAR initializers (e.g. `scaled : LREAL := SCALE*4.0;`)
    // back into ordinary literal initializers, or diagnose. Must run before
    // any other pass touches `InitialValueAssignmentKind::SimpleExpr`.
    // Recoverable: a diagnosed initializer is still normalized, so the
    // transformed library must be kept even when diagnostics are present —
    // reverting would leak `SimpleExpr` nodes to later passes.
    let fallback = library.clone();
    match xform_fold_initializer_expressions::apply(library, options) {
        Ok((result, errs)) => {
            library = result;
            diagnostics.extend(errs);
        }
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Rewrite integer 0/1 initializers on BOOL variables to boolean literals.
    // Short-circuits internally when allow_int_to_bool_initializer is false.
    let fallback = library.clone();
    match xform_int_to_bool_initializer::apply(library, &mut type_environment, options) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
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

    // Recoverable: convert named function call arguments to positional.
    let fallback = library.clone();
    match xform_named_to_positional_args::apply(library, &function_environment) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Recoverable: resolve expression types using the function environment.
    let fallback = library.clone();
    match xform_resolve_expr_types::apply(
        library,
        &mut type_environment,
        &function_environment,
        options,
    ) {
        Ok(result) => library = result,
        Err(errs) => {
            diagnostics.extend(errs);
            library = fallback;
        }
    }

    // Recoverable: fold constant binary and unary expressions.
    let fallback = library.clone();
    match xform_fold_constant_expressions::apply(library) {
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

    let mut context = SemanticContext::new(
        type_environment,
        function_environment,
        symbol_environment,
        reachable,
        *options,
    );
    context.add_diagnostics(diagnostics);

    Ok((library, context))
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub(crate) fn semantic(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    let functions: Vec<fn(&Library, &SemanticContext, &CompilerOptions) -> SemanticResult> = vec![
        rule_abstract_not_instantiated::apply,
        rule_assignment_aggregate_type_compat::apply,
        rule_decl_struct_element_unique_names::apply,
        rule_decl_subrange_limits::apply,
        rule_enumeration_values_unique::apply,
        rule_extends_field_duplicated::apply,
        rule_function_block_call_unsupported::apply,
        rule_function_block_invocation::apply,
        rule_function_call_declared::apply,
        rule_function_call_type_check::apply,
        rule_method_call_declared::apply,
        rule_program_task_definition_exists::apply,
        rule_no_top_level_var_global::apply,
        rule_task_names_unique::apply,
        rule_stdlib_type_redefinition::apply,
        rule_string_encoding_compat::apply,
        rule_struct_initializer_expression_allowed::apply,
        rule_use_declared_enumerated_value::apply,
        rule_use_declared_symbolic_var::apply,
        rule_unsupported_stdlib_type::apply,
        rule_unsupported_extension::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_var_decl_initializer_type_compat::apply,
        rule_var_decl_global_const_requires_external_const::apply,
        rule_mixed_located_var_declarations::apply,
        rule_pou_hierarchy::apply,
        rule_bit_access_range::apply,
        rule_case_bit_string_label::apply,
        rule_ref_to::apply,
    ];

    let mut all_diagnostics = vec![];
    for func in functions {
        match func(library, context, options) {
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
    use ironplc_parser::options::CompilerOptions;
    use ironplc_parser::parse_program;
    use ironplc_test::read_shared_resource;

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let lib = parse_shared_library("first_steps.st");
        let res = analyze(&[&lib], &CompilerOptions::default());
        assert!(res.is_ok());
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_ok_with_diagnostics() {
        let lib = parse_shared_library("first_steps_semantic_error.st");
        let res = analyze(&[&lib], &CompilerOptions::default());
        let (_library, context) = res.unwrap();
        assert!(context.has_diagnostics());
    }

    #[test]
    fn analyze_2() {
        let lib = parse_shared_library("main.st");
        let res = analyze(&[&lib], &CompilerOptions::default());
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
            parse_program(program1, &FileId::default(), &CompilerOptions::default()).unwrap();
        let program2 =
            parse_program(program2, &FileId::default(), &CompilerOptions::default()).unwrap();

        let result = analyze(&[&program1, &program2], &CompilerOptions::default());
        assert!(result.is_ok())
    }

    fn parse_shared_library(name: &'static str) -> Library {
        let src = read_shared_resource(name);
        parse_program(&src, &FileId::default(), &CompilerOptions::default()).unwrap()
    }

    // ---------------------------------------------------------------------
    // A diagnosed constant-expression initializer must report only its own
    // problem. The initializer-fold transform used to be reverted when it
    // diagnosed, leaking `SimpleExpr` nodes to later rules and raising a
    // P9998 internal error after every legitimate P4037.
    // ---------------------------------------------------------------------

    #[test]
    fn analyze_when_initializer_expression_and_flag_disabled_then_p4037_only() {
        let program = "
FUNCTION func : LREAL
VAR CONSTANT
d2r : LREAL := 3.0/180.0;
END_VAR
func := d2r;
END_FUNCTION";
        let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();

        let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

        let codes: Vec<&str> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert!(codes.contains(&"P4037"), "expected P4037, got: {codes:?}");
        // No internal error from a rule observing an unfolded initializer.
        assert!(!codes.contains(&"P9998"), "unexpected P9998 in: {codes:?}");
        // No cascaded "constant must have initializer" — the declaration
        // does carry an initializer, it was merely diagnosed.
        assert!(!codes.contains(&"P4008"), "unexpected P4008 in: {codes:?}");
    }

    // ---------------------------------------------------------------------
    // Don't revert a whole library's type resolution because one unrelated
    // declaration failed to resolve.
    // See specs/plans/2026-08-02-partial-resolution-revert-on-unrelated-error.md.
    // ---------------------------------------------------------------------

    #[test]
    fn analyze_when_unrelated_pou_has_undeclared_type_then_valid_pou_unaffected() {
        let program = "
FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR
    inst : FB_Callee;
END_VAR
    inst();
END_FUNCTION_BLOCK
        ";
        let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

        let diagnostics = context.diagnostics();
        assert_eq!(
            1,
            diagnostics.len(),
            "expected exactly one diagnostic, got {diagnostics:?}"
        );
        assert_eq!("P2008", diagnostics[0].code);
    }

    // ---------------------------------------------------------------------
    // Constant-expression VAR initializers.
    // ---------------------------------------------------------------------

    fn opts_with_constant_initializer_expressions() -> CompilerOptions {
        CompilerOptions {
            allow_constant_initializer_expressions: true,
            // Constants are only collected from true top-level VAR_GLOBAL
            // declarations, which since #1251 (P4028) require this flag.
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn analyze_when_constant_initializer_expression_and_flag_enabled_then_resolves() {
        let program = "
VAR_GLOBAL CONSTANT
    SCALE : LREAL := 2.5;
END_VAR
FUNCTION_BLOCK FB_Example
VAR
    scaled : LREAL := SCALE*4.0;
END_VAR
END_FUNCTION_BLOCK";
        let lib = parse_program(
            program,
            &FileId::default(),
            &opts_with_constant_initializer_expressions(),
        )
        .unwrap();
        let (_library, context) =
            analyze(&[&lib], &opts_with_constant_initializer_expressions()).unwrap();

        assert!(
            !context.has_diagnostics(),
            "unexpected diagnostics: {:?}",
            context.diagnostics()
        );
    }

    #[test]
    fn analyze_when_constant_initializer_expression_and_flag_disabled_then_diagnostics() {
        let program = "
VAR_GLOBAL CONSTANT
    SCALE : LREAL := 2.5;
END_VAR
FUNCTION_BLOCK FB_Example
VAR
    scaled : LREAL := SCALE*4.0;
END_VAR
END_FUNCTION_BLOCK";
        let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

        assert!(context.has_diagnostics());
    }

    // ---------------------------------------------------------------------
    // FB-instance call-style initializer (distinct node).
    // See specs/plans/2026-08-01-fb-call-style-initializer-distinct-node.md.
    // ---------------------------------------------------------------------

    #[test]
    fn analyze_when_fb_call_style_init_references_earlier_declared_fb_then_only_not_implemented() {
        // End-to-end: the call-style initializer references an earlier-declared
        // FB. It must produce exactly the "not yet supported" diagnostic
        // (P9999 NotImplemented) from the deferring rule -- and crucially NOT
        // a spurious P2011 "Parent type is not declared", which would appear
        // if the new FunctionBlockCall node were not wired into toposort/type
        // resolution like the FunctionBlock node.
        use ironplc_problems::Problem;

        let program = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3);
END_VAR
END_FUNCTION_BLOCK";
        let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

        let codes: Vec<&str> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert!(codes.contains(&"P9999"), "expected P9999, got: {codes:?}");
        assert!(
            !codes.contains(&Problem::ParentTypeNotDeclared.code()),
            "unexpected spurious P2011: {codes:?}"
        );
    }

    // ---------------------------------------------------------------------
    // THIS^ / SUPER^ (parsed, not analyzed or executed).
    // ---------------------------------------------------------------------

    /// A program using `THIS^` is rejected, and P9999 is among the reasons.
    ///
    /// Deliberately asserts presence rather than an exact diagnostic set:
    /// several passes meet the construct and each says so, and pinning the
    /// set would turn every later improvement into a test edit. What must
    /// hold is that no pass quietly accepts it.
    #[rstest::rstest]
    #[case::this_field_write("    THIS^.count := 1;")]
    #[case::super_field_read("    count := SUPER^.count;")]
    #[case::this_method_call("    THIS^.Start();")]
    fn analyze_when_self_ref_then_rejected_with_not_implemented(#[case] body: &str) {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let program = format!(
            "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Start
    count := 1;
END_METHOD
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
        );
        let lib = parse_program(&program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&lib], &options).unwrap();

        let codes: Vec<&str> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert!(
            codes.contains(&"P9999"),
            "expected P9999 among diagnostics, got: {codes:?}"
        );
    }

    /// The same function block without `THIS^` analyzes cleanly -- the new
    /// arms must not report anything for programs that do not use it.
    #[test]
    fn analyze_when_no_self_ref_then_no_not_implemented() {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let program = "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Start
    count := 1;
END_METHOD
END_FUNCTION_BLOCK";
        let lib = parse_program(program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&lib], &options).unwrap();

        let codes: Vec<&str> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.as_str())
            .collect();
        assert!(codes.is_empty(), "expected no diagnostics, got: {codes:?}");
    }
}
