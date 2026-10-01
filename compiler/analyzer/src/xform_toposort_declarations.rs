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
    core::{FileId, Id, Located, SourceSpan},
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

    // Compute the set of declarations reachable from the roots: the
    // programs and the global declarations. This allows downstream passes
    // (e.g. codegen) to skip unused functions.
    let reachable = data_type_visitor
        .declarations
        .reachable_from(&data_type_visitor.root_nodes);

    // Split based on the type so that we put all of the data type declarations
    // at the beginning. Every declaration is kept, a repeated name included:
    // the environments built from the sorted library diagnose the repeat and
    // keep the first declaration, so dropping one here would hide it.
    let mut types_by_name: HashMap<Id, Vec<DataTypeDeclarationKind>> = HashMap::new();
    let mut elems_by_name: HashMap<Id, Vec<LibraryElementKind>> = HashMap::new();
    let mut global_var_decls: Vec<Vec<VarDecl>> = Vec::new();
    for element in lib.elements {
        match element {
            LibraryElementKind::DataTypeDeclaration(decl) => {
                types_by_name
                    .entry(data_type_name(&decl))
                    .or_default()
                    .push(decl);
            }
            LibraryElementKind::FunctionDeclaration(decl) => {
                elems_by_name
                    .entry(decl.name.clone())
                    .or_default()
                    .push(LibraryElementKind::FunctionDeclaration(decl));
            }
            LibraryElementKind::FunctionBlockDeclaration(decl) => {
                elems_by_name
                    .entry(decl.name.name.clone())
                    .or_default()
                    .push(LibraryElementKind::FunctionBlockDeclaration(decl));
            }
            LibraryElementKind::ProgramDeclaration(decl) => {
                elems_by_name
                    .entry(decl.name.clone())
                    .or_default()
                    .push(LibraryElementKind::ProgramDeclaration(decl));
            }
            LibraryElementKind::ConfigurationDeclaration(decl) => {
                elems_by_name
                    .entry(decl.name.clone())
                    .or_default()
                    .push(LibraryElementKind::ConfigurationDeclaration(decl));
            }
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                global_var_decls.push(decls);
            }
            LibraryElementKind::InterfaceDeclaration(decl) => {
                elems_by_name
                    .entry(decl.name.clone())
                    .or_default()
                    .push(LibraryElementKind::InterfaceDeclaration(decl));
            }
        }
    }

    // Merge things back together
    let mut elements = Vec::new();
    // Global var declarations go first so they are available for constant resolution
    for decls in global_var_decls {
        elements.push(LibraryElementKind::GlobalVarDeclarations(decls));
    }
    elements.extend(
        sorted_ids
            .iter()
            .filter_map(|id| types_by_name.remove(id))
            .flatten()
            .map(LibraryElementKind::DataTypeDeclaration),
    );
    elements.extend(
        sorted_ids
            .iter()
            .filter_map(|id| elems_by_name.remove(id))
            .flatten(),
    );

    Ok((Library { elements }, reachable))
}

/// The declared name of a data type declaration.
fn data_type_name(decl: &DataTypeDeclarationKind) -> Id {
    match decl {
        DataTypeDeclarationKind::Enumeration(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::Subrange(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::Simple(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::Array(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::Structure(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::StructureInitialization(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::String(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::Reference(d) => d.type_name.name.clone(),
        DataTypeDeclarationKind::LateBound(d) => d.data_type_name.name.clone(),
    }
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
    // Graph node indices of the roots for reachability analysis: the
    // PROGRAM declarations, and what the global declarations instantiate --
    // each CONFIGURATION, and the type of each top-level VAR_GLOBAL.
    root_nodes: Vec<NodeIndex>,
}
impl RuleGraphReferenceableElements {
    fn new() -> Self {
        Self {
            declarations: DeclarationsGraph::new(),
            current_from: None,
            root_nodes: Vec::new(),
        }
    }
}

/// The declared type a reference target depends on: the named type itself,
/// or the element type of an inline array target.
fn reference_target_type_name(target: &ReferenceTarget) -> Id {
    match target {
        ReferenceTarget::Named(type_name) => type_name.name.clone(),
        ReferenceTarget::Array(subranges) => subranges.type_name.to_type_name().name,
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
            // participate in the ordering. They are unconditionally placed
            // first in the output so that their constants are available for
            // subsequent passes. Skip recursion to avoid hitting visitor
            // methods that require current_from context. The type of each is
            // a root: a function block instance declared here exists, called
            // or not, so its type is compiled.
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                for decl in decls {
                    if let TypeReference::Named(type_name) = decl.initializer.type_reference() {
                        let idx = self.declarations.add_node(&type_name.name);
                        self.root_nodes.push(idx);
                    }
                }
                Ok(())
            }
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

    fn visit_reference_declaration(
        &mut self,
        node: &ReferenceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        // `REF_TO T` depends on `T`, and `REF_TO ARRAY [..] OF T` on the
        // element type, exactly as `visit_array_declaration` does. Without
        // this edge a `REF_TO` to a type declared in the same source may be
        // resolved before its target exists and fail with a spurious P2011.
        let this = self.declarations.add_node(&node.type_name.name);
        let depends_on = self
            .declarations
            .add_node(&reference_target_type_name(&node.target));
        self.declarations.graph.add_edge(depends_on, this, ());

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
        self.root_nodes.push(idx);
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
        // A root, as a program is: the instances its VAR_GLOBAL declares
        // exist whether or not a program calls them.
        self.current_from = Some(node.name.clone());
        let idx = self.declarations.add_node(&node.name);
        self.root_nodes.push(idx);
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
            None => {
                return Err(Diagnostic::not_implemented(Label::span(
                    node.name.span(),
                    "Function call outside a program organization unit",
                )))
            }
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
            None => {
                return Err(Diagnostic::not_implemented(Label::span(
                    init.type_name.span(),
                    "Function block instance outside a program organization unit",
                )))
            }
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
                    InitialValueAssignmentKind::Simple(simple) => {
                        // A VAR_GLOBAL or VAR_EXTERNAL of a function block
                        // type is `Simple` until type resolution, so the
                        // named type may be a function block the declaring
                        // POU needs. An elementary type name adds a node
                        // that orders and reaches nothing.
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&simple.type_name.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
                    InitialValueAssignmentKind::String(_) => {}
                    InitialValueAssignmentKind::EnumeratedValues(_) => {}
                    InitialValueAssignmentKind::EnumeratedType(enum_init) => {
                        // An enum-typed field or variable depends on its
                        // enumeration type, exactly as the LateResolvedType
                        // arm below records for the uninitialized form
                        // `c : Color;`. The parser produces this arm directly
                        // for a qualified initializer (`c : Color := Color#GREEN`)
                        // and for located declarations, so without this edge
                        // the enumeration may be ordered after the declaration
                        // that references it and is then missing from the type
                        // environment, surfacing as a spurious P2021/P2004.
                        let from = self.declarations.add_node(from);
                        let to = self.declarations.add_node(&enum_init.type_name.name);
                        self.declarations.graph.add_edge(to, from, ());
                    }
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
                    InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                        type_name: lrt,
                        ..
                    }) => {
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
mod tests;
