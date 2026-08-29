//! Comparison-operator codes used as the first operand of `CMP_BR_*`.
//!
//! Negation pairs (used by codegen to emit a "branch if false" predicate
//! from a "branch if true" opcode):
//!   EQ ↔ NE,  LT_S ↔ GE_S,  LE_S ↔ GT_S.
//!
//! Commutation pairs (used to rewrite `const <cmp> var` to `var <cmp> const`):
//!   EQ ↔ EQ,  NE ↔ NE,  LT_S ↔ GT_S,  LE_S ↔ GE_S.

pub const EQ: u8 = 0;
pub const NE: u8 = 1;
pub const LT_S: u8 = 2;
pub const LE_S: u8 = 3;
pub const GT_S: u8 = 4;
pub const GE_S: u8 = 5;

/// Returns the negation of the given comparison operator (e.g.
/// `LT_S` ↔ `GE_S`). Returns `None` for unrecognised codes.
pub const fn negate(cmp_op: u8) -> Option<u8> {
    match cmp_op {
        EQ => Some(NE),
        NE => Some(EQ),
        LT_S => Some(GE_S),
        GE_S => Some(LT_S),
        LE_S => Some(GT_S),
        GT_S => Some(LE_S),
        _ => None,
    }
}

/// Returns the commutation of the given comparison operator
/// (i.e. the operator equivalent under operand swap).
/// Returns `None` for unrecognised codes.
pub const fn commute(cmp_op: u8) -> Option<u8> {
    match cmp_op {
        EQ => Some(EQ),
        NE => Some(NE),
        LT_S => Some(GT_S),
        GT_S => Some(LT_S),
        LE_S => Some(GE_S),
        GE_S => Some(LE_S),
        _ => None,
    }
}

/// Whether `cmp_op` is a recognised comparison operator code.
pub const fn is_valid(cmp_op: u8) -> bool {
    matches!(cmp_op, EQ | NE | LT_S | LE_S | GT_S | GE_S)
}
