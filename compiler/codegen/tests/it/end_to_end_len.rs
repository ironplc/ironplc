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
