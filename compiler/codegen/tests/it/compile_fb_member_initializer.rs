//! A function-block instance declared with a member initializer,
//! `inst : FB := (x := 1)`, is recognized as an instance but the member
//! values are not applied yet, so codegen says so (P9999) instead of
//! allocating the instance and dropping them.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;

#[test]
fn compile_when_fb_instance_has_member_initializer_then_p9999() {
    let source = "
FUNCTION_BLOCK FB_Counter
VAR
    limit : INT := 10;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Counter := (limit := 20);
END_VAR
    inst();
END_PROGRAM";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(result.is_err(), "expected the compile to be rejected");
    let diagnostic = result.unwrap_err();
    assert_eq!("P9999", diagnostic.code);
    assert!(diagnostic.primary.message.contains("member initializer"));
}
