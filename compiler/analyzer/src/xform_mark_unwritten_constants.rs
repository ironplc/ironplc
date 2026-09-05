//! Transform that marks variables the program never writes as `CONSTANT`.
//!
//! A variable with an initializer and no write anywhere in the library holds
//! its initial value for the whole run. Making that explicit lets every later
//! stage -- the semantic rules and, above all, code generation -- treat
//! "declared `CONSTANT`" and "never written" as one thing, so a constant fold
//! (such as `LEN` of a string that never changes) has a single case to
//! recognize.
//!
//! The transform is conservative: it may leave a variable unmarked that is in
//! fact never written, but it never marks one that is. Writes are tracked by
//! **name** across the whole library, not by scope -- a write to `x` in any
//! program organization unit blocks every `x`. That is sound by construction
//! and makes inheritance, methods and `VAR_EXTERNAL` aliasing fall out for
//! free; the price is precision when two units reuse a name.
//!
//! See `specs/design/constant-variable-inference.md`.
//!
//! ## Before
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     greeting : STRING := 'Hello';
//!     count : INT := 0;
//! END_VAR
//!     count := LEN(greeting);
//! END_PROGRAM
//! ```
//!
//! ## After
//!
//! ```ignore
//! PROGRAM main
//! VAR CONSTANT
//!     greeting : STRING := 'Hello';
//! END_VAR
//! VAR
//!     count : INT := 0;
//! END_VAR
//!     count := LEN(greeting);
//! END_PROGRAM
//! ```
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::configuration::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::sfc::ActionAssociation;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;

use crate::call_assignment_check::bind_inputs;
use crate::callee_resolution::{FunctionBlocks, InstanceTypes};
use crate::function_environment::FunctionEnvironment;
use crate::type_environment::TypeEnvironment;

/// Marks every never-written variable in `lib` as `CONSTANT`.
///
/// Infallible: the transform only ever adds a qualifier, and it adds one only
/// where the semantic rules for `CONSTANT` declarations are known to pass.
pub fn apply(
    lib: Library,
    type_environment: &TypeEnvironment,
    function_environment: &FunctionEnvironment,
) -> Library {
    let function_blocks = FunctionBlocks::from_library(&lib);
    let mut collector = WriteCollector {
        function_blocks: &function_blocks,
        type_environment,
        function_environment,
        instances: InstanceTypes::default(),
        written: HashSet::new(),
        globals: HashMap::new(),
        externals: HashSet::new(),
    };
    let Ok(()) = collector.walk(&lib);
    let mut marker = Marker::new(&collector);
    let Ok(lib) = marker.fold_library(lib);
    lib
}

/// Returns whether `init` is an initializer that `rule_var_decl_const_initialized`
/// accepts on a `CONSTANT` declaration without any further check.
///
/// Exhaustive on purpose: a new initializer kind must decide here whether a
/// never-written variable of that kind may be marked.
fn has_constant_initializer(init: &InitialValueAssignmentKind) -> bool {
    match init {
        InitialValueAssignmentKind::Simple(si) => si.initial_value.is_some(),
        InitialValueAssignmentKind::String(si) => si.initial_value.is_some(),
        InitialValueAssignmentKind::EnumeratedValues(ev) => ev.initial_value.is_some(),
        InitialValueAssignmentKind::EnumeratedType(et) => et.initial_value.is_some(),
        InitialValueAssignmentKind::Array(arr) => !arr.initial_values.is_empty(),
        // A structure `CONSTANT` must initialize every field without a type
        // default, which a never-written variable need not do. A function
        // block cannot be `CONSTANT` (P4010). The remaining kinds either
        // carry no value or should have been resolved away by now.
        InitialValueAssignmentKind::None(_)
        | InitialValueAssignmentKind::FunctionBlock(_)
        | InitialValueAssignmentKind::FunctionBlockCall(_)
        | InitialValueAssignmentKind::Subrange(_)
        | InitialValueAssignmentKind::Structure(_)
        | InitialValueAssignmentKind::Reference(_)
        | InitialValueAssignmentKind::LateResolvedType(_)
        | InitialValueAssignmentKind::SimpleExpr(_) => false,
    }
}

/// The variable a call argument passes, when it passes one.
fn argument_variable(param: &ParamAssignmentKind) -> Option<&Variable> {
    let expr = match param {
        ParamAssignmentKind::PositionalInput(input) => &input.expr,
        ParamAssignmentKind::NamedInput(input) => &input.expr,
        ParamAssignmentKind::Output(_) => return None,
    };
    match &expr.kind {
        ExprKind::Variable(variable) => Some(variable),
        _ => None,
    }
}

/// Gathers the name of every variable the library can write, and the global
/// and external declarations the marking has to keep consistent.
struct WriteCollector<'a> {
    function_blocks: &'a FunctionBlocks<'a>,
    type_environment: &'a TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    /// The function-block instances of the unit being walked.
    instances: InstanceTypes,
    written: HashSet<Id>,
    /// `VAR_GLOBAL` names, with whether every declaration of that name
    /// qualifies to be marked.
    globals: HashMap<Id, bool>,
    /// `VAR_EXTERNAL` names.
    externals: HashSet<Id>,
}

impl WriteCollector<'_> {
    fn mark(&mut self, name: &Id) {
        self.written.insert(name.clone());
    }

    fn mark_variable(&mut self, variable: &Variable) {
        match variable {
            Variable::Direct(_) => {}
            Variable::Symbolic(kind) => self.mark_symbolic(kind),
        }
    }

    /// Marks the root variable of an access chain, and every field of the
    /// chain that may be a function-block member: `inst.count := 5` writes
    /// the block's `count`, not only the instance `inst`.
    fn mark_symbolic(&mut self, kind: &SymbolicVariableKind) {
        match kind {
            SymbolicVariableKind::Named(named) => self.mark(&named.name),
            SymbolicVariableKind::Array(array) => self.mark_symbolic(&array.subscripted_variable),
            SymbolicVariableKind::BitAccess(bit) => self.mark_symbolic(&bit.variable),
            SymbolicVariableKind::PartialAccess(partial) => self.mark_symbolic(&partial.variable),
            SymbolicVariableKind::Deref(deref) => self.mark_symbolic(&deref.variable),
            SymbolicVariableKind::Structured(structured) => {
                if self.may_be_fb_member(&structured.record) {
                    self.mark(&structured.field);
                }
                self.mark_symbolic(&structured.record);
            }
            SymbolicVariableKind::SelfRef(_) => {}
        }
    }

    /// Whether a field accessed on `record` may belong to a function block.
    /// Only a plain variable that is not an instance is ruled out; any other
    /// record (an array element, a nested field, `THIS^`) is assumed to be
    /// one.
    fn may_be_fb_member(&self, record: &SymbolicVariableKind) -> bool {
        match record {
            SymbolicVariableKind::Named(named) => self.instances.type_of(&named.name).is_some(),
            _ => true,
        }
    }

    /// Whether `type_name` names a function block, declared in the library
    /// or supplied by the standard library.
    fn is_function_block(&self, type_name: &TypeName) -> bool {
        self.function_blocks.contains(type_name)
            || self
                .type_environment
                .get(type_name)
                .is_some_and(|attrs| attrs.representation.is_function_block())
    }

    /// Marks every variable argument that a `VAR_IN_OUT` parameter of
    /// `owner` writes. An argument that binds to no parameter is taken to be
    /// written: with no declaration to consult, nothing rules a write out.
    fn mark_bound_arguments(&mut self, owner: &dyn HasVariables, params: &[ParamAssignmentKind]) {
        for (param, declared) in bind_inputs(owner, params) {
            let in_out = declared.is_none_or(|decl| decl.var_type == VariableType::InOut);
            if in_out {
                if let Some(variable) = argument_variable(param) {
                    self.mark_variable(variable);
                }
            }
        }
    }

    /// Marks every variable argument of a call whose callee cannot be
    /// resolved: any of them may be written.
    fn mark_all_arguments(&mut self, params: &[ParamAssignmentKind]) {
        for param in params {
            if let Some(variable) = argument_variable(param) {
                self.mark_variable(variable);
            }
        }
    }

    /// Marks every member name an instance initializer sets, at any depth.
    fn mark_element_inits(&mut self, inits: &[StructureElementInit]) {
        for init in inits {
            self.mark(&init.name);
            if let StructInitialValueAssignmentKind::Structure(nested) = &init.init {
                self.mark_element_inits(nested);
            }
        }
    }

    /// Marks every access path the communication services may write through.
    fn mark_access_path(&mut self, direction: &Option<Direction>, variable: &SymbolicVariableKind) {
        if *direction != Some(Direction::ReadOnly) {
            self.mark_symbolic(variable);
        }
    }

    fn record_global_or_external(&mut self, node: &VarDecl, name: &Id) {
        match node.var_type {
            VariableType::Global => {
                let qualifies = node.qualifier == DeclarationQualifier::Unspecified
                    && matches!(node.identifier, VariableIdentifier::Symbol(_))
                    && has_constant_initializer(&node.initializer);
                self.globals
                    .entry(name.clone())
                    .and_modify(|all| *all &= qualifies)
                    .or_insert(qualifies);
            }
            VariableType::External => {
                self.externals.insert(name.clone());
            }
            VariableType::Var
            | VariableType::VarTemp
            | VariableType::Input
            | VariableType::Output
            | VariableType::InOut
            | VariableType::Access => {}
        }
    }
}

impl Visitor<Infallible> for WriteCollector<'_> {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Infallible> {
        let result = node.recurse_visit(self);
        self.instances.clear();
        result
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Infallible> {
        let result = node.recurse_visit(self);
        self.instances.clear();
        result
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Infallible> {
        let result = node.recurse_visit(self);
        self.instances.clear();
        result
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        let function_blocks = self.function_blocks;
        let type_environment = self.type_environment;
        self.instances.declare(node, &|type_name| {
            function_blocks.contains(type_name)
                || type_environment
                    .get(type_name)
                    .is_some_and(|attrs| attrs.representation.is_function_block())
        });
        if let Some(name) = node.identifier.symbolic_id() {
            self.record_global_or_external(node, name);
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.mark_variable(&node.target);
        node.recurse_visit(self)
    }

    fn visit_for(&mut self, node: &For) -> Result<(), Infallible> {
        self.mark(&node.control);
        node.recurse_visit(self)
    }

    fn visit_output(&mut self, node: &Output) -> Result<(), Infallible> {
        self.mark_variable(&node.tgt);
        node.recurse_visit(self)
    }

    fn visit_expr_kind(&mut self, node: &ExprKind) -> Result<(), Infallible> {
        if let ExprKind::Ref(variable) = node {
            self.mark_variable(variable);
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
        let Some(signature) = self.function_environment.get(&node.name) else {
            self.mark_all_arguments(&node.param_assignment);
            return node.recurse_visit(self);
        };
        // Positional arguments occupy the input-compatible parameters in
        // declaration order, which is the order
        // `xform_named_to_positional_args` laid them out in. A named input
        // still present was never rewritten, so it bound to nothing.
        let mut declared = signature
            .parameters
            .iter()
            .filter(|param| param.is_input_compatible());
        for param in &node.param_assignment {
            let written = match param {
                ParamAssignmentKind::PositionalInput(_) => match declared.next() {
                    Some(declared) => declared.is_inout,
                    // Past the declared parameters an extensible function
                    // takes further inputs; anything else is unbound.
                    None => !signature.is_extensible,
                },
                ParamAssignmentKind::NamedInput(_) => true,
                ParamAssignmentKind::Output(_) => false,
            };
            if written {
                if let Some(variable) = argument_variable(param) {
                    self.mark_variable(variable);
                }
            }
        }
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.mark(&node.var_name);
        let fb_type = self.instances.type_of(&node.var_name).cloned();
        match fb_type {
            Some(fb_type) => match self.function_blocks.get(&fb_type) {
                Some(fb) => self.mark_bound_arguments(fb, &node.params),
                // A standard-library function block declares no VAR_IN_OUT,
                // so its inputs are reads. Anything else is unknown.
                None if self.is_function_block(&fb_type) => {}
                None => self.mark_all_arguments(&node.params),
            },
            None => self.mark_all_arguments(&node.params),
        }
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<(), Infallible> {
        let method = match &node.receiver {
            MethodReceiver::Instance(instance) => {
                self.mark(instance);
                self.instances
                    .type_of(instance)
                    .and_then(|fb_type| self.function_blocks.resolve_method(fb_type, &node.method))
                    .map(|(_, method)| method)
            }
            MethodReceiver::SelfRef(_) => None,
        };
        match method {
            Some(method) => self.mark_bound_arguments(method, &node.params),
            None => self.mark_all_arguments(&node.params),
        }
        node.recurse_visit(self)
    }

    fn visit_function_block_initial_value_assignment(
        &mut self,
        node: &FunctionBlockInitialValueAssignment,
    ) -> Result<(), Infallible> {
        self.mark_element_inits(&node.init);
        node.recurse_visit(self)
    }

    fn visit_structure_initialization_declaration(
        &mut self,
        node: &StructureInitializationDeclaration,
    ) -> Result<(), Infallible> {
        // A member initializer on a function-block instance sets the
        // block's variables; one on a structure sets fields, which are not
        // variables and are left alone.
        if self.is_function_block(&node.type_name) {
            self.mark_element_inits(&node.elements_init);
        }
        node.recurse_visit(self)
    }

    fn visit_function_block_call_initializer(
        &mut self,
        node: &FunctionBlockCallInitializer,
    ) -> Result<(), Infallible> {
        // The arguments reach the block's constructor, whose parameters are
        // not modelled; every named one may set a member of that name.
        for param in &node.params {
            if let ParamAssignmentKind::NamedInput(input) = param {
                self.mark(&input.name);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_function_block_init(&mut self, node: &FunctionBlockInit) -> Result<(), Infallible> {
        self.mark(&node.fb_name);
        self.mark_element_inits(&node.initializer);
        node.recurse_visit(self)
    }

    fn visit_located_var_init(&mut self, node: &LocatedVarInit) -> Result<(), Infallible> {
        for name in &node.fb_path {
            self.mark(name);
        }
        node.recurse_visit(self)
    }

    fn visit_program_connection_source(
        &mut self,
        node: &ProgramConnectionSource,
    ) -> Result<(), Infallible> {
        self.mark_symbolic(&node.dst);
        node.recurse_visit(self)
    }

    fn visit_program_connection_sink(
        &mut self,
        node: &ProgramConnectionSink,
    ) -> Result<(), Infallible> {
        if let ProgramConnectionSinkKind::GlobalVarReference(global) = &node.dst {
            self.mark(&global.global_var_name);
        }
        node.recurse_visit(self)
    }

    fn visit_program_access_decl(&mut self, node: &ProgramAccessDecl) -> Result<(), Infallible> {
        self.mark_access_path(&node.direction, &node.symbolic_variable);
        node.recurse_visit(self)
    }

    fn visit_access_declaration(&mut self, node: &AccessDeclaration) -> Result<(), Infallible> {
        if let AccessPathKind::Symbolic(path) = &node.path {
            self.mark_access_path(&node.direction, &path.variable);
        }
        node.recurse_visit(self)
    }

    fn visit_action_association(&mut self, node: &ActionAssociation) -> Result<(), Infallible> {
        // The action name may be a Boolean variable the step sets, and each
        // indicator is a Boolean variable the action sets.
        self.mark(&node.name);
        for indicator in &node.indicators {
            self.mark(indicator);
        }
        node.recurse_visit(self)
    }
}

/// Applies the verdicts: which declarations become `CONSTANT`.
struct Marker {
    /// Names a `VAR`/`VAR_TEMP` declaration may not be marked under: every
    /// written name, plus every global or external name whose global is not
    /// itself being marked (P4009 collects local constants by name too).
    blocked: HashSet<Id>,
    /// Global names whose `VAR_GLOBAL` and `VAR_EXTERNAL` declarations are
    /// all marked together.
    constant_globals: HashSet<Id>,
}

impl Marker {
    fn new(collected: &WriteCollector<'_>) -> Self {
        let constant_globals: HashSet<Id> = collected
            .globals
            .iter()
            .filter(|(name, qualifies)| **qualifies && !collected.written.contains(*name))
            .map(|(name, _)| name.clone())
            .collect();
        let blocked = collected
            .written
            .iter()
            .chain(collected.globals.keys())
            .chain(collected.externals.iter())
            .filter(|name| !constant_globals.contains(*name))
            .cloned()
            .collect();
        Marker {
            blocked,
            constant_globals,
        }
    }

    fn should_mark(&self, decl: &VarDecl) -> bool {
        if decl.qualifier != DeclarationQualifier::Unspecified {
            return false;
        }
        let VariableIdentifier::Symbol(name) = &decl.identifier else {
            return false;
        };
        match decl.var_type {
            VariableType::Var | VariableType::VarTemp => {
                has_constant_initializer(&decl.initializer) && !self.blocked.contains(name)
            }
            VariableType::Global | VariableType::External => self.constant_globals.contains(name),
            VariableType::Input
            | VariableType::Output
            | VariableType::InOut
            | VariableType::Access => false,
        }
    }
}

impl Fold<Infallible> for Marker {
    fn fold_var_decl(&mut self, mut node: VarDecl) -> Result<VarDecl, Infallible> {
        if self.should_mark(&node) {
            node.qualifier = DeclarationQualifier::Constant;
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests;
