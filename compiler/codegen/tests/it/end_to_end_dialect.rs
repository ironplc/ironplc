//! End-to-end integration tests for the dialect system.
//!
//! These tests exercise the full pipeline: parse → semantic analysis → codegen → VM execution.
//! They verify that the RuSTy dialect allows Edition 3 keywords (LDT, LTIME, etc.)
//! as identifiers while still supporting REF_TO syntax.

use crate::common::{parse, parse_and_run};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;

// In the RuSTy dialect, LDT is demoted to an identifier so it can be
// used as a variable name (as OSCAT libraries do).
// var layout: __SYSTEM_UP_TIME=0, __SYSTEM_UP_LTIME=1, LDT=2, result=3
e2e_i32_with!(
    end_to_end_when_rusty_dialect_then_ldt_usable_as_variable_name,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "
PROGRAM main
VAR
    LDT : DINT := 42;
    result : DINT;
END_VAR
    result := LDT;
END_PROGRAM
",
    &[(3, 42)],
);

// The RuSTy dialect enables REF_TO even though Edition 3 types
// (LTIME, LDT, etc.) are not keywords.
// var layout: __SYSTEM_UP_TIME=0, __SYSTEM_UP_LTIME=1, counter=2, r=3, result=4
e2e_i32_with!(
    end_to_end_when_rusty_dialect_then_ref_to_works,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "
PROGRAM main
VAR
    counter : DINT := 99;
    r : REF_TO DINT := REF(counter);
    result : DINT;
END_VAR
    result := r^;
END_PROGRAM
",
    &[(4, 99)],
);

// Core OSCAT scenario: LDT used as a variable name alongside REF_TO.
// var layout: __SYSTEM_UP_TIME=0, __SYSTEM_UP_LTIME=1, LDT=2, r=3, result=4
e2e_i32_with!(
    end_to_end_when_rusty_dialect_then_ldt_and_ref_to_coexist,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "
PROGRAM main
VAR
    LDT : DINT := 42;
    r : REF_TO DINT := REF(LDT);
    result : DINT;
END_VAR
    result := r^;
END_PROGRAM
",
    &[(4, 42)],
);

#[test]
fn end_to_end_when_rusty_dialect_then_struct_with_ldt_member_parses() {
    // Struct member access is not yet implemented in codegen, but we can
    // verify that parsing and semantic analysis succeed with the RuSTy
    // dialect when a struct uses LDT as a member name.
    let source = "
TYPE MY_STRUCT :
  STRUCT
      LDT : DINT;
      value : REAL;
  END_STRUCT;
END_TYPE
PROGRAM main
VAR
    s : MY_STRUCT;
END_VAR
END_PROGRAM
";
    let (_lib, _ctx) = parse(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // If we get here, parsing and type resolution succeeded.
}

#[test]
fn end_to_end_when_codesys_dialect_then_ldt_is_reserved_keyword() {
    // The CODESYS dialect enables the long-time-type keywords, so LDT is
    // reserved and cannot be used as an ordinary variable name.
    let source = "
PROGRAM main
VAR
    LDT : DINT := 42;
    result : DINT;
END_VAR
    result := LDT;
END_PROGRAM
";
    let result = parse_program(
        source,
        &FileId::default(),
        &CompilerOptions::from_dialect(Dialect::Codesys),
    );
    assert!(
        result.is_err(),
        "LDT should be a reserved keyword under the CODESYS dialect"
    );
}

// The CODESYS dialect enables REF_TO.
// var layout: counter=0, r=1, result=2
e2e_i32_with!(
    end_to_end_when_codesys_dialect_then_ref_to_works,
    CompilerOptions::from_dialect(Dialect::Codesys),
    "
PROGRAM main
VAR
    counter : DINT := 99;
    r : REF_TO DINT := REF(counter);
    result : DINT;
END_VAR
    result := r^;
END_PROGRAM
",
    &[(2, 99)],
);

#[test]
fn end_to_end_when_codesys_dialect_then_sizeof_and_c_style_comments_work() {
    // CODESYS supports SIZEOF() and C-style comments. The program contains a
    // `//` line comment, so it must stay multi-line (collapsing to one line
    // would comment out the rest of the program).
    let source = "
PROGRAM main
VAR
    x : DINT;        // C-style line comment
    result : DINT;
END_VAR
    /* block comment */
    result := SIZEOF(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Codesys));
    // var layout: x=0, result=1
    assert_eq!(bufs.vars[1].as_i32(), 4);
}

#[test]
fn end_to_end_when_twincat_dialect_then_ldt_is_reserved_keyword() {
    // The TwinCAT dialect enables the long-time-type keywords, so LDT is
    // reserved and cannot be used as an ordinary variable name.
    let source = "
PROGRAM main
VAR
    LDT : DINT := 42;
    result : DINT;
END_VAR
    result := LDT;
END_PROGRAM
";
    let result = parse_program(
        source,
        &FileId::default(),
        &CompilerOptions::from_dialect(Dialect::TwinCat),
    );
    assert!(
        result.is_err(),
        "LDT should be a reserved keyword under the TwinCAT dialect"
    );
}

// Curly-brace pragmas ({attribute '...'}) appear in virtually every
// TwinCAT source file. The TwinCAT dialect skips them as opaque trivia so
// the surrounding code compiles and runs unchanged.
// var layout: x=0, result=1
e2e_i32_with!(
    end_to_end_when_twincat_dialect_then_pragmas_are_skipped,
    CompilerOptions::from_dialect(Dialect::TwinCat),
    "
{attribute 'qualified_only'}
PROGRAM main
VAR
    x : DINT := 99;
    result : DINT;
END_VAR
    result := x;
END_PROGRAM
",
    &[(1, 99)],
);

#[test]
fn end_to_end_when_twincat_dialect_then_sizeof_and_c_style_comments_work() {
    // TwinCAT supports SIZEOF() and C-style comments. The program contains a
    // `//` line comment, so it must stay multi-line (collapsing to one line
    // would comment out the rest of the program).
    let source = "
PROGRAM main
VAR
    x : DINT;        // C-style line comment
    result : DINT;
END_VAR
    /* block comment */
    result := SIZEOF(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::TwinCat));
    // var layout: x=0, result=1
    assert_eq!(bufs.vars[1].as_i32(), 4);
}

#[test]
fn end_to_end_when_rusty_dialect_then_oscat_style_struct_with_ldt_member_access() {
    // Full OSCAT scenario: struct with LDT as member name, function that reads
    // the member, and a program that writes and reads through the struct.
    // Struct member access is not yet implemented in codegen, so this test
    // verifies parsing and semantic analysis only.
    let source = "
TYPE MY_STRUCT :
  STRUCT
      LDT : DINT;
      value : REAL;
  END_STRUCT;
END_TYPE
FUNCTION MY_FUNC : DINT
VAR_INPUT
    x : MY_STRUCT;
END_VAR
    MY_FUNC := x.LDT;
END_FUNCTION
PROGRAM main
VAR
    s : MY_STRUCT;
    result : DINT;
END_VAR
    s.LDT := 42;
    result := MY_FUNC(x := s);
END_PROGRAM
";
    let (_lib, _ctx) = parse(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // If we get here, parsing and type resolution succeeded.
}
