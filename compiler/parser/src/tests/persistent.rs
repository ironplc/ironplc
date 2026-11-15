//! The `--allow-persistent-var` extension: `PERSISTENT`, alone or combined
//! with `RETAIN` in either order, on the variable blocks that accept a
//! storage qualifier.
//!
//! The `VAR PERSISTENT` and `VAR_GLOBAL PERSISTENT` cases live in
//! `var_declarations.rs`.

use super::common::*;

fn opts_with_persistent_var() -> CompilerOptions {
    CompilerOptions {
        allow_persistent_var: true,
        ..CompilerOptions::default()
    }
}

fn parse_fb_qualifiers(source: &str, options: &CompilerOptions) -> Vec<DeclarationQualifier> {
    let lib = parse_program(source, &FileId::default(), options).unwrap();
    let fb = cast!(
        &lib.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    fb.variables.iter().map(|v| v.qualifier.clone()).collect()
}

/// Every storage qualifier on every block of a `FUNCTION_BLOCK` that takes
/// one.
#[rstest]
#[case::input_persistent("VAR_INPUT", "PERSISTENT", DeclarationQualifier::Persistent)]
#[case::input_retain_persistent(
    "VAR_INPUT",
    "RETAIN PERSISTENT",
    DeclarationQualifier::RetainPersistent
)]
#[case::input_persistent_retain(
    "VAR_INPUT",
    "PERSISTENT RETAIN",
    DeclarationQualifier::RetainPersistent
)]
#[case::output_persistent("VAR_OUTPUT", "PERSISTENT", DeclarationQualifier::Persistent)]
#[case::output_retain_persistent(
    "VAR_OUTPUT",
    "RETAIN PERSISTENT",
    DeclarationQualifier::RetainPersistent
)]
#[case::output_persistent_retain(
    "VAR_OUTPUT",
    "PERSISTENT RETAIN",
    DeclarationQualifier::RetainPersistent
)]
#[case::var_retain_persistent("VAR", "RETAIN PERSISTENT", DeclarationQualifier::RetainPersistent)]
#[case::var_persistent_retain("VAR", "PERSISTENT RETAIN", DeclarationQualifier::RetainPersistent)]
#[case::var_comment_between(
    "VAR",
    "RETAIN (* c *) PERSISTENT",
    DeclarationQualifier::RetainPersistent
)]
fn parse_when_function_block_block_qualified_and_flag_on_then_qualifier_kept(
    #[case] keyword: &str,
    #[case] qualifier: &str,
    #[case] expected: DeclarationQualifier,
) {
    let source = format!(
        "FUNCTION_BLOCK FB_Example
{keyword} {qualifier}
    nCounter : DINT;
END_VAR
END_FUNCTION_BLOCK"
    );

    let qualifiers = parse_fb_qualifiers(&source, &opts_with_persistent_var());

    assert_eq!(qualifiers, vec![expected]);
}

#[rstest]
#[case::retain_persistent("RETAIN PERSISTENT")]
#[case::persistent_retain("PERSISTENT RETAIN")]
fn parse_when_program_var_retain_and_persistent_then_retain_persistent(#[case] qualifier: &str) {
    let source = format!(
        "PROGRAM main
VAR {qualifier}
    nCounter : DINT;
END_VAR
END_PROGRAM"
    );

    let lib = parse_program(&source, &FileId::default(), &opts_with_persistent_var()).unwrap();

    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(
        prog.variables[0].qualifier,
        DeclarationQualifier::RetainPersistent
    );
}

#[rstest]
#[case::retain_persistent("RETAIN PERSISTENT")]
#[case::persistent_retain("PERSISTENT RETAIN")]
fn parse_when_var_global_retain_and_persistent_then_retain_persistent(#[case] qualifier: &str) {
    let options = CompilerOptions {
        allow_persistent_var: true,
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    };
    let source = format!(
        "VAR_GLOBAL {qualifier}
    nCounter : DINT;
END_VAR"
    );

    let lib = parse_program(&source, &FileId::default(), &options).unwrap();

    let vars = cast!(&lib.elements[0], LibraryElementKind::GlobalVarDeclarations);
    assert_eq!(vars[0].qualifier, DeclarationQualifier::RetainPersistent);
}

/// With the flag off `PERSISTENT` is an identifier, so a `RETAIN` block
/// whose first variable is named `PERSISTENT` still parses as standard
/// IEC 61131-3.
#[test]
fn parse_when_retain_then_variable_named_persistent_and_flag_off_then_retain() {
    let lib = parse_text(
        "FUNCTION_BLOCK FB_Example
VAR_INPUT RETAIN
    PERSISTENT : DINT;
END_VAR
END_FUNCTION_BLOCK",
    );

    let fb = cast!(
        &lib.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(fb.variables[0].qualifier, DeclarationQualifier::Retain);
    assert_eq!(
        fb.variables[0].identifier.symbolic_id().unwrap().original(),
        "PERSISTENT"
    );
}

#[rstest]
#[case::input("VAR_INPUT")]
#[case::output("VAR_OUTPUT")]
fn parse_when_io_block_persistent_and_flag_off_then_error(#[case] keyword: &str) {
    let source = format!(
        "FUNCTION_BLOCK FB_Example
{keyword} PERSISTENT
    nCounter : DINT;
END_VAR
END_FUNCTION_BLOCK"
    );

    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());

    assert!(result.is_err());
}
