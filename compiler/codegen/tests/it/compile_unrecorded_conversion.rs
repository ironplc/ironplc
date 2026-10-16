//! Tests that codegen converts nothing the analyzer did not record: a
//! program analyzed as usual, with its recorded conversions then removed,
//! is an internal error (P9998) where a conversion was needed, rather than
//! compiled with a conversion codegen chose itself (ADR-0056).

use ironplc_container::{opcode, Container, FunctionId};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::{Expr, ExprKind};
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use ironplc_codegen::Route;

use crate::common::{bc, compile_by_route, parse};

/// Removes every recorded conversion, leaving its operand in its place.
struct RemoveConversions;

impl Fold<()> for RemoveConversions {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
        match node.kind {
            ExprKind::ImplicitConversion(conversion) => self.fold_expr(conversion.value),
            _ => node.recurse_fold(self),
        }
    }
}

/// Compiles `x := <value>` for an `x` of type `target` with every recorded
/// conversion removed, or returns the problem code of the failure.
fn compile_without_conversions(target: &str, value: &str) -> Result<Container, String> {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; t : TIME; lt : LTIME; da : DATE; lda : LDATE; d : DINT;
             l : LINT; r : REAL; a : ARRAY[0..3] OF DINT; dw : DWORD; lw : LWORD;
             st : STRING[10]; ud : UDINT; i : INT; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let (library, context) = parse(&source, &options);
    let library = RemoveConversions.fold_library(library).unwrap();
    // A program whose recorded conversions were removed is not one lowering
    // can build, so this compiles every statement from the AST.
    compile_by_route(&library, &context, &options, Route::Ast).map_err(|diagnostic| diagnostic.code)
}

/// The problem code of compiling `x := <value>` for an `x` of type `target`
/// with every recorded conversion removed, or `None` when it compiles.
fn code_without_conversions(target: &str, value: &str) -> Option<String> {
    compile_without_conversions(target, value).err()
}

#[spec_test(REQ_IC_codegen_013)]
#[rstest]
#[case::short_time_of_long_form("LTIME", "lt + t")]
#[case::short_date_of_long_form("LTIME", "SUB_LDATE_LDATE(lda, da)")]
#[case::dint_scaling_long_time("LTIME", "lt * d")]
#[case::real_scaling_long_time("LTIME", "MUL_LTIME(lt, r)")]
#[case::lint_scaling_short_time("TIME", "t * l")]
fn compile_when_time_operand_conversion_removed_then_internal_error(
    #[case] target: &str,
    #[case] value: &str,
) {
    assert_eq!(
        code_without_conversions(target, value).as_deref(),
        Some("P9998")
    );
}

#[test]
fn compile_when_time_operands_need_no_conversion_then_compiles() {
    assert_eq!(code_without_conversions("LTIME", "lt + lt"), None);
}

#[spec_test(REQ_IC_codegen_015)]
#[rstest]
#[case::lint_subscript("DINT", "a[l]")]
#[case::lint_arithmetic_subscript("DINT", "a[d + l]")]
#[case::lint_count_of_32_bit_value("DWORD", "SHL(dw, l)")]
#[case::dint_count_of_64_bit_value("LWORD", "SHL(lw, d)")]
fn compile_when_subscript_or_count_conversion_removed_then_internal_error(
    #[case] target: &str,
    #[case] value: &str,
) {
    assert_eq!(
        code_without_conversions(target, value).as_deref(),
        Some("P9998")
    );
}

#[rstest]
#[case::dint_subscript("DINT", "a[d]")]
#[case::dint_count("DWORD", "SHL(dw, d)")]
fn compile_when_subscript_or_count_needs_no_conversion_then_compiles(
    #[case] target: &str,
    #[case] value: &str,
) {
    assert_eq!(code_without_conversions(target, value), None);
}

#[spec_test(REQ_IC_codegen_018)]
#[rstest]
#[case::lint_length("STRING", "LEFT(st, l)")]
#[case::lint_position("STRING", "MID(st, d, l)")]
#[case::lint_insert_position("STRING", "INSERT(st, 'ab', l)")]
#[case::lint_replace_length("STRING", "REPLACE(st, 'ab', l, d)")]
fn compile_when_string_position_conversion_removed_then_internal_error(
    #[case] target: &str,
    #[case] value: &str,
) {
    assert_eq!(
        code_without_conversions(target, value).as_deref(),
        Some("P9998")
    );
}

#[test]
fn compile_when_string_position_needs_no_conversion_then_compiles() {
    assert_eq!(code_without_conversions("STRING", "LEFT(st, d)"), None);
}

/// Every expression compiles at its own type, and codegen converts nothing
/// to the type of the expression it is used in: with the recorded
/// conversions removed, the value is stored as it is, without the conversion
/// codegen used to choose itself.
#[spec_test(REQ_IC_codegen_019)]
#[rstest]
#[case::unsigned_variable_widened("LINT", "ud", opcode::builtin::CONV_U32_TO_I64)]
#[case::integer_variable_to_real("REAL", "i", opcode::builtin::CONV_I32_TO_F32)]
#[case::arithmetic_result_widened("LREAL", "r * r", opcode::builtin::CONV_F32_TO_F64)]
fn compile_when_value_conversion_removed_then_codegen_converts_nothing(
    #[case] target: &str,
    #[case] value: &str,
    #[case] conversion: u16,
) {
    let container = compile_without_conversions(target, value).unwrap();
    let bytecode = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();

    let conversion = bc::builtin(conversion);
    assert!(
        !bytecode
            .windows(conversion.len())
            .any(|window| window == conversion.as_slice()),
        "codegen converted {value} itself"
    );
}
