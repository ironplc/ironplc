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
    Direction,
};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn apply(lib: Library) -> Result<(Library, HashSet<Id>), Vec<Diagnostic>> {
    // Walk to build a graph of types, POUs and their relationships
    let mut data_type_visitor = RuleGraphReferenceableElements::new();
    data_type_visitor.walk(&lib).map_err(|e| vec![e])?;

    debug!("Sorted declarations {:?}", data_type_visitor.declarations);

    let sorted_ids = data_type_visitor
        .declarations
        .sorted_ids()
        .map_err(|err| vec![err])?;

    debug!("Sorted identifiers {sorted_ids:?}");

    // Compute the set of declarations reachable from PROGRAM roots.
    // This allows downstream passes (e.g. codegen) to skip unused functions.
    let reachable = data_type_visitor
        .declarations
        .reachable_from(&data_type_visitor.program_nodes);

    // Split based on the type so that we put all of the data type declarations
    // at the beginning.
    let mut types_by_name: HashMap<Id, DataTypeDeclarationKind> = HashMap::new();
    let mut elems_by_name: HashMap<Id, LibraryElementKind> = HashMap::new();
    let mut global_var_decls: Vec<Vec<VarDecl>> = Vec::new();
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
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                global_var_decls.push(decls);
            }
            LibraryElementKind::InterfaceDeclaration(decl) => {
                elems_by_name.insert(
                    decl.name.clone(),
                    LibraryElementKind::InterfaceDeclaration(decl),
                );
            }
        }
    }

    // Merge things back together
    let mut elements = Vec::new();
    // Global var declarations go first so they are available for constant resolution
    for decls in global_var_decls {
        elements.push(LibraryElementKind::GlobalVarDeclarations(decls));
    }
    elements.extend(sorted_ids.iter().filter_map(|id| {
        types_by_name
            .remove(id)
            .map(LibraryElementKind::DataTypeDeclaration)
    }));
    elements.extend(sorted_ids.iter().filter_map(|id| elems_by_name.remove(id)));

    Ok((Library { elements }, reachable))
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

    /// Computes the set of `Id`s reachable from the given root nodes by
    /// following edges in the *incoming* direction (callee -> caller edges
    /// mean incoming neighbors of a caller are its callees).
    fn reachable_from(&self, roots: &[NodeIndex]) -> HashSet<Id> {
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        let mut queue: VecDeque<NodeIndex> = VecDeque::new();

        for &root in roots {
            queue.push_back(root);
        }

        while let Some(node) = queue.pop_front() {
            if !visited.insert(node) {
                continue;
            }
            for neighbor in self.graph.neighbors_directed(node, Direction::Incoming) {
                queue.push_back(neighbor);
            }
        }

        visited
            .into_iter()
            .filter_map(|idx| self.index_to_id.get(&idx).cloned())
            .collect()
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
    // Graph node indices for PROGRAM declarations, used as roots for
    // reachability analysis.
    program_nodes: Vec<NodeIndex>,
}
impl RuleGraphReferenceableElements {
    fn new() -> Self {
        Self {
            declarations: DeclarationsGraph::new(),
            current_from: None,
            program_nodes: Vec::new(),
        }
    }
}

impl Visitor<Diagnostic> for RuleGraphReferenceableElements {
    type Value = ();

    fn visit_library_element_kind(
        &mut self,
        node: &LibraryElementKind,
    ) -> Result<Self::Value, Diagnostic> {
        match node {
            // Global variable declarations are not POUs or types and don't
            // participate in the dependency graph. They are unconditionally
            // placed first in the output so that their constants are available
            // for subsequent passes. Skip recursion to avoid hitting visitor
            // methods that require current_from context.
            LibraryElementKind::GlobalVarDeclarations(_) => Ok(()),
            _ => node.recurse_visit(self),
        }
    }

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

        if let SpecificationKind::Named(parent) = &node.spec_init.spec {
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

        if let SpecificationKind::Named(parent) = &node.spec {
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
            SpecificationKind::Named(parent) => {
                let depends_on = self.declarations.add_node(&parent.name);
                self.declarations.graph.add_edge(depends_on, this, ());
            }
            SpecificationKind::Inline(array_subranges) => {
                let depends_on = self
                    .declarations
                    .add_node(&array_subranges.type_name.to_type_name().name);
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
        // Save and restore current_from because this visitor can be called
        // both as a top-level type declaration and nested within a program's
        // VarDecl initializer (e.g., `s : MyStruct := (a := 10, b := 20)`).
        // Unconditionally resetting to None would wipe the enclosing
        // program's context when visited as a nested node.
        let prev = self.current_from.take();
        self.current_from = Some(node.type_name.name.clone());
        self.declarations.add_node(&node.type_name.name);
        let res = node.recurse_visit(self);
        self.current_from = prev;
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
        let this = self.declarations.add_node(&node.name.name);
        if let Some(parent) = node.oop.as_ref().and_then(|oop| oop.base.as_ref()) {
            let depends_on = self.declarations.add_node(&parent.name);
            self.declarations.graph.add_edge(depends_on, this, ());
        }
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        let idx = self.declarations.add_node(&node.name);
        self.program_nodes.push(idx);
        let res = node.recurse_visit(self);
        self.current_from = None;
        res
    }

    fn visit_interface_declaration(
        &mut self,
        node: &InterfaceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.current_from = Some(node.name.clone());
        let this = self.declarations.add_node(&node.name);
        for parent in &node.extends {
            let depends_on = self.declarations.add_node(&parent.name);
            self.declarations.graph.add_edge(depends_on, this, ());
        }
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

    fn visit_function(
        &mut self,
        node: &ironplc_dsl::textual::Function,
    ) -> Result<Self::Value, Diagnostic> {
        // A function call creates a dependency: the current POU depends on the
        // called function. Add an edge so the called function is ordered first.
        match &self.current_from {
            Some(from) => {
                let from = self.declarations.add_node(from);
                let to = self.declarations.add_node(&node.name);
                self.declarations.graph.add_edge(to, from, ());
            }
            None => return Err(Diagnostic::todo()),
        }

        node.recurse_visit(self)
    }

    fn visit_function_block_initial_value_assignment(
        &mut self,
        init: &FunctionBlockInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        // Current context has a reference to this function block. The
        // referenced type must be ordered before the containing POU (same
        // convention as the Structure/LateResolvedType arms in
        // visit_initial_value_assignment_kind below), so the edge points
        // from the referenced type to the containing POU, not the reverse.
        match &self.current_from {
            Some(from) => {
                let from = self.declarations.add_node(from);
                let to = self.declarations.add_node(&init.type_name.name);
                self.declarations.graph.add_edge(to, from, ());
            }
            None => return Err(Diagnostic::todo()),
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
                        // Same ordering convention as the Structure/LateResolvedType
                        // arms below: the referenced type must come before the
                        // containing POU.
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&fb.type_name.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::FunctionBlockCall(fbc) => {
                        // The call-style FB instance initializer references
                        // an FB type just like the FunctionBlock arm above,
                        // so it needs the same referenced-type-before-POU
                        // dependency edge -- otherwise a forward reference
                        // (a POU instantiating a later-declared FB) surfaces
                        // as a spurious P2011.
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&fbc.type_name.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::Subrange(_) => {}
                    InitialValueAssignmentKind::Structure(struct_init) => {
                        // Track dependency on the nested structure type
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&struct_init.type_name.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::Array(array_init) => {
                        // An array-typed field depends on its element type
                        // exactly as `visit_array_declaration` does for a
                        // top-level array type. Without this edge, the
                        // element type may be ordered after the containing
                        // declaration and is then missing from the type
                        // environment, surfacing as a spurious P2013.
                        let element_type_name = match &array_init.spec {
                            SpecificationKind::Named(parent) => parent.name.clone(),
                            SpecificationKind::Inline(subranges) => {
                                subranges.type_name.to_type_name().name
                            }
                        };
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&element_type_name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::Reference(_) => {}
                    InitialValueAssignmentKind::LateResolvedType(lrt) => {
                        // We only care about these because these may be references to a function block
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&lrt.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::SimpleExpr(_) => {
                        // References a variable/constant by name in
                        // expression context, not a type — no declaration
                        // ordering edge needed.
                    }
                }
            }
            None => {
                // Global variable declarations have no current_from context
                // because they are not inside a POU or type declaration.
                // They don't need dependency edges — they are always placed
                // first in the output.
            }
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_helpers::parse_only;
    use ironplc_test::cast;

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
        let (library, _reachable) = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Callee"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Caller"));
    }

    // ---------------------------------------------------------------------
    // FUNCTION_BLOCK EXTENDS dependency edge.
    // ---------------------------------------------------------------------

    fn parse_with_fb_inheritance(program: &str) -> Library {
        use ironplc_parser::{options::CompilerOptions, parse_program};

        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        parse_program(program, &FileId::default(), &options).unwrap()
    }

    #[test]
    fn apply_when_function_block_extends_cycle_then_return_error() {
        let program = "
FUNCTION_BLOCK FB_A EXTENDS FB_B
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
END_FUNCTION_BLOCK";

        let library = parse_with_fb_inheritance(program);
        let result = apply(library);
        assert_eq!(
            result.unwrap_err().first().unwrap().code,
            Problem::RecursiveCycle.code().to_string()
        );
    }

    #[test]
    fn apply_when_function_block_extends_forward_reference_then_base_ordered_first() {
        // The derived FB is declared textually *before* its base -- the
        // new dependency edge must still order the base first.
        let program = "
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK";

        let library = parse_with_fb_inheritance(program);
        let (library, _reachable) = apply(library).unwrap();

        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("FB_Base"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("FB_Derived"));
    }

    #[test]
    fn apply_when_function_block_no_extends_then_return_ok() {
        let program = "
FUNCTION_BLOCK FB_Plain
END_FUNCTION_BLOCK";

        let library = parse_with_fb_inheritance(program);
        let result = apply(library);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_eager_function_block_initializer_forward_reference_then_referenced_type_ordered_first(
    ) {
        // Regression for a dependency-graph edge-direction bug: the
        // FunctionBlock arms (both this dedicated visitor and the inline
        // arm in visit_initial_value_assignment_kind) previously added the
        // edge in the opposite direction to the Structure/LateResolvedType
        // arms, ordering a referenced type *after* its referencing POU and
        // producing a spurious P2011 "Parent type is not declared"
        // downstream.
        //
        // A bare `CalleeInstance : Callee;` declaration parses to
        // LateResolvedType (the correct arm, already covered above), so it
        // does not exercise this. An *eager* InitialValueAssignmentKind::
        // FunctionBlock initializer is what the CODESYS/TwinCAT call-style
        // instance initializer (`name : FB_Type(args);`) constructs at
        // parse time -- but that grammar lands in a separate PR. To keep
        // this regression independent of it, construct the eager
        // FunctionBlock initializer directly on the parsed AST.
        let mut library = parse_only(
            "
        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR
        END_FUNCTION_BLOCK",
        );

        // Rewrite Caller's forward reference to Callee into the eager
        // FunctionBlock form. Caller is declared first, so the referenced
        // type Callee must be reordered before it.
        for element in library.elements.iter_mut() {
            if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
                if fb.name == TypeName::from("Caller") {
                    fb.variables[0].initializer = InitialValueAssignmentKind::FunctionBlock(
                        FunctionBlockInitialValueAssignment {
                            type_name: TypeName::from("Callee"),
                            init: vec![],
                        },
                    );
                }
            }
        }

        let (library, _reachable) = apply(library).unwrap();

        // Callee (the referenced type) must come before Caller.
        let decl = library.elements.first().unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Callee"));

        let decl = library.elements.get(1).unwrap();
        let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
        assert_eq!(decl.name, TypeName::from("Caller"));
    }

    #[test]
    fn apply_when_function_block_call_style_init_then_referenced_type_ordered_first() {
        // The call-style FB instance initializer (`name : FB_Type(args)`)
        // parses to InitialValueAssignmentKind::FunctionBlockCall, a distinct
        // node that must get the same referenced-type-before-POU dependency
        // edge as the FunctionBlock arm. Caller is declared first but
        // references Callee, so Callee must be reordered before it.
        let program = "
        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee(IN1 := TRUE);
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Callee
            VAR_INPUT
               IN1: BOOL;
            END_VAR
        END_FUNCTION_BLOCK";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
TYPE_NAME : STRING[5];
END_TYPE";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
    fn apply_when_function_calls_another_function_then_callee_ordered_first() {
        let program = "
        FUNCTION INNER : REAL
        VAR_INPUT
            X : REAL;
        END_VAR
            INNER := X * 2.0;
        END_FUNCTION

        FUNCTION OUTER : REAL
        VAR_INPUT
            Y : REAL;
        END_VAR
            OUTER := INNER(X := Y);
        END_FUNCTION

        PROGRAM main
        VAR
            result : REAL;
        END_VAR
            result := OUTER(Y := 3.0);
        END_PROGRAM";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

        // INNER must come before OUTER (callee before caller), both before main.
        // Collect just the function declarations in order.
        let func_names: Vec<&Id> = library
            .elements
            .iter()
            .filter_map(|e| {
                if let LibraryElementKind::FunctionDeclaration(f) = e {
                    Some(&f.name)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(func_names.len(), 2);
        assert_eq!(func_names[0], &Id::from("INNER"));
        assert_eq!(func_names[1], &Id::from("OUTER"));
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
        let (library, _reachable) = apply(library).unwrap();

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
        let (library, _reachable) = apply(library).unwrap();

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
    fn apply_when_unused_function_then_not_in_reachable_set() {
        let program = "
        FUNCTION INNER : REAL
        VAR_INPUT X : REAL; END_VAR
            INNER := X * 2.0;
        END_FUNCTION

        FUNCTION UNUSED : REAL
        VAR_INPUT X : REAL; END_VAR
            UNUSED := X;
        END_FUNCTION

        FUNCTION OUTER : REAL
        VAR_INPUT A : REAL; END_VAR
            OUTER := INNER(X := A);
        END_FUNCTION

        PROGRAM main
        VAR result : REAL; END_VAR
            result := OUTER(A := 3.0);
        END_PROGRAM";

        let library = parse_only(program);
        let (_library, reachable) = apply(library).unwrap();

        assert!(reachable.contains(&Id::from("main")));
        assert!(reachable.contains(&Id::from("OUTER")));
        assert!(reachable.contains(&Id::from("INNER")));
        assert!(!reachable.contains(&Id::from("UNUSED")));
    }

    #[test]
    fn apply_when_top_level_var_global_then_return_ok() {
        let program = "
        VAR_GLOBAL CONSTANT
            MY_LENGTH : INT := 250;
        END_VAR

        FUNCTION MY_FUNC : INT
        VAR_INPUT
            x : INT;
        END_VAR
            MY_FUNC := x;
        END_FUNCTION

        PROGRAM main
        VAR
            result : INT;
        END_VAR
            result := MY_FUNC(x := 1);
        END_PROGRAM";

        let library = {
            use ironplc_parser::{options::CompilerOptions, parse_program};
            parse_program(
                program,
                &FileId::default(),
                &CompilerOptions {
                    allow_top_level_var_global: true,
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let (library, _reachable) = apply(library).unwrap();

        // Global var declarations should come first
        let first = library.elements.first().unwrap();
        assert!(matches!(
            first,
            LibraryElementKind::GlobalVarDeclarations(_)
        ));
    }

    // ---------------------------------------------------------------------
    // Array element type dependency edge for array-typed struct fields.
    // ---------------------------------------------------------------------

    /// Returns the position of the named structure declaration in the sorted
    /// library, or `None` when the library does not declare that structure.
    fn structure_position(library: &Library, name: &str) -> Option<usize> {
        library.elements.iter().position(|element| match element {
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(decl)) => {
                decl.type_name == TypeName::from(name)
            }
            _ => false,
        })
    }

    #[test]
    fn apply_when_struct_array_field_element_declared_first_then_element_ordered_first() {
        // Declaration order already matches dependency order. The element type
        // must still be ordered ahead of the struct that arrays over it --
        // without a dependency edge the sort is free to emit either order.
        let program = "
TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

        let item = structure_position(&library, "Item").unwrap();
        let holder = structure_position(&library, "Holder").unwrap();
        assert!(item < holder, "Item must be ordered before Holder");
    }

    #[test]
    fn apply_when_struct_array_field_element_declared_last_then_element_ordered_first() {
        // Forward reference: the element type is declared textually *after*
        // the struct whose array field references it.
        let program = "
TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
END_STRUCT;
END_TYPE

TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

        let item = structure_position(&library, "Item").unwrap();
        let holder = structure_position(&library, "Holder").unwrap();
        assert!(item < holder, "Item must be ordered before Holder");
    }

    #[test]
    fn apply_when_struct_array_field_element_is_elementary_then_return_ok() {
        // Elementary element types have no declaration to order against. The
        // added edge must not make the graph unsortable.
        let program = "
TYPE Holder : STRUCT
    Nums : ARRAY[1..4] OF INT;
    Flags : ARRAY[1..2] OF BOOL;
END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let (library, _reachable) = apply(library).unwrap();

        assert!(structure_position(&library, "Holder").is_some());
    }

    #[test]
    fn apply_when_struct_array_field_is_self_recursive_then_return_error() {
        // An array of the enclosing struct is infinitely sized. The new edge
        // makes this a genuine cycle, which must be reported as such.
        let program = "
TYPE A : STRUCT
    Items : ARRAY[1..2] OF A;
END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let result = apply(library);
        assert_eq!(
            result.unwrap_err().first().unwrap().code,
            Problem::RecursiveCycle.code().to_string()
        );
    }

    #[test]
    fn apply_when_struct_array_fields_are_mutually_recursive_then_return_error() {
        let program = "
TYPE A : STRUCT
    Items : ARRAY[1..2] OF B;
END_STRUCT;
END_TYPE

TYPE B : STRUCT
    Items : ARRAY[1..2] OF A;
END_STRUCT;
END_TYPE";

        let library = parse_only(program);
        let result = apply(library);
        assert_eq!(
            result.unwrap_err().first().unwrap().code,
            Problem::RecursiveCycle.code().to_string()
        );
    }

    #[test]
    fn resolve_types_when_struct_array_field_element_declared_before_program_then_return_ok() {
        // Pipeline-level regression guard for the reported symptom: this
        // layout previously failed with P2013 because the element type was
        // absent from the type environment when the array field was resolved.
        // See https://github.com/ironplc/ironplc/issues/1376.
        use ironplc_parser::options::CompilerOptions;

        let program = "
TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
    Other : BOOL;
END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    H : Holder;
END_VAR
    H.Other := TRUE;
END_PROGRAM";

        let library = parse_only(program);
        let result = crate::stages::resolve_types(&[&library], &CompilerOptions::default());
        assert!(
            result.is_ok(),
            "expected type resolution to succeed, got {:?}",
            result.err()
        );
    }
}
