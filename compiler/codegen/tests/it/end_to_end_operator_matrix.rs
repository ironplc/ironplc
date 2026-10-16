//! End-to-end tests for what the operators and standard functions compute at
//! every elementary type they take.
//!
//! Each test compiles one program that evaluates a table of expressions over
//! the inputs `a`, `b`, `c` and the count `n`, writes values to the inputs,
//! runs a scan and compares every result with a Rust oracle. A test passes
//! when no result differs. The oracle states the rules a result follows
//! whatever backend computes it:
//!
//! - an integer operation wraps at its operation width, 32 bits for a type
//!   that fits and 64 otherwise, and the stored result keeps the low bits of
//!   its type (ADR-0001);
//! - division rounds toward zero and `MOD` takes the sign of the dividend;
//! - a shift takes its count modulo the operation width, and a rotation
//!   modulo the storage width;
//! - a comparison is of the values, signed or unsigned as the type is, and
//!   with a NaN operand only `<>` holds.
//!
//! The `rstest` anchors run every type at the ends of its range. The
//! property tests add arbitrary values.

use std::cmp::Ordering;

use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;

use crate::common::scalar::{
    self, scalars, wrap, Mismatch, Num, Scalar, BIT_STRINGS, BOOL, BYTE, DATE, DINT, DT, DWORD,
    INT, INTEGERS, LDATE, LDT, LINT, LREAL, LTIME, LTOD, LWORD, REAL, SINT, TIME, TOD, UDINT, UINT,
    ULINT, USINT, WORD,
};
use crate::common::{run_scans, Value};

/// The integer inputs of one scan: the operands `a` and `b`, the divisor or
/// third operand `c`, which is never zero, and the shift count `n`.
type Ints = [i128; 4];

/// What an expression computes from its inputs.
#[derive(Clone, Copy)]
enum Oracle {
    /// A value of the operand type, before it is wrapped to the type.
    Same(fn(Scalar, Ints) -> i128),
    /// A `BOOL` that depends only on how `a` compares with `b`, which a NaN
    /// operand leaves unordered.
    Compare(fn(Option<Ordering>) -> bool),
}

/// An expression over the inputs, and what it computes.
#[derive(Clone, Copy)]
struct Operation {
    expr: &'static str,
    oracle: Oracle,
}

const fn same(expr: &'static str, oracle: fn(Scalar, Ints) -> i128) -> Operation {
    Operation {
        expr,
        oracle: Oracle::Same(oracle),
    }
}

const fn compare(expr: &'static str, oracle: fn(Option<Ordering>) -> bool) -> Operation {
    Operation {
        expr,
        oracle: Oracle::Compare(oracle),
    }
}

/// The comparisons, as operators and as the standard functions that name
/// them. Every elementary type other than a string takes them.
const COMPARISONS: [Operation; 12] = [
    compare("a < b", |order| order.is_some_and(Ordering::is_lt)),
    compare("a <= b", |order| order.is_some_and(Ordering::is_le)),
    compare("a = b", |order| order.is_some_and(Ordering::is_eq)),
    compare("a <> b", |order| order.is_none_or(Ordering::is_ne)),
    compare("a > b", |order| order.is_some_and(Ordering::is_gt)),
    compare("a >= b", |order| order.is_some_and(Ordering::is_ge)),
    compare("LT(a, b)", |order| order.is_some_and(Ordering::is_lt)),
    compare("LE(a, b)", |order| order.is_some_and(Ordering::is_le)),
    compare("EQ(a, b)", |order| order.is_some_and(Ordering::is_eq)),
    compare("NE(a, b)", |order| order.is_none_or(Ordering::is_ne)),
    compare("GT(a, b)", |order| order.is_some_and(Ordering::is_gt)),
    compare("GE(a, b)", |order| order.is_some_and(Ordering::is_ge)),
];

/// What the integer types compute. An operator and the standard function
/// that names it share an oracle, and so does a function at every width.
const INTEGER_ARITHMETIC: [Operation; 22] = [
    same("a + b", |_, [a, b, ..]| a + b),
    same("a - b", |_, [a, b, ..]| a - b),
    same("a * b", |_, [a, b, ..]| a.wrapping_mul(b)),
    same("-a", |_, [a, ..]| -a),
    same("a / c", |_, [a, _, c, _]| a / c),
    same("a MOD c", |_, [a, _, c, _]| a % c),
    same("a ** 2", |_, [a, ..]| a.wrapping_mul(a)),
    // The sum wraps at the operation width before it is halved, so two INTs
    // of 30000 give 30000, not the -2768 a 16-bit sum would.
    same("(a + b) / 2", |ty, [a, b, ..]| ty.operate(a + b) / 2),
    same("ADD(a, b)", |_, [a, b, ..]| a + b),
    same("SUB(a, b)", |_, [a, b, ..]| a - b),
    same("MUL(a, b)", |_, [a, b, ..]| a.wrapping_mul(b)),
    same("DIV(a, c)", |_, [a, _, c, _]| a / c),
    same("MOD(a, c)", |_, [a, _, c, _]| a % c),
    same("EXPT(a, 2)", |_, [a, ..]| a.wrapping_mul(a)),
    same("ABS(a)", |_, [a, ..]| a.abs()),
    same("MIN(a, b)", |_, [a, b, ..]| a.min(b)),
    same("MAX(a, b)", |_, [a, b, ..]| a.max(b)),
    // LIMIT(MN, IN, MX) raises IN to MN, then lowers it to MX.
    same("LIMIT(a, b, c)", |_, [a, b, c, _]| b.max(a).min(c)),
    same(
        "SEL(a < b, a, b)",
        |_, [a, b, ..]| if a < b { b } else { a },
    ),
    same("MUX(1, a, b)", |_, [_, b, ..]| b),
    same("MUX(0, a, b)", |_, [a, ..]| a),
    same("MOVE(a)", |_, [a, ..]| a),
];

/// What the bit string types compute.
const BIT_STRING_LOGIC: [Operation; 12] = [
    same("a AND b", |_, [a, b, ..]| a & b),
    same("a OR b", |_, [a, b, ..]| a | b),
    same("a XOR b", |_, [a, b, ..]| a ^ b),
    same("NOT a", |_, [a, ..]| !a),
    same("AND(a, b)", |_, [a, b, ..]| a & b),
    same("OR(a, b)", |_, [a, b, ..]| a | b),
    same("XOR(a, b)", |_, [a, b, ..]| a ^ b),
    same("NOT(a)", |_, [a, ..]| !a),
    // A shift is computed at the operation width, so a BYTE shifted left by
    // nine is zero, and its count is taken modulo that width.
    same("SHL(a, n)", |ty, [a, _, _, n]| {
        ty.operate(a << n.rem_euclid(ty.operation_bits().into()))
    }),
    same("SHR(a, n)", |ty, [a, _, _, n]| {
        a >> n.rem_euclid(ty.operation_bits().into())
    }),
    same("ROL(a, n)", |ty, [a, _, _, n]| rotate_left(ty, a, n)),
    same("ROR(a, n)", |ty, [a, _, _, n]| rotate_left(ty, a, -n)),
];

/// `value` rotated left by `count` within the storage width of `ty`.
fn rotate_left(ty: Scalar, value: i128, count: i128) -> i128 {
    let count = count.rem_euclid(ty.bits().into());
    wrap(value << count, ty.bits(), false) | (value >> (i128::from(ty.bits()) - count))
}

/// The expressions a type takes.
fn operations(ty: Scalar) -> Vec<Operation> {
    let arithmetic = INTEGERS.contains(&ty).then_some(&INTEGER_ARITHMETIC[..]);
    let logic = BIT_STRINGS.contains(&ty).then_some(&BIT_STRING_LOGIC[..]);
    [arithmetic.or(logic).unwrap_or_default(), &COMPARISONS].concat()
}

/// The inputs of one scan.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Inputs {
    a: Num,
    b: Num,
    c: Num,
    n: i128,
}

/// The program that evaluates every operation of `ty` into `r0`, `r1`, ...
fn program(ty: Scalar, operations: &[Operation]) -> String {
    let results: String = operations
        .iter()
        .enumerate()
        .map(|(i, operation)| match operation.oracle {
            Oracle::Same(_) => format!("r{i} : {}; ", ty.name),
            Oracle::Compare(_) => format!("r{i} : BOOL; "),
        })
        .collect();
    let body: String = operations
        .iter()
        .enumerate()
        .map(|(i, operation)| format!("  r{i} := {};\n", operation.expr))
        .collect();
    format!(
        "PROGRAM main\n  VAR a : {0}; b : {0}; c : {0}; n : INT; {results}END_VAR\n{body}END_PROGRAM\n",
        ty.name
    )
}

/// What the oracle says `operation` gives for `inputs`.
fn expected(ty: Scalar, operation: Operation, inputs: Inputs) -> Value {
    let Inputs { a, b, c, n } = inputs;
    match operation.oracle {
        Oracle::Same(oracle) => {
            let result = oracle(ty, [a.trunc(), b.trunc(), c.trunc(), n]);
            ty.value(Num::Int(ty.store(result)))
        }
        Oracle::Compare(oracle) => Value::Bool(oracle(a.partial_cmp(&b))),
    }
}

/// Runs every operation of `ty` once for each of `inputs`, and returns the
/// results that differ from what the oracle says.
fn mismatches(ty: Scalar, inputs: &[Inputs]) -> Vec<Mismatch> {
    let operations = operations(ty);
    let mut mismatches = Vec::new();
    run_scans(&program(ty, &operations), &scalar::options(), |session| {
        for (scan, input) in inputs.iter().enumerate() {
            session.write("a", ty.value(input.a));
            session.write("b", ty.value(input.b));
            session.write("c", ty.value(input.c));
            session.write("n", INT.value(Num::Int(input.n)));
            session.scan(scan as u64).unwrap();
            for (i, operation) in operations.iter().enumerate() {
                mismatches.extend(Mismatch::of(
                    || format!("{} of {} with {input:?}", operation.expr, ty.name),
                    session.read(&format!("r{i}")),
                    expected(ty, *operation, *input),
                ));
            }
        }
    });
    mismatches
}

/// Every pairing of the edge values of `ty` as `a` and `b`, with each shift
/// count that sits at a width. `c` follows `b`, or is one where `b` is zero.
fn inputs_at_edges(ty: Scalar) -> Vec<Inputs> {
    let edges = ty.edges();
    let counts = [0, 1, 7, 8, 9, 31, 32, 33, 63, 64, 65, -1];
    edges
        .iter()
        .flat_map(|&a| edges.iter().map(move |&b| (a, b)))
        .zip(counts.iter().cycle())
        .map(|((a, b), &n)| {
            let c = if b == Num::Int(0) { Num::Int(1) } else { b };
            Inputs { a, b, c, n }
        })
        .collect()
}

/// Any inputs for `ty`.
fn any_inputs(ty: Scalar) -> impl Strategy<Value = Inputs> {
    let count = prop_oneof![-70i128..=70, any::<i16>().prop_map(i128::from)];
    (ty.any(), ty.any(), ty.any_nonzero(), count).prop_map(|(a, b, c, n)| Inputs { a, b, c, n })
}

/// Any type, with any inputs for it.
fn any_type_and_inputs() -> impl Strategy<Value = (Scalar, Inputs)> {
    select(scalars()).prop_flat_map(|ty| (Just(ty), any_inputs(ty)))
}

#[rstest]
fn end_to_end_when_integer_operands_at_ends_of_range_then_wraps_at_operation_width(
    #[values(SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT)] ty: Scalar,
) {
    assert_eq!(mismatches(ty, &inputs_at_edges(ty)), []);
}

#[rstest]
fn end_to_end_when_bit_string_operands_at_ends_of_range_then_shifts_at_operation_width(
    #[values(BYTE, WORD, DWORD, LWORD)] ty: Scalar,
) {
    assert_eq!(mismatches(ty, &inputs_at_edges(ty)), []);
}

#[rstest]
fn end_to_end_when_other_operands_at_ends_of_range_then_compares_values(
    #[values(BOOL, REAL, LREAL, TIME, DATE, TOD, DT, LTIME, LDATE, LTOD, LDT)] ty: Scalar,
) {
    assert_eq!(mismatches(ty, &inputs_at_edges(ty)), []);
}

proptest! {
    // Each case compiles a program, and the anchors above already run every
    // type at the ends of its range, so a quarter of the default cases is
    // enough.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn end_to_end_when_operands_arbitrary_then_matches_oracle(
        (ty, inputs) in any_type_and_inputs(),
    ) {
        prop_assert_eq!(mismatches(ty, &[inputs]), []);
    }
}
