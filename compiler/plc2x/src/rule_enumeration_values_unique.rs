//! Semantic rule that enumeration values in declarations must be unique.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//! LOGLEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//! LOGLEVEL : (CRITICAL, CRITICAL) := CRITICAL;
//! END_TYPE
//! ```
use ironplc_dsl::{
    common::*,
    core::{FileId, Id, SourcePosition},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use std::collections::HashSet;

pub fn apply(lib: &Library) -> Result<(), Diagnostic> {
    let mut visitor = RuleEnumerationValuesUnique {};
    visitor.walk(lib)
}

struct RuleEnumerationValuesUnique {}

impl Visitor<Diagnostic> for RuleEnumerationValuesUnique {
    type Value = ();

    fn visit_enum_declaration(&mut self, node: &EnumerationDeclaration) -> Result<(), Diagnostic> {
        match &node.spec_init.spec {
            EnumeratedSpecificationKind::TypeName(_) => Ok(()),
            EnumeratedSpecificationKind::Values(spec) => {
                let mut seen_values: HashSet<&Id> = HashSet::new();
                for current in &spec.values {
                    // TODO this needs to be updated - this doesn't do
                    // a comparision that includes the type of the enumeration
                    let seen = seen_values.get(&current.value);
                    match seen {
                        Some(first) => {
                            return Err(Diagnostic::new(
                                "S0004",
                                format!(
                                    "Enumeration type declaration {} has duplicated value {}",
                                    node.type_name, first
                                ),
                                Label::source_loc(
                                    FileId::default(),
                                    first.position(),
                                    "First instance",
                                ),
                            )
                            .with_secondary(Label::source_loc(
                                FileId::default(),
                                current.position(),
                                "Duplicate value",
                            )));
                        }
                        None => {
                            seen_values.insert(&current.value);
                        }
                    }
                }
                Ok(())
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

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_typename_values_unique_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, ERROR);
LOGLEVEL2 : LOGLEVEL;
END_TYPE";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_value_duplicated_then_error() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, CRITICAL);
END_TYPE";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_err());
    }
}
