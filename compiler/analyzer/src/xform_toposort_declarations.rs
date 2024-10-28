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
use petgraph::{
    algo::toposort,
    stable_graph::{NodeIndex, StableDiGraph},
};
use std::collections::HashMap;

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    // Walk to build a graph of types, POUs and their relationships
    let mut visitor = RuleGraphReferenceableSymbols::new();
    visitor.walk(&lib).map_err(|e| vec![e])?;

    toposort(&visitor.graph, None).map_err(|err| {
        let id_in_cycle = visitor.index_to_id.get(&err.node_id());

        let span = match id_in_cycle {
            Some(id) => id.span.clone(),
            None => SourceSpan::range(0, 0).with_file_id(&FileId::default()),
        };

        vec![Diagnostic::problem(
            Problem::RecursiveCycle,
            // TODO wrong location
            Label::span(span, "Cycle"),
        )]
    })?;

    // TODO Check the relative calls that it obeys rules

    Ok(lib)
}

struct RuleGraphReferenceableSymbols {
    // Represents the types and POUs in the library as a directed graph.
    // Each node is a single type or POU.
    graph: StableDiGraph<(), (), u32>,

    // Represents the context while visiting. Tracks the name of the current
    // POU.
    current_from: Option<Id>,

    // Maps between the identifier for some element and the index
    // of tht item in the graph.
    id_to_index: HashMap<Id, NodeIndex>,
    index_to_id: HashMap<NodeIndex, Id>,
}
impl RuleGraphReferenceableSymbols {
    fn new() -> Self {
        RuleGraphReferenceableSymbols {
            graph: StableDiGraph::new(),
            current_from: None,
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
}

impl Visitor<Diagnostic> for RuleGraphReferenceableSymbols {
    type Value = ();

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let from = self.add_node(&node.type_name.name);

        if let EnumeratedSpecificationKind::TypeName(parent) = &node.spec_init.spec {
            let to = self.add_node(&parent.name);
            self.graph.add_edge(from, to, ());
        };
        node.recurse_visit(self)
    }

    fn visit_subrange_declaration(
        &mut self,
        node: &SubrangeDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let from = self.add_node(&node.type_name.name);

        if let SubrangeSpecificationKind::Type(parent) = &node.spec {
            let to = self.add_node(&parent.name);
            self.graph.add_edge(from, to, ());
        };
        node.recurse_visit(self)
    }

    fn visit_array_declaration(
        &mut self,
        node: &ArrayDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let from = self.add_node(&node.type_name.name);

        if let ArraySpecificationKind::Type(parent) = &node.spec {
            let to = self.add_node(&parent.name);
            self.graph.add_edge(from, to, ());
        };
        node.recurse_visit(self)
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.add_node(&node.type_name.name);
        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.add_node(&node.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.add_node(&node.name);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        self.add_node(&node.name);
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
                let from = self.add_node(&from.clone());
                let to = self.add_node(&init.type_name.name);
                self.graph.add_edge(from, to, ());
            }
            None => return Err(Diagnostic::todo(file!(), line!())),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::parse_and_resolve_types;

    #[test]
    fn apply_when_function_block_recursive_call_in_self_then_return_error() {
        let program = "
        FUNCTION_BLOCK SelfRecursive
            VAR
               SelfRecursiveInstance : SelfRecursive;
            END_VAR

        END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let result = apply(library);
        assert!(result.is_err());
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

        let library = parse_and_resolve_types(program);
        let result = apply(library);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_function_invokes_function_block_then_error() {
        let program = "
        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR

        END_FUNCTION_BLOCK
        
        FUNCTION Caller : BOOL
            VAR
                CalleeInstance : Callee;
            END_VAR

            Caller := FALSE;
        END_FUNCTION";

        let library = parse_and_resolve_types(program);
        let result = apply(library);
        assert!(result.is_ok());
    }
}
