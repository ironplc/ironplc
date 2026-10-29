// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn init_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
}

// Declared before the rule modules so the rule-test macros are in scope for
// their inline `#[cfg(test)] mod tests`.
#[cfg(test)]
#[macro_use]
mod test_macros;

mod builtin_function;
mod call_assignment_check;
mod call_binding;
mod callee_resolution;
mod clean_analysis;
mod constant_folding;
mod enumerated_value_type;
pub mod enumeration_members;
mod function_environment;
mod result;
mod rule_abstract_not_instantiated;
mod rule_array_index_range;
mod rule_array_initializer_length;
mod rule_array_subscript_count;
mod rule_array_subscript_type;
mod rule_assignment_aggregate_type_compat;
mod rule_bit_and_partial_access_range;
mod rule_bit_shift_input_type;
mod rule_call_input_type;
mod rule_case_radix_label;
mod rule_case_selector_type;
mod rule_comparison_operand_type;
mod rule_condition_type;
mod rule_constant_not_written;
mod rule_constant_range;
mod rule_decl_struct_element_unique_names;
mod rule_edge_input_owner;
mod rule_enum_base_type_allowed;
mod rule_enum_explicit_value_allowed;
mod rule_enumeration_values_unique;
mod rule_extends_base_declared;
mod rule_extends_field_duplicated;
mod rule_for_control_type;
mod rule_for_step_nonzero;
mod rule_function_block_call_unsupported;
mod rule_function_block_invocation;
mod rule_function_call_declared;
mod rule_function_call_in_out_argument;
mod rule_function_call_type_check;
mod rule_function_inputs_common_type;
mod rule_interface_conformance;
mod rule_interface_conversion;
mod rule_keyword_name_allowed;
mod rule_loop_control_inside_loop;
mod rule_member_qualifier_allowed;
mod rule_member_qualifier_invalid;
mod rule_method_call_declared;
mod rule_mixed_located_var_declarations;
mod rule_no_top_level_var_global;
mod rule_operator_operand_type_check;
mod rule_override_signature;
mod rule_pou_hierarchy;
mod rule_program_task_definition_exists;
mod rule_program_var_hides_global;
mod rule_range_limits;
mod rule_real_literal_integer_position;
mod rule_real_literal_range;
mod rule_ref_to;
mod rule_return_type_declared;
mod rule_self_reference_context;
mod rule_stdlib_type_redefinition;
mod rule_string_encoding_compat;
mod rule_string_length_range;
mod rule_string_literal_char_range;
mod rule_struct_initializer_expression_allowed;
mod rule_support;
mod rule_task_configuration;
mod rule_task_names_unique;
mod rule_temporal_literal_range;
mod rule_unsupported_extension;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_field;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod rule_var_decl_initializer_type_compat;
mod rule_var_external_matches_global;
mod scoped_table;
mod semantic_context;
pub mod semantic_type;
pub mod stages;
pub mod standard_function_block;
mod string_similarity;
mod supertypes;
pub mod symbol_environment;
pub mod system_globals;
mod type_attributes;
mod type_compat;
mod type_environment;
pub mod type_id;
pub mod value_range;
pub mod value_type;
mod variable_type;
mod write_collector;
mod xform_bind_call_arguments;
mod xform_fold_constant_expressions;
mod xform_fold_initializer_expressions;
mod xform_insert_implicit_conversions;
mod xform_insert_implicit_deref;
mod xform_int_to_bool_initializer;
mod xform_mark_unwritten_constants;
mod xform_remove_unsigned_abs;
mod xform_resolve_adr;
mod xform_resolve_constant_expressions;
mod xform_resolve_decl_types;
mod xform_resolve_expr_types;
pub mod xform_resolve_initial_values;
mod xform_resolve_late_bound_expr_kind;
mod xform_resolve_late_bound_type_initializer;
mod xform_resolve_symbol_and_function_environment;
mod xform_resolve_type_decl_environment;
mod xform_toposort_declarations;

// Type declaration environment helper modules
mod intermediates;

pub mod extractors;

// Re-export public types for external use
pub use builtin_function::{
    BitShift, BuiltinFunction, InputsOfOneType, NumericFunction, OneTypeResult, StringFunction,
    TimeFunction,
};
pub use callee_resolution::MethodOwners;
pub use clean_analysis::CleanAnalysis;
// The options an analysis ran with, which `SemanticContext::compiler_options`
// gives, re-exported so that a pass after the analyzer names them through
// this crate.
pub use function_environment::{
    FunctionEnvironment, FunctionEnvironmentBuilder, FunctionSignature,
};
pub use intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, typed_overload, FoldFailure, Overload,
    TypedOverload,
};
pub use intermediates::operator_function_form::FormOf;
pub use ironplc_parser::options::CompilerOptions;
pub use semantic_context::{SemanticContext, SemanticContextBuilder};
pub use semantic_type::SemanticType;
pub use standard_function_block::{CounterWidth, StandardFunctionBlock};
pub use type_attributes::TypeAttributes;
pub use type_environment::{elementary_type, TypeEnvironment, TypeEnvironmentBuilder};

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod test_rule_conventions;

// Spec conformance testing infrastructure (test-only).
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
#[cfg(test)]
mod spec_conformance_adr;
#[cfg(test)]
mod spec_conformance_arithmetic_operator_overloads;
#[cfg(test)]
mod spec_conformance_constant_inference;
#[cfg(test)]
mod spec_conformance_keyword_function_forms;
#[cfg(test)]
mod spec_conformance_pointer_to;
#[cfg(test)]
mod spec_conformance_variable_binding;
