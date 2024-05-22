//! Semantic rule that references to enumerations use enumeration values
//! that are part of the enumeration declaration.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    LEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    LEVEL : (INFO) := INFO;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::{Id, SourcePosition},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::{HashMap, HashSet};

use crate::result::SemanticResult;

pub fn apply(lib: &Library) -> SemanticResult {
    // Collect the data type definitions from the library into a map so that
    // we can quickly look up invocations
    let mut enum_defs = HashMap::new();
    for elem in lib.elements.iter() {
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
            enum_dec,
        )) = elem
        {
            enum_defs.insert(enum_dec.type_name.clone(), enum_dec);
        }
    }

    // Walk the library to find all references to enumerations
    // checking that all references use an enumeration value
    // that is part of the enumeration
    let mut visitor = RuleDeclaredEnumeratedValues::new(&enum_defs);
    visitor.walk(lib).map_err(|e| vec![e])
}

struct RuleDeclaredEnumeratedValues<'a> {
    enum_defs: &'a HashMap<Id, &'a EnumerationDeclaration>,
}
impl<'a> RuleDeclaredEnumeratedValues<'a> {
    fn new(enum_defs: &'a HashMap<Id, &'a EnumerationDeclaration>) -> Self {
        RuleDeclaredEnumeratedValues { enum_defs }
    }

    /// Returns enumeration values for a given enumeration type name.
    ///
    /// Recursively finds the enumeration values when one enumeration
    /// declaration is an alias of another enumeration declaration.
    ///
    /// Returns Ok containing the list of enumeration values.
    ///
    /// # Errors
    ///
    /// Returns Err(String) description of the error if:
    ///
    /// * a type name does not exist
    /// * recursive type name
    fn find_enum_declaration_values(
        &self,
        type_name: &'a Id,
    ) -> Result<&Vec<EnumeratedValue>, Diagnostic> {
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
                    match &def.spec_init.spec {
                        EnumeratedSpecificationKind::TypeName(n) => name = n,
                        EnumeratedSpecificationKind::Values(values) => return Ok(&values.values),
                    }
                }
                None => {
                    return Err(Diagnostic::problem(
                        Problem::EnumNotDeclared,
                        Label::span(name.position(), "Enumeration reference"),
                    )
                    .with_context_id("name", name))
                }
            }

            // Check that our next name is new and we haven't seen it before
            if seen_names.contains(name) {
                return Err(Diagnostic::problem(
                    Problem::EnumRecursive,
                    Label::span(name.position(), "Current enumeration"),
                )
                .with_context_id("type", name));
            }
        }
    }
}

impl Visitor<Diagnostic> for RuleDeclaredEnumeratedValues<'_> {
    type Value = ();

    fn visit_enumerated_initial_value_assignment(
        &mut self,
        init: &EnumeratedInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        let defined_values = self.find_enum_declaration_values(&init.type_name)?;
        if let Some(value) = &init.initial_value {
            // TODO this is using the Id, but not the full enumerated value
            // and we don't have declared appropriate comparison between things
            // that are known but partially declared
            if !defined_values.contains(value) {
                return Err(Diagnostic::problem(
                    Problem::EnumValueNotDefined,
                    Label::span(value.position(), "Expected value in enumeration"),
                )
                .with_context_id("value", &value.value));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::parse_and_resolve_types;

    #[test]
    fn apply_when_var_init_undefined_enum_value_then_error() {
        let program = "
TYPE
LEVEL : (INFO) := INFO;
END_TYPE
        
FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let result = apply(&library);

        assert!(result.is_err());
    }

    #[test]
    fn apply_when_var_init_valid_enum_value_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let result = apply(&library);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_var_init_valid_enum_value_through_alias_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
NAME : LEVEL_ALIAS := CRITICAL;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let result = apply(&library);

        assert!(result.is_ok());
    }
}
