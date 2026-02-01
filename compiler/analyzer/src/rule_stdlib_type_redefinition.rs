//! Semantic rule that checks for user-defined types that have the same name
//! as standard library types.
//!
//! Standard library types (TON, TOF, TP, CTU, CTD, CTUD, R_TRIG, F_TRIG, RS, SR)
//! are built into the language and cannot be redefined by users.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK TON  // Error: TON is a stdlib type
//!    VAR_INPUT
//!       value : INT;
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
    intermediates::stdlib_function_block::is_stdlib_function_block, result::SemanticResult,
    symbol_environment::SymbolEnvironment, type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = RuleStdlibTypeRedefinition {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleStdlibTypeRedefinition {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleStdlibTypeRedefinition {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        let name_lower = node.name.name.lower_case();
        if is_stdlib_function_block(name_lower.as_str()) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::StdlibTypeRedefinition,
                    Label::span(node.name.span(), "User-defined function block"),
                )
                .with_context_type("name", &node.name),
            );
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_only;

    #[test]
    fn apply_when_user_defines_ton_then_err() {
        // User tries to define a function block named TON
        let program = "
FUNCTION_BLOCK TON
VAR_INPUT
value : INT;
END_VAR
END_FUNCTION_BLOCK";

        let input = parse_only(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::StdlibTypeRedefinition.code(), err[0].code);
    }

    #[test]
    fn apply_when_user_defines_custom_fb_then_ok() {
        // User defines a custom function block with a non-stdlib name
        let program = "
FUNCTION_BLOCK MY_CUSTOM_FB
VAR_INPUT
value : INT;
END_VAR
END_FUNCTION_BLOCK";

        let input = parse_only(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_user_defines_r_trig_then_err() {
        // R_TRIG is a stdlib type
        let program = "
FUNCTION_BLOCK R_TRIG
VAR_INPUT
clk : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let input = parse_only(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&input, &type_env, &symbol_env);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::StdlibTypeRedefinition.code(), err[0].code);
    }
}
