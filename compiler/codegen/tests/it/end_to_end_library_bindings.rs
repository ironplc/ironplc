//! End-to-end tests for the bundled `Tc2_Math`/`Tc2_System` compatibility
//! libraries: activate the real bundled package, compile against it with its
//! bindings threaded, and run the VM.
//!
//! This is the full paved-path pipeline (load library → analyze merged →
//! codegen with bindings → VM), pinning the behaviors from the clean-room
//! specs in `specs/design/library-interfaces/`.

use ironplc_container::{Container, STRING_HEADER_BYTES};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::libraries::{LibraryName, LibraryRegistry};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;

/// Loads the named bundled libraries, merges them ahead of `user_source`,
/// and compiles with the libraries' bindings threaded into codegen.
fn compile_with_bundled_libraries(
    library_names: &[&str],
    user_source: &str,
) -> Result<Container, ironplc_dsl::diagnostic::Diagnostic> {
    let options = CompilerOptions::default();
    let registry = LibraryRegistry::bundled();

    let mut bindings = ironplc_dsl::bindings::LibraryBindings::new();
    let mut compat_libraries = Vec::new();
    for name in library_names {
        let loaded = registry.load(&LibraryName::from(*name)).unwrap();
        bindings.merge(loaded.bindings.clone());
        compat_libraries.push(loaded.library);
    }

    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let analyze_input: Vec<&ironplc_dsl::common::Library> =
        compat_libraries.iter().chain(std::iter::once(&user)).collect();
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&analyze_input, &options).unwrap();

    let codegen_options = ironplc_codegen::CodegenOptions {
        library_bindings: bindings,
        ..Default::default()
    };
    ironplc_codegen::compile(&analyzed, &ctx, &codegen_options, &ironplc_codegen::EmptyLookup)
}

/// Compiles with the bundled libraries and runs one scan cycle.
fn run_with_bundled_libraries(library_names: &[&str], user_source: &str) -> VmBuffers {
    let container = compile_with_bundled_libraries(library_names, user_source).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    bufs
}

/// Compiles a one-expression `LREAL` program against `Tc2_Math` and returns
/// `result`.
///
/// The operands are `LREAL` variables (`a`, `b`), not literals: an untyped
/// real literal is typed `REAL` and narrows through f32 on the user-function
/// call path, which would blur the 1e-9 pins here. Call-site REAL→LREAL
/// widening is separately planned work
/// (specs/plans/2026-07-27-twincat-real-to-lreal-widening.md).
fn eval_tc2_math(a: f64, b: f64, expression: &str) -> f64 {
    let source = format!(
        "PROGRAM main
VAR
    a : LREAL;
    b : LREAL;
    result : LREAL;
END_VAR
    a := {a};
    b := {b};
    result := {expression};
END_PROGRAM
"
    );
    let bufs = run_with_bundled_libraries(&["Tc2_Math"], &source);
    bufs.vars[2].as_f64()
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected ≈ {expected}, got {actual}"
    );
}

#[test]
fn end_to_end_when_ltrunc_then_truncates_toward_zero() {
    assert_near(eval_tc2_math(3.7, 0.0, "LTRUNC(a)"), 3.0);
    assert_near(eval_tc2_math(-3.7, 0.0, "LTRUNC(a)"), -3.0);
}

#[test]
fn end_to_end_when_lmod_then_signed_fractional_remainder() {
    assert_near(eval_tc2_math(400.56, 360.0, "LMOD(a, b)"), 40.56);
    assert_near(eval_tc2_math(-400.56, 360.0, "LMOD(a, b)"), -40.56);
}

#[test]
fn end_to_end_when_modabs_then_unsigned_within_modulo_range() {
    assert_near(eval_tc2_math(400.56, 360.0, "MODABS(a, b)"), 40.56);
    assert_near(eval_tc2_math(-400.56, 360.0, "MODABS(a, b)"), 319.44);
}

#[test]
fn end_to_end_when_frac_then_keeps_sign_of_input() {
    assert_near(eval_tc2_math(3.7, 0.0, "FRAC(a)"), 0.7);
    assert_near(eval_tc2_math(-3.7, 0.0, "FRAC(a)"), -0.7);
}

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

#[test]
fn end_to_end_when_bool_to_string_then_true_false_keywords() {
    // BOOL_TO_STRING is a plain library ST function in Tc2_System — no
    // intrinsic, no func_id (ADR-0042 rule 2).
    let source = "PROGRAM main
VAR
    s : STRING;
END_VAR
    s := BOOL_TO_STRING(TRUE);
END_PROGRAM
";
    let bufs = run_with_bundled_libraries(&["Tc2_System"], source);
    assert_eq!(read_string(&bufs.data_region, 0), "TRUE");

    let source = "PROGRAM main
VAR
    s : STRING;
END_VAR
    s := BOOL_TO_STRING(FALSE);
END_PROGRAM
";
    let bufs = run_with_bundled_libraries(&["Tc2_System"], source);
    assert_eq!(read_string(&bufs.data_region, 0), "FALSE");
}

#[test]
fn end_to_end_when_lreal_to_fmtstr_called_then_p4046_at_compile() {
    // Declare-only: the surface exists (check passes — covered by the
    // analyzer's REQ-CL-analyzer-007 test and the CLI split test), but
    // compiling a call is the dedicated error, never wrong codegen.
    let source = "PROGRAM main
VAR
    s : STRING[255];
END_VAR
    s := LREAL_TO_FMTSTR(1.5, 2, TRUE);
END_PROGRAM
";
    let err = compile_with_bundled_libraries(&["Tc2_Utilities"], source).unwrap_err();
    assert_eq!(err.code, "P4046");
    assert!(err.primary.message.contains("Tc2_Utilities"));
    assert!(err.primary.message.contains("LREAL_TO_FMTSTR"));
}

/// Every bundled library's intrinsic bindings resolve via
/// `intrinsic_func_id` — the packaging conformance guard for the defensive
/// P6010 in codegen.
#[test]
fn bundled_libraries_when_intrinsic_bindings_then_all_resolve() {
    let registry = LibraryRegistry::bundled();
    let names = registry.library_names();
    assert!(!names.is_empty(), "bundled registry must not be empty");
    for name in names {
        let loaded = registry.load(&name).unwrap();
        for (pou, bound) in loaded.bindings.iter() {
            if let ironplc_dsl::bindings::PouBinding::Intrinsic { name: intrinsic } =
                &bound.binding
            {
                assert!(
                    ironplc_container::opcode::builtin::intrinsic_func_id(intrinsic).is_some(),
                    "library `{}` binds `{pou}` to unknown intrinsic `{intrinsic}`",
                    bound.library
                );
            }
        }
    }
}
