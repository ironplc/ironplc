//! End-to-end tests that codegen runs the callee the analyzer resolved a
//! call to: a function block instance runs the block its type is, and a
//! method call runs the method its receiver's block declares or inherits.
//! See `specs/design/call-binding.md`.

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

use crate::common::{assert_run_with, try_parse_and_compile};

/// Methods and `EXTENDS` are an extension.
fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[spec_test(REQ_CB_codegen_001)]
fn end_to_end_when_user_block_named_like_standard_then_runs_user_block() {
    let source = "
FUNCTION_BLOCK CTU_INT
VAR_INPUT CU : BOOL; END_VAR
VAR_OUTPUT CV : INT; END_VAR
CV := 42;
END_FUNCTION_BLOCK

PROGRAM main
VAR c : CTU_INT; cv : INT; END_VAR
c(CU := TRUE);
cv := c.CV;
END_PROGRAM
";
    assert_run_with(source, &CompilerOptions::default(), &[("cv", 42i64)]);
}

#[spec_test(REQ_CB_codegen_002)]
fn end_to_end_when_standard_block_instance_then_runs_its_vm_block() {
    let source = "
PROGRAM main
VAR c : CTU; cl : CTU_LINT; cv : INT; cvl : LINT; END_VAR
c(CU := TRUE, PV := 5);
cl(CU := TRUE, PV := 5);
cv := c.CV;
cvl := cl.CV;
END_PROGRAM
";
    assert_run_with(
        source,
        &CompilerOptions::default(),
        &[("cv", 1i64), ("cvl", 1i64)],
    );
}

#[spec_test(REQ_CB_codegen_003)]
fn end_to_end_when_method_inherited_from_base_without_fields_then_runs_base_method() {
    let source = "
FUNCTION_BLOCK base
METHOD Get : DINT
Get := 7;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; r : DINT; END_VAR
r := d.Get();
END_PROGRAM
";
    assert_run_with(source, &options(), &[("r", 7i64)]);
}

#[spec_test(REQ_CB_codegen_004)]
fn end_to_end_when_method_overridden_then_each_instance_runs_its_own_block_method() {
    let source = "
FUNCTION_BLOCK base
VAR k : DINT := 7; END_VAR
METHOD Get : DINT
Get := k;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
METHOD Get : DINT
Get := 9;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; b : base; rd : DINT; rb : DINT; END_VAR
rd := d.Get();
rb := b.Get();
END_PROGRAM
";
    assert_run_with(source, &options(), &[("rd", 9i64), ("rb", 7i64)]);
}

#[spec_test(REQ_CB_codegen_005)]
fn compile_when_method_inherited_from_base_with_fields_then_not_implemented() {
    let source = "
FUNCTION_BLOCK base
VAR k : DINT := 7; END_VAR
METHOD Get : DINT
Get := k;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; r : DINT; END_VAR
r := d.Get();
END_PROGRAM
";
    let diagnostic = try_parse_and_compile(source, &options()).unwrap_err();

    assert_eq!(diagnostic.code, "P9999");
    let start = source.find("d.Get()").unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + "d.Get()".len());
}
