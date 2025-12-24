//! Transformation rule that changes the order of declarations
//! so that items only have a reference to an already declared item.
//!
//! The transformation succeeds when:
//! 1. there are no cycles and
//! 2. the calls respect the POU hierarchy.
//!
//! Program can call function or function block
//! Function block can call function or other function block
//! Function can call other functions
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//!    VAR
//!       IN1: BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       CalleeInstance : Callee;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK SelfRecursive
//!    VAR
//!       SelfRecursiveInstance : SelfRecursive;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use core::fmt;
use ironplc_dsl::{
    common::*,
    core::{FileId, Id, SourceSpan},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use log::debug;
use petgraph::{
    algo::toposort,
    dot::{Config, Dot},
    stable_graph::{NodeIndex, StableDiGraph},
};
use std::collections::HashMap;

/// Convert a TypeDefinition to a DataTypeDeclarationKind for topological sorting
fn convert_type_definition_to_data_type_declaration(type_def: TypeDefinition) -> DataTypeDeclarationKind {
    match type_def.base_type {
        DataTypeSpecificationKind::Elementary(elem_type) => {
            match elem_type {
                ElementaryTypeName::StringWithLength(len) => {
                    // String with length should create a StringDeclaration
                    DataTypeDeclarationKind::String(StringDeclaration {
                        type_name: type_def.name,
                        length: Integer {
                            span: SourceSpan::default(),
                            value: len as u128,
                        },
                        width: StringType::String,
                        init: match type_def.default_value {
                            Some(ConstantKind::CharacterString(s)) => {
                                Some(s.value.iter().collect::<String>())
                            }
                            _ => None,
                        },
                    })
                }
                _ => {
                    // Other elementary types create simple declarations
                    DataTypeDeclarationKind::Simple(SimpleDeclaration {
                        type_name: type_def.name,
                        spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: TypeName::from(match elem_type {
                                ElementaryTypeName::BOOL => "BOOL",
                                ElementaryTypeName::INT => "INT",
                                ElementaryTypeName::DINT => "DINT",
                                ElementaryTypeName::REAL => "REAL",
                                ElementaryTypeName::TIME => "TIME",
                                ElementaryTypeName::BYTE => "BYTE",
                                _ => "UNKNOWN", // Handle other elementary types
                            }),
                            initial_value: type_def.default_value,
                        }),
                    })
                }
            }
        }
        DataTypeSpecificationKind::UserDefined(type_name) => {
            // Check if this has an initial value - if so, it's a simple declaration
            // If not, it's a late-bound type alias
            if type_def.default_value.is_some() {
                // Type alias with initial value - create a simple declaration
                DataTypeDeclarationKind::Simple(SimpleDeclaration {
                    type_name: type_def.name,
                    spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: type_name,
                        initial_value: type_def.default_value,
                    }),
                })
            } else {
                // Type alias without initial value - create a late-bound declaration
                DataTypeDeclarationKind::LateBound(LateBoundDeclaration {
                    data_type_name: type_def.name,
                    base_type_name: type_name,
                })
            }
        }
        DataTypeSpecificationKind::Enumeration(enum_spec) => {
            DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                type_name: type_def.name,
                spec_init: EnumeratedSpecificationInit {
                    spec: EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
                        values: enum_spec.values,
                    }),
                    default: match type_def.default_value {
                        Some(ConstantKind::CharacterString(s)) => {
                            // Convert string to enumerated value
                            Some(EnumeratedValue {
                                type_name: None,
                                value: Id::from(&s.value.iter().collect::<String>()),
                            })
                        }
                        _ => None,
                    },
                },
            })
        }
        DataTypeSpecificationKind::Subrange(subrange_spec) => {
            DataTypeDeclarationKind::Subrange(SubrangeDeclaration {
                type_name: type_def.name,
                spec: SubrangeSpecificationKind::Specification(SubrangeSpecification {
                    type_name: subrange_spec.base_type,
                    subrange: Subrange {
                        start: subrange_spec.lower_bound,
                        end: subrange_spec.upper_bound,
                    },
                }),
                default: match type_def.default_value {
                    Some(ConstantKind::IntegerLiteral(int_lit)) => Some(int_lit.value),
                    _ => None,
                },
            })
        }
        DataTypeSpecificationKind::Array(array_spec) => {
            // Convert ArraySpecification to ArraySpecificationKind
            let array_subranges = ArraySubranges {
                ranges: array_spec.bounds.into_iter().map(|bounds| {
                    Subrange {
                        start: bounds.lower,
                        end: bounds.upper,
                    }
                }).collect(),
                type_name: match array_spec.element_type.as_ref() {
                    DataTypeSpecificationKind::Elementary(elem_type) => {
                        TypeName::from(elem_type.as_id().original())
                    }
                    DataTypeSpecificationKind::UserDefined(type_name) => type_name.clone(),
                    _ => TypeName::from("UNKNOWN"), // Fallback for complex types
                },
            };
            
            DataTypeDeclarationKind::Array(ArrayDeclaration {
                type_name: type_def.name,
                spec: ArraySpecificationKind::Subranges(array_subranges),
                init: vec![], // TODO: Handle array initialization if needed
            })
        }
        DataTypeSpecificationKind::String(_string_spec) => {
            // Convert string specification to string declaration
            DataTypeDeclarationKind::String(StringDeclaration {
                type_name: type_def.name,
                length: Integer {
                    span: SourceSpan::default(),
                    value: 80, // Default string length
                },
                width: StringType::String, // Default to regular string
                init: match type_def.default_value {
                    Some(ConstantKind::CharacterString(s)) => {
                        Some(s.value.iter().collect::<String>())
                    }
                    _ => None,
                },
            })
        }
    }
}

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    // Walk to build a graph of types, POUs and their relationships
    let mut data_type_visitor = RuleGraphReferenceableElements::new();
    data_type_visitor.walk(&lib).map_err(|e| vec![e])?;

    debug!("Sorted declarations {:?}", data_type_visitor.declarations);

    let sorted_ids = data_type_visitor
        .declarations
        .sorted_ids()
        .map_err(|err| vec![err])?;

    debug!("Sorted identifiers {sorted_ids:?}");

    // Split based on the type so that we put all of the data type declarations
    // at the beginning.
    let mut types_by_name: HashMap<Id, DataTypeDeclarationKind> = HashMap::new();
    let mut elems_by_name: HashMap<Id, LibraryElementKind> = HashMap::new();
    let mut action_blocks: Vec<LibraryElementKind> = Vec::new();
    for element in lib.elements {
        match element {
            LibraryElementKind::DataTypeDeclaration(decl) => {
                match decl {
                    DataTypeDeclarationKind::Enumeration(decl) => {
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Enumeration(decl),
                        );
                    }
                    DataTypeDeclarationKind::Subrange(decl) => {
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Subrange(decl),
                        );
                    }
                    DataTypeDeclarationKind::Simple(decl) => {
                        // Can refer to other declarations, but does not have any declarations itself
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Simple(decl),
                        );
                    }
                    DataTypeDeclarationKind::Array(decl) => {
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Array(decl),
                        );
                    }
                    DataTypeDeclarationKind::Structure(decl) => {
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Structure(decl),
                        );
                    }
                    DataTypeDeclarationKind::StructureInitialization(decl) => {
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::StructureInitialization(decl),
                        );
                    }
                    DataTypeDeclarationKind::String(decl) => {
                        // Can refer to other declarations, but does not have any declarations itself
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::String(decl),
                        );
                    }
                    DataTypeDeclarationKind::Reference(decl) => {
                        // Reference types can refer to other declarations
                        types_by_name.insert(
                            decl.type_name.name.clone(),
                            DataTypeDeclarationKind::Reference(decl),
                        );
                    }
                    DataTypeDeclarationKind::LateBound(decl) => {
                        types_by_name.insert(
                            decl.data_type_name.name.clone(),
                            DataTypeDeclarationKind::LateBound(decl),
                        );
                    }
                }
            }
            LibraryElementKind::FunctionDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.clone(),
                    LibraryElementKind::FunctionDeclaration(decl),
                );
            }
            LibraryElementKind::FunctionBlockDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.name.clone(),
                    LibraryElementKind::FunctionBlockDeclaration(decl),
                );
            }
            LibraryElementKind::ProgramDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.clone(),
                    LibraryElementKind::ProgramDeclaration(decl),
                );
            }
            LibraryElementKind::ConfigurationDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.clone(),
                    LibraryElementKind::ConfigurationDeclaration(decl),
                );
            }
            LibraryElementKind::ClassDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.name.clone(),
                    LibraryElementKind::ClassDeclaration(decl),
                );
            }
            LibraryElementKind::ActionBlockDeclaration(decl) => {
                // Action blocks don't have names that can be referenced by other elements,
                // so they don't participate in topological sorting. We'll add them back at the end.
                action_blocks.push(LibraryElementKind::ActionBlockDeclaration(decl));
            }
            LibraryElementKind::GlobalVariableDeclaration(decl) => {
                // Global variable declarations don't have names that can be referenced by other elements,
                // so they don't participate in topological sorting. We'll add them back at the end.
                action_blocks.push(LibraryElementKind::GlobalVariableDeclaration(decl));
            }
            LibraryElementKind::TypeDefinitionBlock(block) => {
                // Extract individual type definitions from the block and add them to types_by_name
                for type_def in block.definitions {
                    // Convert TypeDefinition to DataTypeDeclarationKind for sorting
                    let data_type_decl = convert_type_definition_to_data_type_declaration(type_def);
                    match data_type_decl {
                        DataTypeDeclarationKind::Simple(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::Simple(decl),
                            );
                        }
                        DataTypeDeclarationKind::String(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::String(decl),
                            );
                        }
                        DataTypeDeclarationKind::Enumeration(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::Enumeration(decl),
                            );
                        }
                        DataTypeDeclarationKind::Subrange(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::Subrange(decl),
                            );
                        }
                        DataTypeDeclarationKind::Array(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::Array(decl),
                            );
                        }
                        DataTypeDeclarationKind::Structure(decl) => {
                            types_by_name.insert(
                                decl.type_name.name.clone(),
                                DataTypeDeclarationKind::Structure(decl),
                            );
                        }
                        DataTypeDeclarationKind::LateBound(decl) => {
                            types_by_name.insert(
                                decl.data_type_name.name.clone(),
                                DataTypeDeclarationKind::LateBound(decl),
                            );
                        }
                        _ => {
                            // Handle other types as needed
                        }
                    }
                }
            }
        }
    }

    // Merge things back together
    let mut elements = Vec::new();
    elements.extend(sorted_ids.iter().filter_map(|id| {
        types_by_name
            .remove(id)
            .map(LibraryElementKind::DataTypeDeclaration)
    }));
    elements.extend(sorted_ids.iter().filter_map(|id| elems_by_name.remove(id)));
    
    // Add action blocks at the end since they don't participate in topological sorting
    elements.extend(action_blocks);

    Ok(Library { elements })
}

struct DeclarationsGraph {
    // Represents the types and POUs in the library as a directed graph.
    // Each node is a single type or POU.
    graph: StableDiGraph<Id, (), u32>,

    // Maps between the identifier for some element and the index
    // of tht item in the graph.
    id_to_index: HashMap<Id, NodeIndex>,
    index_to_id: HashMap<NodeIndex, Id>,
}

impl DeclarationsGraph {
    fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            id_to_index: HashMap::new(),
            index_to_id: HashMap::new(),
        }
    }

    fn add_node(&mut self, id: &Id) -> NodeIndex<u32> {
        let index = match self.id_to_index.get(id) {
            Some(existing_index) => *existing_index,
            None => {
                let new_index = self.graph.add_node(id.clone());
                self.id_to_index.insert(id.clone(), new_index);
                new_index
            }
        };

        match self.index_to_id.get(&index) {
            Some(_id) => {
                // Already exists
            }
            None => {
                self.index_to_id.insert(index, id.clone());
            }
        }

        index
    }

    fn sorted_ids(&self) -> Result<Vec<Id>, Diagnostic> {
        let sorted_nodes = toposort(&self.graph, None).map_err(|err| {
            let id_in_cycle = self.index_to_id.get(&err.node_id());

            let span = match id_in_cycle {
                Some(id) => id.span.clone(),
                None => SourceSpan::range(0, 0).with_file_id(&FileId::default()),
            };

            Diagnostic::problem(
                Problem::RecursiveCycle,
                // TODO wrong location
                Label::span(span, "Cycle"),
            )
        })?;
        let sorted_ids: Vec<Id> = sorted_nodes
            .iter()
            .map(|node| self.index_to_id.get(node).unwrap().clone())
            .collect();
        Ok(sorted_ids)
    }
}

impl fmt::Debug for DeclarationsGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dotfile = Dot::with_config(&self.graph, &[Config::EdgeNoLabel]);
        write!(f, "Graph: {dotfile:?}")
    }
}

struct RuleGraphReferenceableElements {
    declarations: DeclarationsGraph,
    // Represents the context while visiting. Tracks the name of the current
    // POU.
    current_from: Option<Id>,
}
impl RuleGraphReferenceableElements {
    fn new() -> Self {
        Self {
            declarations: DeclarationsGraph::new(),
            current_from: None,
        }
    }
}

impl Visitor<Diagnostic> for RuleGraphReferenceableElements {
    type Value = ();

    // Type declarations

    fn visit_late_bound_declaration(
        &mut self,
        node: &LateBoundDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let this = self.declarations.add_node(&node.data_type_name.name);
        let depends_on = self.declarations.add_node(&node.base_type_name.name);
        self.declarations.graph.add_edge(depends_on, this, ());

        node.recurse_visit(self)
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let this = self.declarations.add_node(&node.type_name.name);

        if let EnumeratedSpecificationKind::TypeName(parent) = &node.spec_init.spec {
            let depends_on = self.declarations.add_node(&parent.name);
            self.declarations.graph.add_edge(depends_on, this, ());
        };

        node.recurse_visit(self)
    }

    fn visit_subrange_declaration(
        &mut self,
        node: &SubrangeDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let this = self.declarations.add_node(&node.type_name.name);

        if let SubrangeSpecificationKind::Type(parent) = &node.spec {
            let depends_on = self.declarations.add_node(&parent.name);
            self.declarations.graph.add_edge(depends_on, this, ());
        };

        node.recurse_visit(self)
    }

    fn visit_array_declaration(
        &mut self,
        node: &ArrayDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let this = self.declarations.add_node(&node.type_name.name);

        match &node.spec {
            ArraySpecificationKind::Type(parent) => {
                let depends_on = self.declarations.add_node(&parent.name);
                self.declarations.graph.add_edge(depends_on, this, ());
            }
            ArraySpecificationKind::Subranges(array_subranges) => {
                let depends_on = self.declarations.add_node(&array_subranges.type_name.name);
                self.declarations.graph.add_edge(depends_on, this, ());
            }
        }

        node.recurse_visit(self)
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.type_name.name.clone());
        self.declarations.add_node(&node.type_name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_structure_initialization_declaration(
        &mut self,
        node: &StructureInitializationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.type_name.name.clone());
        self.declarations.add_node(&node.type_name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_simple_declaration(
        &mut self,
        node: &SimpleDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.type_name.name.clone());
        self.declarations.add_node(&node.type_name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_reference_declaration(
        &mut self,
        node: &ReferenceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let this = self.declarations.add_node(&node.type_name.name);
        let depends_on = self.declarations.add_node(&node.referenced_type.name);
        self.declarations.graph.add_edge(depends_on, this, ());
        Ok(())
    }

    fn visit_string_declaration(
        &mut self,
        node: &StringDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.type_name.name.clone());
        self.declarations.add_node(&node.type_name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_type_definition_block(
        &mut self,
        node: &TypeDefinitionBlock,
    ) -> Result<Self::Value, Diagnostic> {
        // Process each type definition in the block
        for type_def in &node.definitions {
            // Add the type definition to the graph
            self.declarations.add_node(&type_def.name.name);
            
            // Add dependencies based on the base type
            match &type_def.base_type {
                DataTypeSpecificationKind::UserDefined(base_type) => {
                    let this = self.declarations.add_node(&type_def.name.name);
                    let depends_on = self.declarations.add_node(&base_type.name);
                    self.declarations.graph.add_edge(depends_on, this, ());
                }
                DataTypeSpecificationKind::Array(array_spec) => {
                    let this = self.declarations.add_node(&type_def.name.name);
                    match array_spec.element_type.as_ref() {
                        DataTypeSpecificationKind::UserDefined(element_type) => {
                            let depends_on = self.declarations.add_node(&element_type.name);
                            self.declarations.graph.add_edge(depends_on, this, ());
                        }
                        _ => {
                            // Elementary types don't need dependencies
                        }
                    }
                }
                _ => {
                    // Elementary types, enumerations, subranges, and strings don't need dependencies
                }
            }
        }
        Ok(())
    }

    // POU declarations

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.declarations.add_node(&node.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.name.clone());
        self.declarations.add_node(&node.name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.declarations.add_node(&node.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_class_declaration(
        &mut self,
        node: &ironplc_dsl::common::ClassDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.name.clone());
        self.declarations.add_node(&node.name.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_configuration_declaration(
        &mut self,
        node: &ironplc_dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.declarations.add_node(&node.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_function_block_initial_value_assignment(
        &mut self,
        init: &FunctionBlockInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        // Current context has a reference to this function block
        match &self.current_from {
            Some(from) => {
                let from = self.declarations.add_node(from);
                let to = self.declarations.add_node(&init.type_name.name);
                self.declarations.graph.add_edge(from, to, ());
            }
            None => return Err(Diagnostic::todo(file!(), line!())),
        }

        Ok(())
    }

    fn visit_global_variable_declaration(
        &mut self,
        _node: &GlobalVariableDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        // Global variable declarations don't participate in topological sorting
        // since they don't have names that can be referenced by other elements.
        // We can safely skip processing their contents for dependency analysis.
        Ok(())
    }

    fn visit_initial_value_assignment_kind(
        &mut self,
        node: &InitialValueAssignmentKind,
    ) -> Result<Self::Value, Diagnostic> {
        match &self.current_from {
            Some(from) => {
                match node {
                    InitialValueAssignmentKind::None(_) => {}
                    InitialValueAssignmentKind::Simple(_) => {}
                    InitialValueAssignmentKind::String(_) => {}
                    InitialValueAssignmentKind::EnumeratedValues(_) => {}
                    InitialValueAssignmentKind::EnumeratedType(_) => {}
                    InitialValueAssignmentKind::FunctionBlock(fb) => {
                        // We only care about these because these may be references to a function block
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&fb.type_name.name);
                        self.declarations.graph.add_edge(from, to, ());
                    }
                    InitialValueAssignmentKind::Subrange(_) => {}
                    InitialValueAssignmentKind::Structure(_) => {}
                    InitialValueAssignmentKind::Array(_) => {}
                    InitialValueAssignmentKind::LateResolvedType(lrt) => {
                        // We only care about these because these may be references to a function block
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&lrt.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                }
            }
            None => {
                // This can happen when processing global variables or other contexts
                // where there's no current declaration context. In these cases,
                // we don't need to track dependencies since global variables
                // don't participate in topological sorting.
                // Just skip processing the initial value assignments.
            }
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{cast, test_helpers::parse_only};

    #[test]
    fn apply_when_function_block_recursive_call_in_self_then_return_error() {
        let program = "
        FUNCTION_BLOCK SelfRecursive
            VAR
               SelfRecursiveInstance : SelfRecursive;
            END_VAR

        END_FUNCTION_BLOCK";

        let library = parse_only(program);
        let result = apply(library);
        assert_eq!(
            result.unwrap_err().first().unwrap().code,
            Problem::RecursiveCycle.code().to_string()
        );
    }

    #[test]
    fn apply_when_function_block_not_recursive_call_in_self_then_return_ok() {
        let program = "
        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR

        END_FUNCTION_BLOCK
        
        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR

        END_FUNCTION_BLOCK";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Callee"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Caller"));
    }

    #[test]
    fn apply_when_nested_enumeration_types() {
        let program = "
TYPE
LEVEL_ALIAS : LEVEL;
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
        assert_eq!(decl.type_name, TypeName::from("LEVEL"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
        assert_eq!(decl.data_type_name, TypeName::from("LEVEL_ALIAS"));
    }

    #[test]
    fn apply_when_nested_string_types() {
        let program = "
TYPE
TYPE_NAME_ALIAS : TYPE_NAME;
TYPE_NAME : STRING(5);
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::String);
        assert_eq!(decl.type_name, TypeName::from("TYPE_NAME"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
        assert_eq!(decl.data_type_name, TypeName::from("TYPE_NAME_ALIAS"));
    }

    #[test]
    fn apply_when_nested_subrange_types() {
        let program = "
TYPE
TYPE_NAME_ALIAS : TYPE_NAME;
TYPE_NAME : INT (1..128);
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Subrange);
        assert_eq!(decl.type_name, TypeName::from("TYPE_NAME"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
        assert_eq!(decl.data_type_name, TypeName::from("TYPE_NAME_ALIAS"));
    }

    #[test]
    fn apply_when_array_of_enum_types() {
        let program = "
TYPE
COLORS_ARRAY : ARRAY[1..2] OF COLOR;
COLOR : (RED, GREEN, BLUE);
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
        assert_eq!(decl.type_name, TypeName::from("COLOR"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Array);
        assert_eq!(decl.type_name, TypeName::from("COLORS_ARRAY"));
    }

    #[test]
    fn apply_when_nested_simple_types() {
        let program = "
TYPE
DEFAULT_2 : DEFAULT_1 := 2;
DEFAULT_1 : INT := 1;
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Simple);
        assert_eq!(decl.type_name, TypeName::from("DEFAULT_1"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Simple);
        assert_eq!(decl.type_name, TypeName::from("DEFAULT_2"));
    }

    #[test]
    fn apply_when_nested_structure_types() {
        let program = "
TYPE

OUTER_STRUCT : STRUCT
   MEMBER : INNER_STRUCT;
END_STRUCT;

INNER_STRUCT: STRUCT
   MEMBER : ENUM_TYPE;
END_STRUCT;

ENUM_TYPE : (A, B, C);

END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
        assert_eq!(decl.type_name, TypeName::from("ENUM_TYPE"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Structure);
        assert_eq!(decl.type_name, TypeName::from("INNER_STRUCT"));

        let decl = library.elements.get(2).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Structure);
        assert_eq!(decl.type_name, TypeName::from("OUTER_STRUCT"));
    }

    #[test]
    fn apply_when_initialized_structure_types() {
        let program = "
TYPE

INIT_STRUCT : MY_STRUCT := (MEMBER := 2);

MY_STRUCT : STRUCT
   MEMBER : INT := 1;
END_STRUCT;

END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Structure);
        assert_eq!(decl.type_name, TypeName::from("MY_STRUCT"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::StructureInitialization);
        assert_eq!(decl.type_name, TypeName::from("INIT_STRUCT"));
    }

    #[test]
    fn apply_when_array_element_is_struct_then_ok() {
        let program = "TYPE subrange_element_type :
  STRUCT
	DAY : SINT;
  END_STRUCT;
END_TYPE

TYPE
  array_container 	: ARRAY [0..29] OF subrange_element_type;
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Structure);
        assert_eq!(decl.type_name, TypeName::from("subrange_element_type"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Array);
        assert_eq!(decl.type_name, TypeName::from("array_container"));
    }

    #[test]
    fn apply_when_array_element_is_struct_needs_reorder_then_ok() {
        let program = "
TYPE
  array_container 	: ARRAY [0..29] OF subrange_element_type;
END_TYPE

TYPE subrange_element_type :
  STRUCT
	DAY : SINT;
  END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let library = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Structure);
        assert_eq!(decl.type_name, TypeName::from("subrange_element_type"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Array);
        assert_eq!(decl.type_name, TypeName::from("array_container"));
    }
}
