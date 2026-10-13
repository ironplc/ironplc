//! Tests of the `AssignBits` statement's constructor and text form.

use super::*;

/// REQ-LOW-lowering-065: one bit within the width of an integer place is
/// assigned from a `BOOL`.
#[test]
fn assign_bits_when_one_bit_within_width_then_statement() {
    let f = fixture();
    let stmt =
        f.b.assign_bits(f.place(f.int), 15, 1, f.read(f.bool), span())
            .unwrap();
    assert_eq!(stmt.to_string(), "assign-bits v1 15 1 (read bool v3)");
}

/// REQ-LOW-lowering-065: bits within the width of an integer place are
/// assigned from an integer that holds them.
#[test]
fn assign_bits_when_byte_within_width_then_statement() {
    let f = fixture();
    let stmt =
        f.b.assign_bits(f.place(f.int), 8, 8, int(255), span())
            .unwrap();
    assert_eq!(stmt.to_string(), "assign-bits v1 8 8 (const i32s 255)");
}

/// REQ-LOW-lowering-065: the bits lie within the place's width, and there
/// is at least one.
#[test]
fn assign_bits_when_bits_outside_width_then_internal_error() {
    let f = fixture();
    for (shift, bits) in [(8, 9), (16, 1), (0, 0)] {
        assert!(
            is_internal_error_naming(
                f.b.assign_bits(f.place(f.int), shift, bits, int(1), span()),
                "Stmt::assign_bits"
            ),
            "{bits} bits from bit {shift}"
        );
    }
}

/// REQ-LOW-lowering-065: only an integer or a bit string holds bits to
/// assign.
#[test]
fn assign_bits_when_place_not_integer_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.assign_bits(f.place(f.lreal), 0, 1, f.read(f.bool), span()),
        "Stmt::assign_bits"
    ));
}

#[test]
fn assign_bits_when_value_narrower_than_bits_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.assign_bits(f.place(f.int), 0, 2, f.read(f.bool), span()),
        "Stmt::assign_bits"
    ));
}

#[test]
fn assign_bits_when_value_reads_another_program_then_internal_error() {
    let f = fixture();
    let g = fixture();
    assert!(is_internal_error_naming(
        f.b.assign_bits(f.place(f.int), 0, 8, g.read(g.dint), span()),
        "Stmt::assign_bits"
    ));
}
