// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

mod function_environment;
mod intermediate_type;
mod result;
mod rule_decl_struct_element_unique_names;
mod rule_decl_subrange_limits;
mod rule_enumeration_values_unique;
mod rule_function_block_invocation;
mod rule_function_call_declared;
mod rule_pou_hierarchy;
mod rule_program_task_definition_exists;
mod rule_stdlib_type_redefinition;
mod rule_task_names_unique;
mod rule_unsupported_stdlib_type;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod scoped_table;
mod semantic_context;
pub mod stages;
mod stdlib;
mod string_similarity;
mod symbol_environment;
mod type_attributes;
mod type_category;
mod type_environment;
mod type_table;
mod xform_resolve_late_bound_expr_kind;
mod xform_resolve_late_bound_type_initializer;
mod xform_resolve_symbol_and_function_environment;
mod xform_resolve_type_aliases;
mod xform_resolve_type_decl_environment;
mod xform_toposort_declarations;

// Type declaration environment helper modules
mod intermediates;

// Re-export public types for external use
pub use function_environment::{
    FunctionEnvironment, FunctionEnvironmentBuilder, FunctionSignature,
};
pub use intermediate_type::IntermediateType;
pub use semantic_context::{SemanticContext, SemanticContextBuilder};
pub use type_attributes::TypeAttributes;
pub use type_category::TypeCategory;
pub use type_environment::{TypeEnvironment, TypeEnvironmentBuilder, UsageContext};

#[cfg(test)]
mod test_helpers;
