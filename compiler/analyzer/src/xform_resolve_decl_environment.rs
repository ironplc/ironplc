//! Transformation rule that resolves declarations and builds
//! a type environment for all types in the source. This rule
//! handles types that are:
//!
//! * defined in the language
//! * defined by particular implementations
//! * defined by users
//!
//! This rules also transforms late bound declarations (those
//! that are ambiguous during parsing).
//!
//! The transformation succeeds when all data type declarations
//! resolve to a declared type.
use crate::type_environment::{TypeClass, TypeDescription, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_problems::Problem;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    // Populate environment (this also transforms late bound declarations).
    type_environment.fold_library(lib).map_err(|err| vec![err])
}

impl TypeEnvironment {
    fn transform_late_bound_declaration(
        &mut self,
        node: LateBoundDeclaration,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // At this point we should have a type for the late bound declaration
        // so we can replace the late bound declaration with the correct type
        let existing = self.get(&node.base_type_name);
        let existing = existing.unwrap();

        match existing.class {
            TypeClass::Simple => Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                type_name: node.data_type_name,
                spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: node.base_type_name,
                    initial_value: None,
                }),
            })),
            TypeClass::Enumeration => Ok(DataTypeDeclarationKind::Enumeration(
                EnumerationDeclaration {
                    type_name: node.data_type_name,
                    spec_init: EnumeratedSpecificationInit {
                        spec: EnumeratedSpecificationKind::TypeName(node.base_type_name),
                        default: None,
                    },
                },
            )),
            TypeClass::Structure => Ok(DataTypeDeclarationKind::StructureInitialization(
                StructureInitializationDeclaration {
                    type_name: node.data_type_name,
                    elements_init: vec![],
                },
            )),
        }
    }
}

impl Fold<Diagnostic> for TypeEnvironment {
    fn fold_simple_declaration(
        &mut self,
        node: SimpleDeclaration,
    ) -> Result<SimpleDeclaration, Diagnostic> {
        // A simple declaration cannot refer to another type so we
        // just need to insert this into the type environment.
        self.insert(
            &node.type_name,
            TypeDescription {
                // TODO
                span: node.type_name.span(),
                class: TypeClass::Simple,
            },
        )?;
        Ok(node)
    }

    fn fold_enumeration_declaration(
        &mut self,
        node: EnumerationDeclaration,
    ) -> Result<EnumerationDeclaration, Diagnostic> {
        // Enumeration declaration can define a set of values
        // or rename another enumeration.
        if let EnumeratedSpecificationKind::TypeName(base_type_name) = &node.spec_init.spec {
            if self.get(base_type_name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::ParentEnumNotDeclared,
                    Label::span(node.type_name.span(), "Enumeration"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type name")));
            }
        }

        self.insert(
            &node.type_name,
            TypeDescription {
                // TODO
                span: node.type_name.span(),
                class: TypeClass::Enumeration,
            },
        )?;
        Ok(node)
    }

    fn fold_structure_declaration(
        &mut self,
        node: StructureDeclaration,
    ) -> Result<StructureDeclaration, Diagnostic> {
        self.insert(
            &node.type_name,
            TypeDescription {
                // TODO
                span: node.type_name.span(),
                class: TypeClass::Structure,
            },
        )?;
        Ok(node)
    }

    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // The only type we care about here is late bound. We want to transform that
        // into a different type and need to return a different data type declaration
        // kind to do that. So, check the type and only handle it here if it is
        // a late bound kind.
        if let DataTypeDeclarationKind::LateBound(lb) = node {
            self.transform_late_bound_declaration(lb)
        } else {
            node.recurse_fold(self)
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::apply;
    use ironplc_dsl::{common::*, core::FileId};
    use ironplc_parser::options::ParseOptions;

    #[test]
    fn apply_when_ambiguous_enum_then_resolves_type() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let library = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Type::from("LEVEL"),
                        spec_init: EnumeratedSpecificationInit::values_and_default(
                            vec!["CRITICAL"],
                            "CRITICAL",
                        ),
                    },
                )),
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Type::from("LEVEL_ALIAS"),
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::TypeName(Type::from("LEVEL")),
                            default: None,
                        },
                    },
                )),
            ],
        };

        assert_eq!(library, expected)
    }

    #[test]
    fn apply_when_has_duplicate_items_then_error() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let result = apply(input);

        assert!(result.is_err())
    }
}
 */
