#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{
    common::TypeName,
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
    rule_abstract_not_instantiated, rule_array_index_range, rule_array_initializer_length,
    rule_array_subscript_count, rule_array_subscript_operand, rule_array_subscript_type,
    rule_assignment_aggregate_type_compat, rule_bit_and_partial_access_range,
    rule_bit_shift_input_type, rule_call_input_type, rule_case_radix_label,
    rule_case_selector_type, rule_comparison_operand_type, rule_condition_type,
    rule_constant_not_written, rule_constant_range, rule_decl_struct_element_unique_names,
    rule_edge_input_owner, rule_enum_base_type_allowed, rule_enum_explicit_value_allowed,
    rule_enumeration_values_unique, rule_extends_base_declared, rule_extends_field_duplicated,
    rule_for_control_type, rule_for_step_nonzero, rule_function_block_call_unsupported,
    rule_function_block_invocation, rule_function_call_declared,
    rule_function_call_in_out_argument, rule_function_call_type_check,
    rule_function_inputs_common_type, rule_interface_conformance, rule_interface_conversion,
    rule_keyword_name_allowed, rule_loop_control_inside_loop, rule_member_qualifier_allowed,
    rule_member_qualifier_invalid, rule_method_call_declared, rule_mixed_located_var_declarations,
    rule_no_top_level_var_global, rule_operator_operand_type_check, rule_override_signature,
    rule_pou_hierarchy, rule_program_task_definition_exists, rule_program_var_hides_global,
    rule_range_limits, rule_real_literal_integer_position, rule_real_literal_range, rule_ref_to,
    rule_return_type_declared, rule_self_reference_context, rule_stdlib_type_redefinition,
    rule_string_encoding_compat, rule_string_length_range, rule_string_literal_char_range,
    rule_struct_initializer_expression_allowed, rule_task_configuration, rule_task_names_unique,
    rule_temporal_literal_range, rule_unsupported_extension, rule_use_declared_enumerated_value,
    rule_use_declared_field, rule_use_declared_symbolic_var, rule_var_decl_const_initialized,
    rule_var_decl_const_not_fb, rule_var_decl_global_const_requires_external_const,
    rule_var_decl_initializer_type_compat, rule_var_external_matches_global,
    semantic_context::SemanticContext,
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
    system_globals::SYSTEM_UPTIME_GLOBALS,
    type_environment::{TypeEnvironment, TypeEnvironmentBuilder},
    xform_bind_call_arguments, xform_fold_constant_expressions, xform_fold_initializer_expressions,
    xform_insert_implicit_conversions, xform_insert_implicit_deref, xform_int_to_bool_initializer,
    xform_mark_unwritten_constants, xform_remove_unsigned_abs, xform_resolve_adr,
    xform_resolve_constant_expressions, xform_resolve_decl_types, xform_resolve_expr_types,
    xform_resolve_initial_values, xform_resolve_late_bound_expr_kind,
    xform_resolve_late_bound_type_initializer, xform_resolve_symbol_and_function_environment,
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

    // Record the implicit conversions the backends compile and the language
    // server shows. After the rules, so that a rule checks the operands the
    // program wrote rather than their conversions. See ADR-0056.
    let library = xform_insert_implicit_conversions::apply(library, &context, options);

    // A rule that reads the type the pass records on each literal, so it
    // runs on the library the pass returns. It reads an operand as the
    // program wrote it through the conversion that wraps it.
    if let Err(diagnostics) = rule_constant_range::apply(&library, &context, options) {
        context.add_diagnostics(diagnostics);
    }

    Ok((library, context))
}

/// Runs a transform whose failure discards the whole library.
///
/// The pre-pass library is restored when the transform returns `Err`, so every
/// transformation it had already completed is thrown away with the one that
/// failed. Reserve this for a pass whose output is meaningless when any part
/// of it failed, and prefer [`run_best_effort`].
///
/// The reason the distinction exists is not one failing test. Corpus testing
/// showed what looked like merge-order or file-pairing sensitivity: a source
/// that analyzed cleanly alone failed once merged with unrelated code. The
/// cause was never ordering -- a transform that accumulates diagnostics and
/// then discards its whole result throws away every unrelated resolution it
/// had already completed. A pass that returns `Err` after a user-level
/// diagnostic reintroduces that, so a new pass reports per-declaration
/// problems through `run_best_effort` instead.
fn run_reverting_on_error(
    library: Library,
    diagnostics: &mut Vec<Diagnostic>,
    xform: impl FnOnce(Library) -> Result<Library, Vec<Diagnostic>>,
) -> Library {
    let fallback = library.clone();
    match xform(library) {
        Ok(result) => result,
        Err(errs) => {
            diagnostics.extend(errs);
            fallback
        }
    }
}

/// Runs a transform that reports per-declaration problems without discarding
/// the declarations it did transform.
///
/// `Ok((library, diagnostics))` means "here is the transformed library, and
/// here is what was wrong with parts of it": the transformed library is kept
/// and the diagnostics are collected alongside it. A best-effort pass reserves
/// `Err` for a failure that left it with no library to return at all, which
/// still reverts because there is nothing else to keep.
fn run_best_effort(
    library: Library,
    diagnostics: &mut Vec<Diagnostic>,
    xform: impl FnOnce(Library) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>,
) -> Library {
    let fallback = library.clone();
    match xform(library) {
        Ok((result, errs)) => {
            diagnostics.extend(errs);
            result
        }
        Err(errs) => {
            diagnostics.extend(errs);
            fallback
        }
    }
}

/// Resolves every declaration's and expression's type (stage 2).
///
/// The library this returns does not yet record implicit conversions: those
/// are inserted by [`analyze`] after the semantic rules, so a backend compiles
/// what [`analyze`] returns.
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

    let mut symbol_environment =
        SymbolEnvironment::new().with_bare_globals(options.allow_top_level_var_global);

    // Register implicit system globals when the uptime feature is enabled.
    if options.allow_system_uptime_global {
        for global in &SYSTEM_UPTIME_GLOBALS {
            symbol_environment
                .insert_compiler_provided(
                    &Id::from(global.name),
                    SymbolKind::Variable,
                    &ScopeKind::Global,
                    type_environment.id_of(&TypeName::from(global.type_name)),
                )
                .map_err(|e| vec![e])?;
        }
    }

    // Resolve constant references in type parameters (STRING lengths, array bounds).
    // Must run before toposort so that concrete integer values are available.
    // Best effort: an unresolvable reference is diagnosed and left as a
    // `Constant`, which is the state reverting would leave every reference in.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_constant_expressions::apply(lib, options)
    });

    // Hard failure: declaration ordering is required for all subsequent transforms.
    // Also computes the set of declarations reachable from PROGRAM roots,
    // which codegen uses to skip unused functions. A repeated declaration
    // name survives the sort; the environments built below diagnose it.
    let (mut library, reachable) = xform_toposort_declarations::apply(library)?;

    // Best effort: a repeated type or function block name is diagnosed by
    // the type environment, which keeps the first declaration, so the rest
    // of the library still resolves. `Err` is a declaration that cannot be
    // resolved at all, which still reverts.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_type_decl_environment::apply(lib, &mut type_environment)
    });

    // Best effort: an unresolvable declaration is diagnosed but does not
    // discard the rest of the library's successfully resolved declarations.
    let recoverable_xforms: Vec<
        fn(Library, &mut TypeEnvironment) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>,
    > = vec![xform_resolve_late_bound_type_initializer::apply];

    for xform in recoverable_xforms {
        library = run_best_effort(library, &mut diagnostics, |lib| {
            xform(lib, &mut type_environment)
        });
    }

    // Fold constant-expression VAR initializers (e.g. `scaled : LREAL := SCALE*4.0;`)
    // back into ordinary literal initializers, or diagnose. Must run before
    // any other pass touches `InitialValueAssignmentKind::SimpleExpr`.
    // Best effort: a diagnosed initializer is still normalized, so the
    // transformed library must be kept even when diagnostics are present —
    // reverting would leak `SimpleExpr` nodes to later passes.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_fold_initializer_expressions::apply(lib, &type_environment, options)
    });

    // Rewrite integer 0/1 initializers on BOOL variables to boolean literals.
    // Short-circuits internally when allow_int_to_bool_initializer is false.
    library = run_reverting_on_error(library, &mut diagnostics, |lib| {
        xform_int_to_bool_initializer::apply(lib, &mut type_environment, options)
    });

    // Record the type id each declaration declares, entering types spelled
    // out in place as anonymous types. Runs before the symbol environment
    // is built so that each variable symbol records its declared type id.
    library = run_reverting_on_error(library, &mut diagnostics, |lib| {
        xform_resolve_decl_types::apply(lib, &mut type_environment)
    });

    // Resolve the default of every declared type while the declarations
    // still name the types they were written with: resolving expression
    // types rewrites an alias in an initializer to the type it aliases.
    let type_defaults = xform_resolve_initial_values::TypeDefaults::of(&type_environment, &library);

    // Best effort: a repeated declaration name is diagnosed here, by the
    // environments, and the first declaration is kept, so the rest of the
    // library still resolves instead of reverting on the first repeat.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_symbol_and_function_environment::apply(
            lib,
            &mut symbol_environment,
            &mut function_environment,
            &mut type_environment,
        )
    });

    // Decide what each bare identifier in an expression is: a variable, or an
    // enumerated value. It needs the symbol environment: a name is a variable
    // when the environment resolves it to one the current scope may use,
    // however it is declared (a method's variable, a global, an inherited
    // field, the implicit input of a property accessor). Runs before the
    // passes that need bare names to already be variables.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_late_bound_expr_kind::apply(lib, &symbol_environment)
    });

    // Give TwinCAT `REFERENCE TO` variables their auto-dereferencing semantics
    // (bare reads/writes go through the reference) and lower `__ISVALIDREF`.
    // Runs after late-bound expression resolution (so bare identifiers are
    // already `ExprKind::Variable`) but before call binding
    // and expression typing (so `__ISVALIDREF` is lowered before either sees
    // it as a call to an undeclared function) and before the reference
    // semantic rules. See specs/design/reference-to-twincat.md (PR 2).
    library = run_reverting_on_error(library, &mut diagnostics, |lib| {
        xform_insert_implicit_deref::apply(lib, options)
    });

    // Rewrite the `ADR(x)` address-of operator into `ExprKind::Ref` when
    // `allow_adr` is set. Runs after implicit-deref (so a `REFERENCE TO`
    // operand is not mis-addressed) and before call binding
    // and expression typing (so a recognized `ADR` is not seen as a call to an
    // undeclared function).
    // Best effort: a diagnosed call is lowered to a placeholder, so the
    // transformed library is kept even when diagnostics are present.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_adr::apply(lib, options)
    });

    // Write every call's arguments in normal form: named, in the order the
    // callee declares its parameters, which is the order they are evaluated
    // in (specs/design/call-binding.md). Best effort: a call that does not
    // bind keeps its arguments, and the problem is reported, here for a
    // function call and by the rules for the others.
    let (bound, binding_diagnostics) = xform_bind_call_arguments::apply(
        library,
        &symbol_environment,
        &type_environment,
        &function_environment,
    );
    library = bound;
    diagnostics.extend(binding_diagnostics);

    // Resolve expression types using the function environment. Best effort:
    // an unqualified enumerated value whose type is ambiguous is diagnosed
    // and left without a type, and the rest of the library keeps its types.
    library = run_best_effort(library, &mut diagnostics, |lib| {
        xform_resolve_expr_types::apply(
            lib,
            &symbol_environment,
            &mut type_environment,
            &function_environment,
            options,
        )
    });

    // Fold constant binary and unary expressions.
    library = run_reverting_on_error(library, &mut diagnostics, |lib| {
        xform_fold_constant_expressions::apply(lib)
    });

    // ABS of an unsigned value is the value itself; no back end sees it.
    library = run_reverting_on_error(library, &mut diagnostics, |lib| {
        xform_remove_unsigned_abs::apply(lib, &type_environment)
    });

    // Mark every variable the program never writes as CONSTANT, so the
    // semantic rules and codegen see one notion of a constant variable, and
    // record the value each constant holds in the symbol environment.
    // Runs last: it needs bare identifiers resolved to variables, `ADR`
    // rewritten to `Ref`, user functions in the function environment and
    // named arguments made positional. Infallible, so nothing to revert.
    // See specs/design/constant-variable-inference.md.
    let library = xform_mark_unwritten_constants::apply(
        library,
        &type_environment,
        &function_environment,
        &mut symbol_environment,
    );

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
    )
    .with_type_defaults(type_defaults);
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
        rule_array_initializer_length::apply,
        rule_assignment_aggregate_type_compat::apply,
        rule_decl_struct_element_unique_names::apply,
        rule_range_limits::apply,
        rule_real_literal_range::apply,
        rule_enum_base_type_allowed::apply,
        rule_enum_explicit_value_allowed::apply,
        rule_enumeration_values_unique::apply,
        rule_loop_control_inside_loop::apply,
        rule_extends_base_declared::apply,
        rule_extends_field_duplicated::apply,
        rule_function_block_call_unsupported::apply,
        rule_function_block_invocation::apply,
        rule_function_call_declared::apply,
        rule_function_call_in_out_argument::apply,
        rule_function_call_type_check::apply,
        rule_function_inputs_common_type::apply,
        rule_interface_conformance::apply,
        rule_interface_conversion::apply,
        rule_keyword_name_allowed::apply,
        rule_member_qualifier_allowed::apply,
        rule_member_qualifier_invalid::apply,
        rule_method_call_declared::apply,
        rule_program_task_definition_exists::apply,
        rule_program_var_hides_global::apply,
        rule_no_top_level_var_global::apply,
        rule_operator_operand_type_check::apply,
        rule_override_signature::apply,
        rule_task_configuration::apply,
        rule_comparison_operand_type::apply,
        rule_task_names_unique::apply,
        rule_self_reference_context::apply,
        rule_stdlib_type_redefinition::apply,
        rule_string_encoding_compat::apply,
        rule_string_length_range::apply,
        rule_string_literal_char_range::apply,
        rule_temporal_literal_range::apply,
        rule_struct_initializer_expression_allowed::apply,
        rule_use_declared_enumerated_value::apply,
        rule_use_declared_field::apply,
        rule_use_declared_symbolic_var::apply,
        rule_return_type_declared::apply,
        rule_unsupported_extension::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_var_decl_initializer_type_compat::apply,
        rule_var_decl_global_const_requires_external_const::apply,
        rule_var_external_matches_global::apply,
        rule_mixed_located_var_declarations::apply,
        rule_pou_hierarchy::apply,
        rule_edge_input_owner::apply,
        rule_bit_and_partial_access_range::apply,
        rule_array_index_range::apply,
        rule_array_subscript_count::apply,
        rule_array_subscript_operand::apply,
        rule_array_subscript_type::apply,
        rule_bit_shift_input_type::apply,
        rule_case_radix_label::apply,
        rule_call_input_type::apply,
        rule_case_selector_type::apply,
        rule_condition_type::apply,
        rule_for_control_type::apply,
        rule_for_step_nonzero::apply,
        rule_real_literal_integer_position::apply,
        rule_constant_not_written::apply,
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
mod tests;
