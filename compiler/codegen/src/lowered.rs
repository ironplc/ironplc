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

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{NamedVariable, StmtKind as AstStmtKind, SymbolicVariableKind};
use ironplc_ir::lowered::{
    BinaryOp, CompareOp, Const, Expr, ExprKind, NarrowWidth, Place as LoweredPlace, Projection,
    ScalarType, ShortCircuitOp, Signedness as LoweredSignedness, Stmt, StmtKind, UnaryOp,
};
use ironplc_lowering::{Lowering, StatementLowering};

use crate::compile::{CompileContext, OpType, OpWidth, Signedness};
use crate::compile_arith::convert;
use crate::compile_expr::{
    emit_add, emit_div, emit_eq, emit_ge, emit_gt, emit_le, emit_lt, emit_mod, emit_mul, emit_ne,
    emit_neg, emit_sub,
};
use crate::compile_place::Place;
use crate::emit::Emitter;

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
            emit_stmt(emitter, ctx, &route.lowering, &lowered)?;
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
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            reaches(lowering, ctx, place, value.ty()) && expr_can_emit(lowering, ctx, value)
        }
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
    let Some(name) = storage_name(lowering, place) else {
        return false;
    };
    let base = named(name);
    match Place::resolve(ctx, &base) {
        Ok(storage) => op_type(ty) == Some(storage.op_type()),
        Err(_) => false,
    }
}

/// The name this backend's layout knows `place` by: the variable's name, or
/// the name of a field of the current instance, which this backend copies
/// into a slot of that name. `None` for any other place.
fn storage_name(lowering: &Lowering<'_>, place: &LoweredPlace) -> Option<Id> {
    let root = lowering.variables().get(place.root())?;
    match place.path() {
        [] => Some(Id::from(&root.name().to_string())),
        [Projection::Deref, Projection::Field(field)] => {
            let (instance, _) = lowering.types().reference(root.ty())?;
            let fields = lowering.types().fields(instance)?;
            let field = fields.get(field.index() as usize)?;
            Some(Id::from(&field.name.to_string()))
        }
        _ => None,
    }
}

fn named(name: Id) -> SymbolicVariableKind {
    SymbolicVariableKind::Named(NamedVariable { name })
}

/// The operation type this backend computes a value of `ty` at. A `BOOL` is
/// a 32-bit signed integer and a reference a 64-bit unsigned one. `None` for
/// an interface value, which this backend does not compute yet.
fn op_type(ty: ScalarType) -> Option<OpType> {
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

fn emit_stmt(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    stmt: &Stmt,
) -> Result<(), Diagnostic> {
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            emit_expr(emitter, ctx, lowering, value)?;
            let storage = named(storage(lowering, place, stmt.span())?);
            Place::resolve(ctx, &storage)?.emit_store(emitter, ctx)
        }
    }
}

/// The name of `place`, which [`can_emit`] has already found.
fn storage(
    lowering: &Lowering<'_>,
    place: &LoweredPlace,
    span: &ironplc_dsl::core::SourceSpan,
) -> Result<Id, Diagnostic> {
    storage_name(lowering, place).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(span.clone(), "A lowered place has no storage"))
    })
}

fn emit_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    expr: &Expr,
) -> Result<(), Diagnostic> {
    let op = op_type(expr.ty()).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            expr.span().clone(),
            "A lowered expression has no operation type",
        ))
    })?;
    match expr.kind() {
        ExprKind::Const(constant) => emit_constant(emitter, ctx, *constant),
        ExprKind::Read(place) => {
            let storage = named(storage(lowering, place, expr.span())?);
            Place::resolve(ctx, &storage)?.emit_load(emitter, ctx)?;
        }
        ExprKind::Convert(value) => {
            emit_expr(emitter, ctx, lowering, value)?;
            let from = op_type(value.ty()).unwrap_or(op);
            convert(emitter, from, op)?;
        }
        ExprKind::Truncate { width, value } => {
            emit_expr(emitter, ctx, lowering, value)?;
            match (width, op.1) {
                (NarrowWidth::B8, Signedness::Signed) => emitter.emit_trunc_i8(),
                (NarrowWidth::B8, Signedness::Unsigned) => emitter.emit_trunc_u8(),
                (NarrowWidth::B16, Signedness::Signed) => emitter.emit_trunc_i16(),
                (NarrowWidth::B16, Signedness::Unsigned) => emitter.emit_trunc_u16(),
                (NarrowWidth::B32, _) => {
                    return Err(Diagnostic::internal_error_at(Label::span(
                        expr.span().clone(),
                        "A truncation to 32 bits",
                    )))
                }
            }
        }
        ExprKind::Unary { op: unary, value } => {
            emit_expr(emitter, ctx, lowering, value)?;
            match unary {
                UnaryOp::Neg => emit_neg(emitter, op),
                UnaryOp::BitNot => match op.0 {
                    OpWidth::W64 => emitter.emit_bit_not_64(),
                    OpWidth::W32 | OpWidth::F32 | OpWidth::F64 => emitter.emit_bit_not_32(),
                },
                UnaryOp::BoolNot => emitter.emit_bool_not(),
            }
        }
        ExprKind::Binary {
            op: binary,
            lhs,
            rhs,
        } => {
            emit_expr(emitter, ctx, lowering, lhs)?;
            emit_expr(emitter, ctx, lowering, rhs)?;
            let wide = op.0 == OpWidth::W64;
            match binary {
                BinaryOp::Add => emit_add(emitter, op),
                BinaryOp::Sub => emit_sub(emitter, op),
                BinaryOp::Mul => emit_mul(emitter, op),
                BinaryOp::Div => emit_div(emitter, op),
                BinaryOp::Mod => emit_mod(emitter, op),
                BinaryOp::BitAnd if wide => emitter.emit_bit_and_64(),
                BinaryOp::BitAnd => emitter.emit_bit_and_32(),
                BinaryOp::BitOr if wide => emitter.emit_bit_or_64(),
                BinaryOp::BitOr => emitter.emit_bit_or_32(),
                BinaryOp::BitXor if wide => emitter.emit_bit_xor_64(),
                BinaryOp::BitXor => emitter.emit_bit_xor_32(),
                BinaryOp::BoolAnd => emitter.emit_bool_and(),
                BinaryOp::BoolOr => emitter.emit_bool_or(),
                BinaryOp::BoolXor => emitter.emit_bool_xor(),
            }
        }
        ExprKind::Compare {
            op: compare,
            lhs,
            rhs,
        } => {
            emit_expr(emitter, ctx, lowering, lhs)?;
            emit_expr(emitter, ctx, lowering, rhs)?;
            let operands = op_type(lhs.ty()).unwrap_or(op);
            match compare {
                CompareOp::Eq => emit_eq(emitter, operands),
                CompareOp::Ne => emit_ne(emitter, operands),
                CompareOp::Lt => emit_lt(emitter, operands),
                CompareOp::Le => emit_le(emitter, operands),
                CompareOp::Gt => emit_gt(emitter, operands),
                CompareOp::Ge => emit_ge(emitter, operands),
            }
        }
        ExprKind::ShortCircuit {
            op: short_circuit,
            lhs,
            rhs,
        } => {
            // As `compile_short_circuit` emits it: the right operand runs only
            // when the left one does not decide the result.
            let alternative = emitter.create_label();
            let end = emitter.create_label();
            emit_expr(emitter, ctx, lowering, lhs)?;
            emitter.emit_jmp_if_not(alternative);
            let depth_before_arms = emitter.stack_depth();
            match short_circuit {
                ShortCircuitOp::AndThen => emit_expr(emitter, ctx, lowering, rhs)?,
                ShortCircuitOp::OrElse => emitter.emit_load_true(),
            }
            emitter.emit_jmp(end);
            emitter.bind_label(alternative);
            emitter.reset_stack_depth(depth_before_arms);
            match short_circuit {
                ShortCircuitOp::AndThen => emitter.emit_load_false(),
                ShortCircuitOp::OrElse => emit_expr(emitter, ctx, lowering, rhs)?,
            }
            emitter.bind_label(end);
        }
    }
    Ok(())
}

fn emit_constant(emitter: &mut Emitter, ctx: &mut CompileContext, constant: Const) {
    match constant {
        Const::Bool(true) => emitter.emit_load_true(),
        Const::Bool(false) => emitter.emit_load_false(),
        Const::I32(value) => {
            let index = ctx.add_i32_constant(value);
            emitter.emit_load_const_i32(index);
        }
        Const::I64(value) => {
            let index = ctx.add_i64_constant(value);
            emitter.emit_load_const_i64(index);
        }
        Const::F32(value) => {
            let index = ctx.add_f32_constant(value);
            emitter.emit_load_const_f32(index);
        }
        Const::F64(value) => {
            let index = ctx.add_f64_constant(value);
            emitter.emit_load_const_f64(index);
        }
    }
}
