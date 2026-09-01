//! The encoding of a string value, and the one way to produce one.
//!
//! `STRING` and `WSTRING` are the same type to almost everything in codegen —
//! a data-region slot with a header, addressed by a byte offset — and differ
//! only in the per-code-unit byte width recorded in that header: Latin-1 at
//! one byte, UTF-16LE at two (ADR-0016, ADR-0035). Every string opcode checks
//! that width at runtime and traps (`V9014`) when a source and a destination
//! disagree, so codegen has to get it right at every site that produces a
//! string value.
//!
//! Getting it right is not the same as knowing it: a string *literal* has no
//! encoding of its own. `'abc'` and `"abc"` are the same six characters, and
//! the spelling is a hint rather than a type — which is why `w := 'abc'` on a
//! `WSTRING` is accepted and stores UTF-16LE. A literal takes the encoding of
//! whatever it is used with, and that is a property of the *use*, not of the
//! literal.
//!
//! This module is where that rule lives, so that no site has to restate it.
//! [`compile_string_value`] is the single entry point for "produce this
//! expression as a string value at this encoding"; every destination — an
//! assignment target, an array element, a function parameter, a scratch slot
//! holding an operand — names the width it needs and gets a value that
//! matches it.

use ironplc_container::CharWidth;
use ironplc_dsl::common::ConstantKind;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr, ExprKind};

use crate::compile::{emit_string_literal_load, CompileContext, DEFAULT_OP_TYPE};
use crate::compile_expr::compile_expr;
use crate::emit::Emitter;

/// Compiles `expr` so that it leaves a temp buffer encoded at `char_width`.
///
/// A string literal is interned at `char_width`, because a literal has no
/// encoding until it is used (see the module documentation). Every other
/// expression already carries an encoding of its own — a variable's
/// declaration, a function's return type — and is compiled unchanged; the
/// caller is responsible for having resolved `char_width` to agree with it.
pub(crate) fn compile_string_value(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &Expr,
    char_width: CharWidth,
) -> Result<(), Diagnostic> {
    if let ExprKind::Const(ConstantKind::CharacterString(lit)) = &expr.kind {
        emit_string_literal_load(emitter, ctx, &lit.value, char_width);
        return Ok(());
    }

    compile_expr(emitter, ctx, expr, DEFAULT_OP_TYPE)
}
