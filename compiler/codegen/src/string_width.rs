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
//! A literal's delimiter is what types it: `'abc'` is a `STRING` literal and
//! `"abc"` a `WSTRING` one (IEC 61131-3 Table 5). That is the rule everywhere
//! two string values meet as peers — the operands of a comparison, the
//! arguments of `CONCAT` — because there is nothing else there to take an
//! encoding from, and mixing the two types is the error `P4034` exists for.
//!
//! A literal written into a declared destination — an assignment target, an
//! array element, a structure field, a function parameter being copied in —
//! is encoded for that destination rather than checked against it, which is
//! what [`compile_string_value`] does with the width its caller names. For a
//! `WSTRING` destination that is currently unreachable: the analyzer types
//! every character-string literal as `STRING`, so it rejects the assignment
//! (P4035) before codegen sees it. The encoding step is still what a `STRING`
//! destination needs, and is written to be correct if that typing is ever
//! fixed.
//!
//! This module is where all of that lives, so that no site has to restate it:
//!
//! - [`operand_char_width`] answers what encoding one string expression has.
//! - [`resolve_operand_char_width`] settles the one encoding an operation's
//!   operands share, and reports `P4034` when they do not share one.
//! - [`compile_string_value`] produces an expression as a string value at a
//!   given encoding — the single entry point every destination uses, and the
//!   only place a literal takes an encoding other than its own.

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

/// Returns the encoding of a string expression.
///
/// The width is known at compile time in every case codegen has to handle: a
/// literal's delimiter spells it, a declaration states it, a width-preserving
/// standard function takes it from its own string arguments, and everything
/// else that produces a string produces the encoding its return type names.
///
/// An expression whose width cannot be determined is a compiler bug rather
/// than a program error — the analyzer has already established that this is a
/// string — so it is reported as one (P9998) rather than guessed at, which
/// would defer the same problem to an encoding-mismatch trap at run time.
pub(crate) fn operand_char_width(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<CharWidth, Diagnostic> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::CharacterString(lit)) => {
            Ok(char_width_for_string_type(&lit.width))
        }
        ExprKind::Expression(inner) => operand_char_width(ctx, inner),
        ExprKind::Variable(variable) => variable_char_width(ctx, variable),
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
) -> Result<CharWidth, Diagnostic> {
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
            .map(|ret| ret.char_width)
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
/// string arguments, which [`resolve_operand_char_width`] has already required
/// to agree with each other.
fn width_of_string_args(
    ctx: &CompileContext,
    func: &Function,
    count: usize,
) -> Result<CharWidth, Diagnostic> {
    let args: Vec<&Expr> = positional_args(func).take(count).collect();
    let span = func.name.span();
    if args.is_empty() {
        return Err(unknown_string_encoding(
            span,
            "a string function call with no arguments",
        ));
    }

    resolve_operand_char_width(ctx, &args, &span)
}

/// The encoding an expression's analyzer-assigned type names, when it names a
/// string type at all.
fn resolved_type_width(expr: &Expr) -> Option<CharWidth> {
    let elementary = expr
        .resolved_type
        .as_ref()
        .and_then(|t| ElementaryTypeName::try_from(&t.name).ok())?;

    match elementary {
        ElementaryTypeName::STRING => Some(CharWidth::Narrow),
        ElementaryTypeName::WSTRING => Some(CharWidth::Wide),
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
/// data-region slots and the runtime requires all of them to agree. Among
/// peers there is no destination for a literal to take an encoding from, so
/// every operand answers with its own: a declaration for a variable, a
/// delimiter for a literal.
///
/// Operands that do not agree have no encoding they can share. That is a
/// program error — `CONCAT(s, w)` mixing a `STRING` and a `WSTRING`, or
/// `w = 'abc'` comparing one against a `STRING` literal — and is reported as
/// P4034 rather than emitted for the VM to trap on.
pub(crate) fn resolve_operand_char_width(
    ctx: &CompileContext,
    operands: &[&Expr],
    span: &SourceSpan,
) -> Result<CharWidth, Diagnostic> {
    let mut resolved: Option<CharWidth> = None;

    for operand in operands {
        let width = operand_char_width(ctx, operand)?;
        match resolved {
            Some(existing) if existing != width => {
                return Err(encoding_mismatch(
                    existing,
                    width,
                    &operand_span(operand, span),
                ));
            }
            Some(_) => {}
            None => resolved = Some(width),
        }
    }

    Ok(resolved.unwrap_or(NARROW_CHAR_WIDTH))
}

/// Compiles `expr` so that it leaves a temp buffer encoded at `char_width`.
///
/// This is the one place a literal takes an encoding other than the one its
/// delimiter spells: a declared destination — an assignment target, an array
/// element, a structure field, a function parameter being copied in — decides
/// the encoding of a literal written into it, rather than being compared
/// against it.
///
/// Any other expression carries an encoding of its own. When that encoding is
/// not `char_width`, no valid bytecode exists for the store the caller is
/// about to emit, so this reports P4034 instead of emitting it. An encoding
/// codegen cannot work out is left to the destination, which is the one that
/// decides the store: this is not the site that reports it.
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

    if let Ok(width) = operand_char_width(ctx, expr) {
        if width != char_width {
            return Err(encoding_mismatch(char_width, width, &expr.span()));
        }
    }

    compile_expr(emitter, ctx, expr, DEFAULT_OP_TYPE)
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
    fn operand_char_width_when_single_quoted_literal_then_narrow() {
        let ctx = CompileContext::new();
        assert_eq!(
            operand_char_width(&ctx, &literal("abc", false)).unwrap(),
            CharWidth::Narrow
        );
    }

    #[test]
    fn operand_char_width_when_double_quoted_literal_then_wide() {
        let ctx = CompileContext::new();
        assert_eq!(
            operand_char_width(&ctx, &literal("abc", true)).unwrap(),
            CharWidth::Wide
        );
    }

    #[test]
    fn resolve_operand_char_width_when_no_operands_then_narrow() {
        let ctx = CompileContext::new();
        let width = resolve_operand_char_width(&ctx, &[], &SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Narrow);
    }

    #[test]
    fn resolve_operand_char_width_when_literals_agree_then_that_width() {
        let ctx = CompileContext::new();
        let first = literal("a", true);
        let second = literal("b", true);
        let width =
            resolve_operand_char_width(&ctx, &[&first, &second], &SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn resolve_operand_char_width_when_literal_spellings_differ_then_p4034() {
        // Among peers there is no destination to adapt to, so the delimiters
        // are the types and these two have no encoding in common.
        let ctx = CompileContext::new();
        let wide = literal("a", true);
        let narrow = literal("b", false);
        let diagnostic =
            resolve_operand_char_width(&ctx, &[&wide, &narrow], &SourceSpan::default())
                .unwrap_err();
        assert_eq!(diagnostic.code, Problem::StringEncodingMismatch.code());
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
