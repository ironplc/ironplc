//! Semantic rule that enumeration values in declarations must be unique.
//!
//! ## Passes
//!
//! TYPE
//! LOGLEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//!
//! ## Fails
//!
//! //! TYPE
//! LOGLEVEL : (CRITICAL, CRITICAL) := CRITICAL;
//! END_TYPE
use ironplc_dsl::{core::Id, dsl::*, visitor::Visitor};
use std::collections::HashSet;

use crate::error::SemanticDiagnostic;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor = RuleEnumerationValuesUnique {};
    visitor.walk(&lib)
}

struct RuleEnumerationValuesUnique {}

impl Visitor<SemanticDiagnostic> for RuleEnumerationValuesUnique {
    type Value = ();

    fn visit_enum_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        match &node.spec {
            EnumeratedSpecificationKind::TypeName(_) => return Ok(Self::Value::default()),
            EnumeratedSpecificationKind::Values(spec) => {
                let mut seen_values: HashSet<&Id> = HashSet::new();
                for current in &spec.ids {
                    let seen = seen_values.get(&current);
                    match seen {
                        Some(first) => {
                            return Err(SemanticDiagnostic::error(
                                "S0004",
                                format!(
                                    "Enumeration declaration {} has duplicated value {}",
                                    node.name, first
                                ),
                            )
                            .with_label(first.location(), "First instance")
                            .with_label(current.location(), "Duplicate value"));
                        }
                        None => {
                            seen_values.insert(current);
                        }
                    }
                }
                return Ok(Self::Value::default());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_values_unique_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, ERROR);
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok());
    }

    #[test]
    fn apply_when_typename_values_unique_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, ERROR);
LOGLEVEL2 : LOGLEVEL;
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok());
    }

    #[test]
    fn apply_when_value_duplicated_then_error() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, CRITICAL);
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err());
    }
}
