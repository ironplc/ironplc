//! Tests of lowering calls of methods.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::{lower, lower_bodies};

/// A block with a method that has no result, one with a result, and one
/// with outputs.
const BLOCK: &str = "
FUNCTION_BLOCK Counter
VAR n : DINT; END_VAR
METHOD Add
  VAR_INPUT k : DINT; END_VAR
  n := n + k;
END_METHOD
METHOD Get : INT
  VAR_INPUT scale : DINT; END_VAR
  Get := DINT_TO_INT(n * scale);
END_METHOD
METHOD Split : DINT
  VAR_INPUT k : DINT; END_VAR
  VAR_OUTPUT high : DINT; done : BOOL; END_VAR
  high := n / k;
  done := high > 0;
  Split := n MOD k;
END_METHOD
END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-076: a call of a method whose result is not used is a
/// `Call` statement whose first argument is a `Ref` to the receiver, then
/// its inputs in declaration order. A call whose result is used is a `Call`
/// expression of the result's type.
#[test]
fn lower_statement_when_method_called_then_call_with_receiver_first() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             PROGRAM main VAR c : Counter; x : DINT; r : DINT; END_VAR
               c.Add(k := x);
               r := c.Get(scale := 2) + 1;
               c.Get(scale := 3);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "call pou2 (ref v0^.f0) (read i32s v0^.f1)",
            "assign v0^.f2 (add i32s (call pou3 i32s (ref v0^.f0) (const i32s 2)) \
             (const i32s 1))",
            "call pou3 (ref v0^.f0) (const i32s 3)",
        ]
    );
}

/// REQ-LOW-lowering-092, REQ-LOW-lowering-172: a method's outputs are
/// passed as references to temporaries, and assigned from them after the
/// call: after a `Call` statement, or as the stores of an in-place call.
#[test]
fn lower_statement_when_method_with_outputs_called_then_outputs_after_call() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             PROGRAM main VAR c : Counter; x : DINT; b : BOOL; r : LINT; END_VAR
               c.Split(k := 2, high => x);
               r := c.Split(k := 3, NOT done => b);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "call pou2 (ref v0^.f0) (const i32s 2) (ref v6) (ref v7)\n\
             assign v0^.f1 (read i32s v6)",
            "assign v0^.f3 (convert i64s (call-in-place pou2 i32s (ref v0^.f0) (const i32s 3) \
             (ref v8) (ref v9) {assign v0^.f2 (bool_not bool (read bool v9))}))",
        ]
    );
}

/// A method's body reaches its block's fields through its instance
/// parameter, and its own parameters, locals and result as variables of its
/// own; an output through the reference the call passes for it. The call
/// and the body agree on the variables.
#[test]
fn lower_statement_when_method_body_then_fields_through_instance() {
    let outcome = lower_bodies(
        &format!(
            "{BLOCK}
             PROGRAM main VAR c : Counter; x : DINT; END_VAR
               x := c.Split(k := 2, high => x);
             END_PROGRAM"
        ),
        &["main", "Counter.Split"],
    );
    assert_eq!(
        &outcome.statements[1..],
        [
            "assign v3^ (div i32s (read i32s v1^.f0) (read i32s v2))",
            "assign v4^ (gt (read i32s v3^) (const i32s 0))",
            "assign v5 (mod i32s (read i32s v1^.f0) (read i32s v2))",
        ]
    );
}

/// A call of a method the receiver's block inherits from a block that
/// declares fields, whose type the receiver's does not extend in the
/// lowered program yet (REQ-LOW-lowering-177), is not lowered yet.
#[test]
fn lower_statement_when_inherited_method_then_unsupported() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             FUNCTION_BLOCK Derived EXTENDS Counter
             END_FUNCTION_BLOCK
             PROGRAM main VAR d : Derived; END_VAR
               d.Add(k := 1);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}

/// REQ-LOW-lowering-072, REQ-LOW-lowering-076: a method's string input is
/// passed a `Str` of its encoding after the receiver, in a call whose result
/// is used and in a call statement. The method's body reads the variable.
#[test]
fn lower_statement_when_method_has_string_input_then_str_after_receiver() {
    let outcome = lower_bodies(
        "FUNCTION_BLOCK Named
           METHOD Len2 : DINT
             VAR_INPUT v : STRING[4]; w : WSTRING[8]; END_VAR
             Len2 := LEN(v) * 10 + LEN(w);
           END_METHOD
           METHOD Set
             VAR_INPUT v : STRING; END_VAR
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR f : Named; t : STRING := 'ab'; r : DINT; END_VAR
           r := f.Len2(v := 'hello', w := \"pq\");
           f.Set(v := t);
         END_PROGRAM",
        &["main", "Named.Len2"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call pou2 i32s (ref v0^.f0) (literal string[5] \"hello\") \
             (literal wstring[2] \"pq\"))",
            "call pou3 (ref v0^.f0) (read string[254] v0^.f1)",
            "assign v4 (add i32s (mul i32s (call len_string i32s (read string[4] v2)) \
             (const i32s 10)) (call len_wstring i32s (read wstring[8] v3)))",
        ]
    );
}

/// A block with a method of a result and no fields, a block that extends
/// it with a field of its own, one that extends it with none, one that
/// extends that, and one that extends a block with a field.
const INHERITED: &str = "
FUNCTION_BLOCK Base
METHOD Get : DINT
  VAR_INPUT k : DINT; END_VAR
  Get := k;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base VAR n : DINT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Middle EXTENDS Base END_FUNCTION_BLOCK
FUNCTION_BLOCK Leaf EXTENDS Middle END_FUNCTION_BLOCK
FUNCTION_BLOCK Under EXTENDS Derived END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-177: a call of a method a block inherits from a block
/// that declares no fields is a call of that block's method with a `Ref` to
/// the receiver first, whatever fields the receiver's own block declares,
/// and through a block in between that declares none.
#[test]
fn lower_statement_when_method_inherited_from_base_without_fields_then_call_of_base_method() {
    let outcome = lower(
        &format!(
            "{INHERITED}
             PROGRAM main VAR d : Derived; l : Leaf; r : DINT; END_VAR
               r := d.Get(k := 1);
               r := l.Get(k := 2);
               d.Get(k := 3);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call pou5 i32s (ref v0^.f0) (const i32s 1))",
            "assign v0^.f2 (call pou5 i32s (ref v0^.f1) (const i32s 2))",
            "call pou5 (ref v0^.f0) (const i32s 3)",
        ]
    );
}

/// REQ-LOW-lowering-177: a block whose type does not begin with the fields
/// of the block it extends, as one that extends a block with a field does
/// while inherited fields are not in its type, records as its base the
/// nearest block above that declares none. So a call of a method it
/// inherits from that block, through the block with the field, is a call
/// of that block's method.
#[test]
fn lower_statement_when_method_inherited_through_block_with_fields_then_call_of_base_method() {
    let outcome = lower(
        &format!(
            "{INHERITED}
             PROGRAM main VAR u : Under; r : DINT; END_VAR
               r := u.Get(k := 1);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f1 (call pou4 i32s (ref v0^.f0) (const i32s 1))"]
    );
}

/// REQ-LOW-lowering-187: a method whose result is a structure or an array
/// is declared without a result, so a call of it, which only a statement
/// can make, is a `Call` statement, followed by the stores of its outputs.
#[test]
fn lower_statement_when_method_result_is_aggregate_then_call_statement() {
    let outcome = lower(
        "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE
         FUNCTION_BLOCK BP VAR n : DINT; END_VAR
           METHOD MKP : P VAR_INPUT v : DINT; END_VAR n := n + v; END_METHOD
           METHOD MKA : A3 VAR_INPUT v : DINT; END_VAR VAR_OUTPUT k : DINT; END_VAR
             k := n;
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR b : BP; r : DINT; END_VAR
           b.MKP(v := 3);
           b.MKA(v := 2, k => r);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "call pou2 (ref v0^.f0) (const i32s 3)",
            "call pou3 (ref v0^.f0) (const i32s 2) (ref v8)\nassign v0^.f1 (read i32s v8)",
        ]
    );
}
