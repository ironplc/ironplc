//! Semantic rule that checks for references to types that are not supported.
//! This gives a nicer error message than "unknown type" when the problem is
//! the compiler.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FUNC
//!    VAR_INPUT
//!       NAME : TON;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult, stdlib::is_unsupported_standard_type,
    symbol_environment::SymbolEnvironment, type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = RuleUnsupportedStdLibType {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleUnsupportedStdLibType {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleUnsupportedStdLibType {
    type Value = ();

    fn visit_function_block_initial_value_assignment(
        &mut self,
        node: &FunctionBlockInitialValueAssignment,
    ) -> Result<(), Diagnostic> {
        if is_unsupported_standard_type(&node.type_name) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::UnsupportedStdLibType,
                Label::span(node.type_name.span(), "Unsupported variable type name"),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::parse_and_resolve_types;

    #[test]
    fn apply_when_has_ton_supported_type_then_ok() {
        let program = "
FUNCTION_BLOCK DUMMY
VAR_INPUT
name : TON;
END_VAR
         
END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        // TON is now supported, so this should not generate an error
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_has_tof_supported_type_then_ok() {
        let program = "
FUNCTION_BLOCK DUMMY
VAR_INPUT
name : TOF;
END_VAR
         
END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        // TOF is now supported, so this should not generate an error
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_has_ctu_unsupported_type_then_err() {
        let program = "
FUNCTION_BLOCK DUMMY
VAR_INPUT
name : CTU;
END_VAR
         
END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        // CTU is still unsupported, so this should generate an error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::UnsupportedStdLibType.code(), err[0].code);
    }
}
