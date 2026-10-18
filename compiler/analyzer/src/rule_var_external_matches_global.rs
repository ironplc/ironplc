//! Semantic rule that a `VAR_EXTERNAL` declaration names a global variable
//! and declares it with the global's type.
//!
//! A `VAR_EXTERNAL` declares no variable of its own. It gives a program or
//! a function block access to a global: a read or a write through it
//! reaches the global's storage. An external with no global to name has
//! nothing to reach, and one that declares another type would read that
//! storage as something it is not. See `specs/design/variable-binding.md`.
//!
//! The rule is the same on every dialect.
//!
//! See section 2.4.3.
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     MaxSpeed : INT := 100;
//!   END_VAR
//! END_CONFIGURATION
//!
//! PROGRAM main
//!   VAR_EXTERNAL
//!     MaxSpeed : INT;
//!   END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     MaxSpeed : INT := 100;
//!   END_VAR
//! END_CONFIGURATION
//!
//! PROGRAM main
//!   VAR_EXTERNAL
//!     MaxSpeed : DINT;
//!     MinSpeed : INT;
//!   END_VAR
//! END_PROGRAM
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    type_id::TypeId,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::SymbolInfo,
    system_globals::SYSTEM_UPTIME_GLOBALS,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleVarExternalMatchesGlobal {
            context,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleVarExternalMatchesGlobal<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl RuleVarExternalMatchesGlobal<'_> {
    /// Reports the external `name` when its type `external` is not the type
    /// of `global`. A type that did not resolve is another rule's to report.
    fn check_type(&mut self, name: &Id, external: Option<TypeId>, global: &SymbolInfo) {
        let (Some(external), Some(declared)) = (external, global.type_id) else {
            return;
        };
        let types = self.context.types();
        // An array is the only type a `VAR_GLOBAL` or a `VAR_EXTERNAL` can
        // spell in place.
        if types.same_type(external, declared) {
            return;
        }
        let mut diagnostic = Diagnostic::problem(
            Problem::ExternalMatchesNoGlobal,
            Label::span(
                name.span(),
                "External variable is not declared with the type of the global variable",
            ),
        )
        .with_context_id("variable", name);
        if let Some(type_name) = types.name_of(declared) {
            diagnostic = diagnostic.with_context_type("global type", type_name);
        }
        // A global the compiler provides has no declaration to point at.
        if !global.compiler_provided {
            diagnostic =
                diagnostic.with_secondary(Label::span(global.span.clone(), "Global variable"));
        }
        self.diagnostics.push(diagnostic);
    }

    /// Reports the external `name`, which no global variable is named.
    fn report_no_global(&mut self, name: &Id) {
        let mut diagnostic = Diagnostic::problem(
            Problem::ExternalMatchesNoGlobal,
            Label::span(name.span(), "No global variable has this name"),
        )
        .with_context_id("variable", name);
        // The likeliest cause for one of the compiler's own globals is that
        // the option that declares them is off.
        let is_system_global = SYSTEM_UPTIME_GLOBALS
            .iter()
            .any(|global| Id::from(global.name) == *name);
        if is_system_global {
            diagnostic = diagnostic.with_help(
                "The compiler declares this global when --allow-system-uptime-global is on.",
            );
        }
        self.diagnostics.push(diagnostic);
    }
}

impl DiagnosticVisitor for RuleVarExternalMatchesGlobal<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleVarExternalMatchesGlobal<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        if node.var_type != VariableType::External {
            return Ok(());
        }
        let Some(name) = node.identifier.symbolic_id() else {
            return Ok(());
        };
        match self.context.symbols().global_variable(name) {
            Some((_, global)) => self.check_type(name, node.type_id, global),
            None => self.report_no_global(name),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    use crate::test_helpers::{
        codes, configuration_with_globals as config, diagnostic_codes,
        program_with_externals as program, rule_codes, rule_diagnostics,
    };

    const TYPES: &str = "
TYPE
  Point : STRUCT x : INT; y : INT; END_STRUCT;
  Vector : STRUCT x : INT; y : INT; END_STRUCT;
  Speed : INT;
  Color : (RED, GREEN);
  Triple : ARRAY[1..3] OF INT;
END_TYPE
";

    #[rstest]
    #[case::elementary("g : INT;", "g : INT;")]
    #[case::other_case("MaxSpeed : INT;", "MAXSPEED : INT;")]
    #[case::structure("g : Point;", "g : Point;")]
    #[case::alias("g : Speed;", "g : Speed;")]
    #[case::enumeration("g : Color;", "g : Color;")]
    #[case::named_array("g : Triple;", "g : Triple;")]
    #[case::array_in_place("g : ARRAY[1..3] OF INT;", "g : ARRAY[1..3] OF INT;")]
    #[case::function_block("g : TON;", "g : TON;")]
    #[case::initialized_global("g : INT := 5;", "g : INT;")]
    #[case::one_of_several("a : INT; g : DINT; b : BOOL;", "g : DINT;")]
    fn apply_when_external_has_type_of_global_then_ok(
        #[case] globals: &str,
        #[case] externals: &str,
    ) {
        let source = format!("{TYPES}{}{}", config(globals), program(externals));

        let reported = rule_codes(super::apply, &source, &CompilerOptions::default());

        assert_eq!(reported, codes(&[]));
    }

    #[rstest]
    #[case::elementary("g : INT;", "g : DINT;")]
    #[case::signedness("g : INT;", "g : UINT;")]
    #[case::other_structure_of_same_shape("g : Point;", "g : Vector;")]
    #[case::alias_and_its_base("g : Speed;", "g : INT;")]
    #[case::named_array_and_same_shape_in_place("g : Triple;", "g : ARRAY[1..3] OF INT;")]
    #[case::array_extent("g : ARRAY[1..3] OF INT;", "g : ARRAY[1..4] OF INT;")]
    #[case::array_element("g : ARRAY[1..3] OF INT;", "g : ARRAY[1..3] OF DINT;")]
    #[case::array_dimensions("g : ARRAY[1..6] OF INT;", "g : ARRAY[1..2, 1..3] OF INT;")]
    #[case::function_block("g : TON;", "g : TOF;")]
    fn apply_when_external_has_another_type_then_error(
        #[case] globals: &str,
        #[case] externals: &str,
    ) {
        let source = format!("{TYPES}{}{}", config(globals), program(externals));

        let reported = rule_codes(super::apply, &source, &CompilerOptions::default());

        assert_eq!(reported, codes(&[Problem::ExternalMatchesNoGlobal]));
    }

    /// The type of a function block holds no copy of the block's fields,
    /// so an array of a block declared before the block -- a top-level
    /// `VAR_GLOBAL` is placed ahead of every declaration -- is the same type
    /// as one declared after it.
    #[spec_test(REQ_ETR_analyzer_005)]
    #[test]
    fn apply_when_global_array_of_block_declared_before_block_then_ok() {
        let source = "
VAR_GLOBAL g : ARRAY[1..2] OF Counter; END_VAR
FUNCTION_BLOCK Counter
  VAR_OUTPUT count : INT := 7; label : STRING[4]; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
  VAR_EXTERNAL g : ARRAY[1..2] OF Counter; END_VAR
END_PROGRAM
";
        let options = CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        };

        let reported = rule_codes(super::apply, source, &options);

        assert_eq!(reported, codes(&[]));
    }

    rule_err_at!(
        apply_when_external_names_no_global_then_error_at_name,
        &format!("{}{}", config("g : INT;"), program("missing : INT;")),
        Problem::ExternalMatchesNoGlobal,
        "missing"
    );

    // The program comes first, so that the first `speed` in the source is
    // the external's.
    rule_err_at!(
        apply_when_external_has_another_type_then_error_at_name,
        &format!("{}{}", program("speed : DINT;"), config("speed : INT;")),
        Problem::ExternalMatchesNoGlobal,
        "speed"
    );

    rule_err!(
        apply_when_no_configuration_then_error,
        &program("g : INT;"),
        [Problem::ExternalMatchesNoGlobal]
    );

    rule_err!(
        apply_when_external_names_a_type_then_error,
        &format!("{TYPES}{}{}", config("g : INT;"), program("Point : INT;")),
        [Problem::ExternalMatchesNoGlobal]
    );

    rule_err!(
        apply_when_external_names_another_units_variable_then_error,
        &format!(
            "
FUNCTION_BLOCK other
  VAR
    local : INT;
  END_VAR
END_FUNCTION_BLOCK
{}{}",
            config("g : INT;"),
            program("local : INT;")
        ),
        [Problem::ExternalMatchesNoGlobal]
    );

    rule_err!(
        apply_when_two_externals_do_not_match_then_one_error_each,
        &format!(
            "{}{}",
            config("g : INT;"),
            program("g : DINT; missing : INT;")
        ),
        [Problem::ExternalMatchesNoGlobal; 2]
    );

    rule_ok!(
        apply_when_function_block_external_matches_then_ok,
        &format!(
            "
FUNCTION_BLOCK fb
  VAR_EXTERNAL
    g : INT;
  END_VAR
END_FUNCTION_BLOCK
{}{}",
            config("g : INT;"),
            program("g : INT;")
        )
    );

    rule_err!(
        apply_when_function_block_external_has_another_type_then_error,
        &format!(
            "
FUNCTION_BLOCK fb
  VAR_EXTERNAL
    g : BOOL;
  END_VAR
END_FUNCTION_BLOCK
{}{}",
            config("g : INT;"),
            program("g : INT;")
        ),
        [Problem::ExternalMatchesNoGlobal]
    );

    fn top_level_globals() -> CompilerOptions {
        CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        }
    }

    rule_ok!(
        apply_when_external_names_top_level_global_then_ok,
        &format!("VAR_GLOBAL g : INT; END_VAR{}", program("g : INT;")),
        top_level_globals()
    );

    rule_err!(
        apply_when_external_of_top_level_global_has_another_type_then_error,
        &format!("VAR_GLOBAL g : INT; END_VAR{}", program("g : DINT;")),
        [Problem::ExternalMatchesNoGlobal],
        top_level_globals()
    );

    fn system_uptime() -> CompilerOptions {
        CompilerOptions {
            allow_system_uptime_global: true,
            ..CompilerOptions::default()
        }
    }

    rule_ok!(
        apply_when_external_names_compiler_provided_global_then_ok,
        &program("__SYSTEM_UP_TIME : TIME; __SYSTEM_UP_LTIME : LTIME;"),
        system_uptime()
    );

    rule_err!(
        apply_when_external_of_compiler_provided_global_has_another_type_then_error,
        &program("__SYSTEM_UP_TIME : DINT;"),
        [Problem::ExternalMatchesNoGlobal],
        system_uptime()
    );

    #[test]
    fn apply_when_compiler_provided_global_not_enabled_then_error_names_the_option() {
        let source = program("__SYSTEM_UP_TIME : TIME;");

        let diagnostics = rule_diagnostics(super::apply, &source, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            codes(&[Problem::ExternalMatchesNoGlobal])
        );
        assert!(diagnostics[0]
            .help()
            .iter()
            .any(|help| help.contains("--allow-system-uptime-global")));
    }

    #[test]
    fn apply_when_external_names_no_global_then_no_help() {
        let source = program("missing : INT;");

        let diagnostics = rule_diagnostics(super::apply, &source, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            codes(&[Problem::ExternalMatchesNoGlobal])
        );
        assert!(diagnostics[0].help().is_empty());
    }

    // The examples of `docs/reference/compiler/problems/P4082.rst`.
    const DOCUMENTED_PROGRAM: &str = "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
    MinSpeed : INT;
  END_VAR
  VAR
    currentSpeed : DINT;
  END_VAR
  currentSpeed := MaxSpeed;
END_PROGRAM";

    rule_err!(
        apply_when_documented_failing_example_then_one_error_each,
        &format!(
            "{}{}",
            config("MaxSpeed : INT := 100;"),
            DOCUMENTED_PROGRAM.replace("MaxSpeed : INT;", "MaxSpeed : DINT;")
        ),
        [Problem::ExternalMatchesNoGlobal; 2]
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        &format!(
            "{}{DOCUMENTED_PROGRAM}",
            config("MaxSpeed : INT := 100;\n    MinSpeed : INT := 10;")
        )
    );

    rule_ok!(
        apply_when_external_type_not_declared_then_left_to_other_rules,
        &format!("{}{}", config("g : INT;"), program("g : Undeclared;"))
    );
}
