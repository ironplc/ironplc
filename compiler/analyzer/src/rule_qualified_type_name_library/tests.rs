//! Tests for `rule_qualified_type_name_library`. Each program is several
//! source files, some of which belong to a library: `brot.st` to the
//! project `BROTLib`, `math.st` to the library `Tc2_Math`.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use rstest::rstest;

use crate::library_files::LibraryFiles;
use crate::stages::resolve_types;
use crate::test_helpers::codes;

/// What `BROTLib` declares.
const BROT: &str = "
TYPE E_Blink : (off, slow); END_TYPE
FUNCTION_BLOCK FB_Blinker
END_FUNCTION_BLOCK
INTERFACE I_Comm
END_INTERFACE";

/// What `Tc2_Math` declares: no types, as the bundled library.
const MATH: &str = "FUNCTION LMOD : LREAL LMOD := 0.0; END_FUNCTION";

/// What the user's own source, in no library, declares.
const OWN: &str = "TYPE E_Own : (a, b); END_TYPE";

fn options() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

/// The diagnostics the rule reports for `main`, analyzed with the files
/// above.
fn diagnostics_of(main: &str) -> Vec<ironplc_dsl::diagnostic::Diagnostic> {
    let sources: Vec<_> = [
        ("brot.st", BROT),
        ("math.st", MATH),
        ("own.st", OWN),
        ("main.st", main),
    ]
    .iter()
    .map(|(file, text)| parse_program(text, &FileId::from_string(file), &options()).unwrap())
    .collect();
    let mut files = LibraryFiles::new();
    files.insert(FileId::from_string("brot.st"), "BROTLib");
    files.insert(FileId::from_string("math.st"), "Tc2_Math");

    let inputs: Vec<_> = sources.iter().collect();
    let (library, context) = resolve_types(&inputs, &options()).unwrap();
    let context = context.with_library_files(files);
    super::apply(&library, &context, &options())
        .err()
        .unwrap_or_default()
}

fn codes_of(main: &str) -> Vec<String> {
    diagnostics_of(main)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

/// A program declaring the variable `declaration`.
fn in_variable(declaration: &str) -> String {
    format!("PROGRAM main VAR {declaration} END_VAR END_PROGRAM")
}

#[rstest]
#[case::variable_type(in_variable("mode : BROTLib.E_Blink;"))]
#[case::prefix_in_other_case(in_variable("mode : brotlib.E_Blink;"))]
#[case::function_block_instance(in_variable("fb : BROTLib.FB_Blinker;"))]
#[case::dotted_initial_value(in_variable("mode : E_Blink := BROTLib.E_Blink.slow;"))]
#[case::dotted_case_label(
    "PROGRAM main VAR mode : E_Blink; END_VAR CASE mode OF BROTLib.E_Blink.off: ; END_CASE; END_PROGRAM".to_string()
)]
#[case::extends("FUNCTION_BLOCK FB_Mine EXTENDS BROTLib.FB_Blinker END_FUNCTION_BLOCK".to_string())]
#[case::implements("FUNCTION_BLOCK FB_Mine IMPLEMENTS BROTLib.I_Comm END_FUNCTION_BLOCK".to_string())]
fn apply_when_prefix_names_declaring_library_then_ok(#[case] main: String) {
    assert_eq!(codes_of(&main), codes(&[]));
}

#[rstest]
#[case::unknown_library(in_variable("mode : Nonsense.E_Blink;"))]
// `Tc2_Math` is a library, but `E_Blink` is not one of its types.
#[case::other_library(in_variable("mode : Tc2_Math.E_Blink;"))]
// IronPLC provides `TON` itself; no library declares it.
#[case::type_ironplc_provides(in_variable("t : Tc2_Standard.TON;"))]
// `E_Own` is declared by a source in no library.
#[case::type_in_no_library(in_variable("e : Main.E_Own;"))]
#[case::dotted_value_of_other_library(in_variable("mode : E_Blink := Tc2_Math.E_Blink.slow;"))]
#[case::extends_unknown_library(
    "FUNCTION_BLOCK FB_Mine EXTENDS Nonsense.FB_Blinker END_FUNCTION_BLOCK".to_string()
)]
#[case::implements_unknown_library(
    "FUNCTION_BLOCK FB_Mine IMPLEMENTS Nonsense.I_Comm END_FUNCTION_BLOCK".to_string()
)]
fn apply_when_prefix_names_other_or_no_library_then_mismatch(#[case] main: String) {
    assert_eq!(
        codes_of(&main),
        codes(&[Problem::QualifiedTypeNameLibraryMismatch])
    );
}

// A type nothing declares is reported by the rules that resolve type names.
#[test]
fn apply_when_prefixed_type_not_declared_then_ok() {
    assert_eq!(codes_of(&in_variable("x : Nonsense.FB_Nope;")), codes(&[]));
}

#[test]
fn apply_when_prefix_names_other_library_then_label_on_prefix_and_names_declaring_library() {
    let diagnostics = diagnostics_of(&in_variable("mode : Tc2_Math.E_Blink;"));

    assert_eq!(diagnostics.len(), 1);
    let main = in_variable("mode : Tc2_Math.E_Blink;");
    let location = &diagnostics[0].primary.location;
    assert_eq!(&main[location.start..location.end], "Tc2_Math");
    assert!(
        format!("{diagnostics:?}").contains("BROTLib"),
        "the diagnostic must name the declaring library: {diagnostics:?}"
    );
}
