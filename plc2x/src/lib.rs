use stages::{parse, semantic};

extern crate ironplc_dsl;
extern crate ironplc_parser;

mod rule_constant_vars_initialized;
mod rule_enumeration_values_unique;
mod rule_program_task_definition_exists;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_fb;
mod rule_use_declared_symbolic_var;
mod stages;
mod symbol_table;
mod xform_resolve_late_bound_types;

#[cfg(test)]
mod test_helpers;


pub fn main() {
    let library = parse("").unwrap();
    semantic(&library).unwrap();

    // Code generation
}
