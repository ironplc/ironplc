//! The encoding and the capacity of a string expression.
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
//! The encoding is the analyzer's answer: every string expression has the
//! `STRING` or `WSTRING` type it recorded, which [`string_expr_char_width`]
//! reads. The capacity is not recorded -- a literal is as long as it is
//! spelled, and a call can produce no more than its own string arguments
//! allow -- so [`string_expr_bound`] works it out by walking the expression;
//! the helpers below it are the cases it delegates to.
//!
//! Two further questions belong with it. An operation with several string
//! operands needs them to share an encoding, which
//! [`resolve_operand_char_width`] settles: operands that disagree have no
//! encoding in common, which the analyzer rejects. And a value being written
//! into a declared destination is *encoded for* that destination rather than
//! checked against it, which is [`compile_string_value`].

use ironplc_analyzer::{BuiltinFunction, SemanticType, StringFunction};
use ironplc_container::CharWidth;
use ironplc_dsl::common::{ConstantKind, ElementaryTypeName};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, Function, SymbolicVariableKind, Variable};

use super::call_args::collect_input_args;
use super::compile::{
    emit_string_literal_load, CompileContext, DEFAULT_STRING_MAX_LENGTH, NARROW_CHAR_WIDTH,
};
use super::compile_expr::{compile_expr, variable_span};
use super::type_info::expr_representation;
use crate::emit::Emitter;

/// Returns the encoding a string-valued expression produces: the one its
/// `STRING` or `WSTRING` type states, which the analyzer recorded.
///
/// Every string slot records its encoding in its header and the VM rejects a
/// store whose source and destination disagree (ADR-0034), so the temporary
/// that [`crate::compile_string::resolve_string_arg`] allocates has to be
/// initialized at the width the expression yields rather than at a fixed
/// one.
///
/// An expression the analyzer did not type as a string is a compiler bug
/// rather than a program error -- the analyzer has already established that
/// this argument is a string. It is reported as one instead of guessing a
/// width, which would defer the same problem to an encoding-mismatch trap at
/// run time.
pub(crate) fn string_expr_char_width(
    ctx: &CompileContext,
    expr: &Expr,
) -> Result<CharWidth, Diagnostic> {
    match expr_representation(ctx, expr) {
        Some(SemanticType::String { char_width, .. }) => Ok(*char_width),
        _ => Err(Diagnostic::internal_error_at(Label::span(
            expr.span(),
            "Cannot determine the string encoding of an expression the analyzer did not type as a string",
        ))),
    }
}

/// Returns the most code units a string-valued expression can produce: the
/// capacity a declaration gives it, the length a literal is spelled at, or
/// the widest result a call can build from its string arguments. `None` for
/// a call whose result states no length, which is held at the capacity a
/// bare `STRING` declaration would give it.
///
/// Knowing it is what keeps `LEN(x[1])`, `LEN('...')` and `LEN(CONCAT(a, b))`
/// from answering with a truncated copy's length when the operand is wider
/// than the default.
///
/// A bound is a static over-approximation, never a measurement. It saturates
/// at the header ceiling (ADR-0035): a slot records its capacity as a `u16`,
/// so no expression can be materialized wider than that whatever its
/// operands add up to.
///
/// An expression whose bound cannot be worked out is a compiler bug rather
/// than a program error, and is reported as one.
fn string_expr_bound(ctx: &CompileContext, expr: &Expr) -> Result<Option<u16>, Diagnostic> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::CharacterString(lit)) => {
            Ok(Some(saturate_length(lit.value.len())))
        }
        ExprKind::Expression(inner) => string_expr_bound(ctx, inner),
        ExprKind::Variable(variable) => variable_bound(ctx, variable),
        ExprKind::Function(func) => function_bound(ctx, func),
        _ => Err(unknown_string_bound(
            expr.span(),
            "a string expression of an unexpected kind",
        )),
    }
}

/// Returns the bound of a string variable, array element or structure field.
///
/// Subscripts and dereferences do not change the capacity -- it belongs to
/// the element rather than to the array -- so the access is walked back to
/// the variable it is rooted in: a name, resolved against the declared
/// strings and string arrays, or a structure field, whose declared type
/// carries it.
fn variable_bound(ctx: &CompileContext, variable: &Variable) -> Result<Option<u16>, Diagnostic> {
    let Variable::Symbolic(kind) = variable else {
        return Err(unknown_string_bound(
            variable_span(variable),
            "a directly represented variable",
        ));
    };

    match access_root(kind) {
        SymbolicVariableKind::Named(named) => {
            if let Some(info) = ctx.string_var(&named.name) {
                return Ok(Some(info.max_length));
            }
            ctx.array_var(&named.name)
                .filter(|info| info.is_string_element)
                .map(|info| Some(info.string_max_len))
                .ok_or_else(|| {
                    unknown_string_bound(
                        variable_span(variable),
                        "a variable that is not a declared string",
                    )
                })
        }
        SymbolicVariableKind::Structured(structured) => {
            // `a[i].s` -- the record is an array element, which the struct
            // chain walk cannot follow; the array-of-struct path locates it.
            let field_type =
                if matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) {
                    crate::compile_array_struct::struct_array_element_field_type(ctx, structured)
                } else {
                    crate::compile_struct::walk_struct_chain(
                        ctx,
                        &structured.record,
                        &structured.field,
                        0,
                    )
                    .map(|(_, _, field_type)| field_type)
                }
                .map_err(|_| {
                    unknown_string_bound(variable_span(variable), "an unresolvable structure field")
                })?;
            string_bound_of(&field_type).ok_or_else(|| {
                unknown_string_bound(
                    variable_span(variable),
                    "a structure field that is not a string",
                )
            })
        }
        _ => Err(unknown_string_bound(
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

/// Returns the bound of a STRING type, or of a STRING array's element, or
/// `None` when `field_type` is neither.
///
/// A declaration that names no length -- a bare `STRING` field -- has no
/// capacity of its own and takes the default. So does one whose length does
/// not fit a `u16`, which is not a length any slot can be given: the analyzer
/// rejects it before codegen sees the field, and answering no bound here
/// keeps an unreachable case from silently wrapping to a small capacity.
fn string_bound_of(field_type: &SemanticType) -> Option<Option<u16>> {
    match field_type {
        SemanticType::String { max_len, .. } => {
            Some(max_len.and_then(|len| u16::try_from(len).ok()))
        }
        SemanticType::Array { element_type, .. } => string_bound_of(element_type),
        _ => None,
    }
}

/// Returns the bound of a function call's string result.
///
/// The bound of a standard string function follows from what it does:
/// `CONCAT`, `INSERT` and `REPLACE` can hand back every code unit of both
/// string arguments, so their bound is the sum; `LEFT`, `RIGHT`, `MID` and
/// `DELETE` only ever drop code units from their one string argument, so its
/// bound is theirs. A user-defined function declares its return type. The
/// only other call that yields a string is a `*_TO_STRING` conversion, which
/// states no length of its own.
///
/// That is the whole set: no other standard function yields a string. A
/// call outside the set in a string position is a compiler defect -- the
/// analyzer typed it as a string and codegen does not know how -- so it is
/// reported as one, naming the call, rather than sized at a guess.
fn function_bound(ctx: &CompileContext, func: &Function) -> Result<Option<u16>, Diagnostic> {
    match ctx.builtin_functions.get(&func.name) {
        Some(BuiltinFunction::String(function)) => string_function_bound(ctx, func, *function),
        Some(BuiltinFunction::Conversion {
            target: ElementaryTypeName::STRING,
            ..
        }) => Ok(None),
        Some(_) => Err(yields_no_string(func)),
        None => match ctx
            .user_function(func.name.lower_case())
            .and_then(|info| info.return_string_info.as_ref())
        {
            Some(info) => Ok(Some(info.max_length)),
            None => Err(yields_no_string(func)),
        },
    }
}

/// Returns the bound of the result of a call to the standard string
/// function `function`, as [`function_bound`] describes.
fn string_function_bound(
    ctx: &CompileContext,
    func: &Function,
    function: StringFunction,
) -> Result<Option<u16>, Diagnostic> {
    match function {
        StringFunction::Concat | StringFunction::Insert | StringFunction::Replace => {
            let args = collect_input_args(func);
            let Some(first) = args.first() else {
                return Err(unknown_string_bound(
                    func.name.span(),
                    "a string function call with no arguments",
                ));
            };
            let first = string_expr_bound(ctx, first)?;
            let second = match args.get(1) {
                Some(second) => bound_or_default(string_expr_bound(ctx, second)?),
                None => 0,
            };
            Ok(Some(bound_or_default(first).saturating_add(second)))
        }
        StringFunction::Left
        | StringFunction::Right
        | StringFunction::Mid
        | StringFunction::Delete => match collect_input_args(func).first() {
            Some(first) => string_expr_bound(ctx, first),
            None => Err(unknown_string_bound(
                func.name.span(),
                "a string function call with no arguments",
            )),
        },
        StringFunction::Len | StringFunction::Find => Err(yields_no_string(func)),
    }
}

/// Reports that a call in a string position yields no string.
fn yields_no_string(func: &Function) -> Diagnostic {
    unknown_string_bound(
        func.name.span(),
        &format!(
            "a call of {}, which yields no string",
            func.name.lower_case()
        ),
    )
}

/// The bound an operand contributes to a call's result: its own, or the
/// default capacity where it states none.
fn bound_or_default(bound: Option<u16>) -> u16 {
    bound.unwrap_or(DEFAULT_STRING_MAX_LENGTH)
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

/// Reports that codegen could not determine a string expression's bound.
fn unknown_string_bound(span: SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span,
        format!("Cannot determine the string capacity of {what}"),
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
/// An expression whose bound cannot be worked out gets that default too.
/// Sizing a slot is not the place to discover that a string operand is not a
/// string: the caller already holds an encoding for it, obtained from the
/// declared destination where this module could not name one, and reporting
/// here would turn that accommodation into a hard error.
pub(crate) fn string_operand_capacity(ctx: &CompileContext, expr: &Expr) -> u16 {
    string_expr_bound(ctx, expr)
        .ok()
        .flatten()
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
/// `w = 'abc'` comparing one against a `STRING` literal -- that analysis
/// reports (`rule_string_encoding_compat`, P4034), so reaching here with one
/// is a compiler bug. It is an internal error rather than bytecode for the VM
/// to trap on one scan later.
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
/// Analysis rejects a literal whose delimiter spells the other encoding from
/// its destination -- P4035 for a named variable, P4026 for a function
/// parameter, P4034 for an array element or a structure field -- so a literal
/// that reaches here spells the destination's encoding unless the caller
/// compiled without analysis. The destination decides then.
///
/// Any other expression carries an encoding of its own, and one that is not
/// `char_width` has no valid bytecode for the store the caller is about to
/// emit. Analysis reports that too (`rule_string_encoding_compat`), so it is
/// an internal error here. An encoding codegen cannot work out is left to the
/// destination, which is the one that decides the store.
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

    compile_expr(emitter, ctx, expr)
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

/// The error for two string encodings that cannot be reconciled. Analysis
/// rejects every STRING/WSTRING mix (P4034, or P4026 for a function
/// argument), so reaching codegen with one is a compiler bug.
#[track_caller]
pub(crate) fn encoding_mismatch(
    expected: CharWidth,
    actual: CharWidth,
    span: &SourceSpan,
) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "String operand of an encoding the operation does not share",
    ))
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
    use crate::compile_call::builtin_functions_by_name;
    use ironplc_analyzer::FunctionEnvironmentBuilder;

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

    /// A `*_TO_STRING` conversion call.
    fn conversion() -> Expr {
        call("int_to_string", vec![literal(0)])
    }

    /// A context that knows the standard functions, as one compiling an
    /// analyzed program does.
    fn context() -> CompileContext {
        let mut ctx = CompileContext::new();
        ctx.builtin_functions = builtin_functions_by_name(
            &FunctionEnvironmentBuilder::new()
                .with_stdlib_functions()
                .build(),
        );
        ctx
    }

    /// A context that declares one narrow string variable `s` of capacity
    /// `max_length`.
    fn context_with_string(max_length: u16) -> CompileContext {
        let mut ctx = context();
        ctx.string_vars.insert(
            ctx.scope,
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
    fn string_expr_bound_when_literal_then_its_length(#[case] len: usize, #[case] expected: u16) {
        let ctx = context();

        assert_eq!(
            string_expr_bound(&ctx, &literal(len)).unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn string_expr_bound_when_wide_literal_then_its_length() {
        let ctx = context();

        assert_eq!(
            string_expr_bound(&ctx, &wide_literal(300)).unwrap(),
            Some(300)
        );
    }

    #[test]
    fn string_expr_bound_when_parenthesized_then_inner_bound() {
        let ctx = context();
        let expr = Expr::new(ExprKind::Expression(Box::new(literal(7))));

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(7));
    }

    #[test]
    fn string_expr_bound_when_named_variable_then_declared_capacity() {
        let ctx = context_with_string(300);
        let expr = Expr::new(ExprKind::named_variable("s"));

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(300));
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::insert("insert")]
    #[case::replace("replace")]
    fn string_expr_bound_when_joining_function_then_sum_of_both(#[case] name: &str) {
        let ctx = context();
        let expr = call(name, vec![literal(128), literal(128)]);

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(256));
    }

    #[rstest]
    #[case::left("left")]
    #[case::right("right")]
    #[case::mid("mid")]
    #[case::delete("delete")]
    fn string_expr_bound_when_shortening_function_then_first_argument(#[case] name: &str) {
        let ctx = context();
        let expr = call(name, vec![literal(300), literal(5)]);

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(300));
    }

    #[test]
    fn string_expr_bound_when_nested_concat_then_bounds_add_recursively() {
        let ctx = context_with_string(100);
        let s = || Expr::new(ExprKind::named_variable("s"));
        let expr = call("concat", vec![call("concat", vec![s(), s()]), s()]);

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(300));
    }

    #[test]
    fn string_expr_bound_when_concat_of_wide_literals_then_sum_of_both() {
        let ctx = context();
        let expr = call("concat", vec![wide_literal(3), wide_literal(4)]);

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(7));
    }

    #[test]
    fn string_expr_bound_when_concat_operand_states_no_bound_then_default_is_added() {
        let ctx = context();
        let expr = call("concat", vec![literal(10), conversion()]);

        assert_eq!(
            string_expr_bound(&ctx, &expr).unwrap(),
            Some(10 + DEFAULT_STRING_MAX_LENGTH)
        );
    }

    #[test]
    fn string_expr_bound_when_sum_exceeds_header_capacity_then_saturates() {
        let ctx = context();
        let expr = call("concat", vec![literal(40_000), literal(40_000)]);

        assert_eq!(string_expr_bound(&ctx, &expr).unwrap(), Some(u16::MAX));
    }

    #[rstest]
    #[case::concat("concat")]
    #[case::left("left")]
    fn string_expr_bound_when_string_function_has_no_arguments_then_internal_error(
        #[case] name: &str,
    ) {
        let ctx = context();
        let expr = call(name, vec![]);

        let diagnostic = string_expr_bound(&ctx, &expr).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }

    #[test]
    fn string_expr_bound_when_conversion_then_no_bound() {
        let ctx = context();

        assert_eq!(string_expr_bound(&ctx, &conversion()).unwrap(), None);
    }

    #[test]
    fn string_operand_capacity_when_long_literal_then_its_length() {
        let ctx = context();

        assert_eq!(string_operand_capacity(&ctx, &literal(300)), 300);
    }

    #[test]
    fn string_operand_capacity_when_no_bound_then_default() {
        let ctx = context();

        assert_eq!(
            string_operand_capacity(&ctx, &conversion()),
            DEFAULT_STRING_MAX_LENGTH
        );
    }

    #[rstest]
    #[case::numeric_builtin("abs")]
    #[case::string_to_num("string_to_int")]
    #[case::unknown_user_function("my_func")]
    fn string_expr_bound_when_call_yields_no_string_then_internal_error_names_the_call(
        #[case] name: &str,
    ) {
        // Unreachable by construction -- the analyzer has established that a
        // string argument is a string. Should it happen anyway, it stops the
        // compile and says which call it could not size, rather than passing
        // a guessed capacity down to a silent truncation at run time.
        let ctx = context();

        let diagnostic = string_expr_bound(&ctx, &call(name, vec![literal(1)])).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(
            diagnostic.primary.message.contains(name),
            "the message should name the call, got: {}",
            diagnostic.primary.message
        );
    }

    #[test]
    fn string_expr_char_width_when_not_typed_as_string_then_internal_error() {
        // The analyzer types every string expression. One it did not is
        // reported rather than given a guessed encoding.
        let ctx = context();

        let diagnostic = string_expr_char_width(&ctx, &literal(3)).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }
}
