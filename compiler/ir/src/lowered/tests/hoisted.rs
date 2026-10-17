//! Tests of the order in which an assignment reads the results of the calls
//! with outputs lowered ahead of it. The variables `d`, `i` and `s` of the
//! fixture stand for result temporaries.

use super::*;
use crate::lowered::hoisted::leading_reads;

fn add(lhs: Expr, rhs: Expr) -> Expr {
    Expr::binary(BinaryOp::Add, lhs, rhs, span()).unwrap()
}

fn neg(value: Expr) -> Expr {
    Expr::unary(UnaryOp::Neg, value, span()).unwrap()
}

/// The reads of the operands of an operation are its leading reads while
/// each operand is itself a read, and a read of another place, a constant or
/// an operation that finishes first ends the run.
#[test]
fn leading_reads_when_operands_then_reads_before_another_node_finishes() {
    let f = fixture();
    let results = [f.dint, f.int, f.sint];
    let both = add(f.read(f.dint), f.read(f.int));
    let nested = add(add(f.read(f.dint), f.read(f.int)), f.read(f.sint));
    let after_negation = add(neg(f.read(f.dint)), f.read(f.int));
    let after_constant = add(int(1), f.read(f.dint));
    let after_other = add(f.read(f.dint), add(f.read(f.int), f.read(f.sint)));
    assert_eq!(leading_reads(&both, &results), [f.dint, f.int]);
    assert_eq!(leading_reads(&nested, &results), [f.dint, f.int]);
    assert_eq!(leading_reads(&after_negation, &results), [f.dint]);
    assert_eq!(leading_reads(&after_constant, &results), []);
    assert_eq!(
        leading_reads(&after_other, &results),
        [f.dint, f.int, f.sint]
    );
}

/// The right operand of a short circuit runs only on a condition, so it has
/// no leading reads.
#[test]
fn leading_reads_when_short_circuit_then_left_operand_only() {
    let f = fixture();
    let value = Expr::short_circuit(
        ShortCircuitOp::AndThen,
        f.read(f.bool),
        f.read(f.bool),
        span(),
    )
    .unwrap();
    assert_eq!(leading_reads(&value, &[f.bool]), [f.bool]);
}

/// REQ-LOW-lowering-174: two results read in the order their parts made
/// them, and a part that reads the result of the part before it, are read
/// as a stack.
#[test]
fn reads_results_in_order_when_read_as_stack_then_true() {
    let f = fixture();
    let (first, second) = (int(3), int(4));
    let sum = add(f.read(f.dint), f.read(f.int));
    assert!(reads_results_in_order(
        &[(&first, f.dint), (&second, f.int)],
        &sum
    ));
    let inner = neg(f.read(f.dint));
    let outer = f.read(f.int);
    assert!(reads_results_in_order(
        &[(&first, f.dint), (&inner, f.int)],
        &outer
    ));
}

/// REQ-LOW-lowering-174: results read out of the order their parts made
/// them, after another node finishes, twice or not at all are not read as a
/// stack.
#[test]
fn reads_results_in_order_when_not_read_as_stack_then_false() {
    let f = fixture();
    let (first, second) = (int(3), int(4));
    let parts = [(&first, f.dint), (&second, f.int)];
    let reversed = add(f.read(f.int), f.read(f.dint));
    let late = add(add(int(1), f.read(f.dint)), f.read(f.int));
    assert!(!reads_results_in_order(&parts, &reversed));
    assert!(!reads_results_in_order(&parts, &late));
    assert!(!reads_results_in_order(&parts, &f.read(f.int)));
    let twice = add(f.read(f.dint), f.read(f.dint));
    assert!(!reads_results_in_order(&[(&first, f.dint)], &twice));
    assert!(!reads_results_in_order(&[(&first, f.dint)], &int(1)));
}
