//! `RETURN(condition)` conditional return (`allow_conditional_return`).

use super::common::*;

/// The parser stores `RETURN(cond)` as `IF cond THEN RETURN; END_IF`, so the
/// renderer writes that standard spelling. The round trip proves it re-parses
/// to the same AST; the second parse pins that the rendering no longer needs
/// the flag.
#[test]
fn write_to_string_when_conditional_return_then_renders_standard_if() {
    let source = "
FUNCTION_BLOCK FB
VAR
    b : BOOL;
    x : INT;
END_VAR
RETURN(b);
RETURN(x > 5);
x := 1;
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_conditional_return: true,
        ..CompilerOptions::default()
    };
    let rendered = assert_round_trips(source, &options);

    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .unwrap_or_else(|e| panic!("Rendering needs the flag: {e:?}\n{rendered}"));
}
