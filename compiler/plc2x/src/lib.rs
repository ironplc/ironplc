// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

pub mod cli;
mod compilation_set;
pub mod logger;
pub mod lsp;
pub mod lsp_project;
pub mod project;
mod result;
mod rule_decl_struct_element_unique_names;
mod rule_decl_subrange_limits;
mod rule_enumeration_values_unique;
mod rule_function_block_invocation;
mod rule_pous_no_cycles;
mod rule_program_task_definition_exists;
mod rule_unsupported_stdlib_type;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod source;
mod stages;
mod stdlib;
mod symbol_graph;
mod symbol_table;
mod xform_assign_file_id;
mod xform_resolve_late_bound_data_decl;
mod xform_resolve_late_bound_type_initializer;

#[cfg(test)]
mod test_helpers;
