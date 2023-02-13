//! Transform that resolves late bound types into specific types.
//!
//! The IEC 61131-3 syntax has some ambiguous types that are initially
//! parsed into a placeholder. This transform replaces the placeholders
//! with well-known types.
use ironplc_dsl::core::SourcePosition;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_dsl::{common::*, core::Id};
use phf::{phf_set, Set};
use std::collections::HashMap;

use crate::error::SemanticDiagnostic;

static ELEMENTARY_TYPES_LOWER_CASE: Set<&'static str> = phf_set! {
    // signed_integer_type_name
    "sint",
    "int",
    "dint",
    "lint",
    // unsigned_integer_type_name
    "usint",
    "uint",
    "udint",
    "ulint",
    // real_type_name
    "real",
    "lreal",
    // date_type_name
    "date",
    "time_of_day",
    "tod",
    "date_and_time",
    "dt",
    // bit_string_type_name
    "bool",
    "byte",
    "word",
    "dword",
    "lword",
    // remaining elementary_type_name
    "string",
    "wstring",
    "time"
};

pub fn apply(lib: Library) -> Result<Library, SemanticDiagnostic> {
    let mut type_map = HashMap::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    let mut visitor = GlobalTypeDefinitionVisitor {
        types: &mut type_map,
    };
    visitor.walk(&lib)?;

    // Set the types for each item.
    let mut resolver = TypeResolver { types: type_map };
    resolver.fold(lib)
}

// Finds types that are valid as variable types. These include enumerations,
// function blocks, functions, structures.
struct GlobalTypeDefinitionVisitor<'a> {
    types: &'a mut HashMap<Id, TypeDefinitionKind>,
}
impl GlobalTypeDefinitionVisitor<'_> {
    fn insert(&mut self, to_add: &Id, kind: TypeDefinitionKind) -> Result<(), SemanticDiagnostic> {
        if let Some((key, _)) = self.types.get_key_value(to_add) {
            return Err(SemanticDiagnostic::error(
                "S0001",
                format!("Duplicated definitions for name {}", to_add),
            )
            .maybe_with_label(key.position(), "First use")
            .maybe_with_label(to_add.position(), "Second name"));
        }

        self.types.insert(to_add.clone(), kind);

        Ok(())
    }
}
impl<'a> Visitor<SemanticDiagnostic> for GlobalTypeDefinitionVisitor<'a> {
    type Value = ();
    fn visit_enum_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.insert(&node.type_name, TypeDefinitionKind::Enumeration)
    }
    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.insert(&node.name, TypeDefinitionKind::FunctionBlock)
    }
    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<(), SemanticDiagnostic> {
        self.insert(&node.name, TypeDefinitionKind::FunctionBlock)
    }
    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        self.insert(&node.type_name, TypeDefinitionKind::Structure)
    }
}

struct TypeResolver {
    types: HashMap<Id, TypeDefinitionKind>,
}

impl TypeResolver {
    fn is_elementary_type(id: &Id) -> bool {
        ELEMENTARY_TYPES_LOWER_CASE.contains(&id.lower_case().to_string())
    }
}

impl Fold<SemanticDiagnostic> for TypeResolver {
    fn fold_type_initializer(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, SemanticDiagnostic> {
        match node {
            InitialValueAssignmentKind::LateResolvedType(name) => {
                // Try to find the type for the specified name.
                if TypeResolver::is_elementary_type(&name) {
                    return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: name,
                        initial_value: None,
                    }));
                }

                // TODO error handling
                let maybe_type_kind = self.types.get(&name);
                match maybe_type_kind {
                    Some(type_kind) => {
                        match type_kind {
                            TypeDefinitionKind::Enumeration => {
                                Ok(InitialValueAssignmentKind::EnumeratedType(
                                    EnumeratedInitialValueAssignment {
                                        type_name: name,
                                        initial_value: None,
                                    },
                                ))
                            }
                            TypeDefinitionKind::FunctionBlock => {
                                Ok(InitialValueAssignmentKind::FunctionBlock(
                                    FunctionBlockInitialValueAssignment { type_name: name },
                                ))
                            }
                            TypeDefinitionKind::Function => {
                                // TODO this is wrong and should be an error
                                panic!()
                            }
                            TypeDefinitionKind::Structure => {
                                Ok(InitialValueAssignmentKind::Structure(
                                    StructureInitializationDeclaration {
                                        type_name: name,
                                        elements_init: vec![],
                                    },
                                ))
                            }
                        }
                    }
                    None => {
                        todo!()
                    }
                }
            }
            _ => Ok(node),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::SemanticDiagnostic;
    use crate::test_helpers::*;
    use crate::xform_resolve_late_bound_types::TypeResolver;
    use ironplc_dsl::core::SourceLoc;
    use ironplc_dsl::fold::Fold;
    use ironplc_dsl::{common::*, core::Id};
    use std::collections::HashMap;

    #[test]
    fn fold_when_has_function_block_type_then_resolves_type() {
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                variables: vec![VarDecl::late_bound_var(
                    "var_name",
                    "var_type",
                    SourceLoc::new(0),
                )],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ))
        .unwrap();

        let mut type_map = HashMap::new();
        type_map.insert(Id::from("var_type"), TypeDefinitionKind::FunctionBlock);
        let mut type_resolver = TypeResolver { types: type_map };

        let result = type_resolver.fold(input);

        let expected = new_library::<SemanticDiagnostic>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                variables: vec![VarDecl::function_block_var(
                    "var_name",
                    "var_type",
                    SourceLoc::new(0),
                )],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ));

        assert_eq!(result, expected)
    }

    #[test]
    fn fold_when_has_structure_type_then_resolves_type() {
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                variables: vec![VarDecl::late_bound_var(
                    "var_name",
                    "var_type",
                    SourceLoc::new(0),
                )],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ))
        .unwrap();

        let mut type_map = HashMap::new();
        type_map.insert(Id::from("var_type"), TypeDefinitionKind::Structure);
        let mut type_resolver = TypeResolver { types: type_map };

        let result = type_resolver.fold(input);

        let expected = new_library::<SemanticDiagnostic>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                variables: vec![VarDecl::structure_var(
                    "var_name",
                    "var_type",
                    SourceLoc::new(0),
                )],
                body: FunctionBlockBody::stmts(vec![]),
            },
        ));

        assert_eq!(result, expected)
    }
}
