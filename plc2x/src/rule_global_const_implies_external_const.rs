//! Semantic rule that global variables declared with the CONSTANT
//! qualifier class must be declared constant in contained element.
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

use ironplc_dsl::{core::Id, dsl::*, visitor::Visitor};

use crate::error::SemanticDiagnostic;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut global_consts = HashSet::new();

    // Collect the global constants
    let mut visitor = FindGlobalConstVars {
        global_consts: &mut global_consts,
    };
    visitor.walk(lib)?;

    // Check that externals with the same name are constants
    let mut visitor = RuleExternalGlobalConst {
        global_consts: &mut global_consts,
    };
    visitor.walk(lib)
}

struct FindGlobalConstVars<'a> {
    global_consts: &'a mut HashSet<Id>,
}
impl<'a> Visitor<SemanticDiagnostic> for FindGlobalConstVars<'a> {
    type Value = ();
    fn visit_declaration(&mut self, node: &Declaration) -> Result<Self::Value, SemanticDiagnostic> {
        if node.qualifier == StorageQualifier::Constant {
            self.global_consts.insert(node.name.clone());
        }
        Ok(())
    }
}

struct RuleExternalGlobalConst<'a> {
    global_consts: &'a mut HashSet<Id>,
}
impl<'a> Visitor<SemanticDiagnostic> for RuleExternalGlobalConst<'a> {
    type Value = ();

    fn visit_var_init_decl(
        &mut self,
        node: &VarInitDecl,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        if node.var_type == VariableType::External && node.qualifier != StorageQualifier::Constant {
            if let Some(global) = self.global_consts.get(&node.name) {
                return Err(SemanticDiagnostic::error(
                    "S0001",
                    format!(
                        "External var {} is global constant and must be declared constant",
                        node.name,
                    ),
                )
                .with_label(node.name.location(), "Reference to global variable")
                .with_label(global.location(), "Constant global variable"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::stages::parse;

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

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err())
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

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok())
    }
}
