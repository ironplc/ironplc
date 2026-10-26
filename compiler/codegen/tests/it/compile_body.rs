//! Tests of control flow and places compiled from the lowered program.
//!
//! Every statement is compiled from the lowered program. The edge cases here
//! run and are checked by value, and the shapes are checked to lower and
//! compile.

use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;
use rstest::rstest;

use crate::common::{
    bc, compile_analyzed, parse, parse_and_compile_to_run, run_one_scan, Snapshot,
};

pub(crate) fn options() -> CompilerOptions {
    CompilerOptions {
        allow_continue: true,
        allow_ref_to: true,
        allow_reference_to: true,
        allow_partial_access_syntax: true,
        allow_sizeof: true,
        allow_fb_inheritance: true,
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    }
}

/// Compiles `source` and runs it for one scan.
/// The scan fails the test if it leaves a value on the operand stack.
pub(crate) fn compile_and_run(source: &str) -> Snapshot {
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    Snapshot::run_compiled(container)
}

/// Runs `source`, once one scan of it on every
/// backend of the end-to-end harness is shown to leave every variable with
/// the same value.
pub(crate) fn compile_and_run_agreeing(source: &str) -> Snapshot {
    parse_and_compile_to_run(source, &options());
    compile_and_run(source)
}

/// An `EXIT` inside a `CASE` arm leaves the loop with the selector already
/// popped.
#[test]
fn run_when_exit_in_case_arm_then_leaves_loop_with_stack_balanced() {
    let snapshot = compile_and_run(
        "PROGRAM main VAR i : DINT; hits : DINT; END_VAR
           FOR i := 1 TO 10 DO
             CASE i OF
               3: EXIT;
             ELSE
               hits := hits + 1;
             END_CASE;
           END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("i"), 3);
    assert_eq!(snapshot.read_as::<i64>("hits"), 2);
}

/// A `RETURN` inside a `CASE` arm returns with the selector already popped.
#[test]
fn run_when_return_in_case_arm_then_returns_with_stack_balanced() {
    let snapshot = compile_and_run(
        "PROGRAM main VAR x : DINT := 1; a : DINT; b : DINT; END_VAR
           CASE x OF
             1: a := 10; RETURN;
           END_CASE;
           b := 20;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("a"), 10);
    assert_eq!(snapshot.read_as::<i64>("b"), 0);
}

/// Unsigned and 64-bit selectors compare by their own signedness and width,
/// at the extremes of their types.
#[test]
fn run_when_case_selector_unsigned_or_64_bit_then_matches_by_its_type() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR u : UDINT := 4294967295; l : LINT := -9223372036854775808;
               ul : ULINT := 18446744073709551615; a : DINT; b : DINT; c : DINT; END_VAR
           CASE u OF
             0..10: a := 1;
             3000000000..4294967295: a := 2;
           END_CASE;
           CASE l OF
             -9223372036854775808..-1: b := 1;
           ELSE
             b := 2;
           END_CASE;
           CASE ul OF
             0..10: c := 1;
             18446744073709551615: c := 2;
           END_CASE;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("a"), 2);
    assert_eq!(snapshot.read_as::<i64>("b"), 1);
    assert_eq!(snapshot.read_as::<i64>("c"), 2);
}

/// A `FOR` over a `LINT` counts at 64 bits, with a step the analyzer
/// converted to the control's type.
#[test]
fn run_when_for_over_lint_with_converted_step_then_counts() {
    let snapshot = compile_and_run(
        "PROGRAM main VAR l : LINT; n : DINT; END_VAR
           FOR l := 0 TO 10 BY DINT#3 DO n := n + 1; END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("n"), 4);
    assert_eq!(snapshot.read_as::<i64>("l"), 12);
}

/// A `FOR` counting an `INT` down from a variable keeps the truncation of
/// its control, since no constant bound proves it unneeded.
#[test]
fn run_when_for_counts_int_down_from_variable_then_counts() {
    let snapshot = compile_and_run(
        "PROGRAM main VAR start : INT := 10; k : INT; n : DINT; END_VAR
           FOR k := start TO 1 BY -3 DO n := n + 1; END_FOR;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("n"), 4);
    assert_eq!(snapshot.read_as::<i64>("k"), -2);
}

/// `CONTINUE` goes on with the next iteration of each kind of loop: to the
/// step of a `FOR`, the condition of a `WHILE`, and the `UNTIL` of a
/// `REPEAT`.
#[test]
fn run_when_continue_in_each_loop_then_next_iteration() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR i : DINT; f : DINT; w : DINT; ws : DINT; r : DINT; rs : DINT; END_VAR
           FOR i := 1 TO 5 DO
             IF i = 2 THEN CONTINUE; END_IF;
             f := f + i;
           END_FOR;
           WHILE w < 5 DO
             w := w + 1;
             IF w = 3 THEN CONTINUE; END_IF;
             ws := ws + w;
           END_WHILE;
           REPEAT
             r := r + 1;
             IF r = 2 THEN CONTINUE; END_IF;
             rs := rs + r;
           UNTIL r >= 4 END_REPEAT;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("f"), 13);
    assert_eq!(snapshot.read_as::<i64>("ws"), 12);
    assert_eq!(snapshot.read_as::<i64>("rs"), 8);
}

/// Compiles `source`, and returns the bytecode of its program body.
pub(crate) fn body(source: &str) -> Vec<u8> {
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).unwrap();
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// REQ-LOW-lowering-046: a `FOR` whose first value is a read of a variable
/// of the control's type stores it untruncated, since it already fits.
#[test]
fn compile_when_for_first_value_fits_control_then_stored_untruncated() {
    let source = "PROGRAM main VAR k : INT; s : INT; n : DINT; END_VAR
           FOR k := s TO 10 DO n := n + 1; END_FOR;
         END_PROGRAM";
    let code = body(source);
    // k is var:0, s var:1.
    let start = [bc::load_var_i32(1), bc::store_var_i32(0)].concat();
    assert!(code.starts_with(&start), "{code:?}");
}

/// Each control-flow statement lowers and compiles, fused compare and branch
/// included.
#[rstest]
#[case::while_fused("WHILE w < 5 DO w := w + 1; END_WHILE;")]
#[case::while_unfused("WHILE w < n DO w := w + 1; END_WHILE;")]
#[case::repeat_fused("REPEAT w := w + 1; UNTIL w > 5 END_REPEAT;")]
#[case::repeat_unfused("REPEAT w := w + 1; UNTIL w > n END_REPEAT;")]
#[case::for_fused("FOR w := 1 TO 10 DO n := n + w; END_FOR;")]
#[case::for_unfused("FOR w := 1 TO n DO n := n - 1; END_FOR;")]
#[case::for_down("FOR w := 10 TO 1 BY -2 DO n := n + w; END_FOR;")]
#[case::for_narrow_in_range("FOR k := 1 TO 10 DO n := n + 1; END_FOR;")]
#[case::if_fused("IF w > 3 THEN n := 1; END_IF;")]
#[case::if_elsif_else("IF w > 3 THEN n := 1; ELSIF w < n THEN n := 2; ELSE n := 3; END_IF;")]
#[case::exit_and_continue(
    "FOR w := 1 TO 10 DO IF w = 2 THEN CONTINUE; END_IF; IF w > 5 THEN EXIT; END_IF; END_FOR;"
)]
#[case::narrow_store("WHILE k < 5 DO k := k + 1; END_WHILE;")]
fn compile_when_control_flow_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main VAR w : DINT; n : DINT; k : INT; s : INT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// Fails unless every statement of `source` lowers, and it compiles.
pub(crate) fn assert_compiles(source: &str) {
    let options = options();
    let (library, context) = parse(source, &options);
    compile_analyzed(&library, &context, &options).expect("every statement lowers");
}

/// Elements of arrays, with lower bounds other than zero and two dimensions,
/// fields of structures, a field of an element of an array of structures
/// and an element of an array field, each read and written.
#[test]
fn run_when_array_elements_and_fields_then_values() {
    let snapshot = compile_and_run(
        "TYPE P : STRUCT x : DINT; vals : ARRAY[1..3] OF DINT; END_STRUCT; END_TYPE
         PROGRAM main
           VAR a : ARRAY[1..5] OF DINT; m : ARRAY[1..2, 0..3] OF INT;
               p : P; ps : ARRAY[0..3] OF P; i : DINT := 2;
               r1 : DINT; r2 : DINT; r3 : DINT; r4 : DINT; END_VAR
           a[i] := 7;
           a[i + 1] := a[i] * 2;
           m[2, i] := 5;
           m[1, 0] := m[2, 2] + 1;
           p.x := 3;
           p.vals[i] := p.x + 1;
           ps[i].x := 9;
           ps[1].vals[3] := ps[2].x + 1;
           r1 := a[3];
           r2 := m[1, 0];
           r3 := p.vals[2];
           r4 := ps[1].vals[3];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 14);
    assert_eq!(snapshot.read_as::<i64>("r2"), 6);
    assert_eq!(snapshot.read_as::<i64>("r3"), 4);
    assert_eq!(snapshot.read_as::<i64>("r4"), 10);
}

/// A subscript outside the array traps (ADR-0023).
#[test]
fn run_when_subscript_outside_array_then_traps() {
    let options = options();
    let source = "PROGRAM main VAR a : ARRAY[1..5] OF DINT; i : DINT := 9; END_VAR
           a[i] := 1;
         END_PROGRAM";
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    assert!(run_one_scan(&container).is_err());
}

/// An element of an array and a field of a structure lower and compile. The
/// arrays hold `DINT`s, so no store is narrowed: a read of a narrow place
/// already fits a place of its type.
#[rstest]
#[case::array_element("a[i] := a[i + 1] + 1;")]
#[case::constant_subscript("a[3] := a[2];")]
#[case::two_dimensions("m[1, i] := m[2, 0];")]
#[case::structure_field("p.x := p.x + 1;")]
#[case::array_field("p.vals[i] := 1;")]
#[case::field_of_element("ps[i].x := ps[2].x;")]
fn compile_when_place_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "TYPE P : STRUCT x : DINT; vals : ARRAY[1..3] OF DINT; END_STRUCT; END_TYPE
         PROGRAM main
           VAR a : ARRAY[1..5] OF DINT; m : ARRAY[1..2, 0..3] OF DINT;
               p : P; ps : ARRAY[0..3] OF P; i : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// A subscript whose type holds values a `DINT` does not selects the element
/// it names when its value is a `DINT` (REQ-LOW-codegen-164).
#[test]
fn run_when_subscript_wider_than_dint_then_selects_its_element() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR a : ARRAY[-3..5] OF DINT; ud : UDINT := 1; l : LINT := -3; ul : ULINT := 5;
               dw : DWORD := 2; r1 : DINT; r2 : DINT; END_VAR
           a[ud] := 10;
           a[l] := a[ud] + 1;
           a[ul] := 12;
           a[dw] := a[l] + a[ul];
           r1 := a[-3];
           r2 := a[2];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 11);
    assert_eq!(snapshot.read_as::<i64>("r2"), 23);
}

/// REQ-LOW-codegen-164: a subscript outside the range of `DINT` traps
/// rather than selecting the element its low bits name, `a[-1]` for a
/// `UDINT` of 4294967295.
#[rstest]
#[case::udint("i : UDINT := 4294967295;")]
#[case::lint("i : LINT := -4294967297;")]
#[case::ulint("i : ULINT := 18446744073709551615;")]
fn run_when_subscript_outside_dint_then_traps(#[case] subscript: &str) {
    let options = options();
    let source = format!(
        "PROGRAM main VAR a : ARRAY[-3..5] OF DINT; {subscript} END_VAR
           a[i] := 1;
         END_PROGRAM"
    );
    let (library, context) = parse(&source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let Err(fault) = run_one_scan(&container) else {
        panic!("the scan does not trap");
    };
    assert_eq!(fault.trap.v_code(), "V4007");
}

/// A subscript whose conversion to `DINT` is checked lowers and compiles,
/// with the subscript check of its type.
#[rstest]
#[case::udint("a[ud] := a[ud] + 1;")]
#[case::lint("a[l] := a[l + 1];")]
#[case::ulint_and_lword("a[ul] := a[lw];")]
#[case::dword_in_two_dimensions("m[dw, 0] := 1;")]
fn compile_when_checked_subscript_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR a : ARRAY[0..5] OF DINT; m : ARRAY[0..2, 0..3] OF DINT; ud : UDINT; l : LINT;
               ul : ULINT; dw : DWORD; lw : LWORD; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// What a reference refers to is read and written through it: a variable, a
/// narrow variable, whose store wraps at its width, and an element of an
/// array. The references are bound by their declarations, since `REF(x)` is
/// not lowered yet.
#[test]
fn run_when_read_and_written_through_references_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR x : DINT := 5; n : INT := 100; arr : ARRAY[1..3] OF DINT; i : DINT := 2;
               p : REF_TO DINT := REF(x); q : REF_TO INT := REF(n);
               pa : REF_TO ARRAY[1..3] OF DINT := REF(arr);
               r1 : DINT; r2 : DINT; r3 : DINT; END_VAR
           p^ := p^ + 1;
           q^ := q^ * 400;
           pa^[i] := p^ * 2;
           pa^[3] := pa^[i] + 1;
           r1 := x;
           r2 := n;
           r3 := arr[3];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 6);
    assert_eq!(snapshot.read_as::<i64>("r2"), -25536);
    assert_eq!(snapshot.read_as::<i64>("r3"), 13);
}

/// REQ-LOW-codegen-088: a place reached through a null reference traps,
/// whether it is read or written.
#[rstest]
#[case::read("x := p^;")]
#[case::write("p^ := 1;")]
#[case::element("x := pa^[1];")]
#[case::through_element("x := refs[0]^;")]
fn run_when_null_reference_dereferenced_then_traps(#[case] statement: &str) {
    let options = options();
    let source = format!(
        "PROGRAM main
           VAR x : DINT; p : REF_TO DINT; pa : REF_TO ARRAY[1..3] OF DINT;
               refs : ARRAY[0..2] OF REF_TO DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    let (library, context) = parse(&source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let Err(fault) = run_one_scan(&container) else {
        panic!("the scan does not trap");
    };
    assert_eq!(fault.trap, Trap::NullDereference);
}

/// What a reference refers to, and an element of an array it refers to,
/// lower and compile, the narrowed store through a reference to an `INT`
/// included. So does a read or a store through a reference held in an
/// element of an array, alone, in an expression or in a condition, and a
/// read or a store through a reference a reference refers to
/// (REQ-LOW-codegen-203).
#[rstest]
#[case::read_and_write("p^ := p^ + 1;")]
#[case::read_into_variable("x := p^;")]
#[case::narrow_store("q^ := q^ + 1;")]
#[case::element("pa^[i] := pa^[i + 1];")]
#[case::through_element("x := refs[0]^;")]
#[case::store_through_element("refs[0]^ := 5;")]
#[case::through_reference_to_reference("x := pp^^;")]
#[case::store_through_reference_to_reference("pp^^ := pp^^ + 1;")]
#[case::reference_through_reference_to_reference("pp^ := p;")]
#[case::through_element_by_variable("x := refs[i]^;")]
#[case::through_element_of_two_dimensions("x := grid[1, 0]^;")]
#[case::through_element_in_expression("x := refs[0]^ + 1;")]
#[case::through_element_in_condition("IF refs[0]^ = 42 THEN x := 1; END_IF;")]
#[case::through_reference_to_element("x := rrefs[0]^;")]
fn compile_when_dereference_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR x : DINT; i : DINT; p : REF_TO DINT; q : REF_TO INT;
               pp : REF_TO REF_TO DINT;
               pa : REF_TO ARRAY[1..3] OF DINT; refs : ARRAY[0..2] OF REF_TO DINT;
               grid : ARRAY[0..1, 0..1] OF REF_TO DINT;
               rrefs : ARRAY[0..2] OF REFERENCE TO DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// A field reached through a reference a function receives, `p^.x`, is not
/// a field of an instance. It is not taken for the function's own `x`: the
/// statement is not supported yet (P9999).
#[test]
fn compile_when_field_through_reference_parameter_then_not_taken_for_local() {
    let options = options();
    let source = "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         FUNCTION F : DINT
           VAR_INPUT p : REF_TO P; END_VAR
           VAR x : DINT := 5; END_VAR
           F := p^.x;
         END_FUNCTION
         PROGRAM main VAR s : P; r : DINT; END_VAR
           r := F(p := REF(s));
         END_PROGRAM";
    let (library, context) = parse(source, &options);
    let result = compile_analyzed(&library, &context, &options);
    assert_eq!(result.unwrap_err().code, "P9999");
}

/// Bits and parts are written to a variable, an element of an array, a
/// field of a structure and a field of an element of an array of
/// structures, leaving the other bits as they were. Setting the top bit of
/// an `INT` makes it negative.
#[test]
fn run_when_bits_and_parts_written_then_other_bits_kept() {
    let snapshot = compile_and_run(
        "TYPE P : STRUCT w : WORD; END_STRUCT; END_TYPE
         PROGRAM main
           VAR w : WORD := 16#00F0; n : INT; l : LWORD; a : ARRAY[1..3] OF BYTE;
               p : P; ps : ARRAY[0..1] OF P; i : DINT := 2;
               b : BYTE := 16#AB; d : DWORD := 16#12345678;
               rw : DWORD; rn : DINT; ra : DWORD; rp : DWORD; rps : DWORD; END_VAR
           w.0 := TRUE;
           w.%B1 := b;
           n.15 := TRUE;
           l.%D1 := d;
           l.%B0 := b;
           a[i].7 := TRUE;
           p.w.%B0 := b;
           ps[i - 1].w.3 := TRUE;
           rw := w;
           rn := n;
           ra := a[2];
           rp := p.w;
           rps := ps[1].w;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("rw"), 0xABF1);
    assert_eq!(snapshot.read_as::<i64>("rn"), -32768);
    assert_eq!(snapshot.read_as::<u64>("l"), 0x1234_5678_0000_00AB);
    assert_eq!(snapshot.read_as::<i64>("ra"), 0x80);
    assert_eq!(snapshot.read_as::<i64>("rp"), 0xAB);
    assert_eq!(snapshot.read_as::<i64>("rps"), 8);
}

/// REQ-LOW-codegen-121: a bit write to an element of an array evaluates the
/// element's subscript once, keeping the address for the store.
#[test]
fn compile_when_bit_written_to_element_then_subscript_evaluated_once() {
    let source = "PROGRAM main VAR i : DINT; a : ARRAY[0..3] OF WORD; END_VAR
           a[i].3 := TRUE;
         END_PROGRAM";
    // i is var:0.
    let load_i = bc::load_var_i32(0);
    let loads = |code: Vec<u8>| {
        code.windows(load_i.len())
            .filter(|window| *window == load_i.as_slice())
            .count()
    };
    assert_eq!(loads(body(source)), 1);
}

/// A bit or partial write to a variable lowers and compiles: a variable has
/// no address to evaluate.
#[rstest]
#[case::bit("w.3 := TRUE;")]
#[case::first_bit("w.0 := FALSE;")]
#[case::byte("w.%B1 := b;")]
#[case::dword_of_lword("l.%D1 := d;")]
#[case::lword_of_lword("l.%L0 := l;")]
fn compile_when_bits_written_to_variable_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR w : WORD; b : BYTE; d : DWORD; l : LWORD; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// Shifts and rotates of each width, and bit and partial reads of a
/// variable, an element and a signed integer, give the expected values.
#[test]
fn run_when_shifts_and_bit_reads_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR b : BYTE := 16#81; w : WORD := 16#1234; d : DWORD := 16#80000001;
               l : LWORD := 16#0123456789ABCDEF; n : INT := -2; i : DINT := 1;
               a : ARRAY[0..1] OF WORD; k : DINT := 4;
               rb : BYTE; rw : WORD; rd : DWORD; rl : LWORD; rl2 : LWORD;
               x1 : BOOL; x2 : BOOL; x3 : BOOL; x4 : BOOL; p1 : BYTE; p2 : BYTE; p3 : DWORD; END_VAR
           a[i] := 16#0008;
           rb := ROL(b, 1);
           rw := ROR(w, k);
           rd := SHL(d, 1);
           rl := SHR(l, k);
           rl2 := ROL(l, 8);
           x1 := w.2;
           x2 := a[i].3;
           x3 := n.15;
           x4 := n.0;
           p1 := w.%B1;
           p2 := l.%B7;
           p3 := l.%D0;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<u64>("rb"), 0x03);
    assert_eq!(snapshot.read_as::<u64>("rw"), 0x4123);
    assert_eq!(snapshot.read_as::<u64>("rd"), 0x0000_0002);
    assert_eq!(snapshot.read_as::<u64>("rl"), 0x0012_3456_789A_BCDE);
    assert_eq!(snapshot.read_as::<u64>("rl2"), 0x2345_6789_ABCD_EF01);
    assert_eq!(snapshot.read_as::<u64>("x1"), 1);
    assert_eq!(snapshot.read_as::<u64>("x2"), 1);
    assert_eq!(snapshot.read_as::<u64>("x3"), 1);
    assert_eq!(snapshot.read_as::<u64>("x4"), 0);
    assert_eq!(snapshot.read_as::<u64>("p1"), 0x12);
    assert_eq!(snapshot.read_as::<u64>("p2"), 0x01);
    assert_eq!(snapshot.read_as::<u64>("p3"), 0x89AB_CDEF);
}

/// A shift or rotate, and a partial read of a 32-bit place, lower and
/// compile.
#[rstest]
#[case::shl_dword("d := SHL(d, n);")]
#[case::shr_lword("l := SHR(l, 3);")]
#[case::rol_byte("b := ROL(b, 1);")]
#[case::ror_word("w := ROR(w, n);")]
#[case::rol_lword("l := ROL(l, 2);")]
#[case::partial_read("b := w.%B1;")]
fn compile_when_shift_or_partial_read_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR b : BYTE; w : WORD; d : DWORD; l : LWORD; n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// The function forms of operators and `MOVE` give the values their
/// operators give.
#[test]
fn run_when_operator_function_forms_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR a : DINT := 7; b : DINT := 3; c : DINT := 2; w : WORD := 16#00FF;
               r1 : DINT; r2 : DINT; r3 : DINT; x1 : BOOL; x2 : BOOL; rw : WORD; r4 : DINT; END_VAR
           r1 := ADD(a, b, c);
           r2 := SUB(MUL(a, b), c);
           r3 := MOD(a, b);
           x1 := GT(a, b);
           x2 := AND(x1, LT(c, b), NOT(EQ(a, b)));
           rw := XOR(NOT(w), WORD#16#0F0F);
           r4 := MOVE(DIV(a, c));
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 12);
    assert_eq!(snapshot.read_as::<i64>("r2"), 19);
    assert_eq!(snapshot.read_as::<i64>("r3"), 1);
    assert_eq!(snapshot.read_as::<i64>("x1"), 1);
    assert_eq!(snapshot.read_as::<i64>("x2"), 1);
    assert_eq!(snapshot.read_as::<i64>("rw"), 0xF00F);
    assert_eq!(snapshot.read_as::<i64>("r4"), 3);
}

/// The function form of an operator lowers and compiles.
#[rstest]
#[case::add_fold("r := ADD(a, b, c);")]
#[case::sub("r := SUB(a, b);")]
#[case::mod_("r := MOD(a, b);")]
#[case::gt("x := GT(a, b);")]
#[case::and_bool("x := AND(x, y);")]
#[case::and_word("w := AND(w, v);")]
#[case::not_bool("x := NOT(x);")]
#[case::not_word("w := NOT(w);")]
#[case::move_("r := MOVE(a);")]
fn compile_when_operator_form_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR a : DINT; b : DINT; c : DINT; r : DINT; x : BOOL; y : BOOL; w : WORD; v : WORD; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// Numeric functions give the expected values. `ABS` of the most negative
/// `DINT` wraps to itself, an integer exponent of a real is converted to it,
/// and `**` is `EXPT`.
#[test]
fn run_when_numeric_functions_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR i : INT := -5; d : DINT := -2147483648; l : LINT := -7; r : REAL := -2.5;
               x : LREAL := 2.0; b : DINT := 3; n : INT := 10;
               ri : INT; rd : DINT; rl : LINT; rr : REAL; e1 : DINT; e2 : LREAL; e3 : DINT;
               s : LREAL; t : LREAL; a : LREAL; m : LREAL; tr : REAL; sr : REAL; END_VAR
           ri := ABS(i);
           rd := ABS(d);
           rl := ABS(l);
           rr := ABS(r);
           e1 := EXPT(b, 4);
           e2 := EXPT(x, n);
           e3 := b ** 3;
           s := SQRT(x);
           t := LN(EXP(x));
           a := ATAN2(x, x);
           m := __MOD(LREAL#7.5, x);
           tr := __TRUNC(r);
           sr := SIN(r);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("ri"), 5);
    assert_eq!(snapshot.read_as::<i64>("rd"), -2_147_483_648);
    assert_eq!(snapshot.read_as::<i64>("rl"), 7);
    assert_eq!(snapshot.read_as::<f32>("rr"), 2.5);
    assert_eq!(snapshot.read_as::<i64>("e1"), 81);
    assert_eq!(snapshot.read_as::<f64>("e2"), 1024.0);
    assert_eq!(snapshot.read_as::<i64>("e3"), 27);
    assert_eq!(snapshot.read_as::<f64>("s"), 2.0_f64.sqrt());
    assert_eq!(snapshot.read_as::<f64>("t"), 2.0_f64.exp().ln());
    assert_eq!(snapshot.read_as::<f64>("a"), 2.0_f64.atan2(2.0));
    assert_eq!(snapshot.read_as::<f64>("m"), 1.5);
    assert_eq!(snapshot.read_as::<f32>("tr"), -2.0);
    assert_eq!(snapshot.read_as::<f32>("sr"), (-2.5_f32).sin());
}

/// REQ-LOW-codegen-098: `EXPT` of an integer by a negative exponent traps.
#[test]
fn run_when_integer_raised_to_negative_exponent_then_traps() {
    let options = options();
    let (library, context) = parse(
        "PROGRAM main VAR b : DINT := 2; n : DINT := -1; r : DINT; END_VAR
           r := EXPT(b, n);
         END_PROGRAM",
        &options,
    );
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let Err(fault) = run_one_scan(&container) else {
        panic!("a negative exponent of an integer traps");
    };
    assert_eq!(fault.trap, Trap::NegativeExponent);
}

/// REQ-LOW-codegen-089: `EXPT` of unsigned integers reads its exponent as
/// unsigned, so one past the range of `DINT` wraps the power rather than
/// trapping.
#[test]
fn run_when_unsigned_raised_to_exponent_past_dint_then_wraps() {
    let snapshot = compile_and_run(
        "PROGRAM main VAR b : UDINT := 3; n : UDINT := 3000000000; r : UDINT; END_VAR
           r := EXPT(b, n);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<u64>("r"), 3_840_514_049);
}

/// A numeric function, and `**`, lower and compile.
#[rstest]
#[case::abs_int("i := ABS(i);")]
#[case::abs_lint("l := ABS(l);")]
#[case::abs_real("r := ABS(r);")]
#[case::expt_dint("d := EXPT(d, d);")]
#[case::expt_real_by_dint("r := EXPT(r, d);")]
#[case::expt_udint("u := EXPT(u, u);")]
#[case::pow_ulint("ul := ul ** ul;")]
#[case::pow_lreal("x := x ** x;")]
#[case::sqrt_lreal("x := SQRT(x);")]
#[case::sin_real("r := SIN(r);")]
#[case::atan2_lreal("x := ATAN2(x, x);")]
#[case::trunc_real("r := __TRUNC(r);")]
#[case::mod_lreal("x := __MOD(x, x);")]
fn compile_when_numeric_function_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR i : INT; d : DINT; l : LINT; r : REAL; x : LREAL; u : UDINT; ul : ULINT;
           END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-codegen-089: the selection functions give the expected values.
/// `MIN` and `MAX` of unsigned integers compare them unsigned, `LIMIT`
/// clamps, to its upper bound when the bounds cross, and `SEL` takes its
/// second input when the selector is `TRUE`. Of reals, `MIN` gives the
/// other input when one is NaN, and `LIMIT` gives NaN.
#[test]
fn run_when_selection_functions_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR u : UDINT := 3000000000; one : UDINT := 1; d : DINT := -4; l : LINT := 9;
               r : REAL := 7.5; ul : ULINT := 5; g : BOOL := TRUE; m : REAL := -1.0;
               ru : UDINT; rx : UDINT; rd : DINT; rc : DINT; rl : LINT; rr : REAL;
               rs : ULINT; rn : REAL; rln : REAL; END_VAR
           ru := MIN(u, one);
           rx := MAX(u, one);
           rd := LIMIT(-2, d, 2);
           rc := LIMIT(5, d, 2);
           rl := MAX(l, 3);
           rr := LIMIT(0.0, r, 5.0);
           rs := SEL(g, ul, 6);
           rn := MIN(SQRT(m), 1.0);
           rln := LIMIT(0.0, SQRT(m), 1.0);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<u64>("ru"), 1);
    assert_eq!(snapshot.read_as::<u64>("rx"), 3_000_000_000);
    assert_eq!(snapshot.read_as::<i64>("rd"), -2);
    assert_eq!(snapshot.read_as::<i64>("rc"), 2);
    assert_eq!(snapshot.read_as::<i64>("rl"), 9);
    assert_eq!(snapshot.read_as::<f32>("rr"), 5.0);
    assert_eq!(snapshot.read_as::<u64>("rs"), 6);
    assert_eq!(snapshot.read_as::<f32>("rn"), 1.0);
    assert!(snapshot.read_as::<f32>("rln").is_nan());
}

/// A selection function lowers and compiles.
#[rstest]
#[case::min_dint("d := MIN(d, e);")]
#[case::max_udint("u := MAX(u, v);")]
#[case::limit_lint("l := LIMIT(l, l, l);")]
#[case::limit_real("r := LIMIT(r, r, r);")]
#[case::sel_udint("u := SEL(g, u, v);")]
#[case::sel_lreal("x := SEL(g, x, x);")]
fn compile_when_selection_function_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR d : DINT; e : DINT; u : UDINT; v : UDINT; l : LINT; r : REAL; x : LREAL;
               g : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-codegen-089: `MUX` gives the input its selector names, the first
/// for a selector below zero and the last for one past the last input.
#[test]
fn run_when_mux_then_input_selected_with_selector_clamped() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR one : DINT := 1; below : INT := -1; past : DINT := 5; u : UDINT := 4000000000;
               x : LREAL := 2.5; l : LINT := 2;
               r1 : DINT; r2 : DINT; r3 : DINT; r4 : UDINT; r5 : LREAL; END_VAR
           r1 := MUX(one, 10, 20, 30);
           r2 := MUX(below, 10, 20, 30);
           r3 := MUX(past, 10, 20, 30);
           r4 := MUX(one, UDINT#1, u);
           r5 := MUX(l, 0.5, 1.5, x);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 20);
    assert_eq!(snapshot.read_as::<i64>("r2"), 10);
    assert_eq!(snapshot.read_as::<i64>("r3"), 30);
    assert_eq!(snapshot.read_as::<u64>("r4"), 4_000_000_000);
    assert_eq!(snapshot.read_as::<f64>("r5"), 2.5);
}

/// A `MUX` lowers and compiles, whatever its number of inputs and type.
#[rstest]
#[case::two_dint("d := MUX(k, d, e);")]
#[case::three_udint("u := MUX(k, u, u, u);")]
#[case::lreal("x := MUX(k, x, x);")]
fn compile_when_mux_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR k : DINT; d : DINT; e : DINT; u : UDINT; x : LREAL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-codegen-086, REQ-LOW-codegen-087: explicit conversions give
/// the expected values. A real converts to an integer toward zero,
/// saturating at its operation type, then wraps to a narrow target's
/// storage. A conversion to `BOOL` tests for zero rather than keeping the
/// low bit, and `TRUNC` converts to the integer of its context.
#[test]
fn run_when_conversions_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR big : REAL := 40000.0; neg : REAL := -2.7; huge : REAL := 1.0E20;
               u : UDINT := 3000000000; m : DINT := -1; i : INT := 300; w : WORD := 16#0100;
               g : BOOL := TRUE; zero : LINT := 0; t : TIME := T#1500ms; x : LREAL := 1.0E12;
               r1 : INT; r2 : DINT; r3 : DINT; r4 : LINT; r5 : UDINT; r6 : SINT; r7 : BOOL;
               r8 : INT; r9 : BOOL; r10 : REAL; r11 : DINT; r12 : DINT; r13 : LINT;
               r14 : REAL; END_VAR
           r1 := REAL_TO_INT(big);
           r2 := REAL_TO_DINT(neg);
           r3 := REAL_TO_DINT(huge);
           r4 := UDINT_TO_LINT(u);
           r5 := DINT_TO_UDINT(m);
           r6 := INT_TO_SINT(i);
           r7 := WORD_TO_BOOL(w);
           r8 := BOOL_TO_INT(g);
           r9 := LINT_TO_BOOL(zero);
           r10 := UDINT_TO_REAL(u);
           r11 := TIME_TO_DINT(t);
           r12 := TRUNC(neg);
           r13 := TRUNC(x);
           r14 := TRUNC(neg);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), -25536);
    assert_eq!(snapshot.read_as::<i64>("r2"), -2);
    assert_eq!(snapshot.read_as::<i64>("r3"), 2_147_483_647);
    assert_eq!(snapshot.read_as::<i64>("r4"), 3_000_000_000);
    assert_eq!(snapshot.read_as::<u64>("r5"), 4_294_967_295);
    assert_eq!(snapshot.read_as::<i64>("r6"), 44);
    assert_eq!(snapshot.read_as::<i64>("r7"), 1);
    assert_eq!(snapshot.read_as::<i64>("r8"), 1);
    assert_eq!(snapshot.read_as::<i64>("r9"), 0);
    assert_eq!(snapshot.read_as::<f32>("r10"), 3.0e9);
    assert_eq!(snapshot.read_as::<i64>("r11"), 1500);
    assert_eq!(snapshot.read_as::<i64>("r12"), -2);
    assert_eq!(snapshot.read_as::<i64>("r13"), 1_000_000_000_000);
    assert_eq!(snapshot.read_as::<f32>("r14"), -2.0);
}

/// An explicit conversion, and `TRUNC`, lower and compile.
#[rstest]
#[case::real_to_int("d := REAL_TO_INT(r);")]
#[case::dint_to_lreal("x := DINT_TO_LREAL(d);")]
#[case::udint_to_lint("l := UDINT_TO_LINT(u);")]
#[case::lint_to_dint("d := LINT_TO_DINT(l);")]
#[case::int_to_sint("d := INT_TO_SINT(i);")]
#[case::dint_to_int("i := DINT_TO_INT(d);")]
#[case::int_to_bool("g := INT_TO_BOOL(i);")]
#[case::lreal_to_udint("u := LREAL_TO_UDINT(x);")]
#[case::bool_to_int("d := BOOL_TO_INT(g);")]
#[case::time_to_dint("d := TIME_TO_DINT(t);")]
#[case::int_to_real_widened("x := INT_TO_REAL(i);")]
#[case::trunc_real("d := TRUNC(r);")]
#[case::trunc_to_lint("l := TRUNC(x);")]
#[case::trunc_to_int("i := TRUNC(r);")]
fn compile_when_conversion_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR i : INT; d : DINT; l : LINT; u : UDINT; r : REAL; x : LREAL; g : BOOL;
               t : TIME; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}

/// REQ-LOW-codegen-089: BCD conversions give the expected values.
/// `BCD_TO_INT` decodes four bits above 9 as 0, and `INT_TO_BCD` encodes its
/// input's bits as unsigned, modulo the power of ten its digits hold.
#[test]
fn run_when_bcd_conversions_then_values() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR w : WORD := 16#1234; b : BYTE := 16#9A; lw : LWORD := 16#1234567890123456;
               i : INT := 1234; s : SINT := -1; us : USINT := 123; u : UDINT := 12345678;
               r1 : INT; r2 : DINT; r3 : LINT; r4 : WORD; r5 : BYTE; r6 : BYTE;
               r7 : DWORD; END_VAR
           r1 := BCD_TO_INT(w);
           r2 := BCD_TO_INT(b);
           r3 := BCD_TO_INT(lw);
           r4 := INT_TO_BCD(i);
           r5 := INT_TO_BCD(s);
           r6 := INT_TO_BCD(us);
           r7 := INT_TO_BCD(u);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("r1"), 1234);
    assert_eq!(snapshot.read_as::<i64>("r2"), 90);
    assert_eq!(snapshot.read_as::<i64>("r3"), 1_234_567_890_123_456);
    assert_eq!(snapshot.read_as::<u64>("r4"), 0x1234);
    assert_eq!(snapshot.read_as::<u64>("r5"), 0x55);
    assert_eq!(snapshot.read_as::<u64>("r6"), 0x23);
    assert_eq!(snapshot.read_as::<u64>("r7"), 0x1234_5678);
}

/// A BCD conversion and `SIZEOF` lower and compile.
#[rstest]
#[case::bcd_to_int_word("d := BCD_TO_INT(w);")]
#[case::bcd_to_int_to_int("i := BCD_TO_INT(w);")]
#[case::bcd_to_int_lword("l := BCD_TO_INT(lw);")]
#[case::bcd_to_int_lword_to_dint("d := BCD_TO_INT(lw);")]
#[case::int_to_bcd_int("w := INT_TO_BCD(i);")]
#[case::int_to_bcd_dint("dw := INT_TO_BCD(d);")]
#[case::int_to_bcd_udint("dw := INT_TO_BCD(u);")]
#[case::int_to_bcd_lint("lw := INT_TO_BCD(l);")]
#[case::sizeof_array("d := SIZEOF(arr);")]
#[case::sizeof_bool("d := SIZEOF(g);")]
#[case::sizeof_element("l := SIZEOF(arr[1]);")]
fn compile_when_bcd_or_sizeof_lowered_then_compiles(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR w : WORD; dw : DWORD; lw : LWORD; i : INT; d : DINT; u : UDINT; l : LINT;
               g : BOOL; arr : ARRAY[1..4] OF INT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_compiles(&source);
}
