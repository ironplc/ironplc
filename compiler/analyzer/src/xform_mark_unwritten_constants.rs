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

use crate::function_environment::{FunctionEnvironment, FunctionSignature};
use crate::intermediate_type::{FunctionBlockVarType, IntermediateStructField, IntermediateType};
use crate::intermediates::inherited_fields::collect_inherited_fields;
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
    let declarations = Declarations::collect(&lib, type_environment);
    let written = WriteCollector::collect(&lib, &declarations, function_environment);
    let mut marker = Marker::new(&declarations, &written);
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

/// The input-compatible parameters of a callee in declaration order, each
/// flagged with whether it is `VAR_IN_OUT` (and so writes its argument).
#[derive(Clone, Debug)]
struct CalleeParams {
    params: Vec<(Id, bool)>,
    /// An extensible callee accepts further inputs beyond the declared ones
    /// (`ADD(a, b, c)`); those are always plain inputs.
    extensible: bool,
}

impl CalleeParams {
    fn from_var_decls<'a>(decls: impl Iterator<Item = &'a VarDecl>) -> Self {
        let params = decls
            .filter(|d| d.var_type.is_input_compatible())
            .filter_map(|d| {
                d.identifier
                    .symbolic_id()
                    .map(|name| (name.clone(), d.var_type == VariableType::InOut))
            })
            .collect();
        CalleeParams {
            params,
            extensible: false,
        }
    }

    fn from_signature(signature: &FunctionSignature) -> Self {
        let params = signature
            .parameters
            .iter()
            .filter(|p| p.is_input_compatible())
            .map(|p| (p.name.clone(), p.is_inout))
            .collect();
        CalleeParams {
            params,
            extensible: signature.is_extensible,
        }
    }

    fn from_fields(fields: &[IntermediateStructField]) -> Self {
        let params = fields
            .iter()
            .filter_map(|f| match f.var_type {
                Some(FunctionBlockVarType::Input) => Some((f.name.clone(), false)),
                Some(FunctionBlockVarType::InOut) => Some((f.name.clone(), true)),
                Some(FunctionBlockVarType::Output)
                | Some(FunctionBlockVarType::Internal)
                | None => None,
            })
            .collect();
        CalleeParams {
            params,
            extensible: false,
        }
    }

    /// Whether the positional argument at `index` binds to a `VAR_IN_OUT`
    /// parameter. `None` when there is no such parameter.
    fn positional_is_in_out(&self, index: usize) -> Option<bool> {
        self.params
            .get(index)
            .map(|(_, in_out)| *in_out)
            .or(self.extensible.then_some(false))
    }

    /// Whether the argument named `name` binds to a `VAR_IN_OUT` parameter.
    /// `None` when there is no such parameter.
    fn named_is_in_out(&self, name: &Id) -> Option<bool> {
        self.params
            .iter()
            .find(|(param, _)| param == name)
            .map(|(_, in_out)| *in_out)
            .or(self.extensible.then_some(false))
    }
}

/// What the library declares, gathered before writes are collected so that a
/// use can be interpreted whether it comes before or after the declaration.
struct Declarations<'a> {
    type_environment: &'a TypeEnvironment,
    /// Function-block instance name to the types it is declared with.
    /// Name-only, so one name may have several types across units.
    fb_instances: HashMap<Id, Vec<TypeName>>,
    /// Parameters of every function block declared in the library, its
    /// `EXTENDS` ancestors' parameters included.
    fb_params: HashMap<TypeName, CalleeParams>,
    /// Parameters of every method, by method name, across all function
    /// blocks that declare one of that name.
    method_params: HashMap<Id, Vec<CalleeParams>>,
    /// `VAR_GLOBAL` names, with whether every declaration of that name
    /// qualifies to be marked.
    globals: HashMap<Id, bool>,
    /// `VAR_EXTERNAL` names.
    externals: HashSet<Id>,
}

impl<'a> Declarations<'a> {
    fn collect(lib: &Library, type_environment: &'a TypeEnvironment) -> Self {
        let mut declarations = Declarations {
            type_environment,
            fb_instances: HashMap::new(),
            fb_params: HashMap::new(),
            method_params: HashMap::new(),
            globals: HashMap::new(),
            externals: HashSet::new(),
        };
        let inherited = collect_inherited_fields(lib);
        for element in &lib.elements {
            if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
                let ancestors = inherited.get(&fb.name).map(Vec::as_slice).unwrap_or(&[]);
                declarations.fb_params.insert(
                    fb.name.clone(),
                    CalleeParams::from_var_decls(ancestors.iter().chain(fb.variables.iter())),
                );
                for method in &fb.methods {
                    declarations
                        .method_params
                        .entry(method.name.clone())
                        .or_default()
                        .push(CalleeParams::from_var_decls(method.variables.iter()));
                }
            }
        }
        let Ok(()) = declarations.walk(lib);
        declarations
    }

    /// Whether `type_name` names a function block, declared in the library
    /// or supplied by the standard library.
    fn is_function_block(&self, type_name: &TypeName) -> bool {
        self.fb_params.contains_key(type_name)
            || self
                .type_environment
                .get(type_name)
                .is_some_and(|attrs| attrs.representation.is_function_block())
    }

    /// The parameters of the function block `type_name`, or `None` when it
    /// is not a known function block.
    fn callee_params(&self, type_name: &TypeName) -> Option<CalleeParams> {
        if let Some(params) = self.fb_params.get(type_name) {
            return Some(params.clone());
        }
        match &self.type_environment.get(type_name)?.representation {
            IntermediateType::FunctionBlock { fields, .. } => {
                Some(CalleeParams::from_fields(fields))
            }
            _ => None,
        }
    }

    fn qualifies_as_global(decl: &VarDecl) -> bool {
        decl.qualifier == DeclarationQualifier::Unspecified
            && matches!(decl.identifier, VariableIdentifier::Symbol(_))
            && has_constant_initializer(&decl.initializer)
    }
}

impl Visitor<Infallible> for Declarations<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        let Some(name) = node.identifier.symbolic_id() else {
            return Ok(());
        };
        // An instance with a member initializer, `inst : FB := (x := 1)`,
        // keeps the structure-shaped initializer after type resolution, so
        // the type decides whether this is an instance.
        let fb_type = match &node.initializer {
            InitialValueAssignmentKind::FunctionBlock(init) => Some(&init.type_name),
            InitialValueAssignmentKind::FunctionBlockCall(init) => Some(&init.type_name),
            InitialValueAssignmentKind::Structure(init)
                if self.is_function_block(&init.type_name) =>
            {
                Some(&init.type_name)
            }
            _ => None,
        };
        if let Some(fb_type) = fb_type {
            self.fb_instances
                .entry(name.clone())
                .or_default()
                .push(fb_type.clone());
        }
        match node.var_type {
            VariableType::Global => {
                let qualifies = Self::qualifies_as_global(node);
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
        Ok(())
    }
}

/// Gathers the name of every variable the library can write.
struct WriteCollector<'a> {
    declarations: &'a Declarations<'a>,
    function_environment: &'a FunctionEnvironment,
    written: HashSet<Id>,
}

impl<'a> WriteCollector<'a> {
    fn collect(
        lib: &Library,
        declarations: &'a Declarations<'a>,
        function_environment: &'a FunctionEnvironment,
    ) -> HashSet<Id> {
        let mut collector = WriteCollector {
            declarations,
            function_environment,
            written: HashSet::new(),
        };
        let Ok(()) = collector.walk(lib);
        collector.written
    }

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
    /// Only a plain variable that is not a known instance is ruled out; any
    /// other record (an array element, a nested field, `THIS^`) is assumed
    /// to be one.
    fn may_be_fb_member(&self, record: &SymbolicVariableKind) -> bool {
        match record {
            SymbolicVariableKind::Named(named) => {
                self.declarations.fb_instances.contains_key(&named.name)
            }
            _ => true,
        }
    }

    /// The parameters of every type `instance` is declared with, or `None`
    /// when the instance or one of its types is unknown.
    fn fb_params(&self, instance: &Id) -> Option<Vec<CalleeParams>> {
        let types = self.declarations.fb_instances.get(instance)?;
        types
            .iter()
            .map(|type_name| self.declarations.callee_params(type_name))
            .collect()
    }

    /// Marks each variable argument that a `VAR_IN_OUT` parameter of the
    /// callee writes. With no way to tell the parameter's direction --
    /// `candidates` is `None`, or the argument matches no parameter -- the
    /// argument is taken to be written.
    fn mark_arguments(
        &mut self,
        candidates: Option<&[CalleeParams]>,
        args: &[ParamAssignmentKind],
    ) {
        let mut position = 0;
        for arg in args {
            let (expr, in_out) = match arg {
                ParamAssignmentKind::PositionalInput(input) => {
                    let in_out = candidates.map(|c| {
                        c.iter()
                            .any(|params| params.positional_is_in_out(position).unwrap_or(true))
                    });
                    position += 1;
                    (&input.expr, in_out)
                }
                ParamAssignmentKind::NamedInput(input) => {
                    let in_out = candidates.map(|c| {
                        c.iter()
                            .any(|params| params.named_is_in_out(&input.name).unwrap_or(true))
                    });
                    (&input.expr, in_out)
                }
                // Outputs are writes regardless of the callee; `visit_output`
                // records them as the walk reaches them.
                ParamAssignmentKind::Output(_) => continue,
            };
            if in_out.unwrap_or(true) {
                if let ExprKind::Variable(variable) = &expr.kind {
                    self.mark_variable(variable);
                }
            }
        }
    }

    /// Marks every access path the communication services may write through.
    fn mark_access_path(&mut self, direction: &Option<Direction>, variable: &SymbolicVariableKind) {
        if *direction != Some(Direction::ReadOnly) {
            self.mark_symbolic(variable);
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
}

impl Visitor<Infallible> for WriteCollector<'_> {
    type Value = ();

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
        let candidates = self
            .function_environment
            .get(&node.name)
            .map(|signature| vec![CalleeParams::from_signature(signature)]);
        self.mark_arguments(candidates.as_deref(), &node.param_assignment);
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.mark(&node.var_name);
        let candidates = self.fb_params(&node.var_name);
        self.mark_arguments(candidates.as_deref(), &node.params);
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<(), Infallible> {
        if let MethodReceiver::Instance(instance) = &node.receiver {
            self.mark(instance);
        }
        let candidates = self
            .declarations
            .method_params
            .get(&node.method)
            .map(Vec::as_slice);
        self.mark_arguments(candidates, &node.params);
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
        if self.declarations.is_function_block(&node.type_name) {
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
    fn new(declarations: &Declarations, written: &HashSet<Id>) -> Self {
        let constant_globals: HashSet<Id> = declarations
            .globals
            .iter()
            .filter(|(name, qualifies)| **qualifies && !written.contains(*name))
            .map(|(name, _)| name.clone())
            .collect();
        let blocked = written
            .iter()
            .chain(declarations.globals.keys())
            .chain(declarations.externals.iter())
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
