use crate::error::SemanticDiagnostic;
use crate::symbol_graph::{SymbolGraph, SymbolNode};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_dsl::{common::*, core::Id};
use std::collections::HashMap;

#[derive(Clone)]
enum LateResolvableTypeDecl {
    Simple,
    Enumeration,
    Structure,
    LateBound,
    Unspecified,
}

pub fn apply(lib: Library) -> Result<Library, SemanticDiagnostic> {
    let mut declarations = TypeDeclResolver::new();

    // Populate the graph.
    declarations.walk(&lib)?;

    // Determine the types. We will create a mapping that says for item with
    // a particular type, how should we resolve it.
    let mut resolved_types = HashMap::new();
    for root in declarations.roots {
        let mut dfs = declarations.graph.dfs(root.0);
        while let Some(nx) = dfs.next(&declarations.graph) {
            match declarations.index_to_id.get(&nx) {
                Some(id) => {
                    resolved_types.insert(id.clone(), root.1.clone());
                }
                None => todo!(),
            }
        }
    }

    // Resolve the types. This is a single fold of the library
    let mut resolver = DeclarationResolver {
        ids_to_types: resolved_types,
    };
    resolver.fold(lib)
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

    fn add(&mut self, item: &Id, item_kind: LateResolvableTypeDecl) {
        let added = self.graph.add_node(item, item_kind);
        let data = self.graph.data(item);
        // TODO maybe this should be an error on the unwrap
        self.roots.push((
            added,
            data.map_or_else(|| LateResolvableTypeDecl::Unspecified, |v| v.clone()),
        ));
        self.index_to_id.insert(added, item.clone());
    }
}

impl Visitor<SemanticDiagnostic> for TypeDeclResolver {
    type Value = ();

    fn visit_simple_declaration(
        &mut self,
        node: &SimpleDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        // TODO handle name collisions
        self.add(&node.type_name, LateResolvableTypeDecl::Simple);
        Ok(())
    }

    fn visit_enum_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        self.add(&node.type_name, LateResolvableTypeDecl::Enumeration);
        Ok(())
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        self.add(&node.type_name, LateResolvableTypeDecl::Structure);
        Ok(())
    }

    fn visit_late_bound_declaration(
        &mut self,
        node: &LateBoundDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
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

impl Fold<SemanticDiagnostic> for DeclarationResolver {
    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, SemanticDiagnostic> {
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
                    LateResolvableTypeDecl::LateBound => todo!("Unable to resolve type"),
                    LateResolvableTypeDecl::Unspecified => todo!("Unable to resolve type"),
                }
            } else {
                todo!("Unable to resolve type")
            }
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use ironplc_dsl::{common::*, core::Id};

    #[test]
    fn apply_when_has_function_block_type_then_resolves_type() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let input = ironplc_parser::parse_program(program).unwrap();
        let library = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElement::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Id::from("LEVEL"),
                        spec_init: EnumeratedSpecificationInit::values_and_default(
                            vec!["CRITICAL"],
                            "CRITICAL",
                        ),
                    },
                )),
                LibraryElement::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
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
}
