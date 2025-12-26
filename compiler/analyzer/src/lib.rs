// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

mod intermediate_type;
mod result;
mod rule_action_block_analysis;
mod rule_array_struct_analysis;
mod rule_class_method_analysis;
mod rule_control_flow_analysis;
mod rule_decl_struct_element_unique_names;
mod rule_decl_subrange_limits;
mod rule_enumeration_values_unique;
mod rule_external_function_type_checking;
mod rule_function_block_invocation;
mod rule_null_pointer_analysis;
mod rule_pou_hierarchy;
mod rule_program_task_definition_exists;
mod rule_range_constraint_analysis;
mod rule_reference_parameter_analysis;
mod rule_reference_type_analysis;
mod rule_runtime_error_analysis;
mod rule_unsupported_stdlib_type;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod scoped_table;
pub mod stages;
mod stdlib;
mod symbol_environment;
mod type_environment;
mod type_table;
mod xform_resolve_late_bound_expr_kind;
mod xform_resolve_late_bound_type_initializer;
mod xform_resolve_symbol_environment;
mod xform_resolve_type_aliases;
mod xform_resolve_type_decl_environment;
mod xform_toposort_declarations;

// Type declaration environment helper modules
mod intermediates;

#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod semantic_analysis_tests;
