//! A whole lowered program: every POU with its body, and `init`, bound to
//! the execution model that says what runs (`specs/design/lowered-program.md`,
//! "Program, POUs and variables").
//!
//! A [`ProgramBuilder`] collects the parts. [`ProgramBuilder::finish`] checks
//! that nothing is missing and hands back the [`Program`] a backend consumes,
//! which cannot be added to.

use std::collections::HashMap;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::ids::{PouId, VarId};
use super::invariant::broken;
use super::program::ProgramBuilder;
use super::standard_block::StandardFunctionBlock;
use super::stmt::Stmt;
use super::types::{Nullability, Section, TypeKind, TypeTable};
use super::variables::{Declaration, Owner, PouKind, PouParts, PouTable, Variable, VariableTable};
use crate::execution::{ExecutionModel, GlobalId, InstanceOf, ProgramId};

/// The variable or POU each entry of the execution model is in the lowered
/// program: the body of each program type, the variable of each declared or
/// compiler-provided global, and the variable that holds each program
/// instance.
///
/// The execution model is kept as it is; this says what its ids become
/// (REQ-LOW-ir-029).
#[derive(Clone, Debug, Default)]
pub struct ExecutionBindings {
    programs: HashMap<ProgramId, PouId>,
    globals: HashMap<GlobalId, VarId>,
    /// By the instance's position in [`ExecutionModel::instances`].
    instances: HashMap<usize, VarId>,
}

impl ExecutionBindings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds the program type `program` to the POU `body`.
    pub fn bind_program(&mut self, program: ProgramId, body: PouId) {
        self.programs.insert(program, body);
    }

    /// Binds the global `global` to the variable `variable`.
    pub fn bind_global(&mut self, global: GlobalId, variable: VarId) {
        self.globals.insert(global, variable);
    }

    /// Binds the program instance at `position` of
    /// [`ExecutionModel::instances`] to the variable that holds it.
    pub fn bind_instance(&mut self, position: usize, variable: VarId) {
        self.instances.insert(position, variable);
    }

    /// The body of the program type `program`.
    pub fn program(&self, program: ProgramId) -> Option<PouId> {
        self.programs.get(&program).copied()
    }

    /// The variable of the global `global`.
    pub fn global(&self, global: GlobalId) -> Option<VarId> {
        self.globals.get(&global).copied()
    }

    /// The variable that holds the program instance at `position` of
    /// [`ExecutionModel::instances`].
    pub fn instance(&self, position: usize) -> Option<VarId> {
        self.instances.get(&position).copied()
    }
}

/// A whole lowered program: what a backend consumes.
///
/// It holds the types, the variables, every POU with its parts and body,
/// `init`, which runs once before the first round, and the execution model
/// bound to the program's POUs and variables. Only
/// [`ProgramBuilder::finish`] makes one, so every POU has a body and every
/// entry of the model is bound.
#[derive(Clone, Debug)]
pub struct Program {
    types: TypeTable,
    variables: VariableTable,
    pous: PouTable,
    init: Vec<Stmt>,
    execution: ExecutionModel,
    bindings: ExecutionBindings,
}

impl Program {
    pub fn types(&self) -> &TypeTable {
        &self.types
    }

    pub fn variables(&self) -> &VariableTable {
        &self.variables
    }

    pub fn pous(&self) -> &PouTable {
        &self.pous
    }

    /// The initial values of every global and program instance, as
    /// statements, run once.
    pub fn init(&self) -> &[Stmt] {
        &self.init
    }

    pub fn execution(&self) -> &ExecutionModel {
        &self.execution
    }

    pub fn bindings(&self) -> &ExecutionBindings {
        &self.bindings
    }

    /// The parts of the POU `pou`; `None` for a POU that stands for a
    /// standard function block.
    pub fn parts(&self, pou: PouId) -> Option<&PouParts> {
        self.pous.parts(pou)
    }

    /// What each declaration of the POU `pou` declares, in the order it
    /// declares them; `None` for a POU that stands for a standard function
    /// block.
    pub fn declarations(&self, pou: PouId) -> Option<&[Declaration]> {
        self.parts(pou).map(|parts| parts.declarations.as_slice())
    }

    /// The variable that holds the result of the function or method `pou`;
    /// `None` for any other POU.
    pub fn result(&self, pou: PouId) -> Option<VarId> {
        self.parts(pou)?.result
    }

    /// The prologue of the POU `pou`: the stores at the head of its body
    /// that give its variables their starting values.
    pub fn prologue(&self, pou: PouId) -> Option<&[Stmt]> {
        self.pous.prologue(pou)
    }

    /// The statements of the body of the POU `pou` after its prologue.
    pub fn statements(&self, pou: PouId) -> Option<&[Stmt]> {
        self.pous.statements(pou)
    }
}

impl ProgramBuilder {
    /// Declares the parts of the POU `pou`.
    ///
    /// A method's kind is `Method`, and no other POU's is. A program's,
    /// function block's or method's instance is a variable the POU owns, of
    /// section `Instance` and a non-null reference to the type whose body the
    /// POU is (for a method, its block's); a function has none. Every local is
    /// a variable the POU owns.
    pub fn declare_parts(
        &mut self,
        pou: PouId,
        parts: PouParts,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_parts";
        let method_of = self.pous.method_of(pou);
        if (parts.kind == PouKind::Method) != method_of.is_some() {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{pou} is a {:?} but method_of is {method_of:?}", parts.kind),
            ));
        }
        let owned = |var: VarId| {
            self.variables
                .get(var)
                .filter(|variable| variable.owner() == Owner::Pou(pou))
        };
        if let Some(local) = parts
            .locals
            .iter()
            .chain(&parts.result)
            .find(|var| owned(**var).is_none())
        {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{local} is not a variable of {pou}"),
            ));
        }
        let expected_body = match parts.kind {
            PouKind::Program | PouKind::FunctionBlock => Some(pou),
            PouKind::Method => method_of,
            PouKind::Function => None,
        };
        let instance_ok = match (parts.instance, expected_body) {
            (None, None) => true,
            (Some(this), Some(body)) => owned(this).is_some_and(|this| {
                this.section() == Section::Instance
                    && self.instance_body(this, parts.kind) == Some(body)
            }),
            (Some(_), None) | (None, Some(_)) => false,
        };
        if !instance_ok {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{pou} does not have the instance its kind needs"),
            ));
        }
        self.pous.declare_parts(pou, parts, span)
    }

    /// The body of the program or function block type a variable `this`, of
    /// a non-null reference type, refers to an instance of; `None` for any
    /// other variable. A program's instance refers to a program type, and any
    /// other body's to a function block type.
    fn instance_body(&self, this: &Variable, kind: PouKind) -> Option<PouId> {
        let (target, nullability) = self.types.reference(this.ty())?;
        if nullability != Nullability::NonNull {
            return None;
        }
        let (body, of_program) = match self.types.kind(target)? {
            TypeKind::Program { body, .. } => (*body, true),
            TypeKind::FunctionBlock { body, .. } => (*body, false),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::Reference { .. } => return None,
        };
        (of_program == (kind == PouKind::Program)).then_some(body)
    }

    /// Sets the body of the POU `pou`, the statements it runs, in order:
    /// its prologue, `prologue`, then its own, `statements`.
    pub fn set_body(
        &mut self,
        pou: PouId,
        prologue: Vec<Stmt>,
        statements: Vec<Stmt>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        self.pous.set_body(pou, prologue, statements, span)
    }

    /// Declares that the POU `pou`, which has no parts or body, stands for
    /// the standard function block `block`, which a backend implements
    /// until lowering generates the standard blocks (issue 2288).
    pub fn declare_standard_block(
        &mut self,
        pou: PouId,
        block: StandardFunctionBlock,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        self.pous.declare_standard_block(pou, block, span)
    }

    /// Adds `stmt` to the end of `init`.
    pub fn push_init(&mut self, stmt: Stmt) {
        self.init.push(stmt);
    }

    /// The whole program, with `execution` bound by `bindings`; a P9998 if
    /// anything is missing (REQ-LOW-ir-081, REQ-LOW-ir-029).
    ///
    /// Every POU has its parts and its body, and a function or method its
    /// signature, but a POU that stands for a standard function block, which
    /// has neither. Every global of the model is bound to a global variable.
    /// Every instance of a program type is bound to a global variable of
    /// section `Instance` whose type is the program type of the POU its
    /// program is bound to. An instance of an unresolved type is bound to
    /// nothing.
    pub fn finish(
        self,
        execution: ExecutionModel,
        bindings: ExecutionBindings,
    ) -> Result<Program, Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::finish";
        for pou in self.pous.ids() {
            let span = self
                .pous
                .name(pou)
                .map(|name| name.span())
                .unwrap_or_default();
            if self.pous.standard_block(pou).is_some() {
                continue;
            }
            let Some(parts) = self.pous.parts(pou) else {
                return Err(broken(CONSTRUCTOR, &span, format!("{pou} has no parts")));
            };
            if self.pous.body(pou).is_none() {
                return Err(broken(CONSTRUCTOR, &span, format!("{pou} has no body")));
            }
            let callable = matches!(parts.kind, PouKind::Function | PouKind::Method);
            if callable && self.pous.function(pou).is_none() {
                return Err(broken(
                    CONSTRUCTOR,
                    &span,
                    format!("{pou} has no signature"),
                ));
            }
        }
        for (id, global) in execution.globals() {
            let bound = bindings
                .global(id)
                .and_then(|var| self.global_variable(var, Section::Global));
            if bound.is_none() {
                return Err(broken(
                    CONSTRUCTOR,
                    &global.name.span(),
                    format!("the global {} is bound to no global variable", global.name),
                ));
            }
        }
        let mut count = 0;
        for (position, instance) in execution.instances().enumerate() {
            count += 1;
            let span = instance
                .name
                .as_ref()
                .map(|name| name.span())
                .unwrap_or_default();
            let bound = bindings.instance(position);
            let ok = match &instance.program {
                InstanceOf::Program(program) => bound.is_some_and(|var| {
                    self.instance_variable_body(var) == bindings.program(*program)
                        && bindings.program(*program).is_some_and(|body| {
                            self.pous.parts(body).map(|parts| parts.kind) == Some(PouKind::Program)
                        })
                }),
                InstanceOf::Unresolved(_) => bound.is_none(),
            };
            if !ok {
                return Err(broken(
                    CONSTRUCTOR,
                    &span,
                    format!("program instance {position} is not bound to its program's instance"),
                ));
            }
        }
        if let Some(position) = bindings
            .instances
            .keys()
            .find(|position| **position >= count)
        {
            return Err(broken(
                CONSTRUCTOR,
                &SourceSpan::default(),
                format!("program instance {position} is not an instance of the model"),
            ));
        }
        Ok(Program {
            types: self.types,
            variables: self.variables,
            pous: self.pous,
            init: self.init,
            execution,
            bindings,
        })
    }

    /// The global variable `var`, of `section`; `None` for any other.
    fn global_variable(&self, var: VarId, section: Section) -> Option<&Variable> {
        self.variables
            .get(var)
            .filter(|variable| variable.owner() == Owner::Global && variable.section() == section)
    }

    /// The body of the program type the global instance variable `var`
    /// holds; `None` when `var` is no such variable.
    fn instance_variable_body(&self, var: VarId) -> Option<PouId> {
        let variable = self.global_variable(var, Section::Instance)?;
        match self.types.kind(variable.ty())? {
            TypeKind::Program { body, .. } => Some(*body),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Reference { .. } => None,
        }
    }
}
