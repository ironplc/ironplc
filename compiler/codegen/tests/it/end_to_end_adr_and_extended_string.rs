//! End-to-end integration tests for the `ADR` address operator and
//! `LREAL_TO_FMTSTR` (Beckhoff Tc2_Utilities). Both parse and analyze
//! fine (registered as stdlib functions); codegen does not yet implement
//! their real runtime semantics. See
//! specs/plans/2026-07-26-twincat-stdlib-bool-fmtstr-adr.md.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;

fn adr_options() -> CompilerOptions {
    CompilerOptions {
        allow_address_operator: true,
        ..CompilerOptions::default()
    }
}

fn extended_string_options() -> CompilerOptions {
    CompilerOptions {
        allow_extended_string_functions: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_adr_called_then_returns_not_implemented() {
    let source = "
PROGRAM main
  VAR
    sContent : STRING;
    addr : DWORD;
  END_VAR
  addr := ADR(sContent);
END_PROGRAM
";
    let result = try_parse_and_compile(source, &adr_options());

    assert!(result.is_err(), "expected compilation to fail for ADR");
    assert_eq!(result.unwrap_err().code, "P9999");
}

#[test]
fn end_to_end_when_lreal_to_fmtstr_called_then_returns_not_implemented() {
    let source = "
PROGRAM main
  VAR
    tempM1 : LREAL;
    fmtStr : STRING;
  END_VAR
  fmtStr := LREAL_TO_FMTSTR(tempM1, 2, TRUE);
END_PROGRAM
";
    let result = try_parse_and_compile(source, &extended_string_options());

    assert!(
        result.is_err(),
        "expected compilation to fail for LREAL_TO_FMTSTR"
    );
    assert_eq!(result.unwrap_err().code, "P9999");
}
