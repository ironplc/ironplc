//! Semantic rule that global variables declared with the CONSTANT
//! qualifier class must be declared constant in contained element.
//!
//! See section 2.4.3.
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL CONSTANT
//!     ResetCounterValue : INT := 17;
//!   END_VAR
//! END_CONFIGURATION
//!
//! FUNCTION_BLOCK func
//!   VAR_EXTERNAL CONSTANT
//!     ResetCounterValue : INT
//!   END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL CONSTANT
//!     ResetCounterValue : INT := 17;
//!   END_VAR
//! END_CONFIGURATION
//!
//! FUNCTION_BLOCK func
//!   VAR_EXTERNAL
//!     ResetCounterValue : INT
//!   END_VAR
//! END_FUNCTION_BLOCK
//! ```
use std::collections::HashSet;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult, symbol_environment::SymbolEnvironment,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut global_consts = HashSet::new();

    // Collect the global constants
    let mut visitor = FindGlobalConstVars {
        global_consts: &mut global_consts,
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    // Check that externals with the same name are constants
    let mut visitor = RuleExternalGlobalConst {
        global_consts: &mut global_consts,
    };
    visitor.walk(lib).map_err(|e| vec![e])
}

struct FindGlobalConstVars<'a> {
    global_consts: &'a mut HashSet<Id>,
}
impl Visitor<Diagnostic> for FindGlobalConstVars<'_> {
    type Value = ();
    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if node.qualifier == DeclarationQualifier::Constant {
            match &node.identifier {
                VariableIdentifier::Symbol(name) => {
                    self.global_consts.insert(name.clone());
                }
                VariableIdentifier::Direct(_) => return Err(Diagnostic::todo(file!(), line!())),
            }
        }
        Ok(())
    }
}

struct RuleExternalGlobalConst<'a> {
    global_consts: &'a mut HashSet<Id>,
}
impl Visitor<Diagnostic> for RuleExternalGlobalConst<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if node.var_type == VariableType::External
            && node.qualifier != DeclarationQualifier::Constant
        {
            if let Some(name) = node.identifier.symbolic_id() {
                if let Some(global) = self.global_consts.get(name) {
                    return Err(Diagnostic::problem(
                        Problem::VariableMustBeConst,
                        Label::span(node.identifier.span(), "Reference to global variable"),
                    )
                    .with_context("variable", &node.identifier.to_string())
                    .with_secondary(Label::span(global.span(), "Constant global variable")));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_global_const_external_not_const_then_error() {
        let program = "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        ResetCounterValue : INT := 17;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK func
    VAR_EXTERNAL
        ResetCounterValue : INT;
    END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_global_const_external_const_then_ok() {
        let program = "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        ResetCounterValue : INT := 17;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE

END_CONFIGURATION

FUNCTION_BLOCK func
    VAR_EXTERNAL CONSTANT
        ResetCounterValue : INT;
    END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok())
    }
}
