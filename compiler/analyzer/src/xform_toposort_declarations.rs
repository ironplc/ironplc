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
    stable_graph::{NodeIndex, StableDiGraph},
};
use std::collections::HashMap;

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    // Walk to build a graph of types, POUs and their relationships
    let mut data_type_visitor = RuleGraphReferenceableElements::new();
    data_type_visitor.walk(&lib).map_err(|e| vec![e])?;
    let sorted_ids = data_type_visitor
        .declarations
        .sorted_ids()
        .map_err(|err| vec![err])?;

    debug!("Sorted identifiers {:?}", sorted_ids);

    // Split based on the type so that we put all of the data type declarations
    // at the beginning.
    let mut postfix_types = Vec::new();
    let mut types_by_name: HashMap<Id, DataTypeDeclarationKind> = HashMap::new();
    let mut elems_by_name: HashMap<Id, LibraryElementKind> = HashMap::new();
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
                        postfix_types.push(LibraryElementKind::DataTypeDeclaration(
                            DataTypeDeclarationKind::Simple(decl),
                        ));
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
                        postfix_types.push(LibraryElementKind::DataTypeDeclaration(
                            DataTypeDeclarationKind::String(decl),
                        ));
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
                    decl.name.clone(),
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
        }
    }

    // Merge things back together
    let mut elements = Vec::new();
    elements.extend(sorted_ids.iter().filter_map(|id| {
        types_by_name
            .remove(id)
            .map(LibraryElementKind::DataTypeDeclaration)
    }));
    elements.extend(postfix_types);
    elements.extend(sorted_ids.iter().filter_map(|id| elems_by_name.remove(id)));

    Ok(Library { elements })
}

struct DeclarationsGraph {
    // Represents the types and POUs in the library as a directed graph.
    // Each node is a single type or POU.
    graph: StableDiGraph<(), (), u32>,

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
                let new_index = self.graph.add_node(());
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

        if let ArraySpecificationKind::Type(parent) = &node.spec {
            let depends_on = self.declarations.add_node(&parent.name);
            self.declarations.graph.add_edge(depends_on, this, ());
        };

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
        self.current_from = Some(node.name.clone());
        self.declarations.add_node(&node.name);
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
            None => return Err(Diagnostic::todo(file!(), line!())),
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::parse_only;

    macro_rules! cast {
        ($target: expr, $pat: path) => {{
            if let $pat(a) = $target {
                // #1
                a
            } else {
                panic!("mismatch variant when cast to {}", stringify!($pat)); // #2
            }
        }};
    }

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
            result.unwrap_err().get(0).unwrap().code,
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

        let decl = library.elements.get(0).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, Id::from("Callee"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, Id::from("Caller"));
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

        let decl = library.elements.get(0).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
        assert_eq!(decl.type_name, Type::from("LEVEL"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
        let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
        assert_eq!(decl.data_type_name, Type::from("LEVEL_ALIAS"));
    }
}
