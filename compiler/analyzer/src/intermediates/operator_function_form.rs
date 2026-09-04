//! The function forms of operators: `ADD(a, b)` for `a + b`, `GT(a, b)` for
//! `a > b`, `AND(a, b)` for `a AND b`, `NOT(a)` for `NOT a`, and so on.
//!
//! IEC 61131-3 defines these as standard functions (Sections 2.5.1.5.1 to
//! 2.5.1.5.3), so they are registered in the function environment like any
//! other standard function. What sets them apart is that each one *is* an
//! operator: it accepts what the operator accepts and compiles to what the
//! operator compiles to. [`OPERATOR_FUNCTION_FORMS`] states each of them
//! once, as a row, and both the analyzer (through [`signatures`]) and
//! codegen (through [`operator_function_form`]) read the row.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::textual::{CompareOp, Operator};

use super::stdlib_function::input_param;
use crate::function_environment::FunctionSignature;

/// The operator a standard function is the function form of.
#[derive(Debug, Clone, PartialEq)]
pub enum FormOf {
    /// A binary arithmetic operator: `+`, `-`, `*`, `/`, `MOD`.
    Arithmetic(Operator),
    /// A binary comparison, logical or bitwise operator: `>`, `=`, `AND`, ...
    Compare(CompareOp),
    /// The unary `NOT` operator.
    Not,
}

/// How many operands a function form takes.
#[derive(Debug, Clone, PartialEq)]
enum Arity {
    /// One operand, `IN` (`NOT`).
    Unary,
    /// Exactly two operands, `IN1` and `IN2` (`SUB`, `GT`).
    Binary,
}

/// How the result type of a function form follows from its operands.
#[derive(Debug, Clone, PartialEq)]
enum FormResult {
    /// The result has the operand type (`ADD`, `AND`).
    Operand,
    /// The result is `BOOL` whatever the operand type (`GT`, `EQ`).
    Bool,
}

/// One function form of an operator: the row that both the analyzer's
/// signature and codegen's dispatch are derived from.
///
/// The row states the operand category once. The parameter list and the
/// return type are derived from it by [`OperatorFunctionForm::signature`],
/// so the return type of a function form cannot disagree with its operands.
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorFunctionForm {
    /// The function name as written in source; lookup is case-insensitive.
    pub name: &'static str,
    /// The operator this function is a form of.
    pub operator: FormOf,
    /// How many operands the function takes.
    arity: Arity,
    /// The type category of every operand.
    operands: &'static str,
    /// How the result type follows from the operands.
    result: FormResult,
}

/// Builds one row of [`OPERATOR_FUNCTION_FORMS`].
const fn form(
    name: &'static str,
    operator: FormOf,
    arity: Arity,
    operands: &'static str,
    result: FormResult,
) -> OperatorFunctionForm {
    OperatorFunctionForm {
        name,
        operator,
        arity,
        operands,
        result,
    }
}

/// The function form of every operator that has one.
///
/// A row is the single definition of that function: the analyzer registers
/// the signature [`OperatorFunctionForm::signature`] derives from it, and
/// codegen compiles a call to it as the operator in its `operator` column.
/// The `operands` column is the one fact the row states by hand, so a
/// change to what an operator accepts is a change to that cell.
const OPERATOR_FUNCTION_FORMS: &[OperatorFunctionForm] = &[
    // Arithmetic (IEC 61131-3 Section 2.5.1.5.2): the result has the operand type.
    form(
        "ADD",
        FormOf::Arithmetic(Operator::Add),
        Arity::Binary,
        "ANY_NUM",
        FormResult::Operand,
    ),
    form(
        "SUB",
        FormOf::Arithmetic(Operator::Sub),
        Arity::Binary,
        "ANY_NUM",
        FormResult::Operand,
    ),
    form(
        "MUL",
        FormOf::Arithmetic(Operator::Mul),
        Arity::Binary,
        "ANY_NUM",
        FormResult::Operand,
    ),
    form(
        "DIV",
        FormOf::Arithmetic(Operator::Div),
        Arity::Binary,
        "ANY_NUM",
        FormResult::Operand,
    ),
    form(
        "MOD",
        FormOf::Arithmetic(Operator::Mod),
        Arity::Binary,
        "ANY_NUM",
        FormResult::Operand,
    ),
    // Comparison (IEC 61131-3 Section 2.5.1.5.3, Table 33): defined for
    // ANY_ELEMENTARY, which includes ANY_NUM, ANY_BIT and the string and
    // time types; the result is BOOL.
    form(
        "GT",
        FormOf::Compare(CompareOp::Gt),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    form(
        "GE",
        FormOf::Compare(CompareOp::GtEq),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    form(
        "EQ",
        FormOf::Compare(CompareOp::Eq),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    form(
        "LE",
        FormOf::Compare(CompareOp::LtEq),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    form(
        "LT",
        FormOf::Compare(CompareOp::Lt),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    form(
        "NE",
        FormOf::Compare(CompareOp::Ne),
        Arity::Binary,
        "ANY_ELEMENTARY",
        FormResult::Bool,
    ),
    // Bitwise boolean (IEC 61131-3 Section 2.5.1.5.3): defined for ANY_BIT,
    // so they are the boolean operators on BOOL and the bitwise operators on
    // BYTE, WORD, DWORD and LWORD; the result has the operand type.
    form(
        "AND",
        FormOf::Compare(CompareOp::And),
        Arity::Binary,
        "ANY_BIT",
        FormResult::Operand,
    ),
    form(
        "OR",
        FormOf::Compare(CompareOp::Or),
        Arity::Binary,
        "ANY_BIT",
        FormResult::Operand,
    ),
    form(
        "XOR",
        FormOf::Compare(CompareOp::Xor),
        Arity::Binary,
        "ANY_BIT",
        FormResult::Operand,
    ),
    form(
        "NOT",
        FormOf::Not,
        Arity::Unary,
        "ANY_BIT",
        FormResult::Operand,
    ),
];

impl OperatorFunctionForm {
    /// Derives the function's signature from the row.
    ///
    /// A unary form takes `IN`; a binary form takes `IN1` and `IN2`. Every
    /// parameter has the row's operand category, and so does the return type
    /// unless the row says the result is `BOOL`.
    pub(crate) fn signature(&self) -> FunctionSignature {
        let operand = |name: &str| input_param(name, self.operands);
        let parameters = match self.arity {
            Arity::Unary => vec![operand("IN")],
            Arity::Binary => vec![operand("IN1"), operand("IN2")],
        };
        let return_type = match self.result {
            FormResult::Operand => TypeName::from(self.operands),
            FormResult::Bool => TypeName::from("BOOL"),
        };
        FunctionSignature::stdlib(self.name, return_type, parameters)
    }
}

/// Returns the row for the function form named `name`, or `None` when `name`
/// is not the function form of an operator.
///
/// Function names are case-insensitive, and so is this lookup.
pub fn operator_function_form(name: &str) -> Option<&'static OperatorFunctionForm> {
    OPERATOR_FUNCTION_FORMS
        .iter()
        .find(|form| form.name.eq_ignore_ascii_case(name))
}

/// Returns the signatures of the function forms of operators, in table order.
pub(super) fn signatures() -> Vec<FunctionSignature> {
    OPERATOR_FUNCTION_FORMS
        .iter()
        .map(OperatorFunctionForm::signature)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::Id;
    use rstest::rstest;

    /// Every row of the operator-form table, pinned. A change to what an
    /// operator accepts shows up as a change to the row's cell and to its
    /// case here, and nowhere else.
    #[rstest]
    #[case::add("ADD", &["IN1", "IN2"], "ANY_NUM", "ANY_NUM")]
    #[case::sub("SUB", &["IN1", "IN2"], "ANY_NUM", "ANY_NUM")]
    #[case::mul("MUL", &["IN1", "IN2"], "ANY_NUM", "ANY_NUM")]
    #[case::div("DIV", &["IN1", "IN2"], "ANY_NUM", "ANY_NUM")]
    #[case::modulo("MOD", &["IN1", "IN2"], "ANY_NUM", "ANY_NUM")]
    #[case::gt("GT", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::ge("GE", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::eq("EQ", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::le("LE", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::lt("LT", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::ne("NE", &["IN1", "IN2"], "ANY_ELEMENTARY", "BOOL")]
    #[case::and("AND", &["IN1", "IN2"], "ANY_BIT", "ANY_BIT")]
    #[case::or("OR", &["IN1", "IN2"], "ANY_BIT", "ANY_BIT")]
    #[case::xor("XOR", &["IN1", "IN2"], "ANY_BIT", "ANY_BIT")]
    #[case::not("NOT", &["IN"], "ANY_BIT", "ANY_BIT")]
    fn operator_function_form_when_row_then_signature_is_derived_from_it(
        #[case] name: &str,
        #[case] param_names: &[&str],
        #[case] operands: &str,
        #[case] return_type: &str,
    ) {
        let signature = operator_function_form(name).unwrap().signature();
        assert_eq!(signature.name, Id::from(name));
        assert_eq!(
            signature.return_type.unwrap().to_type_name(),
            TypeName::from(return_type)
        );
        let params: Vec<(Id, TypeName)> = signature
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.param_type.clone()))
            .collect();
        let expected: Vec<(Id, TypeName)> = param_names
            .iter()
            .map(|n| (Id::from(n), TypeName::from(operands)))
            .collect();
        assert_eq!(params, expected);
        assert!(signature.parameters.iter().all(|p| p.is_input));
    }

    #[test]
    fn operator_function_form_when_lower_case_then_found() {
        let form = operator_function_form("and").unwrap();
        assert_eq!(form.name, "AND");
        assert_eq!(form.operator, FormOf::Compare(CompareOp::And));
    }

    #[test]
    fn operator_function_form_when_not_a_form_of_an_operator_then_none() {
        assert!(operator_function_form("ABS").is_none());
        assert!(operator_function_form("SHL").is_none());
        assert!(operator_function_form("MOVE").is_none());
    }
}
