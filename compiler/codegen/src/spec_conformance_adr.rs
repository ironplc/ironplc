//! Spec conformance tests for the TwinCAT/CODESYS `ADR()` operator
//! (codegen-owned requirements): end-to-end execution.
//!
//! `ADR` is rewritten to `ExprKind::Ref` in the analyzer, so codegen needs
//! zero changes — these tests pin the end-to-end behavior through the shared
//! reference backend (push the variable's table index; `^` emits
//! `LOAD_INDIRECT`).
//!
//! Each test is annotated with `#[spec_test(REQ_PTR_codegen_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every codegen-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;
use spec_test_macro::spec_test;

/// The plan's Goal example: a function-block instance binding a pointer to
/// one of its own members and reading it back through `^`. The member value
/// is assigned in the body rather than by a `:= 5` member initializer:
/// declared initial values are not yet applied to user FB instance fields (a
/// pre-existing gap unrelated to `ADR`).
const GOAL_EXAMPLE: &str = "
FUNCTION_BLOCK FB_Point
VAR
   pNumber : POINTER TO INT;
   iNumber1 : INT;
   iNumber2 : INT;
END_VAR
iNumber1 := 5;
pNumber := ADR(iNumber1);
iNumber2 := pNumber^;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    point : FB_Point;
END_VAR
    point();
END_PROGRAM
";

/// The minimal flag set for `ADR`: the pointer type plus the operator,
/// without `allow_ref_to`.
fn adr_options() -> CompilerOptions {
    CompilerOptions {
        allow_pointer_to: true,
        allow_adr: true,
        ..CompilerOptions::default()
    }
}

/// Parse, analyze, compile, and run one scan cycle with the given options.
fn adr_compile_and_run(
    source: &str,
    options: &CompilerOptions,
) -> (ironplc_container::Container, VmBuffers) {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], options).unwrap();
    let codegen_options = crate::CodegenOptions::default();
    let container = crate::compile(&analyzed, &ctx, &codegen_options, &crate::EmptyLookup).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    (container, bufs)
}

/// REQ-PTR-codegen-500: The Goal example executes — inside an FB instance
/// called from a PROGRAM, `pNumber := ADR(iNumber1); iNumber2 := pNumber^;`
/// yields the addressed member's value.
#[spec_test(REQ_PTR_codegen_500)]
fn codegen_spec_req_ptr_500_goal_example_deref_yields_value() {
    let (_c, bufs) = adr_compile_and_run(GOAL_EXAMPLE, &adr_options());
    // vars: point=0, then the FB body's slots pNumber=1, iNumber1=2,
    // iNumber2=3.
    assert_eq!(bufs.vars[2].as_i32(), 5);
    assert_eq!(bufs.vars[3].as_i32(), 5);
}

/// REQ-PTR-codegen-501: Storing through an `ADR`-bound pointer (`p^ := v`)
/// updates the addressed variable.
#[spec_test(REQ_PTR_codegen_501)]
fn codegen_spec_req_ptr_501_store_through_pointer_updates_target() {
    let source = "
PROGRAM main
VAR
    x : INT := 1;
    p : POINTER TO INT;
    v : INT := 99;
END_VAR
    p := ADR(x);
    p^ := v;
END_PROGRAM
";
    let (_c, bufs) = adr_compile_and_run(source, &adr_options());
    // vars: x=0, p=1, v=2. Writing through p must update x.
    assert_eq!(bufs.vars[0].as_i32(), 99);
}

/// REQ-PTR-codegen-502: An `ADR`-bound pointer compares non-equal to `NULL`,
/// and an unbound pointer defaults to `NULL`. (`NULL` is the `allow_ref_to`
/// keyword, as in the `codesys` dialect.)
#[spec_test(REQ_PTR_codegen_502)]
fn codegen_spec_req_ptr_502_null_guard_reflects_binding() {
    let source = "
PROGRAM main
VAR
    x : INT;
    bound : POINTER TO INT;
    unbound : POINTER TO INT;
    boundValid : BOOL;
    unboundValid : BOOL;
END_VAR
    bound := ADR(x);
    boundValid := bound <> NULL;
    unboundValid := unbound <> NULL;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..adr_options()
    };
    let (_c, bufs) = adr_compile_and_run(source, &options);
    // vars: x=0, bound=1, unbound=2, boundValid=3, unboundValid=4.
    assert_eq!(bufs.vars[3].as_i32(), 1, "bound pointer is not NULL");
    assert_eq!(bufs.vars[4].as_i32(), 0, "unbound pointer defaults to NULL");
}

/// REQ-PTR-codegen-510: The `twincat` and `codesys` dialect presets compile
/// and run the Goal example with no explicit flags.
#[spec_test(REQ_PTR_codegen_510)]
fn codegen_spec_req_ptr_510_dialect_presets_run_goal_example() {
    for dialect in [Dialect::TwinCat, Dialect::Codesys] {
        let options = CompilerOptions::from_dialect(dialect);
        let (_c, bufs) = adr_compile_and_run(GOAL_EXAMPLE, &options);
        assert_eq!(
            bufs.vars[3].as_i32(),
            5,
            "Goal example must run under the {dialect} dialect preset"
        );
    }
}
