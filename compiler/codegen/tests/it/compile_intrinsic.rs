//! Bytecode-level integration tests that a call to each family of standard
//! function compiles, through the intrinsic its signature names, to the
//! instruction that computes it.

use ironplc_container::builtin::{self, str_to_num};
use ironplc_container::opcode::{self, Opcode};
use ironplc_container::FunctionId;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_compile;

/// The opcodes of the program body that runs `statement` over the variables
/// in `declarations`, in order, walking the instructions by their sizes so an
/// operand byte is never mistaken for an opcode. A `BUILTIN` comes with its
/// func_id.
fn instructions(declarations: &str, statement: &str) -> Vec<(Opcode, Option<u16>)> {
    let source = format!(
        "PROGRAM main
VAR
{declarations}
END_VAR
{statement}
END_PROGRAM"
    );
    let container = parse_and_compile(&source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    let mut instructions = Vec::new();
    let mut pc = 0;
    while pc < bytecode.len() {
        let op = bytecode[pc];
        let func_id = (op == opcode::BUILTIN)
            .then(|| u16::from_le_bytes([bytecode[pc + 1], bytecode[pc + 2]]));
        instructions.push((op, func_id));
        pc += opcode::instruction_size(op);
    }
    instructions
}

/// The `BUILTIN` func_ids of [`instructions`], in order.
fn builtins(declarations: &str, statement: &str) -> Vec<u16> {
    instructions(declarations, statement)
        .into_iter()
        .filter_map(|(_, func_id)| func_id)
        .collect()
}

/// The `STRING_TO_*` builtin for `target` under the default policies.
fn string_to(target: str_to_num::Target) -> u16 {
    let options = CompilerOptions::default();
    str_to_num::func_id(
        target,
        options.policy_string_to_num_non_numeric,
        options.policy_string_to_num_failure,
    )
}

#[rstest]
#[case::numeric("x : REAL; y : REAL;", "y := SQRT(x);", vec![builtin::SQRT_F32])]
#[case::numeric_unsigned(
    "a : UDINT; b : UDINT; y : UDINT;",
    "y := MAX(a, b);",
    vec![builtin::MAX_U32]
)]
#[case::compiler_intrinsic(
    "a : LREAL; b : LREAL; y : LREAL;",
    "y := __MOD(a, b);",
    vec![builtin::MOD_F64]
)]
#[case::rotate_narrow("x : WORD; y : WORD;", "y := ROL(x, 3);", vec![builtin::ROL_U16])]
#[case::shift_wide("x : LWORD; y : LWORD;", "y := SHR(x, 3);", vec![builtin::SHR_I64])]
#[case::mux(
    "k : INT; a : DINT; b : DINT; c : DINT; y : DINT;",
    "y := MUX(k, a, b, c);",
    vec![builtin::MUX_I32_BASE + 3]
)]
#[case::bcd_to_int("x : WORD; y : INT;", "y := BCD_TO_INT(x);", vec![builtin::BCD_TO_INT_16])]
#[case::int_to_bcd("x : DINT; y : DWORD;", "y := INT_TO_BCD(x);", vec![builtin::INT_TO_BCD_32])]
#[case::trunc("x : REAL; y : DINT;", "y := TRUNC(x);", vec![builtin::CONV_F32_TO_I32])]
#[case::conversion("x : INT; y : REAL;", "y := INT_TO_REAL(x);", vec![builtin::CONV_I32_TO_F32])]
#[case::conversion_from_alias(
    "t : LTOD; y : REAL;",
    "y := LTOD_TO_REAL(t);",
    vec![builtin::CONV_U64_TO_F32]
)]
#[case::conversion_to_bool(
    "x : LINT; y : BOOL;",
    "y := LINT_TO_BOOL(x);",
    vec![builtin::CONV_I64_TO_BOOL]
)]
#[case::conversion_to_string(
    "x : UINT; s : STRING;",
    "s := UINT_TO_STRING(x);",
    vec![builtin::CONV_U32_TO_STR]
)]
#[case::conversion_from_string(
    "s : STRING; y : INT;",
    "y := STRING_TO_INT(s);",
    vec![string_to(str_to_num::Target::I16)]
)]
fn compile_when_standard_function_then_emits_its_builtin(
    #[case] declarations: &str,
    #[case] statement: &str,
    #[case] expected: Vec<u16>,
) {
    assert_eq!(builtins(declarations, statement), expected);
}

#[rstest]
#[case::len("n := LEN(a);", opcode::LEN_STR)]
#[case::find("n := FIND(a, b);", opcode::FIND_STR)]
#[case::replace("s := REPLACE(a, b, 1, 2);", opcode::REPLACE_STR)]
#[case::insert("s := INSERT(a, b, 2);", opcode::INSERT_STR)]
#[case::delete("s := DELETE(a, 1, 2);", opcode::DELETE_STR)]
#[case::left("s := LEFT(a, 2);", opcode::LEFT_STR)]
#[case::right("s := RIGHT(a, 2);", opcode::RIGHT_STR)]
#[case::mid("s := MID(a, 2, 1);", opcode::MID_STR)]
#[case::concat("s := CONCAT(a, b);", opcode::CONCAT_STR)]
fn compile_when_string_function_then_emits_its_opcode(
    #[case] statement: &str,
    #[case] expected: Opcode,
) {
    let instructions = instructions("a : STRING; b : STRING; s : STRING; n : INT;", statement);

    assert!(
        instructions.contains(&(expected, None)),
        "{statement}: {instructions:?}"
    );
}
