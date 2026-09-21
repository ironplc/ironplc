//! The compile-time type of a string expression: its encoding and its bound.
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
//! The same header records the slot's capacity, and a store into a slot that
//! is too small for its source truncates silently (ADR-0035). So the second
//! thing that temporary needs to know is the most code units the operand can
//! ever produce: a literal's length, a declaration's `STRING[n]`, or, for a
//! nested call, the largest result the call can build from its own operands.
//!
//! Answering "which width, and how long?" is a question about types, not
//! about bytecode, and it is the question this module exists to answer.
//! [`string_expr_type`] answers it for one expression; the helpers below it
//! are the cases it delegates to. [`string_expr_char_width`] is the encoding
//! alone, for the callers that need only that.
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
    DEFAULT_STRING_MAX_LENGTH, NARROW_CHAR_WIDTH,
};
use super::compile_expr::{compile_expr, variable_span};
use super::compile_string::collect_positional_args;
use crate::emit::Emitter;

/// The compile-time type of a string-valued expression: the encoding its
/// slot must carry and the most code units the expression can ever produce.
///
/// `max_length` is a static bound, never a measurement: for a declared
/// variable it is the `STRING[n]` capacity whatever the variable currently
/// holds, and for a call it is the widest result the call can build from its
/// operands' own bounds. It saturates at the header ceiling (ADR-0035): a
/// string slot records its capacity in a `u16`, so no expression can be
/// materialized wider than that whatever its operands add up to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StringExprType {
    pub(crate) char_width: CharWidth,
    // Read by the operand temporary once it is sized from the operand; until
    // then the walk computes the bound without a consumer.
    #[allow(dead_code)]
    pub(crate) max_length: u16,
}

/// Returns the encoding and the length bound a string-valued expression
/// produces.
///
/// Every string slot records its encoding and capacity in its header. The VM
/// rejects a store whose source and destination encodings disagree
/// (ADR-0034) and cuts a store whose source is longer than the destination's
/// capacity (ADR-0035), so the temporary that [`resolve_string_arg`]
/// allocates has to be initialized at the width and the bound the expression
/// yields rather than at fixed ones. Both are always known at compile time: a
/// literal spells them, a declaration states them, and every string function
/// derives them from its string arguments.
///
/// An expression whose type cannot be determined is a compiler bug rather
/// than a program error -- the analyzer has already established that this
/// argument is a string. Report it as one instead of guessing, which would
/// defer the same problem to an encoding-mismatch trap or a silent
/// truncation at run time.
///
/// [`resolve_string_arg`]: crate::compile_string::resolve_string_arg
pub(crate) fn string_expr_type(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<StringExprType, Diagnostic> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::CharacterString(lit)) => Ok(StringExprType {
            char_width: char_width_for_string_type(&lit.width),
            max_length: saturate_length(lit.value.len()),
        }),
        ExprKind::Expression(inner) => string_expr_type(ctx, inner),
        ExprKind::Variable(variable) => variable_string_type(ctx, variable),
        ExprKind::Function(func) => function_string_type(ctx, expr, func),
        _ => Err(unknown_string_type(
            expr.span(),
            "a string expression of an unexpected kind",
        )),
    }
}

/// Returns the encoding a string-valued expression produces.
///
/// The encoding half of [`string_expr_type`], for the callers that settle a
/// shared width among operands or check a value against its destination and
/// have no use for the bound.
pub(crate) fn string_expr_char_width(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<CharWidth, Diagnostic> {
    string_expr_type(ctx, expr).map(|string_type| string_type.char_width)
}

/// Returns the type of a string variable, array element or structure field.
///
/// Subscripts and dereferences change neither the encoding nor the capacity,
/// so the access is walked back to the variable it is rooted in: a name,
/// resolved against the declared strings and string arrays, or a structure
/// field, whose declared type carries both.
fn variable_string_type(
    ctx: &CompileContext,
    variable: &Variable,
) -> Result<StringExprType, Diagnostic> {
    let Variable::Symbolic(kind) = variable else {
        return Err(unknown_string_type(
            variable_span(variable),
            "a directly represented variable",
        ));
    };

    match access_root(kind) {
        SymbolicVariableKind::Named(named) => {
            if let Some(info) = ctx.string_vars.get(&named.name) {
                return Ok(StringExprType {
                    char_width: info.char_width,
                    max_length: info.max_length,
                });
            }
            ctx.array_vars
                .get(&named.name)
                .filter(|info| info.is_string_element)
                .map(|info| StringExprType {
                    char_width: info.string_char_width,
                    max_length: info.string_max_len,
                })
                .ok_or_else(|| {
                    unknown_string_type(
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
                unknown_string_type(variable_span(variable), "an unresolvable structure field")
            })?;
            string_type_of(&field_type).ok_or_else(|| {
                unknown_string_type(
                    variable_span(variable),
                    "a structure field that is not a string",
                )
            })
        }
        _ => Err(unknown_string_type(
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

/// Returns the type of a STRING type, or of a STRING array's element.
///
/// A string declared without a length has the default capacity, which is
/// the same default every declaration site applies.
fn string_type_of(field_type: &IntermediateType) -> Option<StringExprType> {
    match field_type {
        IntermediateType::String {
            max_len,
            char_width,
        } => Some(StringExprType {
            char_width: *char_width,
            max_length: max_len.map_or(DEFAULT_STRING_MAX_LENGTH, saturate_length),
        }),
        IntermediateType::Array { element_type, .. } => string_type_of(element_type),
        _ => None,
    }
}

/// Returns the type of a function call's string result.
///
/// The standard string functions return the encoding of their first string
/// argument. Their bound follows from what they do: `CONCAT`, `INSERT` and
/// `REPLACE` can hand back every code unit of both string arguments, so
/// their bound is the sum; `LEFT`, `RIGHT`, `MID` and `DELETE` only ever
/// drop code units from their one string argument, so its bound is theirs.
/// A user-defined function declares its return type. Every other call that
/// yields a string -- the conversions, which build a Latin-1 string -- says
/// so in the return type the analyzer gave it, which is what `resolved_type`
/// on the enclosing expression carries.
fn function_string_type(
    ctx: &CompileContext,
    expr: &Expr,
    func: &Function,
) -> Result<StringExprType, Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "concat" | "insert" | "replace" => {
            let args = collect_positional_args(func);
            let Some(first) = args.first() else {
                return Err(unknown_string_type(
                    func.name.span(),
                    "a string function call with no arguments",
                ));
            };
            let first = string_expr_type(ctx, first)?;
            let second = match args.get(1) {
                Some(second) => string_expr_type(ctx, second)?.max_length,
                None => 0,
            };
            Ok(StringExprType {
                char_width: first.char_width,
                max_length: first.max_length.saturating_add(second),
            })
        }
        "left" | "right" | "mid" | "delete" => match collect_positional_args(func).first() {
            Some(first) => string_expr_type(ctx, first),
            None => Err(unknown_string_type(
                func.name.span(),
                "a string function call with no arguments",
            )),
        },
        _ => match ctx
            .user_functions
            .get(name.as_str())
            .and_then(|info| info.return_string_info.as_ref())
        {
            Some(info) => Ok(StringExprType {
                char_width: info.char_width,
                max_length: info.max_length,
            }),
            None => resolved_string_type(expr, func.name.span()),
        },
    }
}

/// The type an expression's analyzer-assigned type names.
///
/// Every remaining call in a string position is one whose result the analyzer
/// typed, so this answers for all of them. In practice they are all narrow:
/// the calls that reach here are the ones that are neither a bounded
/// standard function nor a user function with a declared string return,
/// which leaves the `*_TO_STRING` conversions, and every one of those builds
/// Latin-1. `WSTRING` is mapped because it is what the name means, not
/// because a program can currently produce it -- `parse_string_conversion`
/// has no `*_TO_WSTRING` form, and `SEL`/`MUX` reject a string argument. A
/// wide result reaches its caller through the user-function branch above,
/// which `end_to_end_wstring` covers.
///
/// The analyzer's type name carries no length, so the bound is the default
/// capacity: a conversion renders a number, which is far shorter than that.
///
/// Failing to answer means the analyzer typed a string-position expression
/// as something that is not a string, which is a defect in the compiler
/// rather than in the program being compiled -- so it is reported as one,
/// and the diagnostic names what was found instead. An internal error that
/// does not say enough to debug it is only half a report.
fn resolved_string_type(expr: &Expr, span: SourceSpan) -> Result<StringExprType, Diagnostic> {
    let Some(type_name) = expr.resolved_type.as_ref() else {
        return Err(unknown_string_type(
            span,
            "a function call the analyzer left untyped",
        ));
    };

    let char_width = match ElementaryTypeName::try_from(&type_name.name) {
        Ok(ElementaryTypeName::STRING) => CharWidth::Narrow,
        Ok(ElementaryTypeName::WSTRING) => CharWidth::Wide,
        _ => {
            return Err(unknown_string_type(
                span,
                &format!("a function call the analyzer typed as {type_name}"),
            ))
        }
    };
    Ok(StringExprType {
        char_width,
        max_length: DEFAULT_STRING_MAX_LENGTH,
    })
}

/// Clamps a length in code units to the string header's capacity field.
///
/// A slot records its capacity as a `u16` (ADR-0035), so 65,535 code units
/// is the most any string can be materialized as. A bound above that is not
/// an error in the program -- the operands are each within range -- and it
/// is capped rather than wrapped so that it stays an over-approximation.
fn saturate_length<T: TryInto<u16>>(length: T) -> u16 {
    length.try_into().unwrap_or(u16::MAX)
}

/// Reports that codegen could not determine a string expression's type.
fn unknown_string_type(span: SourceSpan, what: &str) -> Diagnostic {
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
    use ironplc_dsl::common::{CharacterStringLiteral, TypeName};
    use ironplc_dsl::core::Id;
    use ironplc_dsl::textual::{ParamAssignmentKind, PositionalInput};
    use rstest::rstest;

    use super::*;
    use crate::compile::StringVarInfo;

    /// An expression carrying `type_name` as the type the analyzer resolved.
    /// `resolved_string_type` reads only that, so the kind is arbitrary.
    fn typed_as(type_name: &str) -> Expr {
        Expr::with_type(
            ExprKind::Null(SourceSpan::default()),
            TypeName::from(type_name),
        )
    }

    /// A narrow literal of `len` code units.
    fn literal(len: usize) -> Expr {
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(
            CharacterStringLiteral::new(vec!['a'; len]),
        )))
    }

    /// A wide literal of `len` code units.
    fn wide_literal(len: usize) -> Expr {
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(
            CharacterStringLiteral::new_wide(vec!['a'; len]),
        )))
    }

    /// A call of the standard function `name` with positional `args`.
    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::new(ExprKind::Function(Function {
            name: Id::from(name),
            param_assignment: args
                .into_iter()
                .map(|expr| ParamAssignmentKind::PositionalInput(PositionalInput { expr }))
                .collect(),
        }))
    }

    /// A context that declares one narrow string variable `s` of capacity
    /// `max_length`.
    fn context_with_string(max_length: u16) -> CompileContext {
        let mut ctx = CompileContext::new();
        ctx.string_vars.insert(
            Id::from("s"),
            StringVarInfo {
                data_offset: 0,
                max_length,
                char_width: CharWidth::Narrow,
            },
        );
        ctx
    }

    #[rstest]
    #[case::empty(0, 0)]
    #[case::short(5, 5)]
    #[case::above_default(300, 300)]
    fn string_expr_type_when_literal_then_bound_is_its_length(
        #[case] len: usize,
        #[case] expected: u16,
    ) {
        let ctx = CompileContext::new();

        let string_type = string_expr_type(&ctx, &literal(len)).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Narrow,
                max_length: expected,
            }
        );
    }

    #[test]
    fn string_expr_type_when_wide_literal_then_wide_with_its_length() {
        let ctx = CompileContext::new();

        let string_type = string_expr_type(&ctx, &wide_literal(300)).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Wide,
                max_length: 300,
            }
        );
    }

    #[test]
    fn string_expr_type_when_parenthesized_then_inner_type() {
        let ctx = CompileContext::new();
        let expr = Expr::new(ExprKind::Expression(Box::new(literal(7))));

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(string_type.max_length, 7);
    }

    #[test]
    fn string_expr_type_when_named_variable_then_declared_capacity() {
        let ctx = context_with_string(300);
        let expr = Expr::new(ExprKind::named_variable("s"));

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Narrow,
                max_length: 300,
            }
        );
    }

    #[test]
    fn string_expr_type_when_undeclared_variable_then_internal_error() {
        let ctx = CompileContext::new();
        let expr = Expr::new(ExprKind::named_variable("missing"));

        let diagnostic = string_expr_type(&ctx, &expr).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::insert("insert")]
    #[case::replace("replace")]
    fn string_expr_type_when_joining_function_then_bound_is_sum_of_both(#[case] name: &str) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![literal(128), literal(128)]);

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(string_type.max_length, 256);
    }

    #[rstest]
    #[case::left("left")]
    #[case::right("right")]
    #[case::mid("mid")]
    #[case::delete("delete")]
    fn string_expr_type_when_shortening_function_then_bound_is_first_argument(#[case] name: &str) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![literal(300), literal(5)]);

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(string_type.max_length, 300);
    }

    #[test]
    fn string_expr_type_when_nested_concat_then_bounds_add_recursively() {
        let ctx = context_with_string(100);
        let s = || Expr::new(ExprKind::named_variable("s"));
        let expr = call("concat", vec![call("concat", vec![s(), s()]), s()]);

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(string_type.max_length, 300);
    }

    #[test]
    fn string_expr_type_when_concat_of_wide_literals_then_wide() {
        let ctx = CompileContext::new();
        let expr = call("concat", vec![wide_literal(3), wide_literal(4)]);

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Wide,
                max_length: 7,
            }
        );
    }

    #[test]
    fn string_expr_type_when_sum_exceeds_header_capacity_then_saturates() {
        let ctx = CompileContext::new();
        let expr = call("concat", vec![literal(40_000), literal(40_000)]);

        let string_type = string_expr_type(&ctx, &expr).unwrap();

        assert_eq!(string_type.max_length, u16::MAX);
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::left("left")]
    fn string_expr_type_when_string_function_has_no_arguments_then_internal_error(
        #[case] name: &str,
    ) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![]);

        let diagnostic = string_expr_type(&ctx, &expr).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }

    #[test]
    fn string_expr_char_width_when_literal_then_its_width() {
        let ctx = CompileContext::new();

        let width = string_expr_char_width(&ctx, &wide_literal(1)).unwrap();

        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn string_type_of_when_string_without_length_then_default_capacity() {
        let field_type = IntermediateType::String {
            max_len: None,
            char_width: CharWidth::Narrow,
        };

        let string_type = string_type_of(&field_type).unwrap();

        assert_eq!(string_type.max_length, DEFAULT_STRING_MAX_LENGTH);
    }

    #[test]
    fn string_type_of_when_string_with_length_then_that_capacity() {
        let field_type = IntermediateType::String {
            max_len: Some(300),
            char_width: CharWidth::Wide,
        };

        let string_type = string_type_of(&field_type).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Wide,
                max_length: 300,
            }
        );
    }

    #[test]
    fn string_type_of_when_length_exceeds_header_capacity_then_saturates() {
        let field_type = IntermediateType::String {
            max_len: Some(1 << 20),
            char_width: CharWidth::Narrow,
        };

        let string_type = string_type_of(&field_type).unwrap();

        assert_eq!(string_type.max_length, u16::MAX);
    }

    #[test]
    fn string_type_of_when_not_a_string_then_none() {
        assert_eq!(string_type_of(&IntermediateType::Bool), None);
    }

    #[test]
    fn resolved_string_type_when_string_then_narrow_with_default_capacity() {
        let string_type = resolved_string_type(&typed_as("STRING"), SourceSpan::default()).unwrap();

        assert_eq!(
            string_type,
            StringExprType {
                char_width: CharWidth::Narrow,
                max_length: DEFAULT_STRING_MAX_LENGTH,
            }
        );
    }

    #[test]
    fn resolved_string_type_when_wstring_then_wide() {
        let string_type =
            resolved_string_type(&typed_as("WSTRING"), SourceSpan::default()).unwrap();

        assert_eq!(string_type.char_width, CharWidth::Wide);
    }

    #[test]
    fn resolved_string_type_when_typed_as_non_string_then_internal_error_names_the_type() {
        // Unreachable by construction -- the analyzer has established that a
        // string argument is a string. Should it happen anyway, it stops the
        // compile and says what it found, rather than passing a guessed width
        // down to an encoding-mismatch trap at run time.
        let diagnostic = resolved_string_type(&typed_as("INT"), SourceSpan::default()).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(
            diagnostic.primary.message.contains("INT"),
            "the message should name what was found, got: {}",
            diagnostic.primary.message
        );
    }

    #[test]
    fn resolved_string_type_when_untyped_then_internal_error() {
        let untyped = Expr::new(ExprKind::Null(SourceSpan::default()));
        let diagnostic = resolved_string_type(&untyped, SourceSpan::default()).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(diagnostic.primary.message.contains("untyped"));
    }
}
