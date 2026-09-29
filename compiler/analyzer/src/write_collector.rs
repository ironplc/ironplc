//! Finds every write in a library and resolves it to the declaration it
//! reaches.
//!
//! What counts as a write, and how a write is resolved, is specified in
//! `specs/design/constant-variable-inference.md`. The collector is
//! conservative in one direction only: a write it cannot resolve to one
//! declaration blocks every declaration of that name.
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::configuration::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::sfc::ActionAssociation;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;

use crate::call_assignment_check::bind_inputs;
use crate::callee_resolution::{FunctionBlocks, InstanceTypes};
use crate::function_environment::FunctionEnvironment;
use crate::symbol_environment::{ScopeKind, ScopePath, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;

/// The writes of a library, and its `VAR_GLOBAL` names with whether every
/// declaration of that name could be marked `CONSTANT`.
pub(crate) struct CollectedWrites {
    pub(crate) written: Writes,
    pub(crate) globals: HashMap<Id, bool>,
}

/// Finds every write in `lib`.
pub(crate) fn collect(
    lib: &Library,
    type_environment: &TypeEnvironment,
    function_environment: &FunctionEnvironment,
    symbol_environment: &SymbolEnvironment,
) -> CollectedWrites {
    let function_blocks = FunctionBlocks::from_library(lib);
    let mut collector = WriteCollector {
        function_blocks: &function_blocks,
        type_environment,
        function_environment,
        symbol_environment,
        scope: Vec::new(),
        instances: InstanceTypes::default(),
        written: Writes::default(),
        kind: WriteKind::Other,
        globals: HashMap::new(),
    };
    let Ok(()) = collector.walk(lib);
    CollectedWrites {
        written: collector.written,
        globals: collector.globals,
    }
}

/// Whether a declaration may be marked as far as its initializer goes: it
/// carries a value, and the value is one `rule_var_decl_const_initialized`
/// accepts on a `CONSTANT` declaration without further checking.
///
/// A structure `CONSTANT` must initialize every field without a type
/// default, which a never-written variable need not do; a reference's
/// constancy says nothing about its target.
pub(crate) fn may_be_constant(init: &InitialValueAssignmentKind) -> bool {
    init.has_initial_value()
        && !matches!(
            init,
            InitialValueAssignmentKind::Structure(_) | InitialValueAssignmentKind::Reference(_)
        )
}

/// The scope of the declarations directly inside the unit `name`.
pub(crate) fn unit_scope(name: &Id) -> ScopeKind {
    ScopeKind::Named(ScopePath::from(name.clone()))
}

/// How a statement or declaration writes a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriteKind {
    /// The target of `v := e`.
    Assignment,
    /// The control variable of a `FOR` loop.
    ForControl,
    /// The target of an output binding `q => v`.
    OutputBinding,
    /// An argument bound to a `VAR_IN_OUT` parameter.
    InOutArgument,
    /// An argument the collector cannot bind to a parameter: an unknown
    /// callee, an unknown name, or more arguments than parameters.
    UnboundArgument,
    /// The operand of `REF` or `ADR`, whose address is taken.
    AddressTaken,
    /// A function-block instance being invoked, or the receiver of a method.
    Invocation,
    /// Anything else: an initializer, an access path, an action association.
    Other,
}

/// One write the collector resolved to exactly one declaration.
#[derive(Clone, Debug)]
pub(crate) struct WriteSite {
    /// The scope that declares the written variable.
    pub(crate) scope: ScopeKind,
    /// The written name, as it appears at the write; its span is the write.
    pub(crate) name: Id,
    pub(crate) kind: WriteKind,
}

/// The declarations the program writes.
#[derive(Default)]
pub(crate) struct Writes {
    /// Declarations resolved exactly: the scope that declares the variable,
    /// and its name.
    pub(crate) resolved: HashSet<(ScopeKind, Id)>,
    /// Names written through a path that does not resolve to one
    /// declaration; every declaration of the name counts as written.
    pub(crate) any_scope: HashSet<Id>,
    /// Every write in `resolved`, where it is and how it writes.
    pub(crate) sites: Vec<WriteSite>,
}

impl Writes {
    pub(crate) fn contains(&self, scope: &ScopeKind, name: &Id) -> bool {
        self.any_scope.contains(name) || self.resolved.contains(&(scope.clone(), name.clone()))
    }
}

/// Gathers every declaration the library can write, and the global
/// declarations the marking has to keep consistent with their externals.
struct WriteCollector<'a> {
    function_blocks: &'a FunctionBlocks<'a>,
    type_environment: &'a TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    symbol_environment: &'a SymbolEnvironment,
    /// The declarations the walk is inside, outermost first.
    scope: Vec<Id>,
    /// The function-block instances of the unit being walked.
    instances: InstanceTypes,
    written: Writes,
    /// How the write being marked writes.
    kind: WriteKind,
    /// `VAR_GLOBAL` names, with whether every declaration of that name
    /// qualifies to be marked.
    globals: HashMap<Id, bool>,
}

impl WriteCollector<'_> {
    fn current_scope(&self) -> ScopeKind {
        match self.scope.first() {
            None => ScopeKind::Global,
            Some(_) => ScopeKind::Named(ScopePath::new(self.scope.clone())),
        }
    }

    /// The scope whose declaration a bare `name` reaches from the current
    /// scope: the innermost enclosing unit that declares it, then the
    /// `EXTENDS` chain of the enclosing function block, then the globals. A
    /// `VAR_EXTERNAL` declaration stands for its global.
    fn declaring_scope(&self, name: &Id) -> ScopeKind {
        let symbol = self.symbol_environment.find(name, &self.current_scope());
        match symbol {
            Some(symbol) if symbol.scope != ScopeKind::Global => {
                if symbol.is_external {
                    ScopeKind::Global
                } else {
                    symbol.scope.clone()
                }
            }
            _ => self
                .scope
                .first()
                .and_then(|unit| {
                    self.function_blocks
                        .declaring_block(&TypeName { name: unit.clone() }, name)
                })
                .map(|block| unit_scope(&block.name.name))
                .unwrap_or(ScopeKind::Global),
        }
    }

    /// Runs `mark` with every write it records taken to be of `kind`.
    fn marking_as(&mut self, kind: WriteKind, mark: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.kind, kind);
        mark(self);
        self.kind = outer;
    }

    fn mark(&mut self, name: &Id) {
        let scope = self.declaring_scope(name);
        self.mark_resolved(scope, name);
    }

    fn mark_resolved(&mut self, scope: ScopeKind, name: &Id) {
        self.written.sites.push(WriteSite {
            scope: scope.clone(),
            name: name.clone(),
            kind: self.kind,
        });
        self.written.resolved.insert((scope, name.clone()));
    }

    fn mark_any_scope(&mut self, name: &Id) {
        self.written.any_scope.insert(name.clone());
    }

    /// Marks the member `field` of an instance of `fb_type`, on the block in
    /// the `EXTENDS` chain that declares it. A member of a standard-library
    /// block is not a declaration and needs no mark.
    fn mark_member(&mut self, fb_type: &TypeName, field: &Id) {
        if let Some(block) = self.function_blocks.declaring_block(fb_type, field) {
            self.mark_resolved(unit_scope(&block.name.name), field);
        }
    }

    fn mark_variable(&mut self, variable: &Variable) {
        match variable {
            Variable::Direct(_) => {}
            Variable::Symbolic(kind) => self.mark_symbolic(kind),
        }
    }

    /// Marks the root variable of an access chain, and every function-block
    /// member the chain passes through: `inst.count := 5` writes the block's
    /// `count`, not only the instance `inst`.
    fn mark_symbolic(&mut self, kind: &SymbolicVariableKind) {
        match kind {
            SymbolicVariableKind::Named(named) => self.mark(&named.name),
            SymbolicVariableKind::Array(array) => self.mark_symbolic(&array.subscripted_variable),
            SymbolicVariableKind::BitAccess(bit) => self.mark_symbolic(&bit.variable),
            SymbolicVariableKind::PartialAccess(partial) => self.mark_symbolic(&partial.variable),
            SymbolicVariableKind::Deref(deref) => self.mark_symbolic(&deref.variable),
            SymbolicVariableKind::Structured(structured) => {
                self.mark_field(&structured.record, &structured.field);
                self.mark_symbolic(&structured.record);
            }
            SymbolicVariableKind::SelfRef(_) => {}
        }
    }

    /// Marks `field` written on `record`. An instance variable names its
    /// block; `THIS^` is the enclosing block; a structure variable has no
    /// members that are declarations. Any other record -- an array element,
    /// a nested field -- is not resolved, so every `field` is blocked.
    fn mark_field(&mut self, record: &SymbolicVariableKind, field: &Id) {
        match record {
            SymbolicVariableKind::Named(named) => {
                if let Some(fb_type) = self.instances.type_of(&named.name).cloned() {
                    self.mark_member(&fb_type, field);
                }
            }
            SymbolicVariableKind::SelfRef(_) => self.mark(field),
            _ => self.mark_any_scope(field),
        }
    }

    /// Marks every member an instance initializer sets, at any depth.
    fn mark_member_inits(&mut self, fb_type: &TypeName, inits: &[StructureElementInit]) {
        for init in inits {
            self.mark_member(fb_type, &init.name);
            if let StructInitialValueAssignmentKind::Structure(nested) = &init.init {
                let member_type = self
                    .function_blocks
                    .declaring_block(fb_type, &init.name)
                    .and_then(|block| {
                        block
                            .variables
                            .iter()
                            .find(|decl| decl.identifier.symbolic_id() == Some(&init.name))
                    })
                    .and_then(|decl| match &decl.initializer {
                        InitialValueAssignmentKind::FunctionBlock(member) => {
                            Some(member.type_name.clone())
                        }
                        _ => None,
                    });
                match member_type {
                    Some(member_type) => self.mark_member_inits(&member_type, nested),
                    None => {
                        for nested_init in nested {
                            self.mark_any_scope(&nested_init.name);
                        }
                    }
                }
            }
        }
    }

    /// Marks every variable argument that a `VAR_IN_OUT` parameter of
    /// `owner` writes. An argument that binds to no parameter is taken to be
    /// written: with no declaration to consult, nothing rules a write out.
    fn mark_bound_arguments(&mut self, owner: &dyn HasVariables, params: &[ParamAssignmentKind]) {
        for (param, declared) in bind_inputs(owner, params) {
            match declared {
                None => self.marking_as(WriteKind::UnboundArgument, |c| c.mark_argument(param)),
                Some(decl) if decl.var_type == VariableType::InOut => {
                    self.marking_as(WriteKind::InOutArgument, |c| c.mark_argument(param))
                }
                Some(_) => {}
            }
        }
    }

    /// Marks every variable argument of a call whose callee cannot be
    /// resolved: any of them may be written.
    fn mark_all_arguments(&mut self, params: &[ParamAssignmentKind]) {
        for param in params {
            self.marking_as(WriteKind::UnboundArgument, |c| c.mark_argument(param));
        }
    }

    fn mark_argument(&mut self, param: &ParamAssignmentKind) {
        if let Some(ExprKind::Variable(variable)) = param.input_expr().map(|expr| &expr.kind) {
            self.mark_variable(variable);
        }
    }

    /// Marks every access path the communication services may write through.
    fn mark_access_path(&mut self, direction: &Option<Direction>, variable: &SymbolicVariableKind) {
        if *direction != Some(Direction::ReadOnly) {
            self.mark_symbolic(variable);
        }
    }

    fn is_function_block(&self, type_name: &TypeName) -> bool {
        self.type_environment
            .get(type_name)
            .is_some_and(|attrs| attrs.representation.is_function_block())
    }
}

impl Visitor<Infallible> for WriteCollector<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.push(match node {
            ScopeNode::Function(node) => node.name.clone(),
            ScopeNode::FunctionBlock(node) => node.name.name.clone(),
            ScopeNode::Program(node) => node.name.clone(),
            ScopeNode::Method(node) => node.name.clone(),
        });
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.pop();
        // A method's instances are its block's; they go when the block does.
        if self.scope.is_empty() {
            self.instances.clear();
        }
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        self.instances.declare(node);
        if node.var_type == VariableType::Global {
            if let Some(name) = node.identifier.symbolic_id() {
                let qualifies = node.qualifier == DeclarationQualifier::Unspecified
                    && matches!(node.identifier, VariableIdentifier::Symbol(_))
                    && may_be_constant(&node.initializer);
                self.globals
                    .entry(name.clone())
                    .and_modify(|all| *all &= qualifies)
                    .or_insert(qualifies);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.marking_as(WriteKind::Assignment, |c| c.mark_variable(&node.target));
        node.recurse_visit(self)
    }

    fn visit_for(&mut self, node: &For) -> Result<(), Infallible> {
        self.marking_as(WriteKind::ForControl, |c| c.mark(&node.control));
        node.recurse_visit(self)
    }

    fn visit_output(&mut self, node: &Output) -> Result<(), Infallible> {
        self.marking_as(WriteKind::OutputBinding, |c| c.mark_variable(&node.tgt));
        node.recurse_visit(self)
    }

    fn visit_expr_kind(&mut self, node: &ExprKind) -> Result<(), Infallible> {
        if let ExprKind::Ref(variable) = node {
            self.marking_as(WriteKind::AddressTaken, |c| c.mark_variable(variable));
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
                    Some(declared) => declared.is_inout.then_some(WriteKind::InOutArgument),
                    // Past the declared parameters an extensible function
                    // takes further inputs; anything else is unbound.
                    None => (!signature.is_extensible).then_some(WriteKind::UnboundArgument),
                },
                ParamAssignmentKind::NamedInput(_) => Some(WriteKind::UnboundArgument),
                ParamAssignmentKind::Output(_) => None,
            };
            if let Some(kind) = written {
                self.marking_as(kind, |c| c.mark_argument(param));
            }
        }
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.marking_as(WriteKind::Invocation, |c| c.mark(&node.var_name));
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
                self.marking_as(WriteKind::Invocation, |c| c.mark(instance));
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
        self.mark_member_inits(&node.type_name, &node.init);
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
                self.mark_any_scope(&input.name);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_function_block_init(&mut self, node: &FunctionBlockInit) -> Result<(), Infallible> {
        // A configuration path names an instance and its members from the
        // outside; the path is not resolved, so every name on it is blocked.
        self.mark_any_scope(&node.fb_name);
        for init in &node.initializer {
            self.mark_any_scope(&init.name);
        }
        node.recurse_visit(self)
    }

    fn visit_located_var_init(&mut self, node: &LocatedVarInit) -> Result<(), Infallible> {
        for name in &node.fb_path {
            self.mark_any_scope(name);
        }
        node.recurse_visit(self)
    }

    fn visit_program_connection_source(
        &mut self,
        node: &ProgramConnectionSource,
    ) -> Result<(), Infallible> {
        // The program instance's input, named from outside the program.
        if let SymbolicVariableKind::Named(named) = &node.dst {
            self.mark_any_scope(&named.name);
        }
        node.recurse_visit(self)
    }

    fn visit_program_connection_sink(
        &mut self,
        node: &ProgramConnectionSink,
    ) -> Result<(), Infallible> {
        if let ProgramConnectionSinkKind::GlobalVarReference(global) = &node.dst {
            self.mark_resolved(ScopeKind::Global, &global.global_var_name);
        }
        node.recurse_visit(self)
    }

    fn visit_program_access_decl(&mut self, node: &ProgramAccessDecl) -> Result<(), Infallible> {
        self.mark_access_path(&node.direction, &node.symbolic_variable);
        node.recurse_visit(self)
    }

    fn visit_access_declaration(&mut self, node: &AccessDeclaration) -> Result<(), Infallible> {
        // A configuration-level access path names a variable from outside
        // its unit; the path is not resolved, so every such name is blocked.
        if let AccessPathKind::Symbolic(path) = &node.path {
            if node.direction != Some(Direction::ReadOnly) {
                if let SymbolicVariableKind::Named(named) = &path.variable {
                    self.mark_any_scope(&named.name);
                }
            }
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
