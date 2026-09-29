//! Loop compilation (FOR, WHILE, REPEAT) for IEC 61131-3 code generation.
//!
//! Separated from `compile_stmt.rs` to keep module sizes within the
//! 1000-line guideline.

use ironplc_dsl::common::ConstantKind;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, StmtKind, UnaryOp};

use super::compile::{CompileContext, OpType, OpWidth, Signedness, VarTypeInfo};
use super::compile_expr::{
    compile_expr, condition_op_type, emit_add, emit_classified_cmp_br, emit_ge, emit_le,
    emit_load_var, emit_store_var, emit_truncation, signed_integer_to_i64, try_classify_cmp,
    ClassifiedCmp,
};
use super::compile_stmt::compile_stmts;
use crate::emit::{self, Emitter};
use ironplc_container::opcode;

/// The labels of a loop that its body can jump to.
#[derive(Clone, Copy)]
pub(crate) struct LoopLabels {
    /// Where `EXIT` goes: the first instruction after the loop.
    pub(crate) exit: emit::Label,
    /// Where `CONTINUE` goes: where the loop goes on with its next iteration.
    pub(crate) next: emit::Label,
    /// Whether a `CONTINUE` jumps to `next`. The loop binds `next` only
    /// then, because binding a label ends the peephole window.
    pub(crate) next_used: bool,
}

/// Compiles the body of a loop whose `EXIT` goes to `exit`, and returns the
/// label `CONTINUE` goes to when the body has a `CONTINUE`, for the caller to
/// bind where the next iteration starts.
fn compile_loop_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &[StmtKind],
    exit: emit::Label,
) -> Result<Option<emit::Label>, Diagnostic> {
    ctx.loop_labels.push(LoopLabels {
        exit,
        next: emitter.create_label(),
        next_used: false,
    });
    let result = compile_stmts(emitter, ctx, body);
    let labels = ctx.loop_labels.pop().expect("pushed above");
    result?;
    Ok(labels.next_used.then_some(labels.next))
}

/// Binds the label `CONTINUE` goes to, when the body has a `CONTINUE`. The
/// `NEXT` label of the loop diagrams below is bound only then.
fn bind_next(emitter: &mut Emitter, next: Option<emit::Label>) {
    if let Some(label) = next {
        emitter.bind_label(label);
    }
}

/// Compiles a WHILE statement.
///
/// ```text
/// LOOP:
///   compile(condition)
///   JMP_IF_NOT → END
///   compile(body)
/// NEXT:                                    // CONTINUE jumps here
///   JMP → LOOP
/// END:
/// ```
pub(crate) fn compile_while(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    while_stmt: &ironplc_dsl::textual::While,
) -> Result<(), Diagnostic> {
    // Fast path: when the condition is a fusable `var <cmp> const`, emit a
    // do-while shape so the per-iteration overhead collapses to a single
    // `CMP_BR_*` (zero-trip head + back-edge tail).
    //
    // ```text
    //   CMP_BR_<t>  NEG(cmp), var, k, END     ; zero-trip: exit if !cond
    // BODY:
    //   ...body...
    // NEXT:                                   ; CONTINUE jumps here
    //   CMP_BR_<t>  cmp,      var, k, BODY    ; back-edge: continue if cond
    // END:
    // ```
    if let Some(classified) = try_classify_cmp(ctx, &while_stmt.condition) {
        let body_label = emitter.create_label();
        let end_label = emitter.create_label();
        emit_classified_cmp_br(emitter, classified, false, end_label);
        emitter.bind_label(body_label);
        let next = compile_loop_body(emitter, ctx, &while_stmt.body, end_label)?;
        bind_next(emitter, next);
        emit_classified_cmp_br(emitter, classified, true, body_label);
        emitter.bind_label(end_label);
        return Ok(());
    }

    // Fallback: complex condition. Today's emission, unchanged.
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    emitter.bind_label(loop_label);
    let cond_type = condition_op_type(ctx, &while_stmt.condition)?;
    compile_expr(emitter, ctx, &while_stmt.condition, cond_type)?;
    emitter.emit_jmp_if_not(end_label);
    let next = compile_loop_body(emitter, ctx, &while_stmt.body, end_label)?;
    bind_next(emitter, next);
    emitter.emit_jmp(loop_label);
    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles a REPEAT statement.
///
/// ```text
/// LOOP:
///   compile(body)
/// NEXT:                                    // CONTINUE jumps here
///   compile(condition)
///   JMP_IF_NOT → LOOP
/// ```
pub(crate) fn compile_repeat(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    repeat_stmt: &ironplc_dsl::textual::Repeat,
) -> Result<(), Diagnostic> {
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    // Fast path: when `until` is a fusable `var <cmp> const`, replace the
    // tail `compile(cond) + JMP_IF_NOT LOOP` (4 dispatches) with a single
    // `CMP_BR_*` using the negated comparison (continue while NOT cond).
    let classified_until = try_classify_cmp(ctx, &repeat_stmt.until);

    emitter.bind_label(loop_label);
    let next = compile_loop_body(emitter, ctx, &repeat_stmt.body, end_label)?;
    bind_next(emitter, next);
    if let Some(classified) = classified_until {
        emit_classified_cmp_br(emitter, classified, false, loop_label);
    } else {
        let cond_type = condition_op_type(ctx, &repeat_stmt.until)?;
        compile_expr(emitter, ctx, &repeat_stmt.until, cond_type)?;
        emitter.emit_jmp_if_not(loop_label);
    }
    emitter.bind_label(end_label);

    Ok(())
}

/// Whether a compile-time constant step is positive or negative.
#[derive(Clone, Copy)]
enum StepSign {
    Positive,
    Negative,
}

/// Inspects an expression and returns its sign if it is a compile-time constant
/// integer literal (positive or negative). Returns `None` for non-constant
/// expressions.
fn try_constant_sign(expr: &Expr) -> Option<StepSign> {
    match try_constant_i64(expr)? {
        v if v > 0 => Some(StepSign::Positive),
        v if v < 0 => Some(StepSign::Negative),
        _ => None,
    }
}

/// Returns the `i64` value of an expression if it is a compile-time constant
/// integer literal (positive, negative, or unary-negated). Returns `None`
/// for non-constant expressions or values outside the `i64` range.
fn try_constant_i64(expr: &Expr) -> Option<i64> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            signed_integer_to_i64(&lit.value).ok()
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => match &unary.term.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => signed_integer_to_i64(&lit.value)
                .ok()
                .and_then(i64::checked_neg),
            _ => None,
        },
        _ => None,
    }
}

/// Returns the `(min, max)` value range for a narrow integer type, or `None`
/// for 32- and 64-bit types where `emit_truncation` is already a no-op.
///
/// The bounds come from `value_range`, which is where the range a type can
/// hold is stated; "narrow" is this caller's own concern.
fn narrow_type_range(type_info: VarTypeInfo) -> Option<(i64, i64)> {
    match type_info.storage_bits {
        bits @ (8 | 16) => {
            let (minimum, maximum) = ironplc_analyzer::value_range::for_integer(
                u32::from(bits),
                type_info.signedness == Signedness::Signed,
            );
            Some((minimum as i64, maximum as i64))
        }
        _ => None,
    }
}

/// Returns `true` when both `TRUNC` sites in a FOR loop can safely be elided
/// because every value the control variable will hold (initial, body-visible,
/// and post-final-increment) is provably within the declared narrow type's
/// range. Conservative: any non-constant bound, or any boundary that could
/// trigger wrap-around, returns `false` and preserves the existing TRUNC.
fn for_loop_trunc_can_be_elided(
    from: &Expr,
    to: &Expr,
    step: Option<&Expr>,
    type_info: VarTypeInfo,
) -> bool {
    let Some((t_min, t_max)) = narrow_type_range(type_info) else {
        // Wide type: emit_truncation is already a no-op; flag is irrelevant.
        return true;
    };
    let Some(from_v) = try_constant_i64(from) else {
        return false;
    };
    let Some(to_v) = try_constant_i64(to) else {
        return false;
    };
    let step_v = match step {
        None => 1,
        Some(expr) => match try_constant_i64(expr) {
            Some(v) => v,
            None => return false,
        },
    };
    if from_v < t_min || from_v > t_max {
        return false;
    }
    match step_v {
        s if s > 0 => {
            // Body sees values in [from, to]; post-final stored value is to + step.
            if to_v > t_max {
                return false;
            }
            match to_v.checked_add(s) {
                Some(post) => post <= t_max,
                None => false,
            }
        }
        s if s < 0 => {
            if to_v < t_min {
                return false;
            }
            match to_v.checked_add(s) {
                Some(post) => post >= t_min,
                None => false,
            }
        }
        _ => false,
    }
}

/// Attempts to fuse a FOR loop's head test into a single `CMP_BR_*`
/// instruction. Returns `Some(ClassifiedCmp)` representing the
/// continuation predicate (`i <= to` for positive step, `i >= to` for
/// negative step) when the control variable is a 32- or 64-bit signed
/// integer and `to` is a constant integer literal that fits the
/// variable's width. Returns `None` otherwise so the caller falls back to
/// the unfused emission.
fn try_classify_for_head(
    ctx: &mut CompileContext,
    var_index: ironplc_container::VarIndex,
    op_type: OpType,
    to: &Expr,
    step_sign: StepSign,
) -> Option<ClassifiedCmp> {
    if op_type.1 != Signedness::Signed {
        return None;
    }
    let to_value = try_constant_i64(to)?;
    let cmp_op_byte = match step_sign {
        StepSign::Positive => opcode::cmp_op::LE_S,
        StepSign::Negative => opcode::cmp_op::GE_S,
    };
    match op_type.0 {
        OpWidth::W32 => {
            let v32 = i32::try_from(to_value).ok()?;
            let const_idx = ctx.add_i32_constant(v32);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W32,
            })
        }
        OpWidth::W64 => {
            let const_idx = ctx.add_i64_constant(to_value);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W64,
            })
        }
        OpWidth::F32 | OpWidth::F64 => None,
    }
}

/// Compiles a FOR statement.
///
/// ```text
///   compile(from)
///   STORE_VAR control
/// LOOP:
///   LOAD_VAR control
///   compile(to)
///   LE_I32 (or GE_I32 for negative step)  // continuation predicate
///   JMP_IF_NOT → END                       // exit when continuation fails
///   compile(body)
/// NEXT:                                    // CONTINUE jumps here
///   LOAD_VAR control
///   compile(step)  // default: LOAD_CONST 1
///   ADD_I32
///   STORE_VAR control
///   JMP → LOOP
/// END:
/// ```
///
/// The continuation predicate is `i <= to` (positive step) / `i >= to`
/// (negative step). Inverting the predicate lets the body fall through
/// from the conditional branch, eliminating one `JMP` dispatch per
/// iteration.
pub(crate) fn compile_for(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    for_stmt: &ironplc_dsl::textual::For,
) -> Result<(), Diagnostic> {
    let var_index = ctx.var_index(&for_stmt.control)?;
    let op_type = ctx.var_op_type(&for_stmt.control);
    let type_info = ctx.var_type_info(&for_stmt.control);

    // Determine step sign.
    let step_sign = match &for_stmt.step {
        None => StepSign::Positive,
        Some(step_expr) => match try_constant_sign(step_expr) {
            Some(sign) => sign,
            None => {
                return Err(Diagnostic::not_implemented(Label::span(
                    for_stmt.control.span(),
                    "FOR loop step must be a constant expression",
                )))
            }
        },
    };

    // Decide whether the per-loop TRUNC can be elided based on a local interval
    // check over the constant bounds. See `specs/design/vm-performance.md`
    // §13 "Layer 1: Abstract Interpretation with Richer Domains".
    let elide_trunc = match type_info {
        Some(ti) => {
            for_loop_trunc_can_be_elided(&for_stmt.from, &for_stmt.to, for_stmt.step.as_ref(), ti)
        }
        None => true,
    };

    // Initialize: compile(from), STORE_VAR control
    compile_expr(emitter, ctx, &for_stmt.from, op_type)?;
    if let Some(ti) = type_info {
        if !elide_trunc {
            emit_truncation(emitter, ti);
        }
    }
    emit_store_var(emitter, var_index, op_type);

    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    // LOOP: check continuation condition; exit straight to END when it
    // fails so the body falls through without an extra JMP dispatch.
    emitter.bind_label(loop_label);
    if let Some(classified) =
        try_classify_for_head(ctx, var_index, op_type, &for_stmt.to, step_sign)
    {
        // Fused path: one CMP_BR replacing LOAD_VAR + LOAD_CONST + LE/GE + JMP_IF_NOT.
        // Branch to END when the continuation predicate is FALSE.
        emit_classified_cmp_br(emitter, classified, false, end_label);
    } else {
        emit_load_var(emitter, var_index, op_type);
        compile_expr(emitter, ctx, &for_stmt.to, op_type)?;
        match step_sign {
            StepSign::Positive => emit_le(emitter, op_type),
            StepSign::Negative => emit_ge(emitter, op_type),
        }
        emitter.emit_jmp_if_not(end_label);
    }

    // BODY:
    let next = compile_loop_body(emitter, ctx, &for_stmt.body, end_label)?;

    // NEXT: the target of CONTINUE.
    bind_next(emitter, next);

    // Increment: LOAD_VAR control, compile(step), ADD, truncate, STORE_VAR control
    emit_load_var(emitter, var_index, op_type);
    match &for_stmt.step {
        Some(step_expr) => compile_expr(emitter, ctx, step_expr, op_type)?,
        None => match op_type.0 {
            OpWidth::W32 => {
                let one_index = ctx.add_i32_constant(1);
                emitter.emit_load_const_i32(one_index);
            }
            OpWidth::W64 => {
                let one_index = ctx.add_i64_constant(1);
                emitter.emit_load_const_i64(one_index);
            }
            OpWidth::F32 => {
                let one_index = ctx.add_f32_constant(1.0);
                emitter.emit_load_const_f32(one_index);
            }
            OpWidth::F64 => {
                let one_index = ctx.add_f64_constant(1.0);
                emitter.emit_load_const_f64(one_index);
            }
        },
    }
    emit_add(emitter, op_type);
    if let Some(ti) = type_info {
        if !elide_trunc {
            emit_truncation(emitter, ti);
        }
    }
    emit_store_var(emitter, var_index, op_type);
    emitter.emit_jmp(loop_label);

    // END:
    emitter.bind_label(end_label);

    Ok(())
}
