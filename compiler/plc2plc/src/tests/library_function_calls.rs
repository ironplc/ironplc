//! Round-tripping of user source that calls compatibility-library functions.
//!
//! A call to a library function (e.g. `Tc2_Math`'s `LTRUNC`) is ordinary
//! call syntax in the user's source — activation is out of band, so the
//! renderer must emit the source unchanged with nothing injected
//! (`REQ-CL-plc2plc-001` covers the never-render-library-declarations side).

use super::common::*;

#[test]
fn write_to_string_when_calls_ltrunc_then_round_trips_byte_identical() {
    let source = "
PROGRAM main
VAR
    x : LREAL;
    result : LREAL;
END_VAR
result := LTRUNC(x);
END_PROGRAM
";
    let options = CompilerOptions::default();
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let first = write_to_string(&library_original).unwrap();

    // The rendered text parses back to the identical AST and re-renders
    // byte-identically — the library-bound call is plain call syntax with
    // no marker, qualifier, or rewrite.
    assert!(first.contains("LTRUNC ( x )"));
    let library_rendered = parse_program(&first, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
    let second = write_to_string(&library_rendered).unwrap();
    assert_eq!(first, second);
}
