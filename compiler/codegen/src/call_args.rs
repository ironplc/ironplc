//! Getting the arguments of a call out of its AST node.
//!
//! The arity of a standard function is fixed by its signature, and analysis
//! (`rule_function_call_declared`) has already rejected a call that does not
//! match it. The compiler of a fixed-arity function therefore takes its
//! operands, `[&Expr; N]`, and the dispatcher extracts and count-checks them
//! once with [`fixed_args`]. A compiler that takes operands cannot index past
//! the arity it was written for, and has no use for the call node just to find
//! its arguments.
//!
//! Only a site whose argument list is not a size known where it is written
//! keeps the `&Function` and calls [`collect_positional_args`]: a user-defined
//! call, which matches each argument to its parameter; the functions whose
//! count selects the opcode or comes from a table; and the folds over two or
//! more arguments.

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, Function, ParamAssignmentKind};

/// Collects positional input arguments from a function call.
pub(crate) fn collect_positional_args(func: &Function) -> Vec<&Expr> {
    func.param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect()
}

/// The error for a call whose argument count its signature does not allow.
/// Analysis rejects such a call (`rule_function_call_declared`), so reaching
/// codegen with one is a compiler bug.
#[track_caller]
pub(crate) fn wrong_arg_count(func: &Function) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        func.name.span(),
        "Call has an argument count its signature does not allow",
    ))
}

/// Returns the `N` positional input arguments of a call, or the error
/// [`wrong_arg_count`] describes for any other count.
///
/// The diagnostic records the Rust location it is built at, so this and
/// [`wrong_arg_count`] are `#[track_caller]` and the location is the call to
/// this function. `#[track_caller]` does not pass through a closure, so the
/// failure is handled with `let ... else` rather than `map_err`; a closure
/// would record a location inside this module.
#[track_caller]
pub(crate) fn fixed_args<const N: usize>(func: &Function) -> Result<[&Expr; N], Diagnostic> {
    let Ok(args) = <[&Expr; N]>::try_from(collect_positional_args(func)) else {
        return Err(wrong_arg_count(func));
    };
    Ok(args)
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::{CharacterStringLiteral, ConstantKind};
    use ironplc_dsl::core::Id;
    use ironplc_dsl::textual::{ExprKind, PositionalInput};
    use rstest::rstest;

    use super::*;

    /// A literal of `len` code units. The length tells literals apart.
    fn literal(len: usize) -> Expr {
        Expr::new(ExprKind::Const(ConstantKind::CharacterString(
            CharacterStringLiteral::new(vec!['a'; len]),
        )))
    }

    /// A call with one positional argument for each length in `lens`.
    fn call(lens: &[usize]) -> Function {
        Function {
            name: Id::from("f"),
            param_assignment: lens
                .iter()
                .map(|&len| {
                    ParamAssignmentKind::PositionalInput(PositionalInput { expr: literal(len) })
                })
                .collect(),
        }
    }

    #[test]
    fn fixed_args_when_count_matches_then_returns_arguments_in_order() {
        let one = call(&[1]);
        let two = call(&[1, 2]);
        let four = call(&[1, 2, 3, 4]);

        let [a] = fixed_args::<1>(&one).unwrap();
        let [b, c] = fixed_args::<2>(&two).unwrap();
        let [d, e, f, g] = fixed_args::<4>(&four).unwrap();

        assert_eq!(a.kind, literal(1).kind);
        assert_eq!(b.kind, literal(1).kind);
        assert_eq!(c.kind, literal(2).kind);
        assert_eq!(d.kind, literal(1).kind);
        assert_eq!(e.kind, literal(2).kind);
        assert_eq!(f.kind, literal(3).kind);
        assert_eq!(g.kind, literal(4).kind);
    }

    #[rstest]
    #[case::none(&[])]
    #[case::too_few(&[1])]
    #[case::too_many(&[1, 2, 3])]
    fn fixed_args_when_count_differs_then_p9998(#[case] lens: &[usize]) {
        let func = call(lens);

        let diagnostic = fixed_args::<2>(&func).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
    }

    #[test]
    fn fixed_args_when_count_differs_then_error_location_is_the_caller() {
        let func = call(&[1]);

        // The helper and this test share a file, so the line is what tells
        // the caller from a location inside the helper.
        let call_line = line!() + 1;
        let diagnostic = fixed_args::<2>(&func).unwrap_err();

        assert_eq!(diagnostic.source_file.as_deref(), Some(file!()));
        assert_eq!(diagnostic.source_line, Some(call_line));
    }
}
