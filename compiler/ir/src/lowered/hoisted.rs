//! The order in which an assignment reads the results of the calls with
//! outputs lowered ahead of it (REQ-LOW-lowering-174).
//!
//! An assignment whose value has calls with outputs lowers to the part of
//! each call, the call stored in a temporary then the assignments of its
//! outputs, then the assignment, which reads each call's result from its
//! temporary (REQ-LOW-lowering-172). Running the parts first keeps the
//! order the source runs things in only when nothing else in the value runs
//! before a call. That is so when each result is read once, among the first
//! values the part after it or the assignment evaluates, and the results
//! each of those reads are the most recent not yet read, in the order they
//! were made: as the values of a stack.

use super::{Arg, Expr, ExprKind, Place, Projection, Stmt, StmtKind, StrExpr, StrExprKind, VarId};

/// The leading reads of `expr` among the variables `results`: the reads of
/// the whole of one of them that `expr` evaluates before any other node
/// finishes, in order.
///
/// A read of one is its own leading read. An operation of one operand has
/// its operand's. An operation of several, and a call, has its first
/// operand's, going on into the next while the one before is itself such a
/// read. A short circuit has its left operand's, since its right one runs
/// only on a condition. A constant, `NULL`, a reference, a read of any
/// other place, and a `Ref` or string argument, end the run.
pub(crate) fn leading_reads(expr: &Expr, results: &[VarId]) -> Vec<VarId> {
    let mut reads = Vec::new();
    lead(expr, results, &mut reads);
    reads
}

/// Whether the results `parts` store, each a value and the variable it is
/// stored in, in the order the parts run, are read by the parts after them
/// and then by `value` as the values of a stack (REQ-LOW-lowering-174):
/// each once, among the leading reads of what reads it, and the results
/// each of those reads are the most recent not yet read, in the order they
/// were made.
pub fn reads_results_in_order(parts: &[(&Expr, VarId)], value: &Expr) -> bool {
    let results: Vec<VarId> = parts.iter().map(|(_, result)| *result).collect();
    let readers = parts
        .iter()
        .map(|(part, result)| (*part, Some(*result)))
        .chain(std::iter::once((value, None)));
    let mut unread: Vec<VarId> = Vec::new();
    for (reader, result) in readers {
        let read = leading_reads(reader, &results);
        // A result read anywhere but among the leading reads is read out
        // of order.
        if reads(reader, &results) != read.len() || !unread.ends_with(&read) {
            return false;
        }
        unread.truncate(unread.len() - read.len());
        unread.extend(result);
    }
    unread.is_empty()
}

/// Appends the leading reads of `expr` among `results` to `reads`, and
/// returns whether `expr` is itself one, so that the run goes on into the
/// operand after it.
fn lead(expr: &Expr, results: &[VarId], reads: &mut Vec<VarId>) -> bool {
    match expr.kind() {
        ExprKind::Read(place) => match whole(place) {
            Some(var) if results.contains(&var) => {
                reads.push(var);
                true
            }
            Some(_) | None => false,
        },
        ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Truncate { value, .. }
        | ExprKind::Unary { value, .. } => {
            lead(value, results, reads);
            false
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Compare { lhs, rhs, .. } => {
            if lead(lhs, results, reads) {
                lead(rhs, results, reads);
            }
            false
        }
        ExprKind::ShortCircuit { lhs, .. } => {
            lead(lhs, results, reads);
            false
        }
        ExprKind::Call { args, .. } => {
            lead_args(args, results, reads);
            false
        }
        ExprKind::InPlaceCall(call) => {
            lead_args(call.args(), results, reads);
            false
        }
        ExprKind::Const(_) | ExprKind::RefTo(_) | ExprKind::Null => false,
    }
}

/// Appends the leading reads of a call's `args` to `reads`: those of each
/// value argument while the one before it is itself such a read.
fn lead_args(args: &[Arg], results: &[VarId], reads: &mut Vec<VarId>) {
    for arg in args {
        let Arg::Value(value) = arg else {
            break;
        };
        if !lead(value, results, reads) {
            break;
        }
    }
}

/// How many times `expr` reads, or passes as a `Ref`, the whole of one of
/// `results`, anywhere inside it.
fn reads(expr: &Expr, results: &[VarId]) -> usize {
    match expr.kind() {
        ExprKind::Read(place) => whole_reads(place, results) + subscript_reads(place, results),
        ExprKind::RefTo(place) => subscript_reads(place, results),
        ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Truncate { value, .. }
        | ExprKind::Unary { value, .. } => reads(value, results),
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Compare { lhs, rhs, .. }
        | ExprKind::ShortCircuit { lhs, rhs, .. } => reads(lhs, results) + reads(rhs, results),
        ExprKind::Call { args, .. } => args.iter().map(|arg| arg_reads(arg, results)).sum(),
        ExprKind::InPlaceCall(call) => in_place_reads(call, results),
        ExprKind::Const(_) | ExprKind::Null => 0,
    }
}

/// How many times an in-place call's arguments and stores read, or pass as
/// a `Ref`, the whole of one of `results`.
fn in_place_reads(call: &super::InPlaceCall, results: &[VarId]) -> usize {
    let args: usize = call.args().iter().map(|arg| arg_reads(arg, results)).sum();
    let stores: usize = call
        .stores()
        .iter()
        .map(|store| store_reads(store, results))
        .sum();
    args + stores
}

/// How many times `store`, an assignment, reads the whole of one of
/// `results`, in its value or on the way to its place.
fn store_reads(store: &Stmt, results: &[VarId]) -> usize {
    match store.kind() {
        StmtKind::Assign { place, value } | StmtKind::AssignBits { place, value, .. } => {
            reads(value, results) + subscript_reads(place, results)
        }
        StmtKind::AssignStr { place, value } => {
            str_reads(value, results) + subscript_reads(place, results)
        }
        StmtKind::Copy { dst, src } => {
            whole_reads(src, results)
                + subscript_reads(src, results)
                + subscript_reads(dst, results)
        }
        StmtKind::FbCall { .. }
        | StmtKind::Call { .. }
        | StmtKind::If { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => 0,
    }
}

fn arg_reads(arg: &Arg, results: &[VarId]) -> usize {
    match arg {
        Arg::Value(value) => reads(value, results),
        Arg::Str(value) => str_reads(value, results),
        Arg::Ref(place) => whole_reads(place, results) + subscript_reads(place, results),
    }
}

fn str_reads(value: &StrExpr, results: &[VarId]) -> usize {
    match value.kind() {
        StrExprKind::Literal(_) => 0,
        StrExprKind::Read(place) => subscript_reads(place, results),
        StrExprKind::Call { args, .. } => args.iter().map(|arg| arg_reads(arg, results)).sum(),
        StrExprKind::InPlaceCall { call, .. } => in_place_reads(call, results),
    }
}

/// 1 when `place` is the whole of one of `results`, and 0 otherwise.
fn whole_reads(place: &Place, results: &[VarId]) -> usize {
    usize::from(whole(place).is_some_and(|var| results.contains(&var)))
}

/// How many times the subscripts on the way to `place` read one of
/// `results`.
fn subscript_reads(place: &Place, results: &[VarId]) -> usize {
    place
        .path()
        .iter()
        .map(|projection| match projection {
            Projection::Index(subscripts) => subscripts
                .iter()
                .map(|subscript| reads(subscript, results))
                .sum(),
            Projection::Field(_) | Projection::Deref => 0,
        })
        .sum()
}

/// The variable `place` is the whole of.
fn whole(place: &Place) -> Option<VarId> {
    place.path().is_empty().then(|| place.root())
}
