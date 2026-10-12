//! The builder of one lowered program.
//!
//! A [`ProgramBuilder`] owns the program's type, variable and POU tables,
//! and is the only way to add to them or to build a node that refers into
//! them: a place, a read of a place, an assignment and a loop. Every id, place and
//! expression it makes carries its stamp, and it refuses one carrying another
//! builder's, so nothing from one lowered program can be used in another.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::expr::Expr;
use super::ids::{FieldIdx, LoopId, PouId, Stamp, TypeId, VarId};
use super::invariant::broken;
use super::loops::{build_for, build_loop, expand_for, ForParts, LoopScope};
use super::place::Place;
use super::stmt::{store_narrowing, Stmt};
use super::types::{NarrowWidth, TypeEntry, TypeKind, TypeTable};
use super::variables::{Owner, PouTable, VariableTable};
use crate::execution::DebugName;

/// Builds one lowered program.
#[derive(Clone, Debug)]
pub struct ProgramBuilder {
    stamp: Stamp,
    /// How many loops the builder has opened.
    loops: u32,
    types: TypeTable,
    variables: VariableTable,
    pous: PouTable,
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
        self.pous.push(name)
    }

    /// Adds the type `entry`; see [`TypeTable::push`] for what it must be. The
    /// body of a function block or program type is a POU of this program.
    pub fn push_type(&mut self, entry: TypeEntry, span: &SourceSpan) -> Result<TypeId, Diagnostic> {
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
                "ProgramBuilder::push_type",
                span,
                format!("the body {body} is not a POU of this program"),
            ));
        }
        self.types.push(entry, span)
    }

    /// Adds a variable named `name`, owned by `owner`, of type `ty`. The owner
    /// is a POU of this program and the type a type of it.
    pub fn push_variable(
        &mut self,
        name: DebugName,
        owner: Owner,
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
        self.variables.push(&self.types, name, owner, ty)
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

    /// Stores `value` at `place`; see [`Stmt`] for what the value must be.
    pub fn assign(&self, place: Place, value: Expr, span: SourceSpan) -> Result<Stmt, Diagnostic> {
        Stmt::assign(&self.types, place, value, span)
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
