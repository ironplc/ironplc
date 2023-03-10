extern crate ironplc_dsl;
extern crate ironplc_parser;

mod error;
mod rule_decl_struct_element_unique_names;
mod rule_decl_subrange_limits;
mod rule_enumeration_values_unique;
mod rule_function_block_invocation;
mod rule_pous_no_cycles;
mod rule_program_task_definition_exists;
mod rule_use_declared_enumerated_value;
mod rule_use_declared_symbolic_var;
mod rule_var_decl_const_initialized;
mod rule_var_decl_const_not_fb;
mod rule_var_decl_global_const_requires_external_const;
mod stages;
mod symbol_graph;
mod symbol_table;
mod xform_resolve_late_bound_data_decl;
mod xform_resolve_late_bound_type_initializer;

#[cfg(test)]
mod test_helpers;

use codespan_reporting::diagnostic::Diagnostic;
use stages::{parse, semantic};

pub fn analyze(contents: &str) -> Result<(), Diagnostic<()>> {
    let library = parse(contents)?;
    semantic(&library)
}

#[cfg(test)]
mod test {
    use crate::{analyze, test_helpers::read_resource};

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let src = read_resource("first_steps.st");
        let res = analyze(&src);
        assert!(res.is_ok())
    }

    #[test]
    fn analyze_when_first_steps_syntax_error_then_result_is_err() {
        let src = read_resource("first_steps_syntax_error.st");
        let res = analyze(&src);
        assert!(res.is_err())
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let src = read_resource("first_steps_semantic_error.st");
        let res = analyze(&src);
        assert!(res.is_err())
    }
}
