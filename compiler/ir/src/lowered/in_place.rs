//! Calls run in place: where the expression that holds them is evaluated.
//!
//! A call of a function or method with outputs, or of a function whose
//! result is a string, does more than yield a value: its outputs are stored
//! in their targets after it returns, and a string result is written to a
//! temporary its caller passes. An in-place call holds the call and the
//! stores of its outputs, so the expression that holds it runs all of it
//! where the source evaluates the call: the call, then each store, in order,
//! then the expression goes on with the call's result
//! (`specs/design/lowered-program.md`, "In-place calls").

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::Arg;
use super::expr::{Expr, ExprKind};
use super::ids::{PouId, Stamp, VarId};
use super::invariant::broken;
use super::place::Place;
use super::stmt::{one_program, programs, Stmt, StmtKind};
use super::str_expr::StrExprKind;

/// A call of the user-defined function or method `pou`, then the stores of
/// its outputs (REQ-LOW-ir-182).
#[derive(Clone, Debug)]
pub struct InPlaceCall {
    pou: PouId,
    args: Vec<Arg>,
    stores: Vec<Stmt>,
}

impl InPlaceCall {
    /// The function or method called.
    pub fn pou(&self) -> PouId {
        self.pou
    }

    /// One argument for each parameter, in order, as a `Call` passes them.
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// The stores of the call's outputs, in the order they run, after the
    /// call returns.
    pub fn stores(&self) -> &[Stmt] {
        &self.stores
    }

    /// The program whose places the call's arguments and stores name.
    pub(crate) fn program(&self) -> Option<Stamp> {
        self.args
            .iter()
            .find_map(Arg::program)
            .or_else(|| self.stores.iter().find_map(Stmt::program))
    }

    /// The call of `pou` with `args`, whose types the caller checked, then
    /// `stores`. Each store is an assignment that moves an output: its value
    /// is a place `args` passes by reference, read whole, through any
    /// conversions or narrowing (REQ-LOW-ir-183). The arguments and stores
    /// name places of one program.
    pub(crate) fn new(
        constructor: &str,
        pou: PouId,
        args: Vec<Arg>,
        stores: Vec<Stmt>,
        span: &SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let passed: Vec<VarId> = args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Ref(place) => whole(place),
                Arg::Value(_) | Arg::Str(_) | Arg::Copy(_) => None,
            })
            .collect();
        if let Some(store) = stores.iter().find(|store| !moves_output(store, &passed)) {
            return Err(broken(
                constructor,
                span,
                format!("`{store}` does not store an output of the call"),
            ));
        }
        one_program(
            constructor,
            span,
            args.iter().map(Arg::program).chain(programs(&stores)),
        )?;
        Ok(Self { pou, args, stores })
    }
}

/// Whether `store` is an assignment whose value is the whole of one of
/// `passed`: an `Assign` or `AssignBits` of a read of it, through
/// operations of one operand, an `AssignStr` of a read of it, or a `Copy`
/// of it.
fn moves_output(store: &Stmt, passed: &[VarId]) -> bool {
    let reads = |place: &Place| whole(place).is_some_and(|var| passed.contains(&var));
    match store.kind() {
        StmtKind::Assign { value, .. } | StmtKind::AssignBits { value, .. } => {
            match innermost(value).kind() {
                ExprKind::Read(place) => reads(place),
                ExprKind::Const(_)
                | ExprKind::Convert(_)
                | ExprKind::ConvertChecked(_)
                | ExprKind::Truncate { .. }
                | ExprKind::Unary { .. }
                | ExprKind::Binary { .. }
                | ExprKind::Compare { .. }
                | ExprKind::ShortCircuit { .. }
                | ExprKind::Call { .. }
                | ExprKind::InPlaceCall(_)
                | ExprKind::RefTo(_)
                | ExprKind::Null => false,
            }
        }
        StmtKind::AssignStr { value, .. } => match value.kind() {
            StrExprKind::Read(place) => reads(place),
            StrExprKind::Literal(_)
            | StrExprKind::Call { .. }
            | StrExprKind::InPlaceCall { .. } => false,
        },
        StmtKind::Copy { src, .. } => reads(src),
        StmtKind::FbCall { .. }
        | StmtKind::Call { .. }
        | StmtKind::If { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => false,
    }
}

/// What `expr` applies operations of one operand to: a conversion, a
/// narrowing or a unary operation.
fn innermost(expr: &Expr) -> &Expr {
    match expr.kind() {
        ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Truncate { value, .. }
        | ExprKind::Unary { value, .. } => innermost(value),
        ExprKind::Const(_)
        | ExprKind::Read(_)
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => expr,
    }
}

/// The variable `place` is the whole of.
fn whole(place: &Place) -> Option<VarId> {
    place.path().is_empty().then(|| place.root())
}
