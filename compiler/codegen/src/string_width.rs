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
//! encoding of its own. `'abc'` and `"abc"` are the same three characters, and
//! the spelling is a hint rather than a type — which is why `w := 'abc'` on a
//! `WSTRING` is accepted and stores UTF-16LE. A literal takes the encoding of
//! whatever it is used with, and that is a property of the *use*, not of the
//! literal.
//!
//! This module is where that rule lives, so that no site has to restate it:
//!
//! - [`operand_width`] answers what encoding one string expression has, and
//!   whether that encoding is fixed by a declaration or still open.
//! - [`resolve_operand_char_width`] settles the one encoding an operation's
//!   operands share. Two operands whose encodings are both fixed and disagree
//!   are a program error (`P4034`), not something to emit and let the VM find.
//! - [`compile_string_value`] produces an expression as a string value at a
//!   given encoding — the single entry point every destination uses, whether
//!   that destination is an assignment target, an array element, a function
//!   parameter, or a scratch slot holding a comparison operand.

use ironplc_analyzer::intermediate_type::IntermediateType;
use ironplc_container::CharWidth;
use ironplc_dsl::common::{ConstantKind, ElementaryTypeName};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    Expr, ExprKind, Function, ParamAssignmentKind, SymbolicVariableKind, Variable,
};
use ironplc_problems::Problem;

use crate::compile::{
    char_width_for_string_type, emit_string_literal_load, CompileContext, DEFAULT_OP_TYPE,
    NARROW_CHAR_WIDTH,
};
use crate::compile_expr::compile_expr;
use crate::emit::Emitter;

/// What codegen knows about the encoding of one string expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OperandWidth {
    /// Fixed by a declaration — a variable, a struct field, an array element,
    /// a function's declared return type. Two of these that disagree cannot
    /// both be satisfied.
    Declared(CharWidth),
    /// A literal, which has no encoding until it is used. The width is the one
    /// its spelling suggests, and applies only when nothing else decides.
    Adaptable(CharWidth),
    /// Codegen cannot tell. Nothing is claimed, and nothing is rejected.
    Unknown,
}

/// The standard functions whose result carries the encoding of their string
/// input rather than one of their own.
///
/// The analyzer types all of them as `STRING` (it has a single string type
/// name for both encodings), so their `resolved_type` cannot distinguish a
/// `WSTRING` result from a `STRING` one and the arguments have to answer
/// instead.
const WIDTH_PRESERVING_FUNCTIONS: [&str; 7] = [
    "concat", "left", "right", "mid", "insert", "delete", "replace",
];

/// Returns what codegen knows about the encoding of `expr`.
///
/// Sources are consulted in order of authority. Codegen's own allocation
/// records come first: they describe the header bytes actually emitted for the
/// slot, so an answer drawn from them cannot disagree with the running
/// program. `resolved_type` is the fallback, and only a `WSTRING` there is
/// taken as fixed — the analyzer names both encodings `STRING` in enough
/// places that reading `STRING` as "definitely narrow" would reject working
/// programs.
pub(crate) fn operand_width(ctx: &CompileContext, expr: &Expr) -> OperandWidth {
    match &expr.kind {
        // A literal's spelling is a preference, not a type.
        ExprKind::Const(ConstantKind::CharacterString(lit)) => {
            OperandWidth::Adaptable(char_width_for_string_type(&lit.width))
        }
        ExprKind::Expression(inner) => operand_width(ctx, inner),
        ExprKind::Variable(variable) => variable_width(ctx, variable),
        ExprKind::Function(func) => match function_width(ctx, func) {
            // A call the tables above cannot answer for — a conversion
            // function, or a user function with no compiled return slot yet.
            OperandWidth::Unknown => resolved_type_width(expr),
            known => known,
        },
        _ => resolved_type_width(expr),
    }
}

/// Resolves the encoding of a variable reference from codegen's own metadata:
/// the string variable table, the string array element width, or the struct
/// field's intermediate type.
fn variable_width(ctx: &CompileContext, variable: &Variable) -> OperandWidth {
    let Variable::Symbolic(symbolic) = variable else {
        return OperandWidth::Unknown;
    };

    match symbolic {
        SymbolicVariableKind::Named(named) => match ctx.string_vars.get(&named.name) {
            Some(info) => OperandWidth::Declared(info.char_width),
            None => OperandWidth::Unknown,
        },
        SymbolicVariableKind::Array(array) => {
            let mut base = array.subscripted_variable.as_ref();
            while let SymbolicVariableKind::Array(inner) = base {
                base = inner.subscripted_variable.as_ref();
            }
            match base {
                SymbolicVariableKind::Named(named) => match ctx.array_vars.get(&named.name) {
                    Some(info) if info.is_string_element => {
                        OperandWidth::Declared(info.string_char_width)
                    }
                    _ => OperandWidth::Unknown,
                },
                _ => OperandWidth::Unknown,
            }
        }
        SymbolicVariableKind::Structured(structured) => {
            // A chain codegen cannot walk (an unsupported shape, an unknown
            // field) is not this module's error to report; the site that
            // compiles the access reports it with the right message.
            match crate::compile_struct::walk_struct_chain(
                ctx,
                &structured.record,
                &structured.field,
                0,
            ) {
                Ok((_, _, IntermediateType::String { char_width, .. })) => {
                    OperandWidth::Declared(char_width)
                }
                _ => OperandWidth::Unknown,
            }
        }
        _ => OperandWidth::Unknown,
    }
}

/// Resolves the encoding of a function call's result.
///
/// A user function declares its return type, so its compiled return slot is
/// authoritative. A standard function that passes its input's encoding through
/// is resolved from its own string arguments. Everything else falls back to
/// the resolved type, which covers the conversion functions that genuinely
/// produce a narrow `STRING`.
fn function_width(ctx: &CompileContext, func: &Function) -> OperandWidth {
    let name = func.name.lower_case();

    if let Some(info) = ctx.user_functions.get(name.as_str()) {
        return match &info.return_string_info {
            Some(ret) => OperandWidth::Declared(ret.char_width),
            None => OperandWidth::Unknown,
        };
    }

    if WIDTH_PRESERVING_FUNCTIONS.contains(&name.as_str()) {
        let mut adaptable = OperandWidth::Unknown;
        for arg in positional_args(func) {
            match operand_width(ctx, arg) {
                declared @ OperandWidth::Declared(_) => return declared,
                found @ OperandWidth::Adaptable(_) => {
                    if adaptable == OperandWidth::Unknown {
                        adaptable = found;
                    }
                }
                OperandWidth::Unknown => {}
            }
        }
        return adaptable;
    }

    OperandWidth::Unknown
}

/// Reads an expression's analyzer-assigned type. Only `WSTRING` is conclusive
/// — see [`operand_width`].
fn resolved_type_width(expr: &Expr) -> OperandWidth {
    let is_wstring = expr
        .resolved_type
        .as_ref()
        .and_then(|t| ElementaryTypeName::try_from(&t.name).ok())
        .is_some_and(|e| matches!(e, ElementaryTypeName::WSTRING));

    if is_wstring {
        OperandWidth::Declared(CharWidth::Wide)
    } else {
        OperandWidth::Unknown
    }
}

/// Positional input arguments of a call, in declaration order.
fn positional_args(func: &Function) -> impl Iterator<Item = &Expr> {
    func.param_assignment.iter().filter_map(|p| match p {
        ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
        _ => None,
    })
}

/// Resolves the single encoding every operand of one string operation shares.
///
/// A comparison, a `CONCAT`, a `FIND` — each addresses its operands as
/// data-region slots and the runtime requires all of them to agree. This
/// returns the encoding to produce them all at:
///
/// 1. an encoding fixed by a declaration, if any operand has one;
/// 2. otherwise the encoding the enclosing operation is producing, if this
///    operation is nested inside one — a nested all-literal call adapts the
///    same way a bare literal does;
/// 3. otherwise the spelling of the first literal;
/// 4. otherwise narrow.
///
/// Two operands with different *declared* encodings have no encoding they can
/// share. That is a program error — `CONCAT(s, w)` mixing a `STRING` and a
/// `WSTRING` — and is reported as P4034 rather than emitted for the VM to trap
/// on.
pub(crate) fn resolve_operand_char_width(
    ctx: &CompileContext,
    operands: &[&Expr],
    span: &SourceSpan,
) -> Result<CharWidth, Diagnostic> {
    let mut declared: Option<CharWidth> = None;
    let mut adaptable: Option<CharWidth> = None;

    for operand in operands {
        match operand_width(ctx, operand) {
            OperandWidth::Declared(width) => match declared {
                Some(existing) if existing != width => {
                    return Err(encoding_mismatch(existing, width, &operand.span(), span));
                }
                Some(_) => {}
                None => declared = Some(width),
            },
            OperandWidth::Adaptable(width) => {
                adaptable.get_or_insert(width);
            }
            OperandWidth::Unknown => {}
        }
    }

    Ok(declared
        .or(ctx.string_width_hint)
        .or(adaptable)
        .unwrap_or(NARROW_CHAR_WIDTH))
}

/// Compiles `expr` so that it leaves a temp buffer encoded at `char_width`.
///
/// A string literal is interned at `char_width`, because a literal has no
/// encoding until it is used (see the module documentation), and the width is
/// parked as a hint for the duration of the compile so that a nested operation
/// with nothing but literals of its own adapts to the same encoding.
///
/// Any other expression carries an encoding of its own. When that encoding is
/// known and is not `char_width`, no valid bytecode exists for the store the
/// caller is about to emit, so this reports P4034 instead of emitting it.
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

    if let OperandWidth::Declared(width) = operand_width(ctx, expr) {
        if width != char_width {
            return Err(encoding_mismatch(
                char_width,
                width,
                &expr.span(),
                &expr.span(),
            ));
        }
    }

    let outer_hint = ctx.string_width_hint.replace(char_width);
    let result = compile_expr(emitter, ctx, expr, DEFAULT_OP_TYPE);
    ctx.string_width_hint = outer_hint;
    result
}

/// Builds the P4034 diagnostic for two string encodings that cannot be
/// reconciled, pointing at the operand that disagrees.
pub(crate) fn encoding_mismatch(
    expected: CharWidth,
    actual: CharWidth,
    operand_span: &SourceSpan,
    fallback_span: &SourceSpan,
) -> Diagnostic {
    let span = if operand_span == &SourceSpan::default() {
        fallback_span.clone()
    } else {
        operand_span.clone()
    };

    Diagnostic::problem(
        Problem::StringEncodingMismatch,
        Label::span(span, "String operand"),
    )
    .with_context("expected", &type_name_for(expected).to_string())
    .with_context("found", &type_name_for(actual).to_string())
}

/// The IEC type name for an encoding, for diagnostics.
fn type_name_for(char_width: CharWidth) -> &'static str {
    if char_width.is_wide() {
        "WSTRING"
    } else {
        "STRING"
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::CharacterStringLiteral;

    use super::*;

    fn literal(chars: &str, wide: bool) -> Expr {
        let value: Vec<char> = chars.chars().collect();
        let lit = if wide {
            CharacterStringLiteral::new_wide(value)
        } else {
            CharacterStringLiteral::new(value)
        };
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(lit)))
    }

    #[test]
    fn operand_width_when_narrow_literal_then_adaptable_narrow() {
        let ctx = CompileContext::new();
        assert_eq!(
            operand_width(&ctx, &literal("abc", false)),
            OperandWidth::Adaptable(CharWidth::Narrow)
        );
    }

    #[test]
    fn operand_width_when_wide_literal_then_adaptable_wide() {
        let ctx = CompileContext::new();
        assert_eq!(
            operand_width(&ctx, &literal("abc", true)),
            OperandWidth::Adaptable(CharWidth::Wide)
        );
    }

    #[test]
    fn resolve_operand_char_width_when_no_operands_then_narrow() {
        let ctx = CompileContext::new();
        let width = resolve_operand_char_width(&ctx, &[], &SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Narrow);
    }

    #[test]
    fn resolve_operand_char_width_when_only_literals_then_first_spelling() {
        let ctx = CompileContext::new();
        let wide = literal("a", true);
        let narrow = literal("b", false);
        let width =
            resolve_operand_char_width(&ctx, &[&wide, &narrow], &SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn resolve_operand_char_width_when_hint_set_then_hint_beats_spelling() {
        // A nested operation with nothing but literals adapts to the encoding
        // the enclosing operation is producing, rather than to its own
        // spelling -- `w := CONCAT('a', 'b')`.
        let mut ctx = CompileContext::new();
        ctx.string_width_hint = Some(CharWidth::Wide);
        let narrow = literal("a", false);
        let width = resolve_operand_char_width(&ctx, &[&narrow], &SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn compile_string_value_when_wide_target_then_interns_wide_constant() {
        let mut ctx = CompileContext::new();
        let mut emitter = Emitter::new();
        compile_string_value(
            &mut emitter,
            &mut ctx,
            &literal("hi", false),
            CharWidth::Wide,
        )
        .unwrap();

        // A wide literal forces wide temp-buffer sizing for the whole program.
        assert!(ctx.has_wide_string);
    }
}
