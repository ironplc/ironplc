//! Behavior tests for declare-only compatibility-library bindings.
//!
//! A declare-only POU's declaration (full signature, `;` body) makes
//! `check` pass, while compiling a *call* is the dedicated error P4046 —
//! never a CALL to the empty body, never a runtime trap. The library-file
//! set preserves user shadowing.

use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;

/// A `LibraryBindings` fixture: `pou` declared-only by `Tc2_Utilities`,
/// declared in the library file `lib.st`.
fn library_bindings_fixture(pou: &str) -> ironplc_dsl::bindings::LibraryBindings {
    let mut bindings = ironplc_dsl::bindings::LibraryBindings::new();
    bindings.insert_declare_only(pou, "Tc2_Utilities");
    bindings.add_library_file(FileId::from_string("lib.st"));
    bindings
}

/// Parses `library_source` as a library file (`lib.st`) and `user_source` as
/// user source, analyzes them merged, and compiles with `bindings` threaded.
///
/// The boxed `Err` keeps the return slim (clippy::result_large_err).
fn compile_with_bindings(
    library_source: &str,
    user_source: &str,
    bindings: ironplc_dsl::bindings::LibraryBindings,
) -> Result<Container, Box<ironplc_dsl::diagnostic::Diagnostic>> {
    let options = CompilerOptions::default();
    let lib =
        ironplc_parser::parse_program(library_source, &FileId::from_string("lib.st"), &options)
            .unwrap();
    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&[&lib, &user], &options).unwrap();
    let codegen_options = ironplc_codegen::CodegenOptions {
        library_bindings: bindings,
        ..Default::default()
    };
    ironplc_codegen::compile(
        &analyzed,
        &ctx,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .map_err(Box::new)
}

const DECLARE_ONLY_DECLARATION: &str = "FUNCTION MY_FMT : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
;
END_FUNCTION
";

#[test]
fn compile_when_declare_only_called_then_p4046_naming_library_and_pou() {
    let user_source = "PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := MY_FMT(3.7);
END_PROGRAM
";
    let err = compile_with_bindings(
        DECLARE_ONLY_DECLARATION,
        user_source,
        library_bindings_fixture("MY_FMT"),
    )
    .unwrap_err();
    assert_eq!(err.code, "P4046");
    assert!(err.primary.message.contains("Tc2_Utilities"));
    assert!(err.primary.message.contains("MY_FMT"));
}

#[test]
fn compile_when_declare_only_not_called_then_body_never_compiled() {
    // The surface can land ahead of its implementation: a program that does
    // not call the declare-only POU compiles, and the `;` body contributes
    // no function to the container (init + scan only).
    let user_source = "PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := 1.0;
END_PROGRAM
";
    let container = compile_with_bindings(
        DECLARE_ONLY_DECLARATION,
        user_source,
        library_bindings_fixture("MY_FMT"),
    )
    .unwrap();
    assert_eq!(container.code.functions.len(), 2);
}

#[test]
fn compile_when_user_function_shadows_declare_only_name_then_user_body_compiles() {
    // The user declares their own MY_FMT (in user.st, not a library file)
    // with a real body: it compiles and the call is an ordinary CALL.
    let user_source = "FUNCTION MY_FMT : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
    MY_FMT := IN;
END_FUNCTION

PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := MY_FMT(3.7);
END_PROGRAM
";
    let options = CompilerOptions::default();
    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&user], &options).unwrap();
    let codegen_options = ironplc_codegen::CodegenOptions {
        library_bindings: library_bindings_fixture("MY_FMT"),
        ..Default::default()
    };
    let container = ironplc_codegen::compile(
        &analyzed,
        &ctx,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();

    // The user body compiled (init + scan + the user function) and the call
    // site is a CALL.
    assert_eq!(container.code.functions.len(), 3);
    let scan = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert!(scan.contains(&ironplc_container::opcode::CALL));
}
