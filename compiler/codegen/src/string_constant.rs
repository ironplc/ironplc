//! The length of a string whose value cannot change.
//!
//! `LEN` of such a string is an integer constant, so `compile_len` loads it
//! instead of materializing the operand in a data-region slot and reading
//! `cur_length` back with `LEN_STR`. A string cannot change when it is:
//!
//! - a character string literal;
//! - a `CONCAT` of two such operands;
//! - a `CONSTANT` string variable with an initial value, whether the source
//!   declared it `CONSTANT` or `xform_mark_unwritten_constants` inferred it
//!   (see `specs/design/constant-variable-inference.md`).
//!
//! The fold is made here rather than by the analyzer's constant folding,
//! which runs before the semantic rules: replacing the call with a number
//! there would hide the literal and the call from the rules that check them.
//! And the value to fold is a code generation fact -- the length `LEN_STR`
//! would read, after the stores that put the operand in its slot have
//! truncated it to that slot's capacity.
//!
//! Every answer is the length in code units, which is what `LEN_STR` reads:
//! one per character for both `STRING` (Latin-1) and `WSTRING` (UCS-2), since
//! the parser has already decoded `$` escapes and P4052 rejects a character
//! the type cannot hold in one code unit.

use ironplc_dsl::common::{
    ConstantKind, DeclarationQualifier, StringInitializer, VarDecl, VariableType,
};
use ironplc_dsl::textual::{Expr, ExprKind, Function};

use super::compile::CompileContext;
use super::compile_expr::resolve_variable_name;
use super::compile_string::collect_positional_args;
use super::string_width::{saturate_length, string_expr_char_width};

/// Returns the length, in code units, `expr` has whenever it is evaluated,
/// or `None` when that length is only known at run time.
///
/// `None` is always a safe answer: the caller then compiles the operand and
/// reads its length at run time, reporting any problem with it on the way.
pub(crate) fn constant_string_length(ctx: &CompileContext, expr: &Expr) -> Option<u16> {
    match &expr.kind {
        // A slot's capacity is a `u16` (ADR-0035), so a literal longer than
        // that is stored, and measured, at that many code units.
        ExprKind::Const(ConstantKind::CharacterString(literal)) => {
            Some(saturate_length(literal.value.len()))
        }
        ExprKind::Expression(inner) => constant_string_length(ctx, inner),
        ExprKind::Variable(variable) => {
            ctx.string_vars
                .get(resolve_variable_name(variable)?)?
                .constant_length
        }
        ExprKind::Function(func) if func.name.lower_case().as_str() == "concat" => {
            concat_length(ctx, func)
        }
        _ => None,
    }
}

/// The length of `CONCAT(IN1, IN2)` when both operands have a constant
/// length: their sum, capped at the most a slot can hold, as the slot that
/// receives the result caps it.
///
/// Operands of different encodings have no result at all -- that is P4034,
/// which `compile_concat` reports -- so they are not folded.
fn concat_length(ctx: &CompileContext, func: &Function) -> Option<u16> {
    let [first, second] = collect_positional_args(func)[..] else {
        return None;
    };
    let first_width = string_expr_char_width(ctx, first).ok()?;
    let second_width = string_expr_char_width(ctx, second).ok()?;
    if first_width != second_width {
        return None;
    }
    let first = constant_string_length(ctx, first)?;
    let second = constant_string_length(ctx, second)?;
    Some(first.saturating_add(second))
}

/// Returns the length a string declaration holds for the whole run, or
/// `None` when the program may change it.
///
/// A `CONSTANT` declaration holds its initial value: after
/// `stages::resolve_types` the qualifier means exactly that, whether it was
/// written or inferred. The value is stored into a slot of `max_length` code
/// units, which truncates a longer one. Only the sections in which IEC
/// 61131-3 allows `CONSTANT` are folded; a parameter is written by its
/// caller whatever it is qualified with. A `CONSTANT` without an initial
/// value is P4008 and is not folded either.
pub(crate) fn declared_constant_length(
    decl: &VarDecl,
    string_init: &StringInitializer,
    max_length: u16,
) -> Option<u16> {
    let may_be_constant = matches!(
        decl.var_type,
        VariableType::Var | VariableType::VarTemp | VariableType::Global
    );
    if decl.qualifier != DeclarationQualifier::Constant || !may_be_constant {
        return None;
    }
    let initial_value = string_init.initial_value.as_ref()?;
    Some(saturate_length(initial_value.value.len()).min(max_length))
}

#[cfg(test)]
mod tests {
    use ironplc_container::CharWidth;
    use ironplc_dsl::common::{CharacterStringLiteral, InitialValueAssignmentKind, StringType};
    use ironplc_dsl::core::{Id, SourceSpan};
    use ironplc_dsl::textual::{ParamAssignmentKind, PositionalInput, Variable};
    use rstest::rstest;

    use super::*;
    use crate::compile::StringVarInfo;

    fn literal(value: &str) -> Expr {
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(
            CharacterStringLiteral::new(value.chars().collect()),
        )))
    }

    fn wide_literal(value: &str) -> Expr {
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(
            CharacterStringLiteral::new_wide(value.chars().collect()),
        )))
    }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::new(ExprKind::Function(Function {
            name: Id::from(name),
            param_assignment: args
                .into_iter()
                .map(|expr| ParamAssignmentKind::PositionalInput(PositionalInput { expr }))
                .collect(),
        }))
    }

    fn variable(name: &str) -> Expr {
        Expr::new(ExprKind::Variable(Variable::named(name)))
    }

    /// A context declaring the narrow string `s` with the given constant
    /// length, if any.
    fn context_with_string(constant_length: Option<u16>) -> CompileContext {
        let mut ctx = CompileContext::new();
        ctx.string_vars.insert(
            Id::from("s"),
            StringVarInfo {
                data_offset: 0,
                max_length: 20,
                char_width: CharWidth::Narrow,
                constant_length,
            },
        );
        ctx
    }

    #[rstest]
    #[case::empty(literal(""), Some(0))]
    #[case::narrow(literal("Hello"), Some(5))]
    #[case::wide(wide_literal("héllo"), Some(5))]
    #[case::parenthesized(Expr::new(ExprKind::Expression(Box::new(literal("abc")))), Some(3))]
    #[case::concat(call("CONCAT", vec![literal("ab"), literal("cde")]), Some(5))]
    #[case::concat_of_variable(call("concat", vec![variable("s"), literal("!")]), Some(4))]
    #[case::concat_mixed_encodings(call("CONCAT", vec![literal("a"), wide_literal("b")]), None)]
    #[case::concat_one_argument(call("CONCAT", vec![literal("a")]), None)]
    #[case::concat_of_unknown(call("CONCAT", vec![variable("unknown"), literal("b")]), None)]
    #[case::other_function(call("LEFT", vec![literal("abc"), literal("x")]), None)]
    #[case::constant_variable(variable("s"), Some(3))]
    #[case::undeclared_variable(variable("unknown"), None)]
    fn constant_string_length_when_expression_then_returns_expected(
        #[case] expr: Expr,
        #[case] expected: Option<u16>,
    ) {
        let ctx = context_with_string(Some(3));
        assert_eq!(constant_string_length(&ctx, &expr), expected);
    }

    #[test]
    fn constant_string_length_when_variable_not_constant_then_none() {
        let ctx = context_with_string(None);
        assert_eq!(constant_string_length(&ctx, &variable("s")), None);
    }

    #[test]
    fn constant_string_length_when_literal_longer_than_slot_then_saturates() {
        let ctx = CompileContext::new();
        let long = "a".repeat(70_000);
        assert_eq!(
            constant_string_length(&ctx, &literal(&long)),
            Some(u16::MAX)
        );
        let concat = call("CONCAT", vec![literal(&long), literal("b")]);
        assert_eq!(constant_string_length(&ctx, &concat), Some(u16::MAX));
    }

    /// The constant length of a string declaration of the given section and
    /// qualifier, initialized to `initial_value` when there is one, in a
    /// slot of 4 code units.
    fn declared(
        var_type: VariableType,
        qualifier: DeclarationQualifier,
        initial_value: Option<&str>,
    ) -> Option<u16> {
        let init = StringInitializer {
            length: None,
            width: StringType::String,
            initial_value: initial_value
                .map(|value| CharacterStringLiteral::new(value.chars().collect())),
            keyword_span: SourceSpan::default(),
        };
        let mut decl = VarDecl::string("s", var_type, qualifier);
        decl.initializer = InitialValueAssignmentKind::String(init.clone());
        declared_constant_length(&decl, &init, 4)
    }

    #[rstest]
    #[case::var(
        VariableType::Var,
        DeclarationQualifier::Constant,
        Some("abc"),
        Some(3)
    )]
    #[case::var_temp(
        VariableType::VarTemp,
        DeclarationQualifier::Constant,
        Some("abc"),
        Some(3)
    )]
    #[case::global(
        VariableType::Global,
        DeclarationQualifier::Constant,
        Some("abc"),
        Some(3)
    )]
    #[case::truncated(
        VariableType::Var,
        DeclarationQualifier::Constant,
        Some("Hello"),
        Some(4)
    )]
    #[case::not_constant(
        VariableType::Var,
        DeclarationQualifier::Unspecified,
        Some("abc"),
        None
    )]
    #[case::retain(VariableType::Var, DeclarationQualifier::Retain, Some("abc"), None)]
    #[case::input(VariableType::Input, DeclarationQualifier::Constant, Some("abc"), None)]
    #[case::no_initial_value(VariableType::Var, DeclarationQualifier::Constant, None, None)]
    fn declared_constant_length_when_declaration_then_returns_expected(
        #[case] var_type: VariableType,
        #[case] qualifier: DeclarationQualifier,
        #[case] initial_value: Option<&str>,
        #[case] expected: Option<u16>,
    ) {
        assert_eq!(declared(var_type, qualifier, initial_value), expected);
    }
}
