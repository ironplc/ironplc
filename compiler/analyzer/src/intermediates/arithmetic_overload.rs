//! Resolves which overload of an arithmetic operator a pair of operand
//! types selects, and what the result type is.
//!
//! IEC 61131-3 defines `ADD`, `SUB`, `MUL` and `DIV` twice: generically
//! over `ANY_NUM` (Table 24), and as typed overloads on the time and date
//! types (Table 30), such as `ADD_TIME` for `ADD` on two `TIME` operands.
//! The operators `+`, `-`, `*` and `/` are those functions (Table 55), so
//! an operator accepts exactly what its function's overloads accept. `MOD`
//! has only its `ANY_INT` row.
//!
//! [`resolve_arithmetic_overload`] answers for one pair of operand types.
//! It is a pure function of its arguments, so every pass that needs the
//! answer (the type resolver, the operator rule, codegen) asks it and no
//! annotation on the tree is needed. The steps are, in order:
//!
//! 1. An operand with no resolved type, or with a type the compatibility
//!    predicate cannot judge (a subrange, an enumeration, a structure),
//!    gives [`Overload::Unchecked`]: today's behaviour, which the callers
//!    keep. `**` has no row in the operator-form table and is unchecked too.
//! 2. The numeric overload: both operands in the row's category, and one
//!    acceptable where the other is expected under the implicit widening
//!    rules (ADR-0028, ADR-0029, ADR-0031). The result is the operand the
//!    other widens to.
//! 3. The typed overloads, by [`typed_overload`]: a temporal parameter
//!    matches an operand of the same temporal family at either width, and
//!    the row's form is the long one if either temporal operand is long.
//! 4. Otherwise `None`: the operator is not defined for the pair.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::Operator;
use ironplc_parser::options::CompilerOptions;

use super::operator_function_form::{form_of_operator, FormOf, OperatorFunctionForm};
use super::stdlib_time_function::short_overload;
use crate::type_compat::{are_types_compatible, is_checkable_type, temporal_family};

/// The overload an arithmetic operator resolves to for a pair of operand
/// types.
#[derive(Debug, Clone, PartialEq)]
pub enum Overload {
    /// An operand's type could not be judged, so the pair is not checked
    /// and the caller keeps its own rule for the result type.
    Unchecked,
    /// The generic numeric overload (IEC 61131-3 Table 24), with the result
    /// type: the operand type the other operand widens to.
    Numeric { result: TypeName },
    /// A typed overload on the time and date types (IEC 61131-3 Table 30):
    /// the name of the typed function, in the width the operands select
    /// (`ADD_TIME` or `ADD_LTIME`), and its return type.
    Typed {
        name: &'static str,
        result: TypeName,
    },
}

impl Overload {
    /// The result type, or `None` for an unchecked pair.
    pub fn result(&self) -> Option<&TypeName> {
        match self {
            Overload::Unchecked => None,
            Overload::Numeric { result } | Overload::Typed { result, .. } => Some(result),
        }
    }
}

/// Returns the overload the operator `op` resolves to for operands of
/// types `left` and `right`, or `None` when `op` is not defined for the
/// pair.
///
/// `options` are the compiler options the analyzer runs with: the implicit
/// widening rules the numeric overload applies come from them, so every
/// pass that asks with the same options gets the same answer.
pub fn resolve_arithmetic_overload(
    op: &Operator,
    left: Option<&TypeName>,
    right: Option<&TypeName>,
    options: &CompilerOptions,
) -> Option<Overload> {
    resolve_with(
        op,
        left,
        right,
        options,
        bit_string_arithmetic_allowed(options),
    )
}

/// Whether the numeric overload admits a bit-string operand as the unsigned
/// integer of its width (ADR-0053).
///
/// The flag that enables this lands with the analyzer's adoption of the
/// resolver; until then the rule is reachable only through
/// [`resolve_with`].
fn bit_string_arithmetic_allowed(_options: &CompilerOptions) -> bool {
    false
}

/// [`resolve_arithmetic_overload`] with the bit-string rule stated
/// explicitly rather than read from the options.
pub(crate) fn resolve_with(
    op: &Operator,
    left: Option<&TypeName>,
    right: Option<&TypeName>,
    options: &CompilerOptions,
    bit_string_arithmetic: bool,
) -> Option<Overload> {
    let Some(form) = form_of_operator(&FormOf::Arithmetic(op.clone())) else {
        return Some(Overload::Unchecked);
    };
    let (Some(left), Some(right)) = (left, right) else {
        return Some(Overload::Unchecked);
    };
    if !is_checkable_type(left) || !is_checkable_type(right) {
        return Some(Overload::Unchecked);
    }
    if let Some(result) = numeric_overload(form, left, right, options, bit_string_arithmetic) {
        return Some(Overload::Numeric { result });
    }
    typed_overload(op, left, right)
}

/// Returns the typed overload (IEC 61131-3 Table 30) of the operator `op`
/// for operands of types `left` and `right`, or `None` when no typed row
/// matches the pair.
///
/// This is step 3 of the resolution alone. It takes no options because no
/// typed row depends on a flag, which lets codegen ask it directly to pick
/// the routine for a pair the analyzer has already accepted.
pub fn typed_overload(op: &Operator, left: &TypeName, right: &TypeName) -> Option<Overload> {
    let form = form_of_operator(&FormOf::Arithmetic(op.clone()))?;
    form.typed_overloads().iter().find_map(|short| {
        let row = short_overload(short)?;
        let left_long = matches_param(row.in1, left)?;
        let right_long = matches_param(row.in2, right)?;
        let (name, result) = if left_long || right_long {
            let long = row.long();
            (long.name, long.result)
        } else {
            (row.name, row.result)
        };
        Some(Overload::Typed {
            name,
            result: TypeName::from(result),
        })
    })
}

/// Returns whether `operand` is acceptable where the typed-row parameter
/// `param` is expected, and if so whether it is a long-width temporal type.
///
/// A temporal parameter matches an operand of the same temporal family at
/// either width, because every duration and date literal resolves to the
/// short name of its family whatever prefix it was written with. The
/// `ANY_NUM` parameter of `MUL_TIME` and `DIV_TIME` matches by category.
fn matches_param(param: &str, operand: &TypeName) -> Option<bool> {
    if param == "ANY_NUM" {
        return is_any_num(operand).then_some(false);
    }
    let param_family = ElementaryTypeName::try_from(&Id::from(param))
        .ok()
        .and_then(|e| temporal_family(&e))?
        .0;
    let elem = ElementaryTypeName::try_from(&operand.name).ok()?;
    let (family, long) = temporal_family(&elem)?;
    (family == param_family).then_some(long)
}

/// Returns whether `operand` is in `ANY_NUM`: a numeric elementary type, or
/// the generic type of an untyped integer or real literal.
fn is_any_num(operand: &TypeName) -> bool {
    if let Ok(elem) = ElementaryTypeName::try_from(&operand.name) {
        return GenericTypeName::AnyNum.is_compatible_with(&elem);
    }
    matches!(
        GenericTypeName::try_from(&operand.name),
        Ok(GenericTypeName::AnyInt | GenericTypeName::AnyReal | GenericTypeName::AnyNum)
    )
}

/// Returns the result type of the numeric overload of `form` for the pair,
/// or `None` when the pair is outside the row's category or neither operand
/// widens to the other.
///
/// With `bit_string_arithmetic`, a bit-string operand of `ADD`, `SUB`,
/// `MUL` or `DIV` is judged as the unsigned integer of its width
/// (ADR-0053). The rule is not applied to `MOD`, whose function form is
/// checked against its `ANY_INT` signature and would not follow. Two
/// bit-string operands give the wider bit string, a bit string and a bare
/// integer literal give the bit string, and a bit string and an integer
/// give what the widening picks.
fn numeric_overload(
    form: &OperatorFunctionForm,
    left: &TypeName,
    right: &TypeName,
    options: &CompilerOptions,
    bit_string_arithmetic: bool,
) -> Option<TypeName> {
    let judge_bit_strings = bit_string_arithmetic && form.has_typed_overloads();
    let (l, l_bit) = judged(left, judge_bit_strings);
    let (r, r_bit) = judged(right, judge_bit_strings);
    let category = form.operand_type();
    if !are_types_compatible(&category, &l, options)
        || !are_types_compatible(&category, &r, options)
    {
        return None;
    }
    let result = wider(&l, &r, options)?;
    let keep_left = l_bit && (r_bit || is_generic(right)) && result == l;
    let keep_right = r_bit && (l_bit || is_generic(left)) && result == r;
    Some(if keep_left {
        left.clone()
    } else if keep_right {
        right.clone()
    } else {
        result
    })
}

/// Returns the operand type the other operand widens to: the type itself
/// when the two are the same, or the wider one when one is acceptable where
/// the other is expected.
fn wider(l: &TypeName, r: &TypeName, options: &CompilerOptions) -> Option<TypeName> {
    if l == r || are_types_compatible(l, r, options) {
        Some(l.clone())
    } else if are_types_compatible(r, l, options) {
        Some(r.clone())
    } else {
        None
    }
}

/// Returns the type `operand` is judged as for the numeric overload, and
/// whether it was a bit string judged as its unsigned integer.
fn judged(operand: &TypeName, judge_bit_strings: bool) -> (TypeName, bool) {
    if judge_bit_strings {
        if let Some(unsigned) = unsigned_integer_of(operand) {
            return (unsigned, true);
        }
    }
    (operand.clone(), false)
}

/// Returns the unsigned integer type of the same width as the bit string
/// `operand`, or `None` when `operand` is not `BYTE`, `WORD`, `DWORD` or
/// `LWORD`. `BOOL` is a truth value, not a bit container, and is excluded.
fn unsigned_integer_of(operand: &TypeName) -> Option<TypeName> {
    let unsigned = match ElementaryTypeName::try_from(&operand.name).ok()? {
        ElementaryTypeName::BYTE => ElementaryTypeName::USINT,
        ElementaryTypeName::WORD => ElementaryTypeName::UINT,
        ElementaryTypeName::DWORD => ElementaryTypeName::UDINT,
        ElementaryTypeName::LWORD => ElementaryTypeName::ULINT,
        _ => return None,
    };
    Some(<TypeName as From<ElementaryTypeName>>::from(unsigned))
}

/// Returns whether `operand` is a generic type: the type of an untyped
/// literal.
fn is_generic(operand: &TypeName) -> bool {
    GenericTypeName::try_from(&operand.name).is_ok()
}

/// The step of an extensible fold that failed to resolve, with the operand
/// types of that step.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldFailure {
    /// The 1-based step: step 1 folds the first two inputs, step `n` folds
    /// the accumulated result with input `n + 1`.
    pub step: usize,
    /// The accumulated type on the left of the failing step.
    pub left: TypeName,
    /// The input type on the right of the failing step.
    pub right: TypeName,
}

/// Resolves an extensible call, `ADD(a, b, c, ...)`, by folding from the
/// left: the first two inputs resolve as a pair, then that result with the
/// third, and so on, as the operator expression `a + b + c` does.
///
/// Returns the overload of the last step, or the first step that does not
/// resolve. An unchecked step keeps the accumulated type as it is, as the
/// callers keep the left operand's type for an unchecked pair. Fewer than
/// two inputs is an arity error for another rule to report, and resolves as
/// unchecked.
pub fn resolve_arithmetic_fold(
    op: &Operator,
    inputs: &[Option<&TypeName>],
    options: &CompilerOptions,
) -> Result<Overload, FoldFailure> {
    let Some((first, rest)) = inputs.split_first() else {
        return Ok(Overload::Unchecked);
    };
    let mut acc: Option<TypeName> = first.cloned();
    let mut last = Overload::Unchecked;
    for (index, input) in rest.iter().enumerate() {
        match resolve_arithmetic_overload(op, acc.as_ref(), *input, options) {
            None => {
                return Err(FoldFailure {
                    step: index + 1,
                    left: acc.expect("an unresolvable step has a left type"),
                    right: (*input)
                        .cloned()
                        .expect("an unresolvable step has a right type"),
                });
            }
            Some(overload) => {
                if let Some(result) = overload.result() {
                    acc = Some(result.clone());
                }
                last = overload;
            }
        }
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn ty(name: &str) -> TypeName {
        TypeName::from(name)
    }

    fn resolve(op: Operator, left: &str, right: &str) -> Option<Overload> {
        resolve_arithmetic_overload(
            &op,
            Some(&ty(left)),
            Some(&ty(right)),
            &CompilerOptions::default(),
        )
    }

    #[rstest]
    #[case::same_family_short("TIME", "TIME", Some(false))]
    #[case::same_family_long("TIME", "LTIME", Some(true))]
    #[case::other_family("TIME", "DATE", None)]
    #[case::not_temporal("TIME", "DINT", None)]
    #[case::any_num_integer("ANY_NUM", "DINT", Some(false))]
    #[case::any_num_real_literal("ANY_NUM", "ANY_REAL", Some(false))]
    #[case::any_num_time("ANY_NUM", "TIME", None)]
    #[case::any_num_bit_string("ANY_NUM", "BYTE", None)]
    fn matches_param_when_pair_then_family_and_width(
        #[case] param: &str,
        #[case] operand: &str,
        #[case] expected: Option<bool>,
    ) {
        assert_eq!(matches_param(param, &ty(operand)), expected);
    }

    #[test]
    fn resolve_arithmetic_overload_when_power_operator_then_unchecked() {
        assert_eq!(
            resolve(Operator::Pow, "REAL", "REAL"),
            Some(Overload::Unchecked)
        );
    }

    #[test]
    fn resolve_arithmetic_overload_when_operand_type_missing_then_unchecked() {
        let overload = resolve_arithmetic_overload(
            &Operator::Add,
            Some(&ty("DINT")),
            None,
            &CompilerOptions::default(),
        );
        assert_eq!(overload, Some(Overload::Unchecked));
    }

    #[test]
    fn resolve_arithmetic_overload_when_user_type_then_unchecked_before_typed_step() {
        assert_eq!(
            resolve(Operator::Add, "MyStruct", "TIME"),
            Some(Overload::Unchecked)
        );
    }

    #[test]
    fn overload_result_when_unchecked_then_none() {
        assert_eq!(Overload::Unchecked.result(), None);
        assert_eq!(
            Overload::Numeric { result: ty("DINT") }.result(),
            Some(&ty("DINT"))
        );
    }

    #[rstest]
    #[case::byte("BYTE", Some("USINT"))]
    #[case::word("WORD", Some("UINT"))]
    #[case::dword("DWORD", Some("UDINT"))]
    #[case::lword("LWORD", Some("ULINT"))]
    #[case::bool("BOOL", None)]
    #[case::integer("INT", None)]
    #[case::literal("ANY_INT", None)]
    fn unsigned_integer_of_when_type_then_same_width_unsigned(
        #[case] operand: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(unsigned_integer_of(&ty(operand)), expected.map(ty));
    }

    #[test]
    fn resolve_with_when_bit_string_and_same_integer_then_integer_wins() {
        let overload = resolve_with(
            &Operator::Add,
            Some(&ty("BYTE")),
            Some(&ty("USINT")),
            &CompilerOptions::default(),
            true,
        );
        assert_eq!(
            overload,
            Some(Overload::Numeric {
                result: ty("USINT")
            })
        );
    }

    #[test]
    fn resolve_arithmetic_fold_when_fewer_than_two_inputs_then_unchecked() {
        let options = CompilerOptions::default();
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[], &options),
            Ok(Overload::Unchecked)
        );
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[Some(&ty("DINT"))], &options),
            Ok(Overload::Unchecked)
        );
    }

    #[test]
    fn resolve_arithmetic_fold_when_unchecked_step_then_accumulated_type_kept() {
        let options = CompilerOptions::default();
        let dint = ty("DINT");
        let user = ty("MyStruct");
        let int = ty("INT");
        // DINT + MyStruct is unchecked and keeps DINT, so DINT + INT is DINT.
        let overload = resolve_arithmetic_fold(
            &Operator::Add,
            &[Some(&dint), Some(&user), Some(&int)],
            &options,
        );
        assert_eq!(overload, Ok(Overload::Numeric { result: dint }));
    }
}
