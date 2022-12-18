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
use ironplc_dsl::{dsl::*, visitor::Visitor};
use std::{collections::HashSet, hash::Hash};

pub fn apply(lib: &Library) -> Result<(), String> {
    let mut visitor = RuleEnumerationValuesUnique {};
    visitor.walk(&lib)
}

struct RuleEnumerationValuesUnique {
}

impl Visitor<String> for RuleEnumerationValuesUnique {
    type Value = ();

    fn visit_enum_declaration(&mut self, node: &EnumerationDeclaration) -> Result<(), String> {
        match &node.spec {
            EnumeratedSpecificationKind::TypeName(_) => return Ok(Self::Value::default()),
            EnumeratedSpecificationKind::Values(values) => {
                let mut seen_values = HashSet::new();
                for value in values {
                    if seen_values.contains(&value) {
                        return Err(format!("Enumeration declaration {} has duplicated value {}", node.name, value));
                    }
                    seen_values.insert(value);
                }
                return Ok(Self::Value::default())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::dsl::*;

    use super::*;

    use crate::test_helpers::new_library;

    #[test]
    fn apply_when_values_unique_then_ok() {
        let input = new_library::<String>(LibraryElement::DataTypeDeclaration(
            vec![
                EnumerationDeclaration {
                    name: String::from("LOGLEVEL"),
                    spec: EnumeratedSpecificationKind::Values(vec![
                        String::from("CRITICAL"),
                        String::from("ERROR"),
                    ]),
                    default: None,
                }
            ]
        )).unwrap();

        let result = apply(&input);
        assert_eq!(true, result.is_ok())
    }

    #[test]
    fn apply_when_value_duplicated_then_error() {
        let input = new_library::<String>(LibraryElement::DataTypeDeclaration(
            vec![
                EnumerationDeclaration {
                    name: String::from("LOGLEVEL"),
                    spec: EnumeratedSpecificationKind::Values(vec![
                        String::from("CRITICAL"),
                        String::from("CRITICAL"),
                    ]),
                    default: None,
                }
            ]
        )).unwrap();

        let result = apply(&input);
        assert_eq!(true, result.is_err())
    }
}