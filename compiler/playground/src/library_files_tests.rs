//! Tests for the served library files the browser passes to the compile
//! path: each is named by its library and identified by the path it is
//! served at.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;

use super::parse_activated_libraries;

#[test]
fn parse_activated_libraries_when_file_given_then_named_by_library_and_identified_by_path() {
    let libraries = serde_json::json!([{
        "name": "Tc2_System",
        "path": "libs/Tc2_System/1.0.0/Tc2_System.st",
        "source": "VAR_GLOBAL CONSTANT PI : LREAL := 3.14; END_VAR",
    }])
    .to_string();

    let Ok(parsed) = parse_activated_libraries(&libraries, &CompilerOptions::default()) else {
        panic!("a served library file must parse");
    };

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name.as_str(), "Tc2_System");
    assert_eq!(
        parsed[0].file_id,
        FileId::from_string("libs/Tc2_System/1.0.0/Tc2_System.st")
    );
}

#[test]
fn parse_activated_libraries_when_file_has_no_name_then_error() {
    let libraries =
        serde_json::json!(["VAR_GLOBAL CONSTANT PI : LREAL := 3.14; END_VAR"]).to_string();

    let result = parse_activated_libraries(&libraries, &CompilerOptions::default());

    assert!(matches!(result, Err(failed) if !failed.ok));
}
