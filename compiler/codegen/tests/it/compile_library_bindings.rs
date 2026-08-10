//! Behavioral tests for compatibility-library bindings at codegen.
//!
//! These tests build the [`ironplc_dsl::bindings::LibraryBindings`]
//! side-table directly in code — no bundled library or registry is
//! involved — and verify the three binding behaviors:
//!
//! 1. A call to a library POU bound to an intrinsic lowers to
//!    `BUILTIN func_id` and the POU's `;` body is never compiled.
//! 2. A call to a declare-only library POU fails compilation with P4046,
//!    naming the library and the POU.
//! 3. A user-defined function shadowing a bound name compiles as the
//!    user's function (CALL, not the bound builtin).
//!
//! See `specs/design/compatibility-libraries.md` §Bodies and bindings and
//! `specs/plans/2026-08-08-compatibility-library-bindings.md`.

use ironplc_codegen::{compile, CodegenOptions, EmptyLookup};
use ironplc_container::{opcode, Container, FunctionId};
use ironplc_dsl::bindings::{BoundPou, LibraryBindings, PouBinding};
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;

/// A `LibraryBindings` fixture: `pou` bound as `binding`, declared in the
/// library file `lib.st`.
fn library_bindings_fixture(pou: &str, binding: PouBinding) -> LibraryBindings {
    let mut bindings = LibraryBindings::new();
    bindings.insert(
        pou,
        BoundPou {
            library: "Fixture_Math".to_string(),
            manifest_file: FileId::from_string("library.toml"),
            binding,
        },
    );
    bindings.add_library_file(FileId::from_string("lib.st"));
    bindings
}

/// Parses `library_source` as a library file (`lib.st`) and `user_source` as
/// user source, analyzes them merged, and compiles with `bindings` threaded.
fn compile_with_bindings(
    library_source: &str,
    user_source: &str,
    bindings: LibraryBindings,
) -> Result<Container, Box<Diagnostic>> {
    let options = CompilerOptions::default();
    let lib =
        ironplc_parser::parse_program(library_source, &FileId::from_string("lib.st"), &options)
            .unwrap();
    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&[&lib, &user], &options).unwrap();
    let codegen_options = CodegenOptions {
        library_bindings: bindings,
        ..Default::default()
    };
    compile(&analyzed, &ctx, &codegen_options, &EmptyLookup).map_err(Box::new)
}

/// True when the bytecode contains `BUILTIN` with the given func_id operand.
fn contains_builtin(bytecode: &[u8], func_id: u16) -> bool {
    bytecode
        .windows(3)
        .any(|w| w[0] == opcode::BUILTIN && u16::from_le_bytes([w[1], w[2]]) == func_id)
}

const MY_SQRT_DECLARATION: &str = "FUNCTION MY_SQRT : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
;
END_FUNCTION
";

const MY_SQRT_CALLER: &str = "PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := MY_SQRT(3.7);
END_PROGRAM
";

/// A call to a library POU bound to an intrinsic compiles to
/// `BUILTIN func_id` — not to a CALL of the POU's (empty) ST body, which is
/// never compiled.
#[test]
fn compile_when_intrinsic_bound_library_call_then_lowers_to_builtin() {
    let bindings = library_bindings_fixture(
        "MY_SQRT",
        PouBinding::Intrinsic {
            name: "sqrt_lreal".to_string(),
        },
    );
    let container = compile_with_bindings(MY_SQRT_DECLARATION, MY_SQRT_CALLER, bindings).unwrap();

    // The bound `;` body was never compiled: only init (0) and scan (1).
    assert_eq!(container.code.functions.len(), 2);

    // The call site lowered to BUILTIN SQRT_F64 and no CALL was emitted.
    let scan = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    assert!(
        contains_builtin(scan, opcode::builtin::SQRT_F64),
        "scan bytecode must contain BUILTIN SQRT_F64"
    );
    assert!(
        !scan.contains(&opcode::CALL),
        "an intrinsic-bound call must not emit CALL"
    );
}

/// A call to a declare-only library POU fails compilation with the dedicated
/// diagnostic P4046, naming the library and POU.
#[test]
fn compile_when_declare_only_library_call_then_fails_with_p4046() {
    let bindings = library_bindings_fixture("MY_SQRT", PouBinding::DeclareOnly);
    let err = compile_with_bindings(MY_SQRT_DECLARATION, MY_SQRT_CALLER, bindings).unwrap_err();
    assert_eq!(err.code, "P4046");
    assert!(err.primary.message.contains("Fixture_Math"));
    assert!(err.primary.message.contains("MY_SQRT"));
}

/// A user-defined function with the same name as a bound library POU compiles
/// as the user's function — binding lowering applies only to the library's
/// declaration (the `FileId` check).
#[test]
fn compile_when_user_function_shadows_bound_name_then_user_body_compiles() {
    let bindings = library_bindings_fixture(
        "MY_SQRT",
        PouBinding::Intrinsic {
            name: "sqrt_lreal".to_string(),
        },
    );
    // The user declares their own MY_SQRT (in user.st, not a library file)
    // with a real body. No library declaration is merged at all: everything
    // is user source.
    let user_source = "FUNCTION MY_SQRT : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
    MY_SQRT := IN;
END_FUNCTION

PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := MY_SQRT(3.7);
END_PROGRAM
";
    let options = CompilerOptions::default();
    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&user], &options).unwrap();
    let codegen_options = CodegenOptions {
        library_bindings: bindings,
        ..Default::default()
    };
    let container = compile(&analyzed, &ctx, &codegen_options, &EmptyLookup).unwrap();

    // The user body compiled (init + scan + the user function) and the call
    // lowered to CALL, not to the bound builtin.
    assert_eq!(container.code.functions.len(), 3);
    let scan = container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    assert!(scan.contains(&opcode::CALL));
    assert!(
        !contains_builtin(scan, opcode::builtin::SQRT_F64),
        "a shadowing user function must not lower to the bound builtin"
    );
}
