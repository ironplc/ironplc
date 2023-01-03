//! Semantic rule that references to enumerations use enumeration values
//! that are part of the enumeration declaration.
//!
//! ## Passes
//!
//! TYPE
//!    LOGLEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LOGLEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! ## Fails
//!
//! TYPE
//!    LOGLEVEL : (INFO) := INFO;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LOGLEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
use ironplc_dsl::{ast::Id, dsl::*, visitor::Visitor};
use std::collections::{HashMap, HashSet};

use crate::error::SemanticDiagnostic;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    // Collect the data type definitions from the library into a map so that
    // we can quickly look up invocations
    let mut enum_defs = HashMap::new();
    for x in lib.elems.iter() {
        match x {
            LibraryElement::DataTypeDeclaration(dtds) => {
                for dtd in dtds {
                    enum_defs.insert(dtd.name.clone(), dtd);
                }
            }
            _ => {}
        }
    }

    // Walk the library to find all references to enumerations
    // checking that all references use an enumeration value
    // that is part of the enumeration
    let mut visitor = RuleDeclaredEnumeratedValues::new(&enum_defs);
    visitor.walk(lib)
}

struct RuleDeclaredEnumeratedValues<'a> {
    enum_defs: &'a HashMap<Id, &'a EnumerationDeclaration>,
}
impl<'a> RuleDeclaredEnumeratedValues<'a> {
    fn new(enum_defs: &'a HashMap<Id, &'a EnumerationDeclaration>) -> Self {
        RuleDeclaredEnumeratedValues {
            enum_defs: enum_defs,
        }
    }

    /// Returns enumeration values for a given enumeration type name.
    ///
    /// Recursively finds the enumeration values when one enumeration
    /// declaration is a rename of another enumeration declaration.
    ///
    /// Returns Ok containing the list of enumeration values.
    ///
    /// # Errors
    ///
    /// Returns Err(String) description of the error if:
    ///
    /// * a type name does not exist
    /// * recursive type name
    fn find_enum_declaration_values(&self, type_name: &'a Id) -> Result<&Vec<Id>, SemanticDiagnostic> {
        // Keep track of names we've seen before so that we can be sure that
        // the loop terminates
        let mut seen_names = HashSet::new();

        let mut name = type_name;
        loop {
            match self.enum_defs.get(name) {
                Some(def) => {
                    seen_names.insert(name);
                    // The definition might be the final definition, or it
                    // might be a reference to another name
                    match &def.spec {
                        EnumeratedSpecificationKind::TypeName(n) => name = &n,
                        EnumeratedSpecificationKind::Values(values) => return Ok(values),
                    }
                }
                None => return Err(SemanticDiagnostic { code: "S0001", message: format!("Enumeration type {} is not declared", name)}),
            }

            // Check that our next name is new and we haven't seen it before
            if seen_names.contains(name) {
                return Err(SemanticDiagnostic { code: "S0001",message: format!("Recursive enumeration for type {}", name)});
            }
        }
    }
}

impl Visitor<SemanticDiagnostic> for RuleDeclaredEnumeratedValues<'_> {
    type Value = ();

    fn visit_enumerated_type_initializer(
        &mut self,
        init: &EnumeratedTypeInitializer,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        let defined_values = self.find_enum_declaration_values(&init.type_name)?;
        if let Some(value) = &init.initial_value {
            if !defined_values.contains(&value) {
                return SemanticDiagnostic::error("S0001", format!(
                    "Enumeration uses value {} which is not defined in the enumeration",
                    value
                ));
            }
        }

        return Ok(Self::Value::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_var_init_undefined_enum_value_then_error() {
        let program = "
TYPE
LOGLEVEL : (INFO) := INFO;
END_TYPE
        
FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LOGLEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err());
    }

    #[test]
    fn apply_when_var_init_valid_enum_value_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LOGLEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok());
    }
}
