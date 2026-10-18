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

/// A read of bits of an element reached through a reference is the shift
/// and mask over a read of the element, a `Deref` then an `Index`
/// (REQ-LOW-lowering-100).
#[test]
fn lower_statement_when_bits_of_element_through_reference_read_then_shift_and_mask() {
    let outcome = lower(
        "PROGRAM main VAR x : BOOL; p : REF_TO ARRAY[0..1] OF DWORD; END_VAR
            x := p^[1].3;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0 (ne (bit_and i32u (call shr_u32 i32u \
          (read i32u v0^.f1^[(const i32s 1)]) (const i32s 3)) (const i32u 1)) (const i32u 0))"]
    );
}

/// Bits of the whole of what a reference refers to, `p^.3`, are not read
/// while codegen's AST route does not address them.
#[test]
fn lower_statement_when_bits_read_through_reference_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR x : BOOL; p : REF_TO WORD; END_VAR
            x := p^.3;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
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

/// REQ-LOW-lowering-147: `ABS` is the intrinsic for its function at the
/// signed integer or real type it computes at, `EXPT` at the signed or
/// unsigned integer or real type, as are the real functions of two inputs.
/// The exponent of a real is converted to it.
#[test]
fn lower_statement_when_numeric_function_then_intrinsic_call() {
    let outcome = lower(
        "PROGRAM main VAR i : INT; d : DINT; l : LINT; r : REAL; x : LREAL; u : UDINT;
             ul : ULINT; END_VAR
            i := ABS(i);
            l := ABS(l);
            x := ABS(x);
            d := EXPT(d, 2);
            u := EXPT(u, u);
            ul := ul ** ul;
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
            "assign v0^.f5 (call expt_u32 i32u (read i32u v0^.f5) (read i32u v0^.f5))",
            "assign v0^.f6 (call expt_u64 i64u (read i64u v0^.f6) (read i64u v0^.f6))",
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

/// REQ-LOW-lowering-147: `MUX` is the intrinsic for the type it computes at,
/// taking its `DINT` selector and then each of its inputs. A selector of
/// another integer type is the `DINT` the analyzer converted it to.
#[test]
fn lower_statement_when_mux_then_intrinsic_with_each_input() {
    let outcome = lower(
        "PROGRAM main VAR k : INT; d : DINT; u : UDINT; r : LREAL; l : LINT; END_VAR
            d := MUX(k, d, 2);
            u := MUX(k, u, u, u);
            r := MUX(1, r, r);
            d := MUX(l, d, d);
            d := MUX(u, d, d);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (call mux_i32 i32s (read i32s v0^.f0) (read i32s v0^.f1) (const i32s 2))",
            "assign v0^.f2 (call mux_u32 i32u (read i32s v0^.f0) (read i32u v0^.f2) \
             (read i32u v0^.f2) (read i32u v0^.f2))",
            "assign v0^.f3 (call mux_f64 f64 (const i32s 1) (read f64 v0^.f3) (read f64 v0^.f3))",
            "assign v0^.f1 (call mux_i32 i32s (convert i32s (read i64s v0^.f4)) \
             (read i32s v0^.f1) (read i32s v0^.f1))",
            "assign v0^.f1 (call mux_i32 i32s (convert i32s (read i32u v0^.f2)) \
             (read i32s v0^.f1) (read i32s v0^.f1))",
        ]
    );
}

/// An explicit conversion is a `Convert` where the operation type changes,
/// then a `Truncate` where the target's storage narrows. A conversion to
/// `BOOL` is a comparison with zero.
#[test]
fn lower_statement_when_conversion_then_convert_then_truncate() {
    let outcome = lower(
        "PROGRAM main VAR d : DINT; r : REAL; i : INT; l : LINT; u : UDINT; s : SINT;
             x : BOOL; w : WORD; t : TIME; lr : LREAL; END_VAR
            r := DINT_TO_REAL(d);
            i := REAL_TO_INT(r);
            d := LINT_TO_DINT(l);
            u := DINT_TO_UDINT(d);
            s := INT_TO_SINT(i);
            x := WORD_TO_BOOL(w);
            i := BOOL_TO_INT(x);
            d := TIME_TO_DINT(t);
            lr := INT_TO_REAL(i);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (convert f32 (read i32s v0^.f0))",
            "assign v0^.f2 (truncate 16 (convert i32s (read f32 v0^.f1)))",
            "assign v0^.f0 (convert i32s (read i64s v0^.f3))",
            "assign v0^.f4 (convert i32u (read i32s v0^.f0))",
            "assign v0^.f5 (truncate 8 (read i32s v0^.f2))",
            "assign v0^.f6 (ne (read i32u v0^.f7) (const i32u 0))",
            "assign v0^.f2 (truncate 16 (convert i32s (read bool v0^.f6)))",
            "assign v0^.f0 (read i32s v0^.f8)",
            // The `REAL` the conversion returns is widened to the `LREAL`.
            "assign v0^.f9 (convert f64 (convert f32 (read i32s v0^.f2)))",
        ]
    );
}

/// `TRUNC` converts a real to the integer of its context: 64 bits for an
/// `LINT`, and a `DINT` converted on for a `REAL`. Like an arithmetic
/// result, it is narrowed only where it is stored.
#[test]
fn lower_statement_when_trunc_then_convert_to_integer_of_context() {
    let outcome = lower(
        "PROGRAM main VAR d : DINT; r : REAL; i : INT; l : LINT; lr : LREAL; END_VAR
            d := TRUNC(r);
            l := TRUNC(lr);
            r := TRUNC(r);
            i := TRUNC(r);
            d := TRUNC(r) + i;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (convert i32s (read f32 v0^.f1))",
            "assign v0^.f3 (convert i64s (read f64 v0^.f4))",
            "assign v0^.f1 (convert f32 (convert i32s (read f32 v0^.f1)))",
            "assign v0^.f2 (truncate 16 (convert i32s (read f32 v0^.f1)))",
            "assign v0^.f0 (add i32s (convert i32s (read f32 v0^.f1)) (read i32s v0^.f2))",
        ]
    );
}

/// REQ-LOW-lowering-147: `BCD_TO_INT` is the intrinsic for the width of its
/// bit string, converted to the integer of its context, and `INT_TO_BCD` the
/// intrinsic for the width and signedness of its integer. An integer literal
/// given to `BCD_TO_INT` is a `DWORD` (REQ-IC-analyzer-118).
#[test]
fn lower_statement_when_bcd_conversion_then_intrinsic_at_its_width() {
    let outcome = lower(
        "PROGRAM main VAR w : WORD; b : BYTE; lw : LWORD; i : INT; us : USINT; d : DWORD;
             di : DINT; l : LINT; END_VAR
            i := BCD_TO_INT(w);
            di := BCD_TO_INT(b);
            l := BCD_TO_INT(lw);
            di := BCD_TO_INT(lw);
            di := BCD_TO_INT(16#12);
            w := INT_TO_BCD(i);
            b := INT_TO_BCD(us);
            lw := INT_TO_BCD(l);
            d := INT_TO_BCD(42);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f3 (truncate 16 (call bcd_to_int_u16 i32s (read i32u v0^.f0)))",
            "assign v0^.f6 (call bcd_to_int_u8 i32s (read i32u v0^.f1))",
            "assign v0^.f7 (call bcd_to_int_u64 i64s (read i64u v0^.f2))",
            "assign v0^.f6 (convert i32s (call bcd_to_int_u64 i64s (read i64u v0^.f2)))",
            "assign v0^.f6 (call bcd_to_int_u32 i32s (const i32u 18))",
            "assign v0^.f0 (truncate 16 (call int_to_bcd_i16 i32u (read i32s v0^.f3)))",
            "assign v0^.f1 (truncate 8 (call int_to_bcd_u8 i32u (read i32u v0^.f4)))",
            "assign v0^.f2 (call int_to_bcd_i64 i64u (read i64s v0^.f7))",
            "assign v0^.f5 (call int_to_bcd_i32 i32u (const i32s 42))",
        ]
    );
}

/// REQ-LOW-lowering-103: `SIZEOF` is a constant from the language's storage
/// widths, at the integer of its context: a `BOOL` takes a byte, and an
/// array its element's size for each element.
#[test]
fn lower_statement_when_sizeof_then_constant_of_storage_width() {
    let outcome = lower(
        "TYPE MATRIX : ARRAY[1..2, 1..5] OF REAL; END_TYPE
         PROGRAM main VAR arr : ARRAY[0..3] OF INT; m : MATRIX; x : BOOL; t : TIME;
             d : DINT; l : LINT; END_VAR
            d := SIZEOF(arr);
            d := SIZEOF(m);
            d := SIZEOF(x);
            d := SIZEOF(t);
            l := SIZEOF(arr[1]);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f4 (const i32s 8)",
            "assign v0^.f4 (const i32s 40)",
            "assign v0^.f4 (const i32s 1)",
            "assign v0^.f4 (const i32s 4)",
            "assign v0^.f5 (const i64s 2)",
        ]
    );
}

/// REQ-LOW-lowering-103: a string takes one code unit more than its
/// capacity, a byte for a `STRING` and two for a `WSTRING`, and a string
/// declared without one has the default capacity of 254.
#[test]
fn lower_statement_when_sizeof_string_then_capacity_and_terminator() {
    let outcome = lower(
        "TYPE NAME : STRING[20]; END_TYPE
         PROGRAM main VAR s : STRING[10]; w : WSTRING[10]; u : STRING; n : NAME;
             names : ARRAY[1..3] OF STRING[4]; d : DINT; END_VAR
            d := SIZEOF(s);
            d := SIZEOF(w);
            d := SIZEOF(u);
            d := SIZEOF(n);
            d := SIZEOF(names);
            d := SIZEOF(names[2]);
         END_PROGRAM",
        "main",
    );
    let size = |bytes: u32| format!("assign v0^.f5 (const i32s {bytes})");
    assert_eq!(
        outcome.statements,
        [size(11), size(22), size(255), size(21), size(15), size(5)]
    );
}

/// REQ-LOW-lowering-103: an enumeration takes its underlying type's size,
/// an `INT`'s when its declaration names none (REQ-LOW-lowering-190).
#[test]
fn lower_statement_when_sizeof_enumeration_then_underlying_type_size() {
    let outcome = lower(
        "TYPE COLOR : (RED, GREEN); END_TYPE
         TYPE SMALL : (A, B) BYTE; END_TYPE
         TYPE WIDE : (C, D) LWORD; END_TYPE
         PROGRAM main VAR c : COLOR; s : SMALL; w : WIDE; e : (X, Y);
             colors : ARRAY[1..4] OF COLOR; d : DINT; END_VAR
            d := SIZEOF(c);
            d := SIZEOF(s);
            d := SIZEOF(w);
            d := SIZEOF(e);
            d := SIZEOF(colors);
         END_PROGRAM",
        "main",
    );
    let size = |bytes: u32| format!("assign v0^.f5 (const i32s {bytes})");
    assert_eq!(
        outcome.statements,
        [size(2), size(1), size(8), size(2), size(8)]
    );
}

/// REQ-LOW-lowering-103: the size of a structure is a target's layout, so
/// its `SIZEOF` is not lowered, nor is that of an array of structures.
#[test]
fn lower_statement_when_sizeof_of_layout_then_unsupported() {
    let outcome = lower(
        "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         PROGRAM main VAR p : P; ps : ARRAY[1..2] OF P; d : DINT; END_VAR
            d := SIZEOF(p);
            d := SIZEOF(ps);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"; 2]);
}
