//! The builder of one lowered program.
//!
//! A [`ProgramBuilder`] owns the program's type, variable and POU tables,
//! and is the only way to add to them or to build a node that refers into
//! them: a place, a read of a place, an assignment and a loop. Every id, place and
//! expression it makes carries its stamp, and it refuses one carrying another
//! builder's, so nothing from one lowered program can be used in another.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::{Arg, Param};
use super::expr::Expr;
use super::ids::{FieldIdx, LoopId, PouId, Stamp, TypeId, VarId};
use super::invariant::broken;
use super::loops::{build_for, build_loop, expand_for, ForParts, LoopScope};
use super::place::Place;
use super::stmt::{store_narrowing, Stmt};
use super::str_expr::StrExpr;
use super::types::{NarrowWidth, Nullability, Section, TypeEntry, TypeKind, TypeTable};
use super::variables::{by_value, Declared, FunctionSignature, Owner, PouTable, VariableTable};
use crate::execution::DebugName;

/// Builds one lowered program.
#[derive(Clone, Debug)]
pub struct ProgramBuilder {
    pub(super) stamp: Stamp,
    /// How many loops the builder has opened.
    loops: u32,
    pub(super) types: TypeTable,
    pub(super) variables: VariableTable,
    pub(super) pous: PouTable,
    /// The statements of `init`, in order (see [`super::Program`]).
    pub(super) init: Vec<Stmt>,
}

impl Default for ProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramBuilder {
    /// A builder of a new, empty program, with a stamp no other builder has.
    pub fn new() -> Self {
        let stamp = Stamp::fresh();
        Self {
            stamp,
            loops: 0,
            types: TypeTable::new(stamp),
            variables: VariableTable::new(stamp),
            pous: PouTable::new(stamp),
            init: Vec::new(),
        }
    }

    pub fn types(&self) -> &TypeTable {
        &self.types
    }

    pub fn variables(&self) -> &VariableTable {
        &self.variables
    }

    pub fn pous(&self) -> &PouTable {
        &self.pous
    }

    /// Adds the POU named `name`.
    pub fn push_pou(&mut self, name: DebugName) -> PouId {
        self.pous.push(name, None)
    }

    /// Adds the POU named `name`, a method of the function block whose body
    /// is `block`.
    pub fn push_method_pou(&mut self, name: DebugName, block: PouId) -> PouId {
        self.pous.push(name, Some(block))
    }

    /// Adds the type `entry`; see [`TypeTable::push`] for what it must be. The
    /// body of a function block or program type is a POU of this program.
    pub fn push_type(&mut self, entry: TypeEntry, span: &SourceSpan) -> Result<TypeId, Diagnostic> {
        self.check_body(&entry, "ProgramBuilder::push_type", span)?;
        self.types.push(entry, span)
    }

    /// Declares a type that [`ProgramBuilder::define_type`] gives its entry
    /// later; see [`TypeTable::declare`].
    pub fn declare_type(&mut self) -> TypeId {
        self.types.declare()
    }

    /// Gives the type `id` declares its entry, which is one
    /// [`ProgramBuilder::push_type`] would add; see [`TypeTable::define`].
    pub fn define_type(
        &mut self,
        id: TypeId,
        entry: TypeEntry,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        self.check_body(&entry, "ProgramBuilder::define_type", span)?;
        self.types.define(id, entry, span)
    }

    /// Why `entry` cannot be added, if it is a function block or program type
    /// whose body is not a POU of this program.
    fn check_body(
        &self,
        entry: &TypeEntry,
        constructor: &str,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let body = match &entry.kind {
            TypeKind::FunctionBlock { body, .. } | TypeKind::Program { body, .. } => Some(*body),
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
            | TypeKind::Reference { .. } => None,
        };
        if let Some(body) = body.filter(|body| self.pous.name(*body).is_none()) {
            return Err(broken(
                constructor,
                span,
                format!("the body {body} is not a POU of this program"),
            ));
        }
        Ok(())
    }

    /// Adds a temporary named `name`, owned by `owner`, of type `ty`: a
    /// variable lowering makes, which no declaration declares. The owner is a
    /// POU of this program and the type a type of it.
    pub fn push_variable(
        &mut self,
        name: DebugName,
        owner: Owner,
        ty: TypeId,
    ) -> Result<VarId, Diagnostic> {
        let declared = Declared::in_section(Section::Temporary);
        self.push_declared_variable(name, owner, declared, ty)
    }

    /// Adds a variable named `name`, owned by `owner`, declared as
    /// `declared` says, of type `ty`. The owner is a POU of this program and
    /// the type a type of it.
    pub fn push_declared_variable(
        &mut self,
        name: DebugName,
        owner: Owner,
        declared: Declared,
        ty: TypeId,
    ) -> Result<VarId, Diagnostic> {
        if let Owner::Pou(pou) = owner {
            if self.pous.name(pou).is_none() {
                return Err(broken(
                    "ProgramBuilder::push_variable",
                    &name.span(),
                    format!("the owner {pou} is not a POU of this program"),
                ));
            }
        }
        self.variables.push(&self.types, name, owner, declared, ty)
    }

    /// The whole of the variable `root`, at its type.
    pub fn var(&self, root: VarId, span: &SourceSpan) -> Result<Place, Diagnostic> {
        Place::var(&self.variables, root, span)
    }

    /// The field `field` of `place` (REQ-LOW-lowering-030).
    pub fn field(
        &self,
        place: Place,
        field: FieldIdx,
        span: &SourceSpan,
    ) -> Result<Place, Diagnostic> {
        place.field(&self.types, field, span)
    }

    /// The element of `place` at `subscripts`; see [`Place`] for what they
    /// must be (REQ-LOW-lowering-031, REQ-LOW-lowering-161).
    pub fn index(
        &self,
        place: Place,
        subscripts: Vec<Expr>,
        span: &SourceSpan,
    ) -> Result<Place, Diagnostic> {
        place.index(&self.types, subscripts, span)
    }

    /// What `place` refers to (REQ-LOW-lowering-032).
    pub fn deref(&self, place: Place, span: &SourceSpan) -> Result<Place, Diagnostic> {
        place.deref(&self.types, span)
    }

    /// The field at `position` of the record type `ty`; see
    /// [`TypeTable::field_idx`].
    pub fn field_idx(&self, ty: TypeId, position: usize) -> Option<FieldIdx> {
        self.types.field_idx(ty, position)
    }

    /// The value at `place` (REQ-LOW-lowering-047).
    pub fn read(&self, place: Place, span: SourceSpan) -> Result<Expr, Diagnostic> {
        Expr::read(&self.types, place, span)
    }

    /// Declares the POU `pou` a function that takes each of `parameters`, in
    /// order, and returns the value of `result`. Each is a variable `pou`
    /// owns. A parameter of a non-null reference type is a `VAR_IN_OUT` or
    /// `VAR_OUTPUT`, taken by reference; any other is taken by value, and is
    /// of a type with an operation type or a string (REQ-LOW-lowering-072).
    /// The result is of a type with an operation type.
    pub fn declare_function(
        &mut self,
        pou: PouId,
        parameters: Vec<VarId>,
        result: VarId,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_function";
        if self.pous.method_of(pou).is_some() {
            return Err(broken(CONSTRUCTOR, span, format!("{pou} is a method")));
        }
        self.declare(CONSTRUCTOR, pou, parameters, Some(result), span)
    }

    /// Declares the POU `pou` a function whose result is a string or a
    /// structure, which it returns through its first parameter, the result
    /// parameter: a non-null reference to the result, a variable `pou` owns
    /// (REQ-LOW-lowering-079). Its other parameters are as
    /// [`Self::declare_function`]'s. A call of it passes a reference to a
    /// place of the result's type for the result parameter, and is a
    /// statement or, for a string, a string in-place call (REQ-LOW-ir-181).
    pub fn declare_function_returning_through(
        &mut self,
        pou: PouId,
        parameters: Vec<VarId>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_function_returning_through";
        if self.pous.method_of(pou).is_some() {
            return Err(broken(CONSTRUCTOR, span, format!("{pou} is a method")));
        }
        let result = parameters.first().copied();
        let to_string_or_structure = result
            .and_then(|result| self.variables.get(result))
            .and_then(|result| match self.types.reference(result.ty()) {
                Some((target, Nullability::NonNull)) => self.types.kind(target),
                Some((_, Nullability::Nullable)) | None => None,
            })
            .is_some_and(|target| {
                matches!(target, TypeKind::String(_) | TypeKind::Structure { .. })
            });
        let Some(result) = result.filter(|_| to_string_or_structure) else {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!(
                    "the first parameter of {pou} is not a non-null reference to a string \
                     or a structure"
                ),
            ));
        };
        self.check_parameters(CONSTRUCTOR, pou, &parameters, None, span)?;
        self.pous.declare_function(
            pou,
            FunctionSignature::returning_through(parameters, result),
            span,
        )
    }

    /// Declares the POU `pou`, a method, one that takes each of `parameters`,
    /// in order, and returns the value of `result`, if any. Its first
    /// parameter is its instance parameter, a non-null reference to the
    /// function block it is a method of (REQ-LOW-lowering-076); the others
    /// are as a function's.
    pub fn declare_method(
        &mut self,
        pou: PouId,
        parameters: Vec<VarId>,
        result: Option<VarId>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_method";
        let block = parameters
            .first()
            .and_then(|this| self.variables.get(*this))
            .and_then(|this| match self.types.reference(this.ty()) {
                Some((target, Nullability::NonNull)) => match self.types.kind(target) {
                    Some(TypeKind::FunctionBlock { body, .. }) => Some(*body),
                    Some(_) | None => None,
                },
                Some((_, Nullability::Nullable)) | None => None,
            });
        if block.is_none() || block != self.pous.method_of(pou) {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("the first parameter of {pou} is not an instance of its block"),
            ));
        }
        self.declare(CONSTRUCTOR, pou, parameters, result, span)
    }

    fn declare(
        &mut self,
        constructor: &str,
        pou: PouId,
        parameters: Vec<VarId>,
        result: Option<VarId>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        self.check_parameters(constructor, pou, &parameters, result, span)?;
        self.pous
            .declare_function(pou, FunctionSignature::new(parameters, result), span)
    }

    /// Refuses `parameters` and `result` unless each is a variable `pou`
    /// owns, a parameter taken by value or by reference
    /// ([`FunctionSignature::params`]) and the result a scalar.
    fn check_parameters(
        &self,
        constructor: &str,
        pou: PouId,
        parameters: &[VarId],
        result: Option<VarId>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        // The type of `var`, when `pou` owns it.
        let owned = |var: &VarId| {
            self.variables
                .get(*var)
                .filter(|variable| variable.owner() == Owner::Pou(pou))
                .map(|variable| variable.ty())
        };
        let taken = |var: &&VarId| owned(var).and_then(|ty| by_value(&self.types, ty));
        let scalar = |var: &&VarId| owned(var).and_then(|ty| self.types.operation_type(ty));
        if let Some(var) = parameters.iter().find(|var| taken(var).is_none()) {
            return Err(broken(
                constructor,
                span,
                format!("{var} is not a scalar or string variable of {pou}"),
            ));
        }
        if let Some(var) = result.iter().find(|var| scalar(var).is_none()) {
            return Err(broken(
                constructor,
                span,
                format!("{var} is not a scalar variable of {pou}"),
            ));
        }
        Ok(())
    }

    /// What each parameter of the function `pou` takes, in order; `None`
    /// when `pou` is not a function of this program.
    pub fn params(&self, pou: PouId) -> Option<Vec<Param>> {
        self.pous
            .function(pou)?
            .params(&self.types, &self.variables)
    }

    /// The result of calling the function or method `pou` with `args`; see
    /// [`Expr::call`] for an intrinsic.
    pub fn call(&self, pou: PouId, args: Vec<Arg>, span: SourceSpan) -> Result<Expr, Diagnostic> {
        Expr::call_function(&self.types, &self.variables, &self.pous, pou, args, span)
    }

    /// The result of calling the function or method `pou` with `args` in
    /// place, then `stores`, the stores of its outputs; see
    /// [`InPlaceCall`](super::InPlaceCall) for what they must be.
    pub fn call_in_place(
        &self,
        pou: PouId,
        args: Vec<Arg>,
        stores: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Expr, Diagnostic> {
        Expr::call_in_place(
            &self.types,
            &self.variables,
            &self.pous,
            pou,
            args,
            stores,
            span,
        )
    }

    /// The string the function `pou`, whose result is a string, writes to
    /// the temporary `args` passes for it, called in place, then `stores`,
    /// the stores of its outputs; see [`StrExpr`] for what they must be.
    pub fn str_call_in_place(
        &self,
        pou: PouId,
        args: Vec<Arg>,
        stores: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<StrExpr, Diagnostic> {
        StrExpr::call_in_place(
            &self.types,
            &self.variables,
            &self.pous,
            pou,
            args,
            stores,
            span,
        )
    }

    /// Calls the function or method `pou` with `args`, discarding its
    /// result, if any; see [`Stmt`] for what they must be.
    pub fn call_stmt(
        &self,
        pou: PouId,
        args: Vec<Arg>,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        Stmt::call(&self.types, &self.variables, &self.pous, pou, args, span)
    }

    /// Stores `value` at `place`; see [`Stmt`] for what the value must be.
    pub fn assign(&self, place: Place, value: Expr, span: SourceSpan) -> Result<Stmt, Diagnostic> {
        Stmt::assign(&self.types, place, value, span)
    }

    /// The string at `place`, a place of a string type.
    pub fn read_str(&self, place: Place, span: SourceSpan) -> Result<StrExpr, Diagnostic> {
        StrExpr::read(&self.types, place, span)
    }

    /// Stores the string `value` at `place`; see [`Stmt`] for what they must
    /// be.
    pub fn assign_str(
        &self,
        place: Place,
        value: StrExpr,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        Stmt::assign_str(&self.types, place, value, span)
    }

    /// Replaces the whole value at `dst` with the whole value at `src`; see
    /// [`Stmt`] for what they must be.
    pub fn copy(&self, dst: Place, src: Place, span: SourceSpan) -> Result<Stmt, Diagnostic> {
        Stmt::copy(&self.types, dst, src, span)
    }

    /// Runs the body of the function block `instance` is an instance of, on
    /// `instance`; see [`Stmt`] for what it must be.
    pub fn fb_call(&self, instance: Place, span: SourceSpan) -> Result<Stmt, Diagnostic> {
        Stmt::fb_call(&self.types, instance, span)
    }

    /// Replaces `bits` bits of the value at `place`, starting at bit `shift`,
    /// with the low bits of `value`; see [`Stmt`] for what they must be.
    pub fn assign_bits(
        &self,
        place: Place,
        shift: u8,
        bits: u8,
        value: Expr,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        Stmt::assign_bits(&self.types, place, shift, bits, value, span)
    }

    /// The width `value` must be narrowed to before it is stored at a place of
    /// type `place`; see [`store_narrowing`].
    pub fn store_narrowing(&self, place: TypeId, value: &Expr) -> Option<NarrowWidth> {
        store_narrowing(&self.types, place, value)
    }

    /// Opens a new `Loop` or `For`, whose scope makes the `Exit` and
    /// `Continue` statements that name it.
    pub fn open_loop(&mut self) -> LoopScope {
        let id = LoopId::new(self.stamp, self.loops);
        self.loops += 1;
        LoopScope::new(id)
    }

    /// Closes `scope` as a `Loop` that repeats `body` then `continuing`.
    /// Every `Exit` and `Continue` the scope made is inside them
    /// (REQ-LOW-lowering-062).
    pub fn loop_(
        &self,
        scope: LoopScope,
        body: Vec<Stmt>,
        continuing: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        self.check_scope("ProgramBuilder::loop_", &scope, &span)?;
        build_loop(scope, body, continuing, span)
    }

    /// Closes `scope` as a `For`; see [`ForParts`] for what its parts must
    /// be. Every `Exit` and `Continue` the scope made is inside its body
    /// (REQ-LOW-lowering-062).
    pub fn for_(
        &self,
        scope: LoopScope,
        parts: ForParts,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        self.check_scope("ProgramBuilder::for_", &scope, &span)?;
        build_for(&self.types, scope, parts, span)
    }

    /// Fails unless this builder opened `scope`.
    fn check_scope(
        &self,
        constructor: &str,
        scope: &LoopScope,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        if scope.id().stamp() != self.stamp {
            return Err(broken(
                constructor,
                span,
                format!("{} is a loop of another program", scope.id()),
            ));
        }
        Ok(())
    }

    /// `stmt` with a `For` expanded into an `Assign` and a `Loop` of the same
    /// meaning, for a backend with no use for its bounds (REQ-LOW-ir-068).
    /// Any other statement is returned as it is.
    pub fn expand_for(&self, stmt: Stmt) -> Result<Vec<Stmt>, Diagnostic> {
        expand_for(&self.types, stmt)
    }
}
