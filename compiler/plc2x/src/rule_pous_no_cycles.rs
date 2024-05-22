//! Semantic rule that checks POU hierarchy rules.
//!
//! Program can call function or function block
//! Function block can call function or other function block
//! Function can call other functions
//!
//! In all cases, recursion is never allowed.
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
    algo::is_cyclic_directed,
    stable_graph::{NodeIndex, StableDiGraph},
};
use std::collections::HashMap;

use crate::result::SemanticResult;

pub fn apply(lib: &Library) -> SemanticResult {
    // Walk to build a graph of POUs and their relationships
    let mut visitor = RulePousNoCycles::new();
    visitor.walk(lib).map_err(|e| vec![e])?;

    // Check if there are cycles in the graph.
    // TODO report what the cycle is
    if is_cyclic_directed(&visitor.graph) {
        let span = SourceSpan::range(0, 0).with_file_id(&FileId::default());
        return Err(vec![Diagnostic::problem(
            Problem::RecursiveCycle,
            // TODO wrong location
            Label::span(&span, "Cycle"),
        )]);
    }

    // TODO Check the relative calls that it obeys rules

    Ok(())
}

struct RulePousNoCycles {
    // Represents the POUs in the library as a directed graph.
    // Each node is a single POU.
    graph: StableDiGraph<(), (), u32>,

    // Represents the context while visiting. Tracks the name of the current
    // POU.
    current_from: Option<Id>,

    nodes: HashMap<Id, NodeIndex>,
}
impl RulePousNoCycles {
    fn new() -> Self {
        RulePousNoCycles {
            graph: StableDiGraph::new(),
            current_from: None,
            nodes: HashMap::new(),
        }
    }

    fn add_node(&mut self, id: &Id) -> NodeIndex<u32> {
        match self.nodes.get(id) {
            Some(node) => *node,
            None => {
                let node = self.graph.add_node(());
                self.nodes.insert(id.clone(), node);
                node
            }
        }
    }
}

impl Visitor<Diagnostic> for RulePousNoCycles {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.type_name.clone());
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
                let to = self.add_node(&init.type_name);
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
        let result = apply(&library);
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
        let result = apply(&library);
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
        let result = apply(&library);
        assert!(result.is_ok());
    }
}
