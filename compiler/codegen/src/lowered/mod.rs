//! Compiles a statement from the lowered program rather than from the AST
//! (`specs/design/lowered-program.md`, "Delivery Constraints").
//!
//! While both routes exist, lowering lowers the whole library once, before
//! codegen emits anything ([`ironplc_lowering::Lowering::lower_program`]),
//! and codegen reads what each top-level statement of a body lowered to, by
//! the body's POU and the statement's index. When lowering supports the
//! statement and everything inside it, the statement is emitted from the
//! lowered program; otherwise it is compiled from the AST, as before. Both
//! routes emit into the same body through the same emitter.
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
    Arg, Callee, Const, Expr, ExprKind, FieldIdx, NarrowWidth, Nullability, Place as LoweredPlace,
    PouId, Projection, ScalarType, Signedness as LoweredSignedness, Stmt, StmtKind, TypeId,
    TypeKind, VarId,
};
use ironplc_lowering::{PartialProgram, StatementLowering};

use crate::compile::{CompileContext, OpType, OpWidth, Signedness, UserFunctionInfo};
use crate::compile_subscript::subscript_check;
use crate::compile_user_call::passes_by_reference;
use crate::emit::Emitter;
use crate::layout::ids::LaidOut;

mod aggregate;
mod call_args;
mod expr;
mod fb_call;
mod fn_call;
mod instance_copy;
mod method;
mod place;
mod policy;
mod slot_copy;
mod stmt;
mod string;

use call_args::passes;
pub(crate) use place::type_info;
pub(crate) use policy::parse_policy;

/// Which route codegen compiles a statement by.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Every statement from the AST.
    Ast,
    /// The lowered statement, where lowering supports it, and the AST for
    /// every other.
    #[default]
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

/// The program lowering made for a compilation, and how many statements
/// took each route so far.
pub(crate) struct LoweredRoute {
    lowering: PartialProgram,
    require: bool,
    /// Whether the layout built from the lowered declarations is checked
    /// equal to the one built from the AST.
    check_layout: bool,
    pub(crate) counts: RouteCounts,
}

impl LoweredRoute {
    /// The program lowering made.
    pub(crate) fn lowering(&self) -> &PartialProgram {
        &self.lowering
    }

    /// Whether the layout built from the lowered declarations is checked
    /// equal to the one built from the AST.
    pub(crate) fn check_layout(&self) -> bool {
        self.check_layout
    }

    /// The lowered route for `route`, with the program `lower` lowers, or
    /// `None` when every statement is compiled from the AST, which lowers
    /// nothing. A P9998 lowering reports fails the compilation.
    pub(crate) fn new(
        route: Route,
        check_layout: bool,
        lower: impl FnOnce() -> Result<PartialProgram, Vec<Diagnostic>>,
    ) -> Result<Option<Self>, Diagnostic> {
        let require = match route {
            Route::Ast => return Ok(None),
            Route::PreferLowered => false,
            Route::RequireLowered => true,
        };
        let lowering = lower().map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .next()
                .unwrap_or_else(Diagnostic::internal_error)
        })?;
        Ok(Some(Self {
            lowering,
            require,
            check_layout,
            counts: RouteCounts::default(),
        }))
    }
}

/// Compiles `stmt`, the top-level statement at `index` of the current body,
/// from the lowered program, if the route asks for it and lowering supports
/// it. Returns `false` when the statement is left to the AST.
pub(crate) fn try_compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    index: usize,
    stmt: &AstStmtKind,
) -> Result<bool, Diagnostic> {
    let Some(mut route) = ctx.lowered_route.take() else {
        return Ok(false);
    };
    let compiled = compile_by_route(&mut route, emitter, ctx, index, stmt);
    ctx.lowered_route = Some(route);
    compiled
}

fn compile_by_route(
    route: &mut LoweredRoute,
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    index: usize,
    stmt: &AstStmtKind,
) -> Result<bool, Diagnostic> {
    let lowering = &route.lowering;
    let lowered = match current_pou(ctx).and_then(|pou| lowering.statement(pou, index)) {
        Some(StatementLowering::Lowered(lowered)) => Some(lowered),
        Some(StatementLowering::Unsupported) | None => None,
    };
    match lowered {
        Some(lowered) if can_emit_all(lowering, ctx, lowered) => {
            crate::compile_stmt::record_statement_position(emitter, ctx, stmt);
            stmt::emit_stmts(emitter, ctx, lowering, lowered)?;
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

/// Whether this backend emits every statement of `stmts`, as `stmt.rs` emits
/// them: a call statement, the stores of its outputs and, for a function
/// whose result is a structure, the copy of its result together
/// (`fn_call.rs`), and any other statement on its own.
fn can_emit_all(lowering: &PartialProgram, ctx: &CompileContext, stmts: &[Stmt]) -> bool {
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
/// whose storage this backend does not reach yet is compiled from the AST.
fn can_emit(lowering: &PartialProgram, ctx: &CompileContext, stmt: &Stmt) -> bool {
    let all = |stmts: &[Stmt]| can_emit_all(lowering, ctx, stmts);
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            reaches(lowering, ctx, place, value.ty()) && expr_can_emit(lowering, ctx, value)
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
fn expr_can_emit(lowering: &PartialProgram, ctx: &CompileContext, expr: &Expr) -> bool {
    if op_type(expr.ty()).is_none() {
        return false;
    }
    match expr.kind() {
        ExprKind::Const(_) | ExprKind::Null => true,
        ExprKind::Read(place) => reads(lowering, ctx, place, expr.ty()),
        // A reference to a variable, or to what a `VAR_IN_OUT` parameter
        // refers to, as the AST route makes one.
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
    lowering: &PartialProgram,
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
    match ctx.lowered_ids.pou(pou)? {
        LaidOut::Function(name) => ctx.user_function(name).cloned(),
        LaidOut::Program | LaidOut::Block(_) | LaidOut::Method { .. } => None,
    }
}

/// Whether `place` has storage this backend reaches as one value of type
/// `ty`.
fn reaches(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
    ty: ScalarType,
) -> bool {
    subscripts_can_emit(lowering, ctx, place)
        && place::resolve(lowering, ctx, place, &SourceSpan::default())
            .is_ok_and(|storage| op_type(ty) == Some(storage.op_type()))
}

/// Whether this backend reads `place` as one value of type `ty`: from its
/// storage, or through the reference its storage holds, as one value of the
/// type the place gives what that reference refers to.
fn reads(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
    ty: ScalarType,
) -> bool {
    subscripts_can_emit(lowering, ctx, place)
        && match place::resolve_load(lowering, ctx, place, &SourceSpan::default()) {
            Ok(place::Load::Stored(storage)) => op_type(ty) == Some(storage.op_type()),
            Ok(place::Load::Referenced(storage)) => {
                op_type(ScalarType::Ref) == Some(storage.op_type())
                    && lowering
                        .types()
                        .operation_type(place.ty())
                        .is_some_and(|target| op_type(target) == op_type(ty))
            }
            Err(_) => false,
        }
}

/// Whether this backend emits every subscript on the way to `place`.
fn subscripts_can_emit(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> bool {
    place.path().iter().all(|projection| match projection {
        Projection::Index(subscripts) => subscripts
            .iter()
            .all(|subscript| expr_can_emit(lowering, ctx, subscript)),
        Projection::Field(_) | Projection::Deref => true,
    })
}

/// The name this backend's layout knows a variable `place` by: the
/// variable's name, or the name of the slot a field of the current instance
/// is copied into. `None` for any other place, and for a variable the
/// current body does not see by that name.
pub(super) fn storage_name(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Id> {
    let root = lowering.variables().get(place.root())?;
    if place.path().is_empty() {
        return seen_name(ctx, place.root()).cloned();
    }
    let (instance, field, rest) = instance_field(lowering, root.ty(), place.path())?;
    if !rest.is_empty() {
        return None;
    }
    ctx.lowered_ids.field(instance, field).cloned()
}

/// The name the layout knows the variable `var` by, when the current body
/// sees it by that name; `None` for a variable the layout keeps no slot for,
/// or one the current body does not see.
pub(super) fn seen_name(ctx: &CompileContext, var: VarId) -> Option<&Id> {
    let (scope, name) = ctx.lowered_ids.variable(var)?;
    ctx.sees(*scope, name).then_some(name)
}

/// When `path` starts by reaching a field of the current instance through the
/// instance parameter, of type `root`: the instance's type, that field, and
/// the projections after it. `None` for any other path.
pub(super) fn instance_field<'p>(
    lowering: &PartialProgram,
    root: TypeId,
    path: &'p [Projection],
) -> Option<(TypeId, FieldIdx, &'p [Projection])> {
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
    let (instance, _) = lowering.types().reference(root)?;
    Some((instance, *field, rest))
}

/// The POU whose body is compiled in the current scope; `None` for a body
/// lowering did not lower.
pub(super) fn current_pou(ctx: &CompileContext) -> Option<PouId> {
    ctx.lowered_ids.pou_in(ctx.scope)
}

/// Whether `pou` is the body being compiled.
pub(super) fn is_current_body(ctx: &CompileContext, pou: PouId) -> bool {
    ctx.lowered_ids.scope(pou) == Some(ctx.scope)
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
fn is_instance(lowering: &PartialProgram, ty: TypeId) -> bool {
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
