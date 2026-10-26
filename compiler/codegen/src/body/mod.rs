//! Compiles each body from the lowered program
//! (`specs/design/lowered-program.md`, "Delivery Constraints").
//!
//! Lowering lowers the whole library before codegen emits anything
//! (`ironplc_lowering::lower`), and codegen compiles each POU's body, its
//! prologue and then its statements, as the lowered program gives them. A
//! statement this backend does not emit yet is a P9999.
//!
//! A lowered statement finds its variables' storage in the layout built from
//! the lowered declarations, by the id of each variable and field
//! (REQ-LOW-codegen-123). The name lowering keeps on each is for debug
//! information and diagnostics (REQ-LOW-lowering-006).

use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{
    Arg, Callee, Const, Expr, ExprKind, FieldIdx, NarrowWidth, Nullability, Place as LoweredPlace,
    PouId, Projection, ScalarType, Signedness as LoweredSignedness, Stmt, StmtKind, TypeId,
    TypeKind, VarId,
};
use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

use crate::access::Named;
use crate::compile::{CompileContext, OpType, OpWidth, Signedness, UserFunctionInfo};
use crate::compile_subscript::subscript_check;
use crate::compile_user_call::passes_by_reference;
use crate::emit::Emitter;
use crate::layout::names::ScopeId;

mod aggregate;
mod call_args;
mod expr;
mod fb_call;
mod fn_call;
mod init;
mod instance_copy;
mod method;
mod place;
mod policy;
mod prologue;
mod slot_copy;
mod stmt;
mod string;

use call_args::passes;
pub(crate) use init::emit_init;
pub(crate) use place::slot_type;
pub(crate) use prologue::{emit_prologue, restarts};

/// Emits the statements of the current body after its prologue, from the
/// lowered program: each statement of the source, the lowered statements
/// that carry its span, after its line-map position. One this backend does
/// not emit yet is a P9999 at it.
pub(crate) fn emit_body(emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
    let lowering = ctx.lowering()?;
    let pou = current_pou(ctx).ok_or_else(Diagnostic::internal_error)?;
    let statements = lowering
        .statements(pou)
        .ok_or_else(Diagnostic::internal_error)?;
    // Lowering gives each statement a statement of the source lowers to
    // that statement's span (REQ-LOW-lowering-063), so each run of one span
    // is one statement of the source.
    for source in statements.chunk_by(|stmt, next| same_span(stmt.span(), next.span())) {
        let span = source[0].span();
        if !can_emit_all(lowering, ctx, source) {
            return Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                "Statement code generation does not support yet",
            )));
        }
        crate::compile_stmt::record_position(emitter, ctx, span);
        stmt::emit_stmts(emitter, ctx, lowering, source)?;
    }
    Ok(())
}

/// The P9999 for a store of a starting value this backend does not emit
/// yet.
fn unsupported_store(stmt: &Stmt) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(
        stmt.span().clone(),
        "Starting value code generation does not support yet",
    ))
}

/// Whether this backend emits every statement of `stmts`, as `stmt.rs` emits
/// them: a call statement, the stores of its outputs and, for a function
/// whose result is a structure, the copy of its result together
/// (`fn_call.rs`), and any other statement on its own.
fn can_emit_all(lowering: &Program, ctx: &CompileContext, stmts: &[Stmt]) -> bool {
    let mut rest = stmts;
    while let [first, ..] = rest {
        let taken = match fn_call::output_call(lowering, ctx, rest) {
            Some(call) if call.can_emit(lowering, ctx) => call.len(),
            Some(_) => return false,
            None if can_emit(lowering, ctx, first) => 1,
            None => return false,
        };
        rest = &rest[taken..];
    }
    true
}

/// Whether every place `stmt` names has storage this backend reaches as one
/// value, at the operation type the lowered program gives it. A statement
/// whose storage this backend does not reach yet is not supported yet
/// (P9999).
fn can_emit(lowering: &Program, ctx: &CompileContext, stmt: &Stmt) -> bool {
    let all = |stmts: &[Stmt]| can_emit_all(lowering, ctx, stmts);
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            reads(lowering, ctx, place, value.ty()) && expr_can_emit(lowering, ctx, value)
        }
        StmtKind::AssignStr { place, value } => {
            string::reaches_string(lowering, ctx, place)
                && string::str_can_emit(lowering, ctx, value)
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
        StmtKind::Copy { dst, src } => {
            aggregate::can_emit(lowering, ctx, dst, src)
                || instance_copy::instance_copy(lowering, ctx, dst, src).is_some()
                || slot_copy::slot_copy(lowering, ctx, dst, src).is_some()
        }
        StmtKind::FbCall { instance, block } => {
            fb_call::reaches_block(lowering, ctx, instance, *block)
        }
        // A call statement is emitted with the stores of its outputs and the
        // copy of its result (`can_emit_all`).
        StmtKind::Call { .. } => false,
        StmtKind::Exit(_) | StmtKind::Continue(_) | StmtKind::Return => true,
    }
}

/// Whether this backend emits `expr`.
fn expr_can_emit(lowering: &Program, ctx: &CompileContext, expr: &Expr) -> bool {
    if op_type(expr.ty()).is_none() {
        return false;
    }
    match expr.kind() {
        ExprKind::Const(_) | ExprKind::Null => true,
        ExprKind::Read(place) => reads(lowering, ctx, place, expr.ty()),
        // A reference to a variable, or to what a `VAR_IN_OUT` parameter
        // refers to.
        ExprKind::RefTo(place) => place::variable_name(lowering, ctx, place).is_some(),
        ExprKind::Truncate { value, .. }
        | ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Unary { value, .. } => {
            operation_can_emit(expr) && expr_can_emit(lowering, ctx, value)
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Compare { lhs, rhs, .. }
        | ExprKind::ShortCircuit { lhs, rhs, .. } => {
            expr_can_emit(lowering, ctx, lhs) && expr_can_emit(lowering, ctx, rhs)
        }
        ExprKind::Call {
            callee: Callee::User(pou),
            args,
        } => match user_function(ctx, *pou) {
            Some(function) => {
                passes(&function.params, args.iter()) && args_can_emit(lowering, ctx, args)
            }
            // A method's value: its receiver, then an argument for each input.
            None => method::method_site(lowering, ctx, *pou, args).is_some_and(|site| {
                site.has_return_value
                    && site.takes(args[1..].iter())
                    && args_can_emit(lowering, ctx, &args[1..])
            }),
        },
        ExprKind::Call {
            callee: Callee::Intrinsic(intrinsic),
            args,
        } => {
            expr::instruction_of(*intrinsic, args.len()).is_some()
                && args_can_emit(lowering, ctx, args)
        }
        ExprKind::InPlaceCall(call) => fn_call::in_place_can_emit(lowering, ctx, call),
    }
}

/// Whether this backend applies `expr`, an operation of one operand, to the
/// value of its operand.
fn operation_can_emit(expr: &Expr) -> bool {
    if op_type(expr.ty()).is_none() {
        return false;
    }
    match expr.kind() {
        ExprKind::Truncate { width, .. } => !matches!(width, NarrowWidth::B32),
        ExprKind::Convert(_) | ExprKind::Unary { .. } => true,
        // A checked conversion, which this backend traps on only for a
        // subscript, as a subscript check does.
        ExprKind::ConvertChecked(value) => op_type(value.ty())
            .zip(op_type(expr.ty()))
            .is_some_and(|(from, to)| subscript_check(from, to).is_some()),
        ExprKind::Const(_)
        | ExprKind::Read(_)
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => false,
    }
}

/// Whether this backend passes each of `args`.
fn args_can_emit<'s>(
    lowering: &Program,
    ctx: &CompileContext,
    args: impl IntoIterator<Item = &'s Arg>,
) -> bool {
    args.into_iter().all(|arg| match arg {
        Arg::Value(value) => expr_can_emit(lowering, ctx, value),
        Arg::Str(value) => string::str_can_emit(lowering, ctx, value),
        Arg::Ref(place) => place::variable_name(lowering, ctx, place)
            .is_some_and(|name| passes_by_reference(ctx, &name)),
        Arg::Copy(place) => aggregate::reaches_argument(lowering, ctx, place),
    })
}

/// The function `pou` as this backend's layout knows it, when it returns a
/// value or a string's buffer on the stack: not a structure. `None` for any
/// other function.
pub(super) fn user_function(ctx: &CompileContext, pou: PouId) -> Option<UserFunctionInfo> {
    callee_function(ctx, pou).filter(|function| function.return_struct_desc_index.is_none())
}

/// The function `pou` as this backend's layout knows it, whatever its
/// result: one whose result is a structure leaves the offset of the
/// structure's region on the stack. `None` for a method.
pub(super) fn callee_function(ctx: &CompileContext, pou: PouId) -> Option<UserFunctionInfo> {
    // A method is called by `METHOD_CALL` (`method.rs`).
    ctx.user_function(pou).cloned()
}

/// Whether `place` has storage this backend reaches as one value of type
/// `ty`.
fn reaches(lowering: &Program, ctx: &CompileContext, place: &LoweredPlace, ty: ScalarType) -> bool {
    subscripts_can_emit(lowering, ctx, place)
        && place::resolve(lowering, ctx, place, &SourceSpan::default())
            .is_ok_and(|storage| op_type(ty) == Some(storage.op_type()))
}

/// Whether this backend reads or stores `place` as one value of type `ty`:
/// in its storage, or through the reference its storage holds, as one value
/// of the type the place gives what that reference refers to.
fn reads(lowering: &Program, ctx: &CompileContext, place: &LoweredPlace, ty: ScalarType) -> bool {
    subscripts_can_emit(lowering, ctx, place)
        && match place::resolve_load(lowering, ctx, place, &SourceSpan::default()) {
            Ok(place::Load::Stored(storage)) => op_type(ty) == Some(storage.op_type()),
            Ok(load @ place::Load::Referenced(_)) => {
                op_type(ScalarType::Ref) == Some(load.storage().op_type())
                    && lowering
                        .types()
                        .operation_type(place.ty())
                        .is_some_and(|target| op_type(target) == op_type(ty))
            }
            Err(_) => false,
        }
}

/// Whether this backend emits every subscript on the way to `place`.
fn subscripts_can_emit(lowering: &Program, ctx: &CompileContext, place: &LoweredPlace) -> bool {
    place.path().iter().all(|projection| match projection {
        Projection::Index(subscripts) => subscripts
            .iter()
            .all(|subscript| expr_can_emit(lowering, ctx, subscript)),
        Projection::Field(_) | Projection::Deref => true,
    })
}

/// The variable `place` is the whole of, as this backend's layout keeps
/// it: a variable, or the slot a field of the current instance is copied
/// into. `None` for any other place, and for a variable the layout keeps no
/// slot for.
pub(super) fn stored(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Named> {
    let root = lowering.variables().get(place.root())?;
    if place.path().is_empty() {
        return stored_variable(lowering, ctx, place.root());
    }
    let (instance, field, rest) = instance_field(lowering, root.ty(), place.path())?;
    if !rest.is_empty() {
        return None;
    }
    stored_field(lowering, ctx, instance, field)
}

/// The variable `var`, when the layout keeps a slot for it.
pub(super) fn stored_variable(
    lowering: &Program,
    ctx: &CompileContext,
    var: VarId,
) -> Option<Named> {
    crate::layout::declarations::named(lowering, var).filter(|named| ctx.has_slot(named))
}

/// The field `field` of an instance of `instance`, when the layout keeps a
/// slot for it while the instance's body runs.
pub(super) fn stored_field(
    lowering: &Program,
    ctx: &CompileContext,
    instance: TypeId,
    field: FieldIdx,
) -> Option<Named> {
    let declared = lowering
        .types()
        .fields(instance)?
        .get(field.index() as usize)?;
    let named = crate::layout::declarations::named_field(instance, field, declared);
    ctx.has_slot(&named).then_some(named)
}

/// When `path` starts by reaching a field of an instance: of the current
/// instance through the instance parameter, of type `root`, or of a program
/// instance through the instance itself, as `init` reaches it, running in no
/// body. Gives the instance's type, that field, and the projections after
/// it; `None` for any other path.
pub(super) fn instance_field<'p>(
    lowering: &Program,
    root: TypeId,
    path: &'p [Projection],
) -> Option<(TypeId, FieldIdx, &'p [Projection])> {
    match path {
        [Projection::Deref, Projection::Field(field), rest @ ..] if is_instance(lowering, root) => {
            let (instance, _) = lowering.types().reference(root)?;
            Some((instance, *field, rest))
        }
        [Projection::Field(field), rest @ ..]
            if matches!(lowering.types().kind(root), Some(TypeKind::Program { .. })) =>
        {
            Some((root, *field, rest))
        }
        _ => None,
    }
}

/// The POU whose body is compiled in the current scope; `None` outside any
/// body.
pub(super) fn current_pou(ctx: &CompileContext) -> Option<PouId> {
    match ctx.scope {
        ScopeId::Pou(pou) => Some(pou),
        ScopeId::Global => None,
    }
}

/// The name the lowered program gives the POU whose body is compiled in
/// `scope`, as written.
pub(crate) fn pou_name(ctx: &CompileContext, scope: ScopeId) -> Result<String, Diagnostic> {
    let pou = match scope {
        ScopeId::Pou(pou) => Some(pou),
        ScopeId::Global => None,
    };
    pou.and_then(|pou| ctx.lowering().ok()?.pous().name(pou))
        .map(ToString::to_string)
        .ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                SourceSpan::default(),
                "A body the lowered program does not name",
            ))
        })
}

/// Whether `pou` is the body being compiled.
pub(super) fn is_current_body(ctx: &CompileContext, pou: PouId) -> bool {
    ctx.scope == ScopeId::Pou(pou)
}

/// Whether two spans are the same text. `SourceSpan`'s own equality holds
/// for any two spans.
pub(super) fn same_span(span: &SourceSpan, other: &SourceSpan) -> bool {
    span.start == other.start && span.end == other.end && span.file_id == other.file_id
}

/// The constant `expr` is; `None` for an expression that computes its value.
pub(super) fn constant(expr: &Expr) -> Option<Const> {
    match expr.kind() {
        ExprKind::Const(value) => Some(*value),
        ExprKind::Read(_)
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
        | ExprKind::Null => None,
    }
}

/// The operand of `expr` when it is an operation of one operand: a
/// conversion, a narrowing or a unary operation.
pub(super) fn operand(expr: &Expr) -> Option<&Expr> {
    match expr.kind() {
        ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Truncate { value, .. }
        | ExprKind::Unary { value, .. } => Some(value),
        ExprKind::Const(_)
        | ExprKind::Read(_)
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => None,
    }
}

/// The place `expr` reads; `None` for an expression that computes its value.
pub(super) fn read(expr: &Expr) -> Option<&LoweredPlace> {
    match expr.kind() {
        ExprKind::Read(place) => Some(place),
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
        | ExprKind::Null => None,
    }
}

/// Whether a variable of type `ty` is the instance parameter of a body, the
/// non-null reference to a program or function block through which the body
/// reaches its own fields. A `REF_TO` is nullable, and lowering binds no
/// other non-null reference yet, so no other variable is one.
fn is_instance(lowering: &Program, ty: TypeId) -> bool {
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
