//! Bytecode tests that a variable of a function or method is stored at the
//! type it is declared with, whatever kind of declaration declares it: a
//! function's return value of a subrange of `LINT` and a method-local
//! `REFERENCE TO` are 64 bits wide, and a 64-bit value is stored into them
//! with a 64-bit store.

use ironplc_container::{opcode, Container};
use ironplc_parser::options::CompilerOptions;

use spec_test_macro::spec_test;

use crate::common::parse_and_compile;

/// The offset in a function body of each `STORE_VAR_I32` that stores a value
/// a 64-bit load left on the stack, looking through a `DUP` of it: a 64-bit
/// value stored into a slot codegen operates as 32-bit.
fn narrow_stores_of_wide_values(container: &Container) -> Vec<usize> {
    let mut stores = Vec::new();
    for function in &container.code.functions {
        let bytecode = container
            .code
            .get_function_bytecode(function.function_id)
            .unwrap();
        let mut wide_value = false;
        for instruction in opcode::decode_body(bytecode).map(Result::unwrap) {
            match instruction.opcode {
                opcode::DUP => {}
                opcode::STORE_VAR_I32 if wide_value => stores.push(instruction.offset),
                op => wide_value = matches!(op, opcode::LOAD_VAR_I64 | opcode::LOAD_CONST_I64),
            }
        }
    }
    stores
}

#[spec_test(REQ_IC_codegen_020)]
fn compile_when_function_returns_lint_subrange_then_result_stored_at_64_bits() {
    let source = "
TYPE BIG : LINT (0..10000000000); END_TYPE
FUNCTION f : BIG
  VAR_INPUT p : BIG; END_VAR
  f := p;
END_FUNCTION
PROGRAM main
  VAR b : BIG := 4294967297; r : BIG; END_VAR
  r := f(b);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(
        narrow_stores_of_wide_values(&container),
        Vec::<usize>::new()
    );
}

#[spec_test(REQ_IC_codegen_020)]
fn compile_when_method_local_reference_to_bound_then_stored_at_64_bits() {
    let source = "
FUNCTION_BLOCK FB_A
VAR
    b : INT := 1;
END_VAR
METHOD M
VAR
    target : INT := 42;
    r : REFERENCE TO INT;
END_VAR
    r REF= target;
    b := r;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    a : FB_A;
    result : INT;
END_VAR
    a.M();
    result := a.b;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_reference_to: true,
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let container = parse_and_compile(source, &options);

    assert_eq!(
        narrow_stores_of_wide_values(&container),
        Vec::<usize>::new()
    );
}
