//! Short-circuit boolean operator code generation (`AND_THEN` / `OR_ELSE`).
//!
//! `AND_THEN` and `OR_ELSE` differ from `AND` and `OR` in one respect that is
//! externally visible: the right operand is not evaluated when the left one
//! already decides the answer. That is the reason the operators exist — their
//! motivating use is guarding a dereference, where evaluating the right operand
//! eagerly is the crash the guard was written to prevent:
//!
//! ```text
//! IF (ptr <> 0 AND_THEN ptr^ = 99) THEN
//! ```
//!
//! So both lower to a conditional branch around the right operand rather than
//! to a `BIT_AND` / `BIT_OR` over two eagerly-evaluated values.
//!
//! See `specs/design/beckhoff-twincat-dialect.md` §3.4.

use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareExpr, CompareOp};

use super::compile::CompileContext;
use super::compile_expr::{compile_expr, expr_is_bool, op_type};
use crate::emit::Emitter;

/// A boolean operator that may skip evaluating its right operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortCircuitOp {
    /// `AND_THEN`: a `FALSE` left operand answers `FALSE`.
    AndThen,
    /// `OR_ELSE`: a `TRUE` left operand answers `TRUE`.
    OrElse,
}

impl ShortCircuitOp {
    /// Returns the short-circuit operator to compile `compare` with, or `None`
    /// when it must be compiled by evaluating both operands.
    ///
    /// `AND_THEN` and `OR_ELSE` are `BOOL` operators. The AST admits any
    /// operand type, because the analyzer types them like `AND`/`OR`, which
    /// are also the bit-string operators — and "skip the right operand" has no
    /// meaning for a bit-string result: short-circuiting
    /// `2#1010 AND_THEN 2#0110` would produce `2#0110`, where the bitwise
    /// answer is `2#0010`. So a non-`BOOL` operand falls back to the eager
    /// path, which emits exactly what `AND`/`OR` emit.
    pub(crate) fn for_expr(compare: &CompareExpr) -> Option<Self> {
        let op = match compare.op {
            CompareOp::AndThen => Self::AndThen,
            CompareOp::OrElse => Self::OrElse,
            CompareOp::And
            | CompareOp::Or
            | CompareOp::Xor
            | CompareOp::Eq
            | CompareOp::Ne
            | CompareOp::Lt
            | CompareOp::Gt
            | CompareOp::LtEq
            | CompareOp::GtEq => return None,
        };
        (expr_is_bool(&compare.left) && expr_is_bool(&compare.right)).then_some(op)
    }
}

/// Compiles a short-circuit boolean expression, leaving its `BOOL` result on
/// the stack.
///
/// Both operators share one shape. The left operand is evaluated and branched
/// on; one arm evaluates the right operand, and the other materialises the
/// answer the left operand already forced:
///
/// ```text
///     <left>                  ; push left
///     JMP_IF_NOT alt          ; pops left
///     <then-arm>              ; AND_THEN: <right>     OR_ELSE: LOAD_TRUE
///     JMP end
/// alt:
///     <else-arm>              ; AND_THEN: LOAD_FALSE  OR_ELSE: <right>
/// end:
/// ```
///
/// Exactly one arm runs, so exactly one value reaches `end` — but the emitter
/// counts depth in emission order and would otherwise charge for both. See
/// [`Emitter::reset_stack_depth`].
pub(crate) fn compile_short_circuit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    compare: &CompareExpr,
    op: ShortCircuitOp,
) -> Result<(), Diagnostic> {
    // Both operands are BOOL (`ShortCircuitOp::for_expr` established that), so
    // the left operand's type is the type of the whole expression.
    let bool_op_type = op_type(&compare.left)?;

    let alternative = emitter.create_label();
    let end = emitter.create_label();

    compile_expr(emitter, ctx, &compare.left, bool_op_type)?;
    emitter.emit_jmp_if_not(alternative);
    let depth_before_arms = emitter.stack_depth();

    // Reached when the left operand is TRUE.
    match op {
        ShortCircuitOp::AndThen => compile_expr(emitter, ctx, &compare.right, bool_op_type)?,
        ShortCircuitOp::OrElse => emitter.emit_load_true(),
    }
    emitter.emit_jmp(end);

    emitter.bind_label(alternative);
    emitter.reset_stack_depth(depth_before_arms);

    // Reached when the left operand is FALSE.
    match op {
        ShortCircuitOp::AndThen => emitter.emit_load_false(),
        ShortCircuitOp::OrElse => compile_expr(emitter, ctx, &compare.right, bool_op_type)?,
    }
    emitter.bind_label(end);

    Ok(())
}
