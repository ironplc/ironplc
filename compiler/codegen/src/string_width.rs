//! The shape of a string expression: its encoding and its capacity.
//!
//! `STRING` and `WSTRING` are the same shape to almost everything in codegen —
//! a data-region slot with a header, addressed by a byte offset — and differ
//! only in the per-code-unit byte width recorded in that header: Latin-1 at
//! one byte, UTF-16LE at two (ADR-0016, ADR-0035). Every string opcode checks
//! that width at runtime and traps (`V9014`) when a source and a destination
//! disagree, so the temporary that [`crate::compile_string::resolve_string_arg`]
//! allocates for an operand has to be initialized at the width that operand
//! yields. That temporary also has to be wide enough to hold the operand: a
//! slot narrower than the value stored into it truncates it, and `LEN` of the
//! truncated copy is not the length of the operand.
//!
//! Both are questions about types rather than about bytecode, and they are the
//! questions this module exists to answer. Whatever states an operand's
//! encoding states its capacity too -- a declaration names `STRING[n]`, a
//! literal is as long as it is spelled, and a call can produce no more than
//! its own string arguments allow -- so [`string_expr_shape`] answers both
//! from one walk; the helpers below it are the cases it delegates to.
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
use ironplc_dsl::common::ConstantKind;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, Function, SymbolicVariableKind, Variable};

use ironplc_problems::Problem;

use super::compile::{
    char_width_for_string_type, emit_string_literal_load, CompileContext, DEFAULT_OP_TYPE,
    DEFAULT_STRING_MAX_LENGTH, NARROW_CHAR_WIDTH,
};
use super::compile_call::{parse_string_conversion, StringConversion};
use super::compile_expr::{compile_expr, variable_span};
use super::compile_string::collect_positional_args;
use crate::emit::Emitter;

/// What a string-valued expression is, before any bytecode runs.
///
/// The two properties travel together because the same declaration states
/// both, and because the temporary that holds an operand needs both to be
/// initialized: `STR_INIT` writes the capacity and the encoding into the
/// slot's header in one go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StringShape {
    /// Per-code-unit byte width: `Narrow` for STRING, `Wide` for WSTRING.
    pub(crate) char_width: CharWidth,
    /// The most code units the expression can produce: the capacity a
    /// declaration gives it, the length a literal is spelled at, or the
    /// widest result a call can build from its string arguments. `None` for
    /// a call whose result the analyzer typed by name alone, which states no
    /// length; such a value is held at the capacity a bare `STRING`
    /// declaration would give it.
    ///
    /// A bound is a static over-approximation, never a measurement. It
    /// saturates at the header ceiling (ADR-0035): a slot records its
    /// capacity as a `u16`, so no expression can be materialized wider than
    /// that whatever its operands add up to.
    pub(crate) max_length: Option<u16>,
}

/// Returns the encoding and capacity a string-valued expression produces.
///
/// Every string slot records its encoding in its header and the VM rejects a
/// store whose source and destination disagree (ADR-0034), so the temporary
/// that [`resolve_string_arg`] allocates has to be initialized at the width
/// the expression yields rather than at a fixed one. The width is always
/// known at compile time: a literal spells it, a declaration states it, and
/// every string function returns the encoding of its first string argument.
///
/// The capacity is known wherever the expression states one -- a
/// declaration, a literal's own length, or a call's string arguments --
/// which is what keeps `LEN(x[1])`, `LEN('...')` and `LEN(CONCAT(a, b))`
/// from answering with a truncated copy's length when the operand is wider
/// than the default.
///
/// An expression whose width cannot be determined is a compiler bug rather
/// than a program error -- the analyzer has already established that this
/// argument is a string. Report it as one instead of guessing a width, which
/// would defer the same problem to an encoding-mismatch trap at run time.
pub(crate) fn string_expr_shape(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<StringShape, Diagnostic> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::CharacterString(lit)) => Ok(StringShape {
            char_width: char_width_for_string_type(&lit.width),
            max_length: Some(saturate_length(lit.value.len())),
        }),
        ExprKind::Expression(inner) => string_expr_shape(ctx, inner),
        ExprKind::Variable(variable) => variable_shape(ctx, variable),
        ExprKind::Function(func) => function_shape(ctx, func),
        _ => Err(unknown_string_encoding(
            expr.span(),
            "a string expression of an unexpected kind",
        )),
    }
}

/// Returns the encoding a string-valued expression produces.
pub(crate) fn string_expr_char_width(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<CharWidth, Diagnostic> {
    string_expr_shape(ctx, expr).map(|shape| shape.char_width)
}

/// Returns the shape of a string variable, array element or structure field.
///
/// Subscripts and dereferences change neither the encoding nor the capacity --
/// both belong to the element rather than to the array -- so the access is
/// walked back to the variable it is rooted in: a name, resolved against the
/// declared strings and string arrays, or a structure field, whose declared
/// type carries them.
fn variable_shape(ctx: &CompileContext, variable: &Variable) -> Result<StringShape, Diagnostic> {
    let Variable::Symbolic(kind) = variable else {
        return Err(unknown_string_encoding(
            variable_span(variable),
            "a directly represented variable",
        ));
    };

    match access_root(kind) {
        SymbolicVariableKind::Named(named) => {
            if let Some(info) = ctx.string_vars.get(&named.name) {
                return Ok(StringShape {
                    char_width: info.char_width,
                    max_length: Some(info.max_length),
                });
            }
            ctx.array_vars
                .get(&named.name)
                .filter(|info| info.is_string_element)
                .map(|info| StringShape {
                    char_width: info.string_char_width,
                    max_length: Some(info.string_max_len),
                })
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
            string_shape_of(&field_type).ok_or_else(|| {
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

/// Returns the shape of a STRING type, or of a STRING array's element.
///
/// A declaration that names no length -- a bare `STRING` field -- has no
/// capacity of its own and takes the default. So does one whose length does
/// not fit a `u16`, which is not a length any slot can be given: the analyzer
/// rejects it before codegen sees the field, and answering `None` here keeps
/// an unreachable case from silently wrapping to a small capacity.
fn string_shape_of(field_type: &IntermediateType) -> Option<StringShape> {
    match field_type {
        IntermediateType::String {
            char_width,
            max_len,
        } => Some(StringShape {
            char_width: *char_width,
            max_length: max_len.and_then(|len| u16::try_from(len).ok()),
        }),
        IntermediateType::Array { element_type, .. } => string_shape_of(element_type),
        _ => None,
    }
}

/// Returns the shape of a function call's string result.
///
/// The standard string functions return the encoding of their first string
/// argument. Their bound follows from what they do: `CONCAT`, `INSERT` and
/// `REPLACE` can hand back every code unit of both string arguments, so
/// their bound is the sum; `LEFT`, `RIGHT`, `MID` and `DELETE` only ever
/// drop code units from their one string argument, so its bound is theirs.
/// A user-defined function declares its return type. The only other call
/// that yields a string is a `*_TO_STRING` conversion, which renders a
/// number as Latin-1 and states no length of its own.
///
/// That is the whole set: `compile_function_call` routes every other name
/// to a numeric conversion or a generic builtin, none of which leaves a
/// string. A call outside the set in a string position is a compiler defect
/// -- the analyzer typed it as a string and codegen does not know how -- so
/// it is reported as one, naming the call, rather than sized at a guess.
fn function_shape(ctx: &CompileContext, func: &Function) -> Result<StringShape, Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "concat" | "insert" | "replace" => {
            let args = collect_positional_args(func);
            let Some(first) = args.first() else {
                return Err(unknown_string_encoding(
                    func.name.span(),
                    "a string function call with no arguments",
                ));
            };
            let first = string_expr_shape(ctx, first)?;
            let second = match args.get(1) {
                Some(second) => bound_or_default(string_expr_shape(ctx, second)?),
                None => 0,
            };
            Ok(StringShape {
                char_width: first.char_width,
                max_length: Some(bound_or_default(first).saturating_add(second)),
            })
        }
        "left" | "right" | "mid" | "delete" => match collect_positional_args(func).first() {
            Some(first) => string_expr_shape(ctx, first),
            None => Err(unknown_string_encoding(
                func.name.span(),
                "a string function call with no arguments",
            )),
        },
        _ => {
            if let Some(info) = ctx
                .user_functions
                .get(name.as_str())
                .and_then(|info| info.return_string_info.as_ref())
            {
                return Ok(StringShape {
                    char_width: info.char_width,
                    max_length: Some(info.max_length),
                });
            }
            match parse_string_conversion(name.as_str()) {
                Some(StringConversion::NumToString { .. }) => Ok(StringShape {
                    char_width: NARROW_CHAR_WIDTH,
                    max_length: None,
                }),
                _ => Err(unknown_string_encoding(
                    func.name.span(),
                    &format!("a call of {name}, which yields no string"),
                )),
            }
        }
    }
}

/// The bound an operand contributes to a call's result: its own, or the
/// default capacity where it states none.
fn bound_or_default(shape: StringShape) -> u16 {
    shape.max_length.unwrap_or(DEFAULT_STRING_MAX_LENGTH)
}

/// Clamps a length in code units to the string header's capacity field.
///
/// A slot records its capacity as a `u16` (ADR-0035), so 65,535 code units
/// is the most any string can be materialized as. A bound above that is not
/// an error in the program -- each operand is within range on its own -- and
/// it is capped rather than wrapped so that it stays an over-approximation.
fn saturate_length(length: usize) -> u16 {
    u16::try_from(length).unwrap_or(u16::MAX)
}

/// Reports that codegen could not determine a string expression's encoding.
fn unknown_string_encoding(span: SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span,
        format!("Cannot determine the string encoding of {what}"),
    ))
}

/// The capacity to give the temporary data-region slot that holds `expr`.
///
/// An operand states its own bound -- a declared capacity, a literal's
/// length, or the most a call can build -- and a temporary narrower than
/// that truncates the value copied into it, which `LEN` then reports as the
/// operand's length and a comparison then judges unequal to its own source.
/// An expression that states no bound gets the capacity a bare `STRING`
/// declaration would give it.
///
/// An expression whose shape cannot be worked out gets that default too.
/// Sizing a slot is not the place to discover that a string operand is not a
/// string: the caller already holds an encoding for it, obtained from the
/// declared destination where this module could not name one, and reporting
/// here would turn that accommodation into a hard error.
pub(crate) fn string_operand_capacity(ctx: &CompileContext, expr: &Expr) -> u16 {
    string_expr_shape(ctx, expr)
        .ok()
        .and_then(|shape| shape.max_length)
        .unwrap_or(DEFAULT_STRING_MAX_LENGTH)
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
    use ironplc_dsl::common::CharacterStringLiteral;
    use ironplc_dsl::core::Id;
    use ironplc_dsl::textual::{ParamAssignmentKind, PositionalInput};
    use rstest::rstest;

    use super::*;
    use crate::compile::StringVarInfo;

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

    /// A `*_TO_STRING` conversion call, which no function table knows.
    fn conversion() -> Expr {
        call("int_to_string", vec![literal(0)])
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
    fn string_expr_shape_when_literal_then_bound_is_its_length(
        #[case] len: usize,
        #[case] expected: u16,
    ) {
        let ctx = CompileContext::new();

        let shape = string_expr_shape(&ctx, &literal(len)).unwrap();

        assert_eq!(
            shape,
            StringShape {
                char_width: CharWidth::Narrow,
                max_length: Some(expected),
            }
        );
    }

    #[test]
    fn string_expr_shape_when_wide_literal_then_wide_with_its_length() {
        let ctx = CompileContext::new();

        let shape = string_expr_shape(&ctx, &wide_literal(300)).unwrap();

        assert_eq!(
            shape,
            StringShape {
                char_width: CharWidth::Wide,
                max_length: Some(300),
            }
        );
    }

    #[test]
    fn string_expr_shape_when_parenthesized_then_inner_shape() {
        let ctx = CompileContext::new();
        let expr = Expr::new(ExprKind::Expression(Box::new(literal(7))));

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(7));
    }

    #[test]
    fn string_expr_shape_when_named_variable_then_declared_capacity() {
        let ctx = context_with_string(300);
        let expr = Expr::new(ExprKind::named_variable("s"));

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(
            shape,
            StringShape {
                char_width: CharWidth::Narrow,
                max_length: Some(300),
            }
        );
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::insert("insert")]
    #[case::replace("replace")]
    fn string_expr_shape_when_joining_function_then_bound_is_sum_of_both(#[case] name: &str) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![literal(128), literal(128)]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(256));
    }

    #[rstest]
    #[case::left("left")]
    #[case::right("right")]
    #[case::mid("mid")]
    #[case::delete("delete")]
    fn string_expr_shape_when_shortening_function_then_bound_is_first_argument(#[case] name: &str) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![literal(300), literal(5)]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(300));
    }

    #[test]
    fn string_expr_shape_when_nested_concat_then_bounds_add_recursively() {
        let ctx = context_with_string(100);
        let s = || Expr::new(ExprKind::named_variable("s"));
        let expr = call("concat", vec![call("concat", vec![s(), s()]), s()]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(300));
    }

    #[test]
    fn string_expr_shape_when_concat_of_wide_literals_then_wide() {
        let ctx = CompileContext::new();
        let expr = call("concat", vec![wide_literal(3), wide_literal(4)]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(
            shape,
            StringShape {
                char_width: CharWidth::Wide,
                max_length: Some(7),
            }
        );
    }

    #[test]
    fn string_expr_shape_when_concat_operand_states_no_bound_then_default_is_added() {
        let ctx = CompileContext::new();
        let expr = call("concat", vec![literal(10), conversion()]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(10 + DEFAULT_STRING_MAX_LENGTH));
    }

    #[test]
    fn string_expr_shape_when_sum_exceeds_header_capacity_then_saturates() {
        let ctx = CompileContext::new();
        let expr = call("concat", vec![literal(40_000), literal(40_000)]);

        let shape = string_expr_shape(&ctx, &expr).unwrap();

        assert_eq!(shape.max_length, Some(u16::MAX));
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::left("left")]
    fn string_expr_shape_when_string_function_has_no_arguments_then_internal_error(
        #[case] name: &str,
    ) {
        let ctx = CompileContext::new();
        let expr = call(name, vec![]);

        let diagnostic = string_expr_shape(&ctx, &expr).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }

    #[test]
    fn string_expr_shape_when_conversion_then_narrow_with_no_bound() {
        let ctx = CompileContext::new();

        let shape = string_expr_shape(&ctx, &conversion()).unwrap();

        assert_eq!(
            shape,
            StringShape {
                char_width: CharWidth::Narrow,
                max_length: None,
            }
        );
    }

    #[test]
    fn string_operand_capacity_when_long_literal_then_its_length() {
        let ctx = CompileContext::new();

        assert_eq!(string_operand_capacity(&ctx, &literal(300)), 300);
    }

    #[test]
    fn string_operand_capacity_when_no_bound_then_default() {
        let ctx = CompileContext::new();

        assert_eq!(
            string_operand_capacity(&ctx, &conversion()),
            DEFAULT_STRING_MAX_LENGTH
        );
    }

    #[rstest]
    #[case::numeric_builtin("abs")]
    #[case::string_to_num("string_to_int")]
    #[case::unknown_user_function("my_func")]
    fn string_expr_shape_when_call_yields_no_string_then_internal_error_names_the_call(
        #[case] name: &str,
    ) {
        // Unreachable by construction -- the analyzer has established that a
        // string argument is a string. Should it happen anyway, it stops the
        // compile and says which call it could not size, rather than passing
        // a guessed capacity down to a silent truncation at run time.
        let ctx = CompileContext::new();

        let diagnostic = string_expr_shape(&ctx, &call(name, vec![literal(1)])).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(
            diagnostic.primary.message.contains(name),
            "the message should name the call, got: {}",
            diagnostic.primary.message
        );
    }
}
