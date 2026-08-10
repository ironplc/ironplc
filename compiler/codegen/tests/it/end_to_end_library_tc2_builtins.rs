//! End-to-end tests for the bundled `Tc2_BuiltIns` compatibility library:
//! load the real bundled package, compile user source against it, and run the
//! VM.
//!
//! `Tc2_BuiltIns` mirrors TwinCAT's *built-in* operator surface — implicit
//! conversion operators such as `BOOL_TO_STRING` that TwinCAT always has in
//! scope and that belong to no vendor library there. IronPLC does not add
//! vendor names to its compiler tables (ADR-0042 rule 1), so the operator
//! ships as a plain library ST function — no intrinsic, no func_id — compiled
//! like user code. These tests pin the behavior from the clean-room spec in
//! `specs/design/library-interfaces/bool-to-string.md`.

use ironplc_container::STRING_HEADER_BYTES;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::libraries::{LibraryName, LibraryRegistry};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;

/// Loads the named bundled library, merges its declarations ahead of
/// `user_source` (so a user declaration would shadow a library one), compiles,
/// and runs one scan cycle.
fn run_with_bundled_library(library_name: &str, user_source: &str) -> VmBuffers {
    let options = CompilerOptions::default();
    let registry = LibraryRegistry::bundled();
    let loaded = registry.load(&LibraryName::from(library_name)).unwrap();

    let user =
        ironplc_parser::parse_program(user_source, &FileId::from_string("user.st"), &options)
            .unwrap();
    let analyze_input: Vec<&ironplc_dsl::common::Library> = [&loaded.library, &user].to_vec();
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&analyze_input, &options).unwrap();

    let container = ironplc_codegen::compile(
        &analyzed,
        &ctx,
        &ironplc_codegen::CodegenOptions::default(),
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();

    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    bufs
}

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

fn bool_to_string_result(literal: &str) -> String {
    let source = format!(
        "PROGRAM main
VAR
    s : STRING;
END_VAR
    s := BOOL_TO_STRING({literal});
END_PROGRAM
"
    );
    let bufs = run_with_bundled_library("Tc2_BuiltIns", &source);
    read_string(&bufs.data_region, 0)
}

#[test]
fn end_to_end_when_bool_to_string_true_then_exact_true_keyword() {
    // Exactly the uppercase keyword spelling, no padding.
    assert_eq!(bool_to_string_result("TRUE"), "TRUE");
}

#[test]
fn end_to_end_when_bool_to_string_false_then_exact_false_keyword() {
    assert_eq!(bool_to_string_result("FALSE"), "FALSE");
}
