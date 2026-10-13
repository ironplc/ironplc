//! Tests of lowering calls of standard functions, and the bit and partial
//! reads that use them.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// REQ-LOW-lowering-147: a shift or rotate is the intrinsic for its function
/// at the type of the value shifted, a rotate of a `BYTE` or `WORD` keeping
/// its own width.
#[test]
fn lower_statement_when_shift_or_rotate_then_intrinsic_call() {
    let outcome = lower(
        "PROGRAM main VAR b : BYTE; w : WORD; d : DWORD; l : LWORD; n : DINT; END_VAR
            d := SHL(d, n);
            l := SHR(l, 3);
            b := ROL(b, 1);
            w := ROR(w, n);
            l := ROL(l, 2);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call shl_u32 i32u (read i32u v0^.f2) (read i32s v0^.f4))",
            "assign v0^.f3 (call shr_u64 i64u (read i64u v0^.f3) (const i64s 3))",
            "assign v0^.f0 (truncate 8 (call rol_u8 i32u (read i32u v0^.f0) (const i32s 1)))",
            "assign v0^.f1 (truncate 16 (call ror_u16 i32u (read i32u v0^.f1) (read i32s v0^.f4)))",
            "assign v0^.f3 (call rol_u64 i64u (read i64u v0^.f3) (const i64s 2))",
        ]
    );
}

/// REQ-LOW-lowering-100: a bit or partial read is the same shift and mask
/// over a read of its base, whatever the base's shape; a bit is compared
/// with zero.
#[test]
fn lower_statement_when_bit_or_partial_read_then_shift_and_mask() {
    let outcome = lower(
        "PROGRAM main VAR w : WORD; a : ARRAY[0..3] OF WORD; i : DINT; x : BOOL; b : BYTE;
             n : INT; l : LWORD; END_VAR
            x := w.3;
            x := a[i].3;
            b := w.%B1;
            x := n.15;
            b := l.%B7;
            x := w.0;
         END_PROGRAM",
        "main",
    );
    let bit = |read: &str, shift: u8| {
        format!(
            "assign v0^.f3 (ne (bit_and i32u (call shr_u32 i32u {read} (const i32s {shift})) \
             (const i32u 1)) (const i32u 0))"
        )
    };
    assert_eq!(
        outcome.statements,
        [
            bit("(read i32u v0^.f0)", 3),
            bit("(read i32u v0^.f1[(read i32s v0^.f2)])", 3),
            "assign v0^.f4 (truncate 8 (bit_and i32u (call shr_u32 i32u (read i32u v0^.f0) \
             (const i32s 8)) (const i32u 255)))"
                .to_string(),
            bit("(convert i32u (read i32s v0^.f5))", 15),
            "assign v0^.f4 (truncate 8 (convert i32u (bit_and i64u (call shr_u64 i64u \
             (read i64u v0^.f6) (const i64s 56)) (const i64u 255))))"
                .to_string(),
            "assign v0^.f3 (ne (bit_and i32u (read i32u v0^.f0) (const i32u 1)) (const i32u 0))"
                .to_string(),
        ]
    );
}

/// The bits of a bit selection, `d.%B1.3`, are selected from the inner
/// selection's value.
#[test]
fn lower_statement_when_bits_of_bits_read_then_selected_twice() {
    let outcome = lower(
        "PROGRAM main VAR d : DWORD; x : BOOL; END_VAR
            x := d.%B1.3;
         END_PROGRAM",
        "main",
    );
    let byte = "(bit_and i32u (call shr_u32 i32u (read i32u v0^.f0) (const i32s 8)) \
                (const i32u 255))";
    assert_eq!(
        outcome.statements,
        [format!(
            "assign v0^.f1 (ne (bit_and i32u (call shr_u32 i32u {byte} (const i32s 3)) \
             (const i32u 1)) (const i32u 0))"
        )]
    );
}

/// REQ-LOW-lowering-094: an unsigned shift count is converted to its
/// parameter's type, the conversion the analyzer records for it.
#[test]
fn lower_statement_when_unsigned_shift_count_then_converted_to_parameter_type() {
    let outcome = lower(
        "PROGRAM main VAR d : DWORD; l : LWORD; u : UDINT; ul : ULINT; END_VAR
            d := SHL(d, u);
            l := SHR(l, ul);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (call shl_u32 i32u (read i32u v0^.f0) (convert i32s (read i32u v0^.f2)))",
            "assign v0^.f1 (call shr_u64 i64u (read i64u v0^.f1) (convert i64s (read i64u v0^.f3)))",
        ]
    );
}

/// A call of a user-defined function and bits read through a reference are
/// not lowered yet.
#[test]
fn lower_statement_when_call_or_read_not_supported_then_unsupported() {
    let outcome = lower(
        "FUNCTION F : DINT VAR_INPUT a : DINT; END_VAR F := a; END_FUNCTION
         PROGRAM main VAR x : BOOL; p : REF_TO WORD; r : DINT; END_VAR
            r := F(1);
            x := p^.3;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// REQ-LOW-lowering-101: an operator and its function form lower to the same
/// expression, and an extensible form folds its inputs from the left. `**`
/// is the standard function `EXPT`.
#[test]
fn lower_statement_when_operator_function_form_then_same_as_operator() {
    let pairs = [
        ("r := ADD(a, b);", "r := a + b;"),
        ("r := ADD(a, b, c);", "r := a + b + c;"),
        ("r := SUB(a, b);", "r := a - b;"),
        ("r := MUL(a, b, c);", "r := a * b * c;"),
        ("r := DIV(a, b);", "r := a / b;"),
        ("r := MOD(a, b);", "r := a MOD b;"),
        ("x := GT(a, b);", "x := a > b;"),
        ("x := LE(a, b);", "x := a <= b;"),
        ("x := AND(x, y, z);", "x := x AND y AND z;"),
        ("w := OR(w, v);", "w := w OR v;"),
        ("w := XOR(w, v);", "w := w XOR v;"),
        ("x := NOT(x);", "x := NOT x;"),
        ("w := NOT(w);", "w := NOT w;"),
        ("r := MOVE(a);", "r := a;"),
        ("r := EXPT(a, b);", "r := a ** b;"),
    ];
    for (form, operator) in pairs {
        let outcome = lower(
            &format!(
                "PROGRAM main VAR a : DINT; b : DINT; c : DINT; r : DINT;
                     x : BOOL; y : BOOL; z : BOOL; w : WORD; v : WORD; END_VAR
                    {form}
                    {operator}
                 END_PROGRAM"
            ),
            "main",
        );
        assert_ne!(outcome.statements[0], "unsupported", "{form}");
        assert_eq!(outcome.statements[0], outcome.statements[1], "{form}");
    }
}

/// A typed overload on times, such as `ADD` of two `TIME`s, is not lowered
/// yet, as `t + t` is not.
#[test]
fn lower_statement_when_operator_form_on_times_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR t : TIME; END_VAR
            t := ADD(t, t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}

/// REQ-LOW-lowering-147: `ABS` and `EXPT` are the intrinsic for their
/// function at the signed integer or real type they compute at, as are the
/// real functions of two inputs. The exponent of a real is converted to it.
#[test]
fn lower_statement_when_numeric_function_then_intrinsic_call() {
    let outcome = lower(
        "PROGRAM main VAR i : INT; d : DINT; l : LINT; r : REAL; x : LREAL; END_VAR
            i := ABS(i);
            l := ABS(l);
            x := ABS(x);
            d := EXPT(d, 2);
            r := EXPT(r, d);
            r := ATAN2(r, r);
            x := __MOD(x, x);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (truncate 16 (call abs_i32 i32s (read i32s v0^.f0)))",
            "assign v0^.f2 (call abs_i64 i64s (read i64s v0^.f2))",
            "assign v0^.f4 (call abs_f64 f64 (read f64 v0^.f4))",
            "assign v0^.f1 (call expt_i32 i32s (read i32s v0^.f1) (const i32s 2))",
            "assign v0^.f3 (call expt_f32 f32 (read f32 v0^.f3) (convert f32 (read i32s v0^.f1)))",
            "assign v0^.f3 (call atan2_f32 f32 (read f32 v0^.f3) (read f32 v0^.f3))",
            "assign v0^.f4 (call mod_f64 f64 (read f64 v0^.f4) (read f64 v0^.f4))",
        ]
    );
}

/// REQ-LOW-lowering-147: a real function of one input is the intrinsic for
/// it at `REAL` and at `LREAL`.
#[test]
fn lower_statement_when_real_function_then_intrinsic_at_its_width() {
    let functions = [
        ("SQRT", "sqrt"),
        ("LN", "ln"),
        ("LOG", "log"),
        ("EXP", "exp"),
        ("SIN", "sin"),
        ("COS", "cos"),
        ("TAN", "tan"),
        ("ASIN", "asin"),
        ("ACOS", "acos"),
        ("ATAN", "atan"),
        ("__TRUNC", "trunc"),
    ];
    for (function, name) in functions {
        let outcome = lower(
            &format!(
                "PROGRAM main VAR r : REAL; x : LREAL; END_VAR
                    r := {function}(r);
                    x := {function}(x);
                 END_PROGRAM"
            ),
            "main",
        );
        assert_eq!(
            outcome.statements,
            [
                format!("assign v0^.f0 (call {name}_f32 f32 (read f32 v0^.f0))"),
                format!("assign v0^.f1 (call {name}_f64 f64 (read f64 v0^.f1))"),
            ],
            "{function}"
        );
    }
}

/// `EXPT` of an unsigned integer, and `MUX`, are not lowered yet.
#[test]
fn lower_statement_when_numeric_function_not_supported_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR u : UDINT; d : DINT; END_VAR
            u := EXPT(u, 2);
            d := MUX(1, d, 2);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// REQ-LOW-lowering-147: a selection function is the intrinsic for its
/// function at the type it computes at, signed, unsigned or real, and `SEL`
/// takes its selector as the `BOOL` it is.
#[test]
fn lower_statement_when_selection_function_then_intrinsic_at_its_type() {
    let types = [
        ("DINT", "i32s", "i32"),
        ("UDINT", "i32u", "u32"),
        ("LINT", "i64s", "i64"),
        ("ULINT", "i64u", "u64"),
        ("REAL", "f32", "f32"),
        ("LREAL", "f64", "f64"),
    ];
    for (declared, ty, suffix) in types {
        let outcome = lower(
            &format!(
                "PROGRAM main VAR a : {declared}; b : {declared}; c : {declared}; g : BOOL; END_VAR
                    a := MIN(a, b);
                    a := MAX(a, b);
                    a := LIMIT(a, b, c);
                    a := SEL(g, b, c);
                 END_PROGRAM"
            ),
            "main",
        );
        let (a, b, c) = (
            format!("(read {ty} v0^.f0)"),
            format!("(read {ty} v0^.f1)"),
            format!("(read {ty} v0^.f2)"),
        );
        assert_eq!(
            outcome.statements,
            [
                format!("assign v0^.f0 (call min_{suffix} {ty} {a} {b})"),
                format!("assign v0^.f0 (call max_{suffix} {ty} {a} {b})"),
                format!("assign v0^.f0 (call limit_{suffix} {ty} {a} {b} {c})"),
                format!("assign v0^.f0 (call sel_{suffix} {ty} (read bool v0^.f3) {b} {c})"),
            ],
            "{declared}"
        );
    }
}
