//! `THIS^` and `SUPER^` pass analysis but codegen does not compile them yet.
//! Each case is a program `check` accepts. Compiling must refuse it with a
//! diagnostic rather than panic or emit code that reads the wrong slot.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

#[rstest]
#[case::field_write("    THIS^.count := 1;")]
#[case::field_read("    count := THIS^.count;")]
#[case::inherited_field_through_super("    count := SUPER^.level;")]
#[case::method_call("    THIS^.Start();")]
#[case::super_method_call("    SUPER^.Stop();")]
fn compile_when_self_reference_then_diagnostic_not_panic(#[case] body: &str) {
    let source = format!(
        "
FUNCTION_BLOCK FB_Base
VAR
    level : INT;
END_VAR
METHOD Stop
    level := 0;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
VAR
    count : INT;
END_VAR
METHOD Start
    count := 1;
END_METHOD
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
    m.Run();
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };

    let result = try_parse_and_compile(&source, &options);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "P9999");
}
