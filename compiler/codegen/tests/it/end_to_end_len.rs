//! End-to-end integration tests for the LEN standard function.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;
use proptest::prelude::*;

// s is at variable slot 0, n is at variable slot 1.
e2e_i32!(
    end_to_end_when_len_of_string_with_value_then_returns_length,
    "
PROGRAM main
  VAR
    s : STRING := 'hello';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
    &[(1, 5)],
);

e2e_i32!(
    end_to_end_when_len_of_empty_string_then_returns_zero,
    "
PROGRAM main
  VAR
    s : STRING;
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
    &[(1, 0)],
);

// Current length is 2 ('hi'), not the max length of 10.
e2e_i32!(
    end_to_end_when_len_of_string_with_max_length_then_returns_current_length,
    "
PROGRAM main
  VAR
    s : STRING[10] := 'hi';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
    &[(1, 2)],
);

e2e_i32!(
    end_to_end_when_len_of_single_char_string_then_returns_one,
    "
PROGRAM main
  VAR
    s : STRING := 'x';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
    &[(1, 1)],
);

// n is at variable slot 0. LEN accepts a literal argument directly; this is
// the example published in the LEN reference documentation.
e2e_i32!(
    end_to_end_when_len_of_string_literal_then_returns_length,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('Hello');
END_PROGRAM
",
    &[(0, 5)],
);

e2e_i32!(
    end_to_end_when_len_of_empty_string_literal_then_returns_zero,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('');
END_PROGRAM
",
    &[(0, 0)],
);

// LEN of a WSTRING literal counts code units, not bytes.
e2e_i32!(
    end_to_end_when_len_of_wstring_literal_then_returns_code_unit_count,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(\"Hello\");
END_PROGRAM
",
    &[(0, 5)],
);

// Non-ASCII BMP code points are one code unit each in UTF-16LE.
e2e_i32!(
    end_to_end_when_len_of_non_ascii_wstring_literal_then_counts_code_units,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(\"é€\");
END_PROGRAM
",
    &[(0, 2)],
);

// s is at slot 0, n at slot 1. A nested call is resolved into a temporary,
// so LEN(MID(...)) does not need the intermediate hoisted into a variable.
e2e_i32!(
    end_to_end_when_len_of_nested_string_call_then_returns_length,
    "
PROGRAM main
  VAR
    s : STRING[32] := 'hello world';
    n : INT;
  END_VAR
  n := LEN(MID(s, 3, 1));
END_PROGRAM
",
    &[(1, 3)],
);

// ws is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_nested_wstring_call_then_returns_length,
    "
PROGRAM main
  VAR
    ws : WSTRING[32] := \"hello world\";
    n : INT;
  END_VAR
  n := LEN(MID(ws, 3, 1));
END_PROGRAM
",
    &[(1, 3)],
);

e2e_i32!(
    end_to_end_when_len_of_concat_of_literals_then_returns_total_length,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(CONCAT('ab', 'cde'));
END_PROGRAM
",
    &[(0, 5)],
);

e2e_i32!(
    end_to_end_when_len_of_concat_of_wstring_literals_then_returns_total_length,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(CONCAT(\"ab\", \"cde\"));
END_PROGRAM
",
    &[(0, 5)],
);

// --- LEN of a string reached through an aggregate --------------------------
//
// Neither an array element nor a structure field owns a data-region slot of
// its own that `LEN_STR` could name: an array's variable slot holds the base
// offset of its element region, and a field lives inside its structure's
// region. Both are copied into a temporary whose header LEN then reads, so
// what these pin is that the copy carries the element's own length --
// including when the element is declared wider than a bare STRING
// (github.com/ironplc/ironplc/issues/1485).

// x is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_string_array_element_then_returns_element_length,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF STRING[8] := ['abc', 'wxyz'];
    n : INT;
  END_VAR
  n := LEN(x[2]);
END_PROGRAM
",
    &[(1, 4)],
);

// A subscript the compiler cannot fold: the element offset is computed at run
// time, which is the shape the fixed `data_offset` operand of `LEN_STR` could
// never have expressed. x is at slot 0, i at slot 1, n at slot 2.
e2e_i32!(
    end_to_end_when_len_of_string_array_element_at_variable_subscript_then_returns_element_length,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF STRING[8] := ['abc', 'wxyz'];
    i : INT := 2;
    n : INT;
  END_VAR
  n := LEN(x[i]);
END_PROGRAM
",
    &[(2, 4)],
);

// LEN counts code units, so a wide element answers in characters and not in
// the bytes it occupies. x is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_wstring_array_element_then_returns_code_unit_count,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF WSTRING[8] := [\"ab\", \"cdé\"];
    n : INT;
  END_VAR
  n := LEN(x[2]);
END_PROGRAM
",
    &[(1, 3)],
);

// r is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_string_struct_field_then_returns_field_length,
    "
TYPE
  text : STRUCT
    s : STRING[8];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : text;
    n : INT;
  END_VAR
  r.s := 'hello';
  n := LEN(r.s);
END_PROGRAM
",
    &[(1, 5)],
);

// r is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_wstring_struct_field_then_returns_code_unit_count,
    "
TYPE
  text : STRUCT
    w : WSTRING[8];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : text;
    n : INT;
  END_VAR
  r.w := \"hé\";
  n := LEN(r.w);
END_PROGRAM
",
    &[(1, 2)],
);

// Both aggregates at once: an array of strings reached through a structure
// field, subscripted at run time. A structure holding an array takes two
// variable slots, so i is at slot 2 and n at slot 3.
e2e_i32!(
    end_to_end_when_len_of_string_array_element_in_struct_field_then_returns_element_length,
    "
TYPE
  text : STRUCT
    names : ARRAY[1..2] OF STRING[8];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : text;
    i : INT := 2;
    n : INT;
  END_VAR
  r.names[2] := 'abcd';
  n := LEN(r.names[i]);
END_PROGRAM
",
    &[(3, 4)],
);

// --- LEN of a string that cannot change -----------------------------------
//
// These compile to a constant (see compile_len.rs); what they pin is that the
// constant is the value LEN_STR would have read at run time.

// `$'` is one quote and `$$` one dollar sign: the length counts what the
// literal denotes, not how it is spelled. n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_literal_with_escaped_quote_and_dollar_then_counts_characters,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('it$'s $$5');
END_PROGRAM
",
    &[(0, 7)],
);

// `$N` is a newline and `$41` the character 'A'. n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_literal_with_newline_and_hex_escapes_then_counts_characters,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('a$N$41');
END_PROGRAM
",
    &[(0, 3)],
);

// A WSTRING escape names a four-digit code unit. n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_wstring_literal_with_escapes_then_counts_code_units,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(\"$0041$\"é\");
END_PROGRAM
",
    &[(0, 3)],
);

// The folded length takes part in the surrounding expression. n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_literal_in_arithmetic_then_uses_length,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('abc') * 2 + 1;
END_PROGRAM
",
    &[(0, 7)],
);

// d is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_literal_assigned_to_dint_then_returns_length,
    "
PROGRAM main
  VAR
    d : DINT;
  END_VAR
  d := LEN('Hello');
END_PROGRAM
",
    &[(0, 5)],
);

// l is at slot 0.
e2e_i64!(
    end_to_end_when_len_of_literal_assigned_to_lint_then_returns_length,
    "
PROGRAM main
  VAR
    l : LINT;
  END_VAR
  l := LEN('Hello');
END_PROGRAM
",
    &[(0, 5)],
);

// n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_nested_concat_of_literals_then_returns_total_length,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(CONCAT(CONCAT('a', 'bc'), 'def'));
END_PROGRAM
",
    &[(0, 6)],
);

// msg is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_declared_constant_string_then_returns_length,
    "
PROGRAM main
  VAR CONSTANT
    msg : STRING := 'Hello';
  END_VAR
  VAR
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    &[(1, 5)],
);

// msg is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_declared_constant_wstring_then_returns_code_unit_count,
    "
PROGRAM main
  VAR CONSTANT
    msg : WSTRING := \"héllo\";
  END_VAR
  VAR
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    &[(1, 5)],
);

// An initial value longer than the declaration holds is truncated when it is
// stored, so the constant's length is the declared capacity. msg is at slot
// 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_constant_string_with_longer_initial_value_then_returns_capacity,
    "
PROGRAM main
  VAR CONSTANT
    msg : STRING[3] := 'Hello';
  END_VAR
  VAR
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    &[(1, 3)],
);

// A string the program never writes is marked CONSTANT by the analyzer.
// msg is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_never_written_string_then_returns_length,
    "
PROGRAM main
  VAR
    msg : STRING := 'Hello';
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    &[(1, 5)],
);

// msg is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_concat_of_constant_and_literal_then_returns_total_length,
    "
PROGRAM main
  VAR CONSTANT
    msg : STRING[20] := 'Hello';
  END_VAR
  VAR
    n : INT;
  END_VAR
  n := LEN(CONCAT(msg, ', world'));
END_PROGRAM
",
    &[(1, 12)],
);

// A written variable is not constant: LEN reads its current value. msg is
// at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_written_string_then_returns_current_length,
    "
PROGRAM main
  VAR
    msg : STRING := 'Hello';
    n : INT;
  END_VAR
  msg := 'Hi';
  n := LEN(msg);
END_PROGRAM
",
    &[(1, 2)],
);

// n is at slot 0.
e2e_i32!(
    end_to_end_when_len_of_constant_string_in_function_then_returns_length,
    "
FUNCTION f : INT
  VAR CONSTANT
    s : STRING := 'abcd';
  END_VAR
  f := LEN(s);
END_FUNCTION

PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := f();
END_PROGRAM
",
    &[(0, 4)],
);

// The field's length is its declared initial value's, even though a
// function block's string fields are not yet initialized at run time
// (github.com/ironplc/ironplc/pull/1865), where LEN_STR read 0.
// inst is at slot 0, n at slot 1.
e2e_i32!(
    end_to_end_when_len_of_constant_string_in_function_block_then_returns_length,
    "
FUNCTION_BLOCK fb
  VAR_OUTPUT
    len_out : INT;
  END_VAR
  VAR CONSTANT
    s : STRING := 'abcde';
  END_VAR
  len_out := LEN(s);
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    inst : fb;
    n : INT;
  END_VAR
  inst();
  n := inst.len_out;
END_PROGRAM
",
    &[(1, 5)],
);

/// Compiles and runs `source`, returning the i32 in variable slot `slot`.
fn len_from(source: &str, slot: usize) -> i32 {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    bufs.vars[slot].as_i32()
}

/// A 300-code-unit value -- more than the 254 a bare STRING declaration holds.
/// Stored in an element declared STRING[400], it distinguishes a temporary
/// sized at the operand's own capacity from one sized at the default, which
/// truncates the copy and leaves LEN answering 254.
fn long_value() -> String {
    "a".repeat(300)
}

// x is at slot 0, n at slot 1.
#[test]
fn end_to_end_when_len_of_long_string_array_element_then_returns_untruncated_length() {
    let source = format!(
        "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF STRING[400];
    n : INT;
  END_VAR
  x[1] := '{value}';
  n := LEN(x[1]);
END_PROGRAM
",
        value = long_value(),
    );

    assert_eq!(len_from(&source, 1), 300);
}

// x is at slot 0, n at slot 1.
#[test]
fn end_to_end_when_len_of_long_wstring_array_element_then_returns_untruncated_length() {
    let source = format!(
        "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF WSTRING[400];
    n : INT;
  END_VAR
  x[1] := \"{value}\";
  n := LEN(x[1]);
END_PROGRAM
",
        value = long_value(),
    );

    assert_eq!(len_from(&source, 1), 300);
}

// r is at slot 0, n at slot 1.
#[test]
fn end_to_end_when_len_of_long_string_struct_field_then_returns_untruncated_length() {
    let source = format!(
        "
TYPE
  text : STRUCT
    s : STRING[400];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : text;
    n : INT;
  END_VAR
  r.s := '{value}';
  n := LEN(r.s);
END_PROGRAM
",
        value = long_value(),
    );

    assert_eq!(len_from(&source, 1), 300);
}

// r is at slot 0, n at slot 1.
#[test]
fn end_to_end_when_len_of_long_wstring_struct_field_then_returns_untruncated_length() {
    let source = format!(
        "
TYPE
  text : STRUCT
    w : WSTRING[400];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : text;
    n : INT;
  END_VAR
  r.w := \"{value}\";
  n := LEN(r.w);
END_PROGRAM
",
        value = long_value(),
    );

    assert_eq!(len_from(&source, 1), 300);
}

/// Generates printable ASCII strings safe for IEC 61131-3 string literals.
/// Excludes single quote (0x27) and dollar sign (0x24, the escape character).
fn safe_string_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        (0x20u8..=0x7Eu8).prop_filter("exclude quote and dollar", |&b| b != b'\'' && b != b'$'),
        0..=254,
    )
    .prop_map(|bytes| bytes.into_iter().map(|b| b as char).collect())
}

proptest! {
    #[test]
    fn end_to_end_when_len_of_arbitrary_string_then_returns_correct_length(
        s in safe_string_strategy()
    ) {
        let expected_len = s.len() as i32;
        let source = format!(
            "
PROGRAM main
  VAR
    s : STRING := '{}';
    n : INT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
            s
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());

        prop_assert_eq!(bufs.vars[1].as_i32(), expected_len);
    }
}
