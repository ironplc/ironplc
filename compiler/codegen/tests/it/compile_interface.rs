//! A variable of an interface type has no runtime representation yet
//! (ADR-0041 Phase 2), so codegen refuses the program rather than
//! compiling it with the variable left out.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;

#[test]
fn compile_when_interface_variable_then_not_implemented() {
    let source = "
INTERFACE I_Comm
END_INTERFACE

FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm
END_FUNCTION_BLOCK

PROGRAM main
VAR
    comm : I_Comm;
    serial : FB_Serial;
END_VAR
    comm := serial;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(source, &options);

    assert_eq!(result.unwrap_err().code, "P9999");
}
