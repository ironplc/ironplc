//! End-to-end test for type names written with a library prefix
//! (`--allow-qualified-type-names`): a prefixed type is the type the library
//! that declares it names, so a program using `BROTLib.FB_Blinker` and
//! `BROTLib.E_Blink.slow` runs as one using `FB_Blinker` and `E_Blink.slow`.

use ironplc_analyzer::{stages::analyze_with_library_files, LibraryFiles};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

use crate::common::Snapshot;

/// What the project `BROTLib`, loaded alongside the program, declares.
const BROT_LIB: &str = "
TYPE E_Blink : (off, slow, fast); END_TYPE

FUNCTION_BLOCK FB_Blinker
VAR_INPUT
    mode : E_Blink;
END_VAR
VAR_OUTPUT
    period : DINT;
END_VAR
CASE mode OF
    E_Blink.slow: period := 1000;
    E_Blink.fast: period := 250;
ELSE
    period := 0;
END_CASE;
END_FUNCTION_BLOCK
";

#[test]
fn end_to_end_when_types_have_library_prefix_then_runs_as_unprefixed() {
    let options = CompilerOptions {
        allow_qualified_type_names: true,
        allow_dotted_enum_value: true,
        ..CompilerOptions::default()
    };
    let lib_file = FileId::from_string("BROTLib/POUs.st");
    let lib = parse_program(BROT_LIB, &lib_file, &options).unwrap();
    let main = parse_program(
        "PROGRAM main
VAR
    mode : BROTLib.E_Blink := BROTLib.E_Blink.slow;
    blinker : BROTLib.FB_Blinker;
    period : DINT;
END_VAR
blinker(mode := mode);
period := blinker.period;
END_PROGRAM",
        &FileId::from_string("main.st"),
        &options,
    )
    .unwrap();
    let mut files = LibraryFiles::new();
    files.insert(lib_file, "BROTLib");

    let (analyzed, context) = analyze_with_library_files(&[&lib, &main], &files, &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );

    let snapshot = Snapshot::run_analyzed(&analyzed, &context, &options);
    assert_eq!(snapshot.read_as::<i32>("mode"), 1);
    assert_eq!(snapshot.read_as::<i32>("period"), 1000);
}
