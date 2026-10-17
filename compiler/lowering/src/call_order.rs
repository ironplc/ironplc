//! Whether running the calls moved out of a statement before it keeps the
//! order the source runs things in (REQ-LOW-lowering-180).
//!
//! A call of a function whose result is a string runs, in the lowered
//! program, as a `Call` statement before the statement it is in
//! (`string_result.rs`). In the source it runs where its value is used. The
//! two mean the same only when nothing the statement evaluates before the
//! call could see what the call does, or do something the call could see:
//! a function can write a global, a variable passed to its `VAR_IN_OUT` or
//! what a reference refers to, and an operation can trap before the call
//! runs. So the statement is walked in the order it is evaluated, and each
//! call is met where its value is read, which is where the source runs it:
//!
//! * before a read of a moved call's temporary, the walk meets only
//!   constants, string literals, references to places, and the reads of
//!   temporaries of calls moved before it. A moved call's arguments are
//!   walked the same way where the call is met, since they run just before
//!   it;
//! * the calls are met in the order they were moved, each once;
//! * every call is met. A temporary read anywhere the walk does not go is
//!   never met: the condition of an `ELSIF`, a `WHILE` or a `REPEAT`, the
//!   `TO` of a `FOR`, or any statement inside another, which may run any
//!   number of times or not at all; the right operand of `AND_THEN` or
//!   `OR_ELSE`, which may not run; the selector of a `CASE`, which the route
//!   that reads the AST evaluates once for each label it tests; and a bit or
//!   partial write, which reads its place before its value.

use ironplc_ir::lowered::{
    Arg, Expr, ExprKind, Nullability, Place, Projection, Stmt, StmtKind, StrExpr, StrExprKind,
    TypeKind, TypeTable, VarId, VariableTable,
};

/// Whether running `calls`, the calls moved out of a statement in the order
/// lowering moved them, then `stmts`, the statements the statement lowered
/// to, runs everything in the order the source does. Each of `calls` passes
/// the temporary at its position in `temporaries` for its result.
pub(crate) fn keeps_order(
    types: &TypeTable,
    variables: &VariableTable,
    calls: &[Stmt],
    temporaries: &[VarId],
    stmts: &[Stmt],
) -> bool {
    let mut walk = Walk {
        types,
        variables,
        calls,
        temporaries,
        next: 0,
    };
    let mut evaluated = false;
    stmts.iter().all(|stmt| walk.stmt(stmt, &mut evaluated)) && walk.next == calls.len()
}

/// A walk of a statement in the order it is evaluated. Each step returns
/// `false` when the walk shows the order is not kept. `evaluated` says
/// whether the sequence being walked has evaluated anything a moved call
/// could see, or that could see it, since it began.
struct Walk<'w> {
    types: &'w TypeTable,
    variables: &'w VariableTable,
    calls: &'w [Stmt],
    temporaries: &'w [VarId],
    /// The position of the next moved call the walk expects to meet.
    next: usize,
}

impl Walk<'_> {
    /// The parts of `stmt` evaluated before the statement does anything
    /// else, then the statement itself.
    fn stmt(&mut self, stmt: &Stmt, evaluated: &mut bool) -> bool {
        let walked = match stmt.kind() {
            // The route that reads the AST stores a value, then works out
            // where it goes.
            StmtKind::Assign { place, value } => {
                self.expr(value, evaluated) && self.place(place, evaluated)
            }
            StmtKind::AssignStr { place, value } => {
                self.str_expr(value, evaluated) && self.place(place, evaluated)
            }
            StmtKind::Call { args, .. } => args.iter().all(|arg| self.arg(arg, evaluated)),
            StmtKind::If { cond, .. } => self.expr(cond, evaluated),
            StmtKind::For { from, .. } => self.expr(from, evaluated),
            StmtKind::AssignBits { .. }
            | StmtKind::Copy { .. }
            | StmtKind::FbCall { .. }
            | StmtKind::Case { .. }
            | StmtKind::Loop { .. }
            | StmtKind::Exit(_)
            | StmtKind::Continue(_)
            | StmtKind::Return => true,
        };
        // What the statement does, its store or its branches, comes next.
        *evaluated = true;
        walked
    }

    fn expr(&mut self, expr: &Expr, evaluated: &mut bool) -> bool {
        match expr.kind() {
            ExprKind::Const(_) | ExprKind::Null => true,
            ExprKind::Read(place) => self.read(place, evaluated),
            ExprKind::RefTo(place) => self.place(place, evaluated),
            ExprKind::Convert(value)
            | ExprKind::ConvertChecked(value)
            | ExprKind::Truncate { value, .. }
            | ExprKind::Unary { value, .. } => self.expr(value, evaluated) && done(evaluated),
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Compare { lhs, rhs, .. } => {
                self.expr(lhs, evaluated) && self.expr(rhs, evaluated) && done(evaluated)
            }
            // The right operand may not run, so a call in it is not met.
            ExprKind::ShortCircuit { lhs, .. } => self.expr(lhs, evaluated) && done(evaluated),
            ExprKind::Call { args, .. } => {
                args.iter().all(|arg| self.arg(arg, evaluated)) && done(evaluated)
            }
            // Lowering moves no call out of a statement that runs one in
            // place, so the walk does not go on into it.
            ExprKind::InPlaceCall(_) => false,
        }
    }

    fn str_expr(&mut self, value: &StrExpr, evaluated: &mut bool) -> bool {
        match value.kind() {
            StrExprKind::Literal(_) => true,
            StrExprKind::Read(place) => self.read(place, evaluated),
            StrExprKind::Call { args, .. } => {
                args.iter().all(|arg| self.arg(arg, evaluated)) && done(evaluated)
            }
            StrExprKind::InPlaceCall { .. } => false,
        }
    }

    fn arg(&mut self, arg: &Arg, evaluated: &mut bool) -> bool {
        match arg {
            Arg::Value(value) => self.expr(value, evaluated),
            Arg::Str(value) => self.str_expr(value, evaluated),
            Arg::Ref(place) => self.place(place, evaluated),
        }
    }

    /// A read of `place`: where a moved call's temporary is read, the call
    /// it holds the result of, which runs there in the source.
    fn read(&mut self, place: &Place, evaluated: &mut bool) -> bool {
        let moved = match place.path() {
            [] => self
                .temporaries
                .iter()
                .position(|temporary| *temporary == place.root()),
            [_, ..] => None,
        };
        match moved {
            Some(call) => !*evaluated && self.meet(call),
            None => self.place(place, evaluated) && done(evaluated),
        }
    }

    /// The moved call at position `call`, met where its value is read: its
    /// arguments, which run just before it, then the call, which must be the
    /// next one moved.
    fn meet(&mut self, call: usize) -> bool {
        let Some(StmtKind::Call { args, .. }) = self.calls.get(call).map(Stmt::kind) else {
            return false;
        };
        let mut evaluated = false;
        if !args.iter().all(|arg| self.arg(arg, &mut evaluated)) || call != self.next {
            return false;
        }
        self.next += 1;
        true
    }

    /// What finding `place` evaluates: each subscript, and its check, and a
    /// dereference of a reference that may be `NULL`, which traps on one.
    /// The instance parameter and other non-null references cannot trap.
    fn place(&mut self, place: &Place, evaluated: &mut bool) -> bool {
        let mut ty = self.variables.get(place.root()).map(|root| root.ty());
        for projection in place.path() {
            let types = self.types;
            ty = match projection {
                Projection::Field(field) => ty
                    .and_then(|ty| types.fields(ty))
                    .and_then(|fields| fields.get(field.index() as usize))
                    .map(|field| field.ty),
                Projection::Index(subscripts) => {
                    if !subscripts
                        .iter()
                        .all(|subscript| self.expr(subscript, evaluated))
                    {
                        return false;
                    }
                    *evaluated = true;
                    match ty.and_then(|ty| types.kind(ty)) {
                        Some(TypeKind::Array { element, .. }) => Some(*element),
                        Some(_) | None => None,
                    }
                }
                Projection::Deref => match ty.and_then(|ty| types.reference(ty)) {
                    Some((target, Nullability::NonNull)) => Some(target),
                    Some((target, Nullability::Nullable)) => {
                        *evaluated = true;
                        Some(target)
                    }
                    None => {
                        *evaluated = true;
                        None
                    }
                },
            };
        }
        true
    }
}

/// Records that the walk has evaluated something a moved call could see,
/// and goes on.
fn done(evaluated: &mut bool) -> bool {
    *evaluated = true;
    true
}
