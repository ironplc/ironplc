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
#[cfg(test)]
mod test_helpers;
mod type_resolver;

pub fn main() {
    let library = parse("").unwrap();
    semantic(&library).unwrap();

    // Code generation
}
