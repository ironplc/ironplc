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
//! the spelling is a preference rather than a type — which is why `w := 'abc'`
//! on a `WSTRING` is accepted and stores UTF-16LE. A literal takes the
//! encoding of whatever it is used with, and that is a property of the *use*,
//! not of the literal.
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

use ironplc_analyzer::IntermediateType;
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
use crate::compile_expr::{compile_expr, variable_span};
use crate::emit::Emitter;

/// What codegen knows about the encoding of one string expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OperandWidth {
    /// Fixed by a declaration — a variable, a struct field, an array element,
    /// a function's declared return type, or what a conversion emits. Two of
    /// these that disagree cannot both be satisfied.
    Declared(CharWidth),
    /// A literal, or a call built only out of literals. It has no encoding
    /// until it is used; the width is the one its spelling suggests, and
    /// applies only when nothing else decides.
    Adaptable(CharWidth),
}

/// The standard functions whose result carries the encoding of their string
/// inputs rather than one of their own, and how many leading arguments those
/// string inputs are.
///
/// The analyzer types all of them as `STRING` — it has a single string type
/// name for both encodings — so their resolved type cannot tell a `WSTRING`
/// result from a `STRING` one, and the arguments have to answer instead. The
/// counts are what keeps the walk off the integer arguments that follow, such
/// as `MID`'s length and position.
const WIDTH_PRESERVING_FUNCTIONS: [(&str, usize); 7] = [
    ("concat", 2),
    ("left", 1),
    ("right", 1),
    ("mid", 1),
    ("insert", 2),
    ("delete", 1),
    ("replace", 2),
];

/// Returns what codegen knows about the encoding of `expr`.
///
/// The width is known at compile time in every case codegen has to handle: a
/// literal spells it, a declaration states it, a width-preserving standard
/// function takes it from its own string arguments, and everything else that
/// produces a string produces the encoding its return type names.
///
/// An expression whose width cannot be determined is a compiler bug rather
/// than a program error — the analyzer has already established that this is a
/// string — so it is reported as one (P9998) rather than guessed at, which
/// would defer the same problem to an encoding-mismatch trap at run time.
pub(crate) fn operand_width(ctx: &CompileContext, expr: &Expr) -> Result<OperandWidth, Diagnostic> {
    match &expr.kind {
        // A literal's spelling is a preference, not a type.
        ExprKind::Const(ConstantKind::CharacterString(lit)) => Ok(OperandWidth::Adaptable(
            char_width_for_string_type(&lit.width),
        )),
        ExprKind::Expression(inner) => operand_width(ctx, inner),
        ExprKind::Variable(variable) => {
            variable_char_width(ctx, variable).map(OperandWidth::Declared)
        }
        ExprKind::Function(func) => function_width(ctx, expr, func),
        _ => Err(unknown_string_encoding(
            expr.span(),
            "a string expression of an unexpected kind",
        )),
    }
}

/// Returns the encoding of a string variable, array element or structure field.
///
/// Subscripts and dereferences do not change the encoding, so the access is
/// walked back to the variable it is rooted in: a name, resolved against the
/// declared strings and string arrays, or a structure field, whose declared
/// type carries the width.
fn variable_char_width(ctx: &CompileContext, variable: &Variable) -> Result<CharWidth, Diagnostic> {
    let Variable::Symbolic(kind) = variable else {
        return Err(unknown_string_encoding(
            variable_span(variable),
            "a directly represented variable",
        ));
    };

    match access_root(kind) {
        SymbolicVariableKind::Named(named) => {
            if let Some(info) = ctx.string_vars.get(&named.name) {
                return Ok(info.char_width);
            }
            ctx.array_vars
                .get(&named.name)
                .filter(|info| info.is_string_element)
                .map(|info| info.string_char_width)
                .ok_or_else(|| {
                    unknown_string_encoding(
                        variable_span(variable),
                        "a variable that is not a declared string",
                    )
                })
        }
        SymbolicVariableKind::Structured(structured) => {
            let (_, _, field_type) = crate::compile_struct::walk_struct_chain(
                ctx,
                &structured.record,
                &structured.field,
                0,
            )
            .map_err(|_| {
                unknown_string_encoding(variable_span(variable), "an unresolvable structure field")
            })?;
            string_char_width_of(&field_type).ok_or_else(|| {
                unknown_string_encoding(
                    variable_span(variable),
                    "a structure field that is not a string",
                )
            })
        }
        _ => Err(unknown_string_encoding(
            variable_span(variable),
            "a variable access of an unexpected kind",
        )),
    }
}

/// Walks past subscripts and dereferences to the variable an access is rooted
/// in. `s.names[i]` roots in the structure field `s.names`, `arr[i][j]` in the
/// name `arr`.
fn access_root(kind: &SymbolicVariableKind) -> &SymbolicVariableKind {
    let mut current = kind;
    loop {
        current = match current {
            SymbolicVariableKind::Array(array) => array.subscripted_variable.as_ref(),
            SymbolicVariableKind::Deref(deref) => deref.variable.as_ref(),
            other => return other,
        };
    }
}

/// Returns the encoding of a STRING type, or of a STRING array's element.
fn string_char_width_of(field_type: &IntermediateType) -> Option<CharWidth> {
    match field_type {
        IntermediateType::String { char_width, .. } => Some(*char_width),
        IntermediateType::Array { element_type, .. } => string_char_width_of(element_type),
        _ => None,
    }
}

/// Returns the encoding of a function call's string result.
///
/// A width-preserving standard function is resolved from its own string
/// arguments, and stays adaptable when those are all literals, so that
/// `w := CONCAT('a', 'b')` adapts the same way a bare literal does. A
/// user-defined function declares its return type. Everything else — the
/// conversions, which build a Latin-1 string — is resolved from the return
/// type the analyzer gave the call.
fn function_width(
    ctx: &CompileContext,
    expr: &Expr,
    func: &Function,
) -> Result<OperandWidth, Diagnostic> {
    let name = func.name.lower_case();

    if let Some((_, string_args)) = WIDTH_PRESERVING_FUNCTIONS
        .iter()
        .find(|(known, _)| *known == name.as_str())
    {
        return width_of_string_args(ctx, func, *string_args);
    }

    if let Some(info) = ctx.user_functions.get(name.as_str()) {
        return info
            .return_string_info
            .as_ref()
            .map(|ret| OperandWidth::Declared(ret.char_width))
            .ok_or_else(|| {
                unknown_string_encoding(
                    func.name.span(),
                    "a function call that does not return a string",
                )
            });
    }

    resolved_type_width(expr).ok_or_else(|| {
        unknown_string_encoding(
            func.name.span(),
            "a function call that does not return a string",
        )
    })
}

/// Resolves a width-preserving function's result from its leading `count`
/// string arguments: the first that a declaration fixes, else the first
/// literal's spelling.
fn width_of_string_args(
    ctx: &CompileContext,
    func: &Function,
    count: usize,
) -> Result<OperandWidth, Diagnostic> {
    let args: Vec<&Expr> = positional_args(func).take(count).collect();
    if args.is_empty() {
        return Err(unknown_string_encoding(
            func.name.span(),
            "a string function call with no arguments",
        ));
    }

    let mut adaptable: Option<OperandWidth> = None;
    for arg in args {
        match operand_width(ctx, arg)? {
            declared @ OperandWidth::Declared(_) => return Ok(declared),
            found @ OperandWidth::Adaptable(_) => {
                adaptable.get_or_insert(found);
            }
        }
    }

    adaptable.ok_or_else(|| {
        unknown_string_encoding(
            func.name.span(),
            "a string function call with no string arguments",
        )
    })
}

/// The encoding an expression's analyzer-assigned type names, when it names a
/// string type at all.
fn resolved_type_width(expr: &Expr) -> Option<OperandWidth> {
    let elementary = expr
        .resolved_type
        .as_ref()
        .and_then(|t| ElementaryTypeName::try_from(&t.name).ok())?;

    match elementary {
        ElementaryTypeName::STRING => Some(OperandWidth::Declared(CharWidth::Narrow)),
        ElementaryTypeName::WSTRING => Some(OperandWidth::Declared(CharWidth::Wide)),
        _ => None,
    }
}

/// Positional input arguments of a call, in declaration order.
fn positional_args(func: &Function) -> impl Iterator<Item = &Expr> {
    func.param_assignment.iter().filter_map(|p| match p {
        ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
        _ => None,
    })
}

/// Reports that codegen could not determine a string expression's encoding.
fn unknown_string_encoding(span: SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span,
        format!("Cannot determine the string encoding of {what}"),
    ))
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
/// 3. otherwise the spelling of the first literal.
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
        match operand_width(ctx, operand)? {
            OperandWidth::Declared(width) => match declared {
                Some(existing) if existing != width => {
                    return Err(encoding_mismatch(
                        existing,
                        width,
                        &operand_span(operand, span),
                    ));
                }
                Some(_) => {}
                None => declared = Some(width),
            },
            OperandWidth::Adaptable(width) => {
                adaptable.get_or_insert(width);
            }
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
/// caller is about to emit, so this reports P4034 instead of emitting it. An
/// encoding codegen cannot work out is left to the destination, which is the
/// one that decides the store: this is not the site that reports it.
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

    if let Ok(OperandWidth::Declared(width)) = operand_width(ctx, expr) {
        if width != char_width {
            return Err(encoding_mismatch(char_width, width, &expr.span()));
        }
    }

    let outer_hint = ctx.string_width_hint.replace(char_width);
    let result = compile_expr(emitter, ctx, expr, DEFAULT_OP_TYPE);
    ctx.string_width_hint = outer_hint;
    result
}

/// The span to blame an operand's encoding on: its own when it has one, and
/// the enclosing operation's otherwise. Not every expression position carries
/// a span, and an unplaced diagnostic is worse than one placed on the
/// operation the operand belongs to.
fn operand_span(operand: &Expr, operation: &SourceSpan) -> SourceSpan {
    let span = operand.span();
    if span == SourceSpan::default() {
        operation.clone()
    } else {
        span
    }
}

/// Builds the P4034 diagnostic for two string encodings that cannot be
/// reconciled.
pub(crate) fn encoding_mismatch(
    expected: CharWidth,
    actual: CharWidth,
    span: &SourceSpan,
) -> Diagnostic {
    Diagnostic::problem(
        Problem::StringEncodingMismatch,
        Label::span(span.clone(), "String operand"),
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
            operand_width(&ctx, &literal("abc", false)).unwrap(),
            OperandWidth::Adaptable(CharWidth::Narrow)
        );
    }

    #[test]
    fn operand_width_when_wide_literal_then_adaptable_wide() {
        let ctx = CompileContext::new();
        assert_eq!(
            operand_width(&ctx, &literal("abc", true)).unwrap(),
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
