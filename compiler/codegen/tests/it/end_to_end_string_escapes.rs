//! End-to-end tests for `$` escapes in character string literals: the
//! program stores and measures the characters a literal denotes, not its
//! source spelling (#1836). See `specs/design/string-literals.md`.

use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_run, read_string, string_offset};

// s is at variable slot 0, n is at variable slot 1.
e2e_i32!(
    end_to_end_when_len_of_escaped_literal_then_counts_decoded_characters,
    "
PROGRAM main
  VAR
    s : STRING;
    n : INT;
  END_VAR
  s := '$41$$';
  n := LEN(s);
END_PROGRAM
",
    &[(1, 2)],
);

e2e_i32!(
    end_to_end_when_len_of_named_escapes_then_one_character_each,
    "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('a$Lb$Tc$Rd$Pe$Nf$'');
END_PROGRAM
",
    &[(0, 12)],
);

e2e_i32!(
    end_to_end_when_len_of_wide_hex_escape_then_one_character_each,
    "
PROGRAM main
  VAR
    w : WSTRING;
    n : INT;
  END_VAR
  w := \"$00E9t$00E9\";
  n := LEN(w);
END_PROGRAM
",
    &[(1, 3)],
);

#[test]
fn end_to_end_when_escaped_literal_assigned_then_variable_holds_decoded_characters() {
    let source = "
PROGRAM main
  VAR
    s : STRING;
    t : STRING := 'it$'s $$5';
  END_VAR
  s := '$41$$';
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(read_string(&bufs.data_region, string_offset(&[])), "A$");
    assert_eq!(
        read_string(&bufs.data_region, string_offset(&[254])),
        "it's $5"
    );
}
