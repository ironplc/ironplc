//! Which overload of an arithmetic operator applies to two operand types.
//!
//! IEC 61131-3 defines `+`, `-`, `*` and `/` as the functions `ADD`, `SUB`,
//! `MUL` and `DIV`, and defines each of those twice: generically over
//! `ANY_NUM` (Table 24), and as typed overloads on the time and date types
//! (Table 30), each with its own name such as `ADD_TIME` or `SUB_DATE_DATE`.
//! `MOD` has only the generic definition, over `ANY_INT`. An operator
//! accepts exactly the union of what its overloads accept.
//!
//! [`resolve_arithmetic_overload`] answers, for an operator and two operand
//! types, which overload applies and what its result type is. It is a pure
//! function of its arguments, so the type resolver, the operator rule and
//! codegen can each ask it without annotating the tree. [`typed_overload`]
//! is its second step alone, which takes no options because no typed
//! overload depends on a flag.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::textual::Operator;
use ironplc_parser::options::CompilerOptions;

use super::operator_function_form::{form_of_operator, FormOf, OperatorFunctionForm};
use super::stdlib_time_function::short_overload;
use crate::type_compat::{are_types_compatible, is_checkable_type, same_temporal_family};

/// The overload of an arithmetic operator that applies to a pair of operand
/// types.
#[derive(Debug, Clone, PartialEq)]
pub enum Overload {
    /// An operand has no resolved type, or one the type-compatibility
    /// predicate cannot judge (a subrange, an enumeration, a structure). The
    /// operator is not judged; `result` is the left operand's type, as the
    /// type resolver has always used.
    Unchecked { result: Option<TypeName> },
    /// The generic overload over the operator's numeric category.
    Numeric { result: TypeName },
    /// A typed overload on the time and date types, by the name of the form
    /// that applies: the short form (`ADD_TIME`) or the long one
    /// (`ADD_LTIME`).
    Typed {
        name: &'static str,
        result: TypeName,
    },
}

impl Overload {
    /// The result type of the overload, or `None` for an unchecked operand
    /// pair whose left operand has no type.
    pub fn result(&self) -> Option<&TypeName> {
        match self {
            Overload::Unchecked { result } => result.as_ref(),
            Overload::Numeric { result } | Overload::Typed { result, .. } => Some(result),
        }
    }
}

/// A fold step that no overload applies to: `left` and `right` are the
/// operand types of that step, the left one being the result of the steps
/// before it.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldFailure {
    pub left: TypeName,
    pub right: TypeName,
}

/// Returns the overload of the arithmetic operator `op` that applies to the
/// operand types `left` and `right`, or `None` when the operator is not
/// defined for the pair.
///
/// 1. An operand with no type, or one the predicate cannot judge, is
///    [`Overload::Unchecked`].
/// 2. The numeric overload applies when both operands are in the operator's
///    category and one is acceptable where the other is expected; the result
///    is the operand the other widens to. With
///    `--allow-bit-string-arithmetic`, a `BYTE`, `WORD`, `DWORD` or `LWORD`
///    operand of `ADD`, `SUB`, `MUL` or `DIV` is judged as the unsigned
///    integer of its width (ADR-0053).
/// 3. Otherwise [`typed_overload`] is asked.
pub fn resolve_arithmetic_overload(
    op: &Operator,
    left: Option<&TypeName>,
    right: Option<&TypeName>,
    options: &CompilerOptions,
) -> Option<Overload> {
    let unchecked = || Overload::Unchecked {
        result: left.cloned(),
    };
    let Some(form) = arithmetic_form(op) else {
        // `**` has no function form and is not judged.
        return Some(unchecked());
    };
    let (Some(left), Some(right)) = (left, right) else {
        return Some(unchecked());
    };
    if !is_checkable_type(left) || !is_checkable_type(right) {
        return Some(unchecked());
    }
    numeric_overload(form, op, left, right, options).or_else(|| typed_overload(op, left, right))
}

/// Resolves an extensible call's inputs by folding from the left:
/// `ADD(a, b, c)` resolves `a + b`, then that result `+ c`.
///
/// Returns the last step's overload, or the operand types of the first step
/// that no overload applies to. A call with fewer than two inputs is
/// unchecked; its arity is reported elsewhere.
pub fn resolve_arithmetic_fold(
    op: &Operator,
    inputs: &[Option<&TypeName>],
    options: &CompilerOptions,
) -> Result<Overload, FoldFailure> {
    let Some((first, rest)) = inputs.split_first() else {
        return Ok(Overload::Unchecked { result: None });
    };
    let mut overload = Overload::Unchecked {
        result: first.cloned(),
    };
    for right in rest {
        let left = overload.result().cloned();
        let step = resolve_arithmetic_overload(op, left.as_ref(), *right, options);
        overload = match (step, left, right) {
            (Some(step), _, _) => step,
            // A step with an untyped operand resolves as unchecked, so a
            // step that does not resolve has both types.
            (None, Some(left), Some(right)) => {
                return Err(FoldFailure {
                    left,
                    right: (*right).clone(),
                })
            }
            (None, left, _) => Overload::Unchecked { result: left },
        };
    }
    Ok(overload)
}

/// Returns the typed overload of `op` on the time and date types (IEC
/// 61131-3 Table 30) that applies to `left` and `right`, or `None`.
///
/// Each operand is judged where the overload's parameter is expected. A
/// temporal parameter matches an operand of the same temporal family at
/// either width, because a duration or date literal resolves to the short
/// type of its family whatever prefix it was written with. The `ANY_NUM`
/// parameter of `MUL_TIME` and `DIV_TIME` matches by category. The form that
/// applies is the long one when either operand is of a long temporal type,
/// and the short one otherwise.
///
/// Every temporal type is elementary, so an operand that is not (a user
/// type, an enumeration, a structure) matches no temporal parameter, and
/// the answer is `None`. The one non-elementary operand that can match is
/// an untyped literal's category (`ANY_INT`, `ANY_REAL`) in the `ANY_NUM`
/// slot, as in `t * 2`. The type resolver has already reduced an alias to
/// its elementary type.
pub fn typed_overload(op: &Operator, left: &TypeName, right: &TypeName) -> Option<Overload> {
    let form = arithmetic_form(op)?;
    let left = OperandType::of(left);
    let right = OperandType::of(right);
    let row = form.typed_overloads().iter().find_map(|name| {
        let row = short_overload(name)?;
        (parameter_accepts(row.in1, &left) && parameter_accepts(row.in2, &right)).then_some(row)
    })?;
    let long = [&left, &right]
        .iter()
        .filter_map(|operand| operand.elementary.as_ref())
        .any(is_long_temporal);
    let row = if long { row.long()? } else { *row };
    Some(Overload::Typed {
        name: row.name,
        result: TypeName::from(row.result),
    })
}

/// Returns the operator-form row of the arithmetic operator `op`, or `None`
/// for `**`, which has none.
fn arithmetic_form(op: &Operator) -> Option<&'static OperatorFunctionForm> {
    form_of_operator(&FormOf::Arithmetic(op.clone()))
}

/// The numeric overload: step 2 of [`resolve_arithmetic_overload`].
fn numeric_overload(
    form: &OperatorFunctionForm,
    op: &Operator,
    left: &TypeName,
    right: &TypeName,
    options: &CompilerOptions,
) -> Option<Overload> {
    let category = form.operand_type();
    let judged_left = judged_as_numeric(op, left, options);
    let judged_right = judged_as_numeric(op, right, options);
    if !are_types_compatible(&category, &judged_left, options)
        || !are_types_compatible(&category, &judged_right, options)
    {
        return None;
    }
    // The result is the operand the other is acceptable as: the wider one,
    // or the concrete one when the other is an untyped literal. A literal's
    // category accepts any concrete type in it, so the concrete operand is
    // tried as the expected type first: `1 + d` is `DINT`, not `ANY_INT`.
    // The result is the operand as written, so a bit string judged as its
    // unsigned integer stays a bit string: `b + 1` on `BYTE` is `BYTE`.
    let left = (left, judged_left);
    let right = (right, judged_right);
    let (first, second) = if is_generic(&left.1) && !is_generic(&right.1) {
        (right, left)
    } else {
        (left, right)
    };
    let result = if are_types_compatible(&first.1, &second.1, options) {
        first.0
    } else if are_types_compatible(&second.1, &first.1, options) {
        second.0
    } else {
        return None;
    };
    Some(Overload::Numeric {
        result: result.clone(),
    })
}

/// Returns the type `operand` is judged as by the numeric overload of `op`.
///
/// With `--allow-bit-string-arithmetic` (ADR-0053), a `BYTE`, `WORD`,
/// `DWORD` or `LWORD` operand of `ADD`, `SUB`, `MUL` or `DIV` is judged as
/// the unsigned integer of its width, and widens only as that integer:
/// `w + i` on `WORD` and `INT` does not resolve, since `UINT` and `INT` do
/// not widen to each other. `MOD` is excluded so that `b MOD 2` agrees with
/// its function form `MOD(b, 2)`, which is held to `ANY_INT` by the
/// function-call rule. `BOOL` is never an integer. Every other operand is
/// judged as itself.
fn judged_as_numeric(op: &Operator, operand: &TypeName, options: &CompilerOptions) -> TypeName {
    if !options.allow_bit_string_arithmetic || *op == Operator::Mod {
        return operand.clone();
    }
    let unsigned = match ElementaryTypeName::try_from(&operand.name) {
        Ok(ElementaryTypeName::BYTE) => "USINT",
        Ok(ElementaryTypeName::WORD) => "UINT",
        Ok(ElementaryTypeName::DWORD) => "UDINT",
        Ok(ElementaryTypeName::LWORD) => "ULINT",
        _ => return operand.clone(),
    };
    TypeName::from(unsigned)
}

/// An operand of a typed overload: its type, and that type as an
/// elementary type when it is one.
struct OperandType<'a> {
    type_name: &'a TypeName,
    elementary: Option<ElementaryTypeName>,
}

impl<'a> OperandType<'a> {
    fn of(type_name: &'a TypeName) -> Self {
        OperandType {
            type_name,
            elementary: ElementaryTypeName::try_from(&type_name.name).ok(),
        }
    }
}

/// Returns true if `operand` is acceptable where a typed overload declares a
/// parameter of type `param`: a short temporal type, or `ANY_NUM`.
fn parameter_accepts(param: &str, operand: &OperandType<'_>) -> bool {
    let param = TypeName::from(param);
    match (
        ElementaryTypeName::try_from(&param.name),
        &operand.elementary,
    ) {
        // A temporal parameter accepts its family at either width.
        (Ok(param), Some(actual)) => same_temporal_family(&param, actual),
        // A temporal parameter accepts nothing that is not elementary.
        (Ok(_), None) => false,
        // The only non-temporal parameter is `ANY_NUM`. It accepts the
        // numeric types and a literal's category; no flag widens it, so the
        // default options answer the same as any others.
        (Err(_), _) => are_types_compatible(&param, operand.type_name, &CompilerOptions::default()),
    }
}

/// Returns true if `type_name` is a generic category, the type of an untyped
/// literal (`ANY_INT`, `ANY_REAL`).
fn is_generic(type_name: &TypeName) -> bool {
    GenericTypeName::try_from(&type_name.name).is_ok()
}

/// Returns true for the long-width temporal types.
fn is_long_temporal(elementary: &ElementaryTypeName) -> bool {
    matches!(
        elementary,
        ElementaryTypeName::LTIME
            | ElementaryTypeName::LDATE
            | ElementaryTypeName::LTimeOfDay
            | ElementaryTypeName::LDateAndTime
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_arithmetic_overload_when_power_then_unchecked() {
        let dint = TypeName::from("DINT");
        assert_eq!(
            resolve_arithmetic_overload(
                &Operator::Pow,
                Some(&dint),
                Some(&dint),
                &CompilerOptions::default()
            ),
            Some(Overload::Unchecked {
                result: Some(dint.clone())
            })
        );
    }

    #[test]
    fn resolve_arithmetic_fold_when_fewer_than_two_inputs_then_unchecked() {
        let options = CompilerOptions::default();
        let dint = TypeName::from("DINT");
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[], &options),
            Ok(Overload::Unchecked { result: None })
        );
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[Some(&dint)], &options),
            Ok(Overload::Unchecked {
                result: Some(dint.clone())
            })
        );
    }

    #[test]
    fn resolve_arithmetic_fold_when_step_operand_untyped_then_unchecked_from_there() {
        let options = CompilerOptions::default();
        let dint = TypeName::from("DINT");
        let real = TypeName::from("REAL");
        // The untyped second input makes step one unchecked with the left
        // type, and step two then judges DINT against REAL.
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[Some(&dint), None, Some(&real)], &options),
            Err(FoldFailure {
                left: dint.clone(),
                right: real.clone()
            })
        );
        // An untyped first input leaves every step unchecked.
        assert_eq!(
            resolve_arithmetic_fold(&Operator::Add, &[None, Some(&dint), Some(&dint)], &options),
            Ok(Overload::Unchecked { result: None })
        );
    }

    /// An operand that is not elementary matches no temporal parameter, so a
    /// user type on either side has no typed overload, while an untyped
    /// literal's category still fills the `ANY_NUM` slot.
    #[test]
    fn typed_overload_when_operand_not_elementary_then_only_literal_in_any_num_slot() {
        let time = TypeName::from("TIME");
        let user = TypeName::from("MY_STRUCT");
        let int_literal = TypeName::from("ANY_INT");
        let real_literal = TypeName::from("ANY_REAL");

        assert_eq!(typed_overload(&Operator::Add, &user, &time), None);
        assert_eq!(typed_overload(&Operator::Add, &time, &user), None);
        assert_eq!(typed_overload(&Operator::Mul, &time, &user), None);
        // A literal cannot stand for a duration: `2 * t` and `t + 1` have none.
        assert_eq!(typed_overload(&Operator::Mul, &int_literal, &time), None);
        assert_eq!(typed_overload(&Operator::Add, &time, &int_literal), None);
        // A literal fills the ANY_NUM factor: `t * 2`, `t / 1.5`.
        assert_eq!(
            typed_overload(&Operator::Mul, &time, &int_literal),
            Some(Overload::Typed {
                name: "MUL_TIME",
                result: time.clone()
            })
        );
        assert_eq!(
            typed_overload(&Operator::Div, &time, &real_literal),
            Some(Overload::Typed {
                name: "DIV_TIME",
                result: time.clone()
            })
        );
    }

    #[test]
    fn overload_result_when_each_variant_then_its_type() {
        let time = TypeName::from("TIME");
        assert_eq!(Overload::Unchecked { result: None }.result(), None);
        assert_eq!(
            Overload::Numeric {
                result: time.clone()
            }
            .result(),
            Some(&time)
        );
        assert_eq!(
            Overload::Typed {
                name: "ADD_TIME",
                result: time.clone()
            }
            .result(),
            Some(&time)
        );
    }
}
