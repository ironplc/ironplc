//! The encoding of a string expression.
//!
//! `STRING` and `WSTRING` are the same shape to almost everything in codegen —
//! a data-region slot with a header, addressed by a byte offset — and differ
//! only in the per-code-unit byte width recorded in that header: Latin-1 at
//! one byte, UTF-16LE at two (ADR-0016, ADR-0035). Every string opcode checks
//! that width at runtime and traps (`V9014`) when a source and a destination
//! disagree, so the temporary that [`crate::compile_string::resolve_string_arg`]
//! allocates for an operand has to be initialized at the width that operand
//! yields.
//!
//! Answering "which width is that?" is a question about types, not about
//! bytecode, and it is the question this module exists to answer.
//! [`string_expr_char_width`] answers it for one expression; the helpers
//! below it are the cases it delegates to.
//!
//! Two further questions belong with it. An operation with several string
//! operands needs them to share an encoding, which
//! [`resolve_operand_char_width`] settles -- a literal's delimiter is what
//! types it, `'abc'` being a `STRING` and `"abc"` a `WSTRING` (IEC 61131-3
//! Table 5), so operands that disagree have no encoding in common and the
//! program is rejected rather than left for the VM to trap on. And a value
//! being written into a declared destination is *encoded for* that
//! destination rather than checked against it, which is
//! [`compile_string_value`].

use ironplc_analyzer::IntermediateType;
use ironplc_container::CharWidth;
use ironplc_dsl::common::{ConstantKind, ElementaryTypeName};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, Function, SymbolicVariableKind, Variable};

use ironplc_problems::Problem;

use super::compile::{
    char_width_for_string_type, emit_string_literal_load, CompileContext, DEFAULT_OP_TYPE,
    NARROW_CHAR_WIDTH,
};
use super::compile_expr::{compile_expr, variable_span};
use super::compile_string::collect_positional_args;
use crate::emit::Emitter;

/// Returns the encoding a string-valued expression produces.
///
/// Every string slot records its encoding in its header and the VM rejects a
/// store whose source and destination disagree (ADR-0034), so the temporary
/// that [`resolve_string_arg`] allocates has to be initialized at the width
/// the expression yields rather than at a fixed one. The width is always
/// known at compile time: a literal spells it, a declaration states it, and
/// every string function returns the encoding of its first string argument.
///
/// An expression whose width cannot be determined is a compiler bug rather
/// than a program error -- the analyzer has already established that this
/// argument is a string. Report it as one instead of guessing a width, which
/// would defer the same problem to an encoding-mismatch trap at run time.
pub(crate) fn string_expr_char_width(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<CharWidth, Diagnostic> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::CharacterString(lit)) => {
            Ok(char_width_for_string_type(&lit.width))
        }
        ExprKind::Expression(inner) => string_expr_char_width(ctx, inner),
        ExprKind::Variable(variable) => variable_char_width(ctx, variable),
        ExprKind::Function(func) => function_char_width(ctx, expr, func),
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
/// The standard string functions return the encoding of their first string
/// argument; a user-defined function declares its return type. Every other
/// call that yields a string -- the conversions, which build a Latin-1 string
/// -- says so in the return type the analyzer gave it, which is what
/// `resolved_type` on the enclosing expression carries.
fn function_char_width(
    ctx: &CompileContext,
    expr: &Expr,
    func: &Function,
) -> Result<CharWidth, Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "concat" | "left" | "right" | "mid" | "insert" | "delete" | "replace" => {
            match collect_positional_args(func).first() {
                Some(first) => string_expr_char_width(ctx, first),
                None => Err(unknown_string_encoding(
                    func.name.span(),
                    "a string function call with no arguments",
                )),
            }
        }
        _ => match ctx
            .user_functions
            .get(name.as_str())
            .and_then(|info| info.return_string_info.as_ref())
        {
            Some(info) => Ok(info.char_width),
            None => resolved_string_char_width(expr, func.name.span()),
        },
    }
}

/// The encoding an expression's analyzer-assigned type names.
///
/// Every remaining call in a string position is one whose result the analyzer
/// typed, so this answers for all of them. In practice they are all narrow:
/// the calls that reach here are the ones that are neither a width-preserving
/// standard function nor a user function with a declared string return, which
/// leaves the `*_TO_STRING` conversions, and every one of those builds
/// Latin-1. `WSTRING` is mapped because it is what the name means, not
/// because a program can currently produce it -- `parse_string_conversion`
/// has no `*_TO_WSTRING` form, and `SEL`/`MUX` reject a string argument. A
/// wide result reaches its caller through the user-function branch above,
/// which `end_to_end_wstring` covers. Failing to answer means the
/// analyzer typed a string-position expression as something that is not a
/// string, which is a defect in the compiler rather than in the program being
/// compiled -- so it is reported as one, and the diagnostic names what was
/// found instead. An internal error that does not say enough to debug it is
/// only half a report.
fn resolved_string_char_width(expr: &Expr, span: SourceSpan) -> Result<CharWidth, Diagnostic> {
    let Some(type_name) = expr.resolved_type.as_ref() else {
        return Err(unknown_string_encoding(
            span,
            "a function call the analyzer left untyped",
        ));
    };

    match ElementaryTypeName::try_from(&type_name.name) {
        Ok(ElementaryTypeName::STRING) => Ok(CharWidth::Narrow),
        Ok(ElementaryTypeName::WSTRING) => Ok(CharWidth::Wide),
        _ => Err(unknown_string_encoding(
            span,
            &format!("a function call the analyzer typed as {type_name}"),
        )),
    }
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
/// A comparison, a `CONCAT`, a `FIND` -- each addresses its operands as
/// data-region slots, and the runtime requires all of them to agree. Among
/// peers there is no destination for a literal to take an encoding from, so
/// every operand answers with its own: a declaration for a variable, a
/// delimiter for a literal.
///
/// Operands that do not agree have no encoding they can share. That is a
/// program error -- `CONCAT(s, w)` mixing a `STRING` and a `WSTRING`, or
/// `w = 'abc'` comparing one against a `STRING` literal -- and is reported as
/// P4034 rather than emitted for the VM to trap on one scan later.
pub(crate) fn resolve_operand_char_width(
    ctx: &CompileContext,
    operands: &[&Expr],
    span: &SourceSpan,
) -> Result<CharWidth, Diagnostic> {
    let mut resolved: Option<CharWidth> = None;

    for operand in operands {
        let width = string_expr_char_width(ctx, operand)?;
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
/// delimiter spells: a declared destination -- an assignment target, an array
/// element, a structure field, a function parameter being copied in -- decides
/// the encoding of a literal written into it, rather than being compared
/// against it.
///
/// Where the analyzer type-checks the destination -- a simple named assignment
/// target or a function parameter -- a literal that reaches here already
/// spells the destination's encoding, because a mismatch is P4035 or P4026
/// first. The destination still decides for the targets the analyzer does not
/// check, such as an array element or a structure field.
///
/// Any other expression carries an encoding of its own, and one that is not
/// `char_width` has no valid bytecode for the store the caller is about to
/// emit -- so that is P4034 too. An encoding codegen cannot work out is left
/// to the destination, which is the one that decides the store.
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

    if let Ok(width) = string_expr_char_width(ctx, expr) {
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
    use ironplc_dsl::common::TypeName;

    use super::*;

    /// An expression carrying `type_name` as the type the analyzer resolved.
    /// `resolved_string_char_width` reads only that, so the kind is arbitrary.
    fn typed_as(type_name: &str) -> Expr {
        Expr::with_type(
            ExprKind::Null(SourceSpan::default()),
            TypeName::from(type_name),
        )
    }

    #[test]
    fn resolved_string_char_width_when_string_then_narrow() {
        let width = resolved_string_char_width(&typed_as("STRING"), SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Narrow);
    }

    #[test]
    fn resolved_string_char_width_when_wstring_then_wide() {
        let width =
            resolved_string_char_width(&typed_as("WSTRING"), SourceSpan::default()).unwrap();
        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn resolved_string_char_width_when_typed_as_non_string_then_internal_error_names_the_type() {
        // Unreachable by construction -- the analyzer has established that a
        // string argument is a string. Should it happen anyway, it stops the
        // compile and says what it found, rather than passing a guessed width
        // down to an encoding-mismatch trap at run time.
        let diagnostic =
            resolved_string_char_width(&typed_as("INT"), SourceSpan::default()).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(
            diagnostic.primary.message.contains("INT"),
            "the message should name what was found, got: {}",
            diagnostic.primary.message
        );
    }

    #[test]
    fn resolved_string_char_width_when_untyped_then_internal_error() {
        let untyped = Expr::new(ExprKind::Null(SourceSpan::default()));
        let diagnostic = resolved_string_char_width(&untyped, SourceSpan::default()).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(diagnostic.primary.message.contains("untyped"));
    }
}
