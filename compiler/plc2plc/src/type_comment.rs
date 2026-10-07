//! The comment the annotated rendering writes after each expression: the
//! type the analyzer recorded for it, or for an implicit conversion the type
//! it converts from and to.
//!
//! ```ignore
//! count (* INT *)                (* a value produced at run time *)
//! 1 (* CONSTANT INT *)           (* a value the compiler knows *)
//! total (* DINT -> LINT *)       (* an implicit conversion *)
//! 2 (* CONSTANT ? ANY_INT *)     (* a type the analyzer left undecided *)
//! ```
//!
//! See "Inspecting the annotation" in
//! `specs/design/expression-type-resolution.md`.

use ironplc_dsl::textual::{Expr, ExprKind, ExprType};
use ironplc_dsl::type_id::TypeId;

/// Names the type a [`TypeId`] identifies. Only the analyzer can, so the
/// caller of the annotated rendering supplies it.
pub type TypeNamer<'a> = &'a dyn Fn(TypeId) -> String;

/// The comment written after `expr`.
pub(crate) fn comment(expr: &Expr, type_name: TypeNamer) -> String {
    match &expr.kind {
        ExprKind::ImplicitConversion(inner) => format!(
            "(* {}{} -> {} *)",
            prefix(inner),
            type_text(inner, type_name),
            type_text(expr, type_name)
        ),
        _ => format!("(* {}{} *)", prefix(expr), type_text(expr, type_name)),
    }
}

/// `CONSTANT ` for an expression whose value the compiler knows: a literal
/// (including one constant folding produced), an enumerated value or
/// `NULL`. Anything else produces its value at run time, a variable declared
/// `CONSTANT` included, since only literals are folded.
fn prefix(expr: &Expr) -> &'static str {
    match expr.kind {
        ExprKind::Const(_) | ExprKind::EnumeratedValue(_) | ExprKind::Null(_) => "CONSTANT ",
        _ => "",
    }
}

/// The type recorded for `expr`. A type the analyzer did not decide is
/// marked `?`: none at all, or a generic category, which leaves codegen to
/// choose.
fn type_text(expr: &Expr, type_name: TypeNamer) -> String {
    match &expr.expr_type {
        Some(ExprType::Concrete(id) | ExprType::Inferred(id)) => type_name(*id),
        Some(ExprType::Null) => String::from("NULL"),
        Some(ExprType::Literal(category)) => format!("? {}", category.as_str()),
        None => String::from("?"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::GenericTypeName;
    use ironplc_dsl::core::SourceSpan;

    fn named(id: TypeId) -> String {
        format!("T{}", id.raw())
    }

    fn typed(kind: ExprKind, expr_type: Option<ExprType>) -> Expr {
        let mut expr = Expr::new(kind);
        expr.expr_type = expr_type;
        expr
    }

    fn variable() -> ExprKind {
        ExprKind::named_variable("v")
    }

    #[test]
    fn comment_when_concrete_variable_then_type_name() {
        let expr = typed(variable(), Some(ExprType::Concrete(TypeId::from_raw(3))));

        assert_eq!(comment(&expr, &named), "(* T3 *)");
    }

    #[test]
    fn comment_when_literal_then_constant_prefix() {
        let expr = typed(
            ExprKind::integer_literal("1"),
            Some(ExprType::Concrete(TypeId::from_raw(3))),
        );

        assert_eq!(comment(&expr, &named), "(* CONSTANT T3 *)");
    }

    #[test]
    fn comment_when_null_then_constant_null() {
        let expr = typed(ExprKind::Null(SourceSpan::default()), Some(ExprType::Null));

        assert_eq!(comment(&expr, &named), "(* CONSTANT NULL *)");
    }

    #[test]
    fn comment_when_generic_category_then_question_mark_and_category() {
        let expr = typed(
            ExprKind::integer_literal("1"),
            Some(ExprType::Literal(GenericTypeName::AnyInt)),
        );

        assert_eq!(comment(&expr, &named), "(* CONSTANT ? ANY_INT *)");
    }

    #[test]
    fn comment_when_no_type_then_question_mark() {
        let expr = typed(variable(), None);

        assert_eq!(comment(&expr, &named), "(* ? *)");
    }

    #[test]
    fn comment_when_implicit_conversion_then_from_and_to() {
        let inner = typed(variable(), Some(ExprType::Concrete(TypeId::from_raw(4))));
        let expr = Expr::implicit_conversion(inner, TypeId::from_raw(5));

        assert_eq!(comment(&expr, &named), "(* T4 -> T5 *)");
    }

    #[test]
    fn comment_when_converted_literal_then_constant_from_and_to() {
        let inner = typed(
            ExprKind::integer_literal("1"),
            Some(ExprType::Concrete(TypeId::from_raw(4))),
        );
        let expr = Expr::implicit_conversion(inner, TypeId::from_raw(5));

        assert_eq!(comment(&expr, &named), "(* CONSTANT T4 -> T5 *)");
    }
}
