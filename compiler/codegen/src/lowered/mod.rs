//! Compiles a statement from the lowered program rather than from the AST
//! (`specs/design/lowered-program.md`, "Delivery Constraints").
//!
//! While both routes exist, codegen asks lowering for each top-level
//! statement of a body. When lowering supports the statement and everything
//! inside it, the statement is emitted from the lowered program; otherwise it
//! is compiled from the AST, as before. Both routes emit into the same body
//! through the same emitter.
//!
//! A lowered statement finds its variables' storage in the layout codegen
//! already builds, through the current body's scope, by the name lowering
//! keeps on each variable and field. The lowered program carries that name
//! for debug information (REQ-LOW-lowering-006), and the design allows the
//! lookup by name until a POU's statements all lower.

// A match over a lowered enum names every variant, so a variant added later
// fails to compile wherever it is not handled (REQ-LOW-lowering-082). The
// deny covers this module tree only: the AST route predates the rule.
#![deny(clippy::wildcard_enum_match_arm)]

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::StmtKind as AstStmtKind;
use ironplc_ir::lowered::{
    Arg, Const, Expr, ExprKind, Field, NarrowWidth, Nullability, Place as LoweredPlace, Projection,
    ScalarType, Signedness as LoweredSignedness, Stmt, StmtKind, TypeId, TypeKind,
};
use ironplc_lowering::{Lowering, StatementLowering};

use crate::compile::{CompileContext, OpType, OpWidth, Signedness};
use crate::emit::Emitter;

mod expr;
mod place;
mod stmt;

/// Which route codegen compiles a statement by.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Every statement from the AST.
    #[default]
    Ast,
    /// The lowered statement, where lowering supports it, and the AST for
    /// every other.
    PreferLowered,
    /// The lowered statement; a statement lowering does not support yet is a
    /// P9999.
    RequireLowered,
}

/// How many top-level statements took each route in one compilation. Under
/// [`Route::Ast`] lowering is not asked, and nothing is counted.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RouteCounts {
    /// Statements compiled from the lowered program.
    pub lowered: usize,
    /// Statements compiled from the AST.
    pub from_ast: usize,
}

/// The lowering a compilation asks, and what it has made of the statements
/// so far.
pub(crate) struct LoweredRoute<'a> {
    lowering: Lowering<'a>,
    require: bool,
    pub(crate) counts: RouteCounts,
}

impl<'a> LoweredRoute<'a> {
    /// The lowered route for `route`, or `None` when every statement is
    /// compiled from the AST.
    pub(crate) fn new(route: Route, lowering: impl FnOnce() -> Lowering<'a>) -> Option<Self> {
        let require = match route {
            Route::Ast => return None,
            Route::PreferLowered => false,
            Route::RequireLowered => true,
        };
        Some(Self {
            lowering: lowering(),
            require,
            counts: RouteCounts::default(),
        })
    }
}

/// Compiles the top-level statement `stmt` of the current body from the
/// lowered program, if the route asks for it and lowering supports it.
/// Returns `false` when the statement is left to the AST.
pub(crate) fn try_compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &AstStmtKind,
) -> Result<bool, Diagnostic> {
    let Some(mut route) = ctx.lowered_route.take() else {
        return Ok(false);
    };
    let compiled = compile_by_route(&mut route, emitter, ctx, stmt);
    ctx.lowered_route = Some(route);
    compiled
}

fn compile_by_route(
    route: &mut LoweredRoute<'_>,
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &AstStmtKind,
) -> Result<bool, Diagnostic> {
    let lowered = match ctx.current_pou.clone() {
        Some(pou) => match route.lowering.lower_statement_of(&pou, stmt)? {
            StatementLowering::Lowered(lowered) => Some(lowered),
            StatementLowering::Unsupported => None,
        },
        // A method body: not lowered yet.
        None => None,
    };
    match lowered {
        Some(lowered) if can_emit(&route.lowering, ctx, &lowered) => {
            crate::compile_stmt::record_statement_position(emitter, ctx, stmt);
            stmt::emit_stmt(emitter, ctx, &route.lowering, &lowered)?;
            route.counts.lowered += 1;
            Ok(true)
        }
        Some(_) | None if route.require => Err(Diagnostic::not_implemented(Label::span(
            statement_span(stmt),
            "Statement the lowered route does not support yet",
        ))),
        Some(_) | None => {
            route.counts.from_ast += 1;
            Ok(false)
        }
    }
}

/// Where `stmt` is, for a diagnostic about it.
fn statement_span(stmt: &AstStmtKind) -> ironplc_dsl::core::SourceSpan {
    match stmt {
        AstStmtKind::Assignment(assignment) => assignment.span(),
        AstStmtKind::FbCall(call) => call.span(),
        AstStmtKind::MethodCall(call) => call.span(),
        AstStmtKind::If(stmt) => stmt.span(),
        AstStmtKind::Case(stmt) => stmt.span(),
        AstStmtKind::For(stmt) => stmt.span(),
        AstStmtKind::While(stmt) => stmt.span(),
        AstStmtKind::Repeat(stmt) => stmt.span(),
        AstStmtKind::Return => ironplc_dsl::core::SourceSpan::default(),
        AstStmtKind::Exit(span) | AstStmtKind::Continue(span) => span.clone(),
    }
}

/// Whether every place `stmt` names has storage this backend reaches as one
/// value, at the operation type the lowered program gives it. A statement
/// whose storage this backend does not reach yet is compiled from the AST.
fn can_emit(lowering: &Lowering<'_>, ctx: &CompileContext, stmt: &Stmt) -> bool {
    let all = |stmts: &[Stmt]| stmts.iter().all(|stmt| can_emit(lowering, ctx, stmt));
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            reaches(lowering, ctx, place, value.ty()) && expr_can_emit(lowering, ctx, value)
        }
        StmtKind::AssignBits { place, value, .. } => {
            lowering
                .types()
                .operation_type(place.ty())
                .is_some_and(|ty| reaches(lowering, ctx, place, ty))
                && expr_can_emit(lowering, ctx, value)
        }
        StmtKind::If {
            cond,
            then,
            otherwise,
        } => expr_can_emit(lowering, ctx, cond) && all(then) && all(otherwise),
        StmtKind::Case {
            selector,
            arms,
            otherwise,
        } => {
            expr_can_emit(lowering, ctx, selector)
                && arms.iter().all(|arm| all(arm.body()))
                && all(otherwise)
        }
        StmtKind::For {
            control,
            from,
            to,
            body,
            ..
        } => {
            reaches(lowering, ctx, control, from.ty())
                && expr_can_emit(lowering, ctx, from)
                && expr_can_emit(lowering, ctx, to)
                && all(body)
        }
        StmtKind::Loop {
            body, continuing, ..
        } => all(body) && all(continuing),
        StmtKind::Exit(_) | StmtKind::Continue(_) | StmtKind::Return => true,
    }
}

fn expr_can_emit(lowering: &Lowering<'_>, ctx: &CompileContext, expr: &Expr) -> bool {
    if op_type(expr.ty()).is_none() {
        return false;
    }
    match expr.kind() {
        ExprKind::Const(_) => true,
        ExprKind::Read(place) => reaches(lowering, ctx, place, expr.ty()),
        ExprKind::Truncate { width, value } => {
            !matches!(width, NarrowWidth::B32) && expr_can_emit(lowering, ctx, value)
        }
        ExprKind::Convert(value) | ExprKind::Unary { value, .. } => {
            expr_can_emit(lowering, ctx, value)
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Compare { lhs, rhs, .. }
        | ExprKind::ShortCircuit { lhs, rhs, .. } => {
            expr_can_emit(lowering, ctx, lhs) && expr_can_emit(lowering, ctx, rhs)
        }
        ExprKind::Call { args, .. } => args.iter().all(|arg| match arg {
            Arg::Value(value) => expr_can_emit(lowering, ctx, value),
        }),
    }
}

/// Whether `place` has storage this backend reaches as one value of type
/// `ty`.
fn reaches(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    place: &LoweredPlace,
    ty: ScalarType,
) -> bool {
    let subscripts_can_emit = place.path().iter().all(|projection| match projection {
        Projection::Index(subscripts) => subscripts
            .iter()
            .all(|subscript| expr_can_emit(lowering, ctx, subscript)),
        Projection::Field(_) | Projection::Deref => true,
    });
    subscripts_can_emit
        && place::resolve(lowering, ctx, place, &SourceSpan::default())
            .is_ok_and(|storage| op_type(ty) == Some(storage.op_type()))
}

/// The name this backend's layout knows a variable `place` by: the
/// variable's name, or the name of a field of the current instance, which
/// this backend copies into a slot of that name. `None` for any other place.
pub(super) fn storage_name(lowering: &Lowering<'_>, place: &LoweredPlace) -> Option<Id> {
    let root = lowering.variables().get(place.root())?;
    if place.path().is_empty() {
        return Some(Id::from(&root.name().to_string()));
    }
    let (field, rest) = instance_field(lowering, root.ty(), place.path())?;
    rest.is_empty().then(|| Id::from(&field.name.to_string()))
}

/// When `path` starts by reaching a field of the current instance through the
/// instance parameter, of type `root`: that field, and the projections after
/// it. `None` for any other path.
pub(super) fn instance_field<'l, 'p>(
    lowering: &'l Lowering<'_>,
    root: TypeId,
    path: &'p [Projection],
) -> Option<(&'l Field, &'p [Projection])> {
    let [deref, field, rest @ ..] = path else {
        return None;
    };
    match deref {
        Projection::Deref => {}
        Projection::Field(_) | Projection::Index(_) => return None,
    }
    let field = match field {
        Projection::Field(field) => field,
        Projection::Index(_) | Projection::Deref => return None,
    };
    if !is_instance(lowering, root) {
        return None;
    }
    let types = lowering.types();
    let (instance, _) = types.reference(root)?;
    let declared = types.fields(instance)?.get(field.index() as usize)?;
    Some((declared, rest))
}

/// The constant `expr` is; `None` for an expression that computes its value.
pub(super) fn constant(expr: &Expr) -> Option<Const> {
    match expr.kind() {
        ExprKind::Const(value) => Some(*value),
        ExprKind::Read(_)
        | ExprKind::Convert(_)
        | ExprKind::Truncate { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. } => None,
    }
}

/// The place `expr` reads; `None` for an expression that computes its value.
pub(super) fn read(expr: &Expr) -> Option<&LoweredPlace> {
    match expr.kind() {
        ExprKind::Read(place) => Some(place),
        ExprKind::Const(_)
        | ExprKind::Convert(_)
        | ExprKind::Truncate { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. } => None,
    }
}

/// Whether a variable of type `ty` is the instance parameter of a body, the
/// non-null reference to a program or function block through which the body
/// reaches its own fields. A `REF_TO` is nullable, and lowering binds no
/// other non-null reference yet, so no other variable is one.
fn is_instance(lowering: &Lowering<'_>, ty: TypeId) -> bool {
    let types = lowering.types();
    types.reference(ty).is_some_and(|(target, nullability)| {
        nullability == Nullability::NonNull
            && matches!(
                types.kind(target),
                Some(TypeKind::Program { .. } | TypeKind::FunctionBlock { .. })
            )
    })
}

/// The operation type this backend computes a value of `ty` at. A `BOOL` is
/// a 32-bit signed integer and a reference a 64-bit unsigned one. `None` for
/// an interface value, which this backend does not compute yet.
pub(super) fn op_type(ty: ScalarType) -> Option<OpType> {
    let signedness = |signedness| match signedness {
        LoweredSignedness::Signed => Signedness::Signed,
        LoweredSignedness::Unsigned => Signedness::Unsigned,
    };
    match ty {
        ScalarType::Bool => Some((OpWidth::W32, Signedness::Signed)),
        ScalarType::I32(s) => Some((OpWidth::W32, signedness(s))),
        ScalarType::I64(s) => Some((OpWidth::W64, signedness(s))),
        ScalarType::F32 => Some((OpWidth::F32, Signedness::Signed)),
        ScalarType::F64 => Some((OpWidth::F64, Signedness::Signed)),
        ScalarType::Ref => Some((OpWidth::W64, Signedness::Unsigned)),
        ScalarType::Interface => None,
    }
}

#[cfg(test)]
mod tests;
