//! `S=` / `R=` set/reset assignment operators are parsed and analyzed like
//! any other BOOL assignment, but codegen has no lowering for "write only
//! when true, otherwise leave unchanged" yet -- it refuses explicitly
//! rather than emit the unconditional store a naive fallthrough would
//! produce. Mirrors `compile_this_super.rs`'s
//! `compile_when_self_ref_then_not_implemented`.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

#[rstest]
#[case::set_bind("    bOut S= bCondition;")]
#[case::reset_bind("    bOut R= bCondition;")]
fn compile_when_set_or_reset_bind_then_not_implemented(#[case] body: &str) {
    let source = format!(
        "
PROGRAM main
VAR
    bOut : BOOL;
    bCondition : BOOL;
END_VAR
{body}
END_PROGRAM
"
    );
    let result = try_parse_and_compile(&source, &CompilerOptions::default());

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "P9999");
}
