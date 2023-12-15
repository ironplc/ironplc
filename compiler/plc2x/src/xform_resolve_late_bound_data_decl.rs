//! Transformation rule that changes late bound types into
//! specific types.
//! 
//! Late bound types are those where the type is ambiguous
//! after parsing.
//! 
//! The transformation succeeds when all data type declarations
//! resolve to a declared type.
use crate::symbol_graph::{SymbolGraph, SymbolNode};
use ironplc_dsl::core::SourcePosition;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_dsl::{common::*, core::Id};
use ironplc_problems::Problem;
use std::collections::HashMap;

#[derive(Clone)]
enum LateResolvableTypeDecl {
    Simple,
    Enumeration,
    Structure,
    LateBound,
    Unspecified,
}

pub fn apply(lib: Library) -> Result<Library, Diagnostic> {
    let mut declarations = TypeDeclResolver::new();

    // Populate the graph.
    declarations.walk(&lib)?;

    // Determine the types. Creates a mapping that says for item with
    // a particular type, how should we resolve it.
    let mut resolved_types = HashMap::new();
    for root in declarations.roots {
        let mut dfs = declarations.graph.dfs(root.0);
        while let Some(nx) = dfs.next(&declarations.graph) {
            match declarations.index_to_id.get(&nx) {
                Some(id) => {
                    resolved_types.insert(id.clone(), root.1.clone());
                }
                None => return Err(Diagnostic::todo(file!(), line!())),
            }
        }
    }

    // Resolve the types. This is a single fold of the library
    let mut resolver = DeclarationResolver {
        ids_to_types: resolved_types,
    };
    resolver.fold_library(lib)
}

struct TypeDeclResolver {
    graph: SymbolGraph<LateResolvableTypeDecl>,
    roots: Vec<(SymbolNode, LateResolvableTypeDecl)>,
    index_to_id: HashMap<SymbolNode, Id>,
}

impl TypeDeclResolver {
    fn new() -> Self {
        Self {
            graph: SymbolGraph::new(),
            roots: vec![],
            index_to_id: HashMap::new(),
        }
    }

    fn connect(&mut self, parent: &Id, child: &Id, child_kind: LateResolvableTypeDecl) {
        let parent_node = self
            .graph
            .add_node(parent, LateResolvableTypeDecl::Unspecified);
        self.index_to_id.insert(parent_node, parent.clone());
        let child_node = self.graph.add_node(child, child_kind);
        self.index_to_id.insert(child_node, child.clone());

        self.graph.add_edge(parent_node, child_node);
    }

    /// Adds a node into the graph for the specified name and having the
    /// specified type.
    ///
    /// If the name already exists, returns an diagnostic indicating the
    /// name conflict.
    fn add(&mut self, item: &Id, item_kind: LateResolvableTypeDecl) -> Result<(), Diagnostic> {
        if !self.graph.contains_node(item) {
            let added = self.graph.add_node(item, item_kind);
            let data = self.graph.data(item);
            self.roots.push((
                added,
                data.map_or_else(|| LateResolvableTypeDecl::Unspecified, |v| v.clone()),
            ));
            self.index_to_id.insert(added, item.clone());
            Ok(())
        } else {
            let existing = self
                .graph
                .get_node(item)
                .map(|kv| kv.0)
                .expect("Expected key");
            Err(Diagnostic::problem(
                Problem::DeclarationNameDuplicated,
                Label::source_loc(item.position(), "Duplicate declaration"),
            )
            .with_secondary(Label::source_loc(existing.position(), "First declaration")))
        }
    }
}

impl Visitor<Diagnostic> for TypeDeclResolver {
    type Value = ();

    fn visit_simple_declaration(
        &mut self,
        node: &SimpleDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.add(&node.type_name, LateResolvableTypeDecl::Simple)?;
        Ok(())
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.add(&node.type_name, LateResolvableTypeDecl::Enumeration)?;
        Ok(())
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.add(&node.type_name, LateResolvableTypeDecl::Structure)?;
        Ok(())
    }

    fn visit_late_bound_declaration(
        &mut self,
        node: &LateBoundDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.connect(
            &node.base_type_name,
            &node.data_type_name,
            LateResolvableTypeDecl::LateBound,
        );
        Ok(())
    }
}

struct DeclarationResolver {
    // Defines the desired type for each identifier
    ids_to_types: HashMap<Id, LateResolvableTypeDecl>,
}

impl Fold<Diagnostic> for DeclarationResolver {
    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        if let DataTypeDeclarationKind::LateBound(late_bound) = node {
            if let Some(desired_type) = self.ids_to_types.get(&late_bound.data_type_name) {
                match desired_type {
                    LateResolvableTypeDecl::Simple => {
                        return Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                            type_name: late_bound.data_type_name,
                            spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                                type_name: late_bound.base_type_name,
                                initial_value: None,
                            }),
                        }))
                    }
                    LateResolvableTypeDecl::Enumeration => {
                        return Ok(DataTypeDeclarationKind::Enumeration(
                            EnumerationDeclaration {
                                type_name: late_bound.data_type_name,
                                spec_init: EnumeratedSpecificationInit {
                                    spec: EnumeratedSpecificationKind::TypeName(
                                        late_bound.base_type_name,
                                    ),
                                    default: None,
                                },
                            },
                        ))
                    }
                    LateResolvableTypeDecl::Structure => {
                        return Ok(DataTypeDeclarationKind::StructureInitialization(
                            StructureInitializationDeclaration {
                                type_name: late_bound.data_type_name,
                                elements_init: vec![],
                            },
                        ))
                    }
                    LateResolvableTypeDecl::LateBound => {
                        return Err(Diagnostic::todo(file!(), line!()))
                    }
                    LateResolvableTypeDecl::Unspecified => {
                        return Err(Diagnostic::todo(file!(), line!()))
                    }
                }
            } else {
                return Err(Diagnostic::todo(file!(), line!()));
            }
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use ironplc_dsl::{
        common::*,
        core::{FileId, Id},
    };

    #[test]
    fn apply_when_ambiguous_enum_then_resolves_type() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let library = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Id::from("LEVEL"),
                        spec_init: EnumeratedSpecificationInit::values_and_default(
                            vec!["CRITICAL"],
                            "CRITICAL",
                        ),
                    },
                )),
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Id::from("LEVEL_ALIAS"),
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::TypeName(Id::from("LEVEL")),
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
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let result = apply(input);

        assert!(result.is_err())
    }
}
