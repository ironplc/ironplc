//! Bytecode-level integration tests for CASE statement compilation.

use ironplc_parser::options::CompilerOptions;

use rstest::rstest;

use crate::common::{bc, parse_and_compile, try_parse_and_compile};

#[test]
fn compile_when_case_single_arm_then_produces_eq_and_jmp() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  CASE x OF
    1: y := 10;
  END_CASE;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x=var:0, y=var:1
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   6: EQ_I32
    //   7: JMP_IF_NOT offset:+9 -> 19  (skip arm body)
    //  10: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+3 -> 22         (jump to END)
    //  19: (next_label — no more arms, no ELSE)
    //  19: (end_label)
    //  19: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (1)
            bc::eq_i32(),
            bc::jmp_if_not(9),     // offset:+9
            bc::load_const_i32(1), // pool:1 (10)
            bc::store_var_i32(1),  // var:1
            bc::jmp(0),            // offset:+0 (end_label is right here)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_case_with_else_then_produces_jmp_to_end() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  CASE x OF
    1: y := 10;
  ELSE
    y := 99;
  END_CASE;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   6: EQ_I32
    //   7: JMP_IF_NOT offset:+9 -> 19  (skip arm body)
    //  10: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+6 -> 25         (jump past ELSE to END)
    //  19: LOAD_CONST_I32 pool:2 (99)  (ELSE: y := 99)
    //  22: STORE_VAR_I32 var:1
    //  25: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (1)
            bc::eq_i32(),
            bc::jmp_if_not(9),     // offset:+9
            bc::load_const_i32(1), // pool:1 (10)
            bc::store_var_i32(1),  // var:1
            bc::jmp(6),            // offset:+6
            bc::load_const_i32(2), // pool:2 (99)
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
        ]
    );
}

/// Analysis rejects a `REAL` selector (P4053), so codegen treats one as a
/// broken invariant. These tests resolve types without running the semantic
/// rules, which is the only way to reach it.
#[rstest]
#[case::integer_label("1: alarm := TRUE;")]
#[case::subrange_label("1..2: alarm := TRUE;")]
fn compile_when_case_selector_is_real_then_internal_error_at_selector(#[case] arm: &str) {
    let source = format!(
        "
PROGRAM main
VAR
    level : REAL;
    alarm : BOOL;
END_VAR
    CASE level OF
        {arm}
    END_CASE;
END_PROGRAM
"
    );
    let result = try_parse_and_compile(&source, &CompilerOptions::default());

    assert!(result.is_err());
    let diagnostic = result.unwrap_err();
    assert_eq!(diagnostic.code, "P9998");
    let start = source.find("level OF").unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + "level".len());
}

/// Analysis rejects a label the selector can never equal (P2026). The
/// backend narrows every label by its value, so one that does not fit the
/// selector's width is refused rather than reinterpreted as a bit pattern:
/// `16#FFFFFFFF` is not a `DINT` -1. These tests resolve types without
/// running the semantic rules, which is the only way to reach it.
#[rstest]
#[case::decimal("DINT", "4294967295")]
#[case::hex("DINT", "16#FFFFFFFF")]
#[case::negative_unsigned("UDINT", "-1")]
#[case::subrange_end("DINT", "0..4294967295")]
#[case::hex_64("LINT", "16#FFFFFFFFFFFFFFFF")]
#[case::negative_unsigned_64("ULINT", "-1")]
#[case::beyond_every_type("DINT", "16#FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]
fn compile_when_case_label_does_not_fit_selector_width_then_constant_overflow(
    #[case] selector_type: &str,
    #[case] label: &str,
) {
    let source = format!(
        "
PROGRAM main
VAR
    x : {selector_type};
    y : DINT;
END_VAR
    CASE x OF
        {label}: y := 1;
    END_CASE;
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_bit_string_case_labels: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(&source, &options);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "P2026");
}
