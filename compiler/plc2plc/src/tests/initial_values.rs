//! Rendering of the initializers the analyzer completes.
//!
//! The analyzer completes every declaration's initializer with the value the
//! declaration starts with. A rendering shows the program as written, so
//! the parts the analyzer supplied are left out.

use super::common::*;

use spec_test_macro::spec_test;

/// The declaration lines of `main` in the plain rendering of `source` after
/// analysis.
fn analyzed_declarations(source: &str) -> Vec<String> {
    let options = CompilerOptions::default();
    let (library, _) = analyze_and_render_with_types(source, &options);
    let rendered = crate::write_to_string(&library).unwrap();
    rendered
        .lines()
        .skip_while(|line| !line.starts_with("PROGRAM main"))
        .filter(|line| line.contains(" : "))
        .map(|line| line.trim().to_string())
        .collect()
}

#[spec_test(REQ_IV_plc2plc_001)]
fn write_to_string_when_analyzed_then_initializers_render_as_written() {
    let source = "
TYPE
    P : STRUCT x : INT := 10; y : INT := 3; END_STRUCT;
    Color : (Red, Green) := Green;
END_TYPE
PROGRAM main
VAR
    a : INT;
    b : INT := 2;
    p : P := (y := 4);
    q : P;
    arr : ARRAY[1..4] OF INT := [1, 2];
    c : Color;
    s : STRING[5];
END_VAR
END_PROGRAM";

    assert_eq!(
        analyzed_declarations(source),
        vec![
            "a : int;",
            "b : int := 2;",
            "p : P := ( y := 4 );",
            "q : P;",
            "arr : ARRAY [ 1.. 4 ] OF INT := [ 1 , 2 ];",
            "c : Color;",
            "s : STRING [ 5 ];",
        ]
    );
}
