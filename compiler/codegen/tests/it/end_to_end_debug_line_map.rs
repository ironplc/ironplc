//! End-to-end tests for the debug section's LINE_MAP (tag 1) +
//! SOURCE_FILE_TABLE (tag 6).
//!
//! These tests parse a real `.st` source string, run the full
//! codegen pipeline with a [`SourceLookup`] that returns the bytes,
//! and assert on:
//!
//! - One `SourceFileEntry` per registered POU file, with a BLAKE3
//!   hash matching `blake3::hash(source)`.
//! - At least one `LineMapEntry` per statement, with
//!   `(source_line, source_column)` matching the position of that
//!   statement in the source, and `file_id` pointing at the right
//!   `SourceFileEntry`.
//! - Every `bytecode_offset` lands on an instruction boundary in the
//!   optimized function bytecode (the optimizer offset-remap is wired
//!   up correctly).

use std::collections::HashMap;

use ironplc_analyzer::stages::resolve_types;
use ironplc_codegen::{compile, CodegenOptions, SourceLookup};
use ironplc_container::opcode;
use ironplc_container::{FunctionId, LineMapEntry, SourceFileId, SOURCE_FILE_HASH_LEN};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

/// `SourceLookup` impl backed by an in-memory map. Mirrors the
/// adapter the CLI uses.
struct MapLookup(HashMap<FileId, Vec<u8>>);

impl SourceLookup for MapLookup {
    fn source_bytes(&self, file_id: &FileId) -> Option<&[u8]> {
        self.0.get(file_id).map(Vec::as_slice)
    }
}

/// Parses `source`, runs the analyzer, and compiles with a
/// `SourceLookup` that returns the original bytes. Returns the
/// container plus the FileId used for parsing so tests can assert
/// against it.
fn compile_with_source(source: &str) -> (ironplc_container::Container, FileId) {
    let file_id = FileId::from_string("test.st");
    let options = CompilerOptions::default();
    let library = parse_program(source, &file_id, &options).unwrap();
    let (analyzed, ctx) = resolve_types(&[&library], &options).unwrap();

    let mut bytes_map = HashMap::new();
    bytes_map.insert(file_id.clone(), source.as_bytes().to_vec());
    let lookup = MapLookup(bytes_map);

    let container = compile(&analyzed, &ctx, &CodegenOptions::default(), &lookup).unwrap();
    (container, file_id)
}

#[test]
fn line_map_when_program_has_assignment_statements_then_each_gets_an_entry() {
    let source =
        "PROGRAM main\n  VAR x : DINT; y : DINT; END_VAR\n  x := 10;\n  y := 20;\nEND_PROGRAM\n";
    let (container, _) = compile_with_source(source);
    let debug = container
        .debug_section
        .as_ref()
        .expect("debug section present");

    // SOURCE_FILE_TABLE: one entry for the program's file, BLAKE3
    // hash matches the source bytes.
    assert_eq!(debug.source_files.len(), 1);
    let expected_hash: [u8; SOURCE_FILE_HASH_LEN] = *blake3::hash(source.as_bytes()).as_bytes();
    assert_eq!(debug.source_files[0].content_hash, expected_hash);

    // LINE_MAP: at least one entry per statement that emitted opcodes.
    // The two assignments are on lines 3 and 4. The init function gets
    // its own entries (variable initialization), so we filter to the
    // scan function which contains the body statements.
    let scan_entries: Vec<&LineMapEntry> = debug
        .line_map
        .iter()
        .filter(|e| e.function_id == FunctionId::SCAN)
        .collect();
    let scan_lines: Vec<u16> = scan_entries.iter().map(|e| e.source_line.raw()).collect();
    assert!(
        scan_lines.contains(&3),
        "expected SCAN line_map to cover line 3 (x := 10), got {scan_lines:?}",
    );
    assert!(
        scan_lines.contains(&4),
        "expected SCAN line_map to cover line 4 (y := 20), got {scan_lines:?}",
    );

    // Every line_map entry references file_id 0 (the only registered
    // source file).
    for entry in &debug.line_map {
        assert_eq!(entry.file_id, SourceFileId::new(0));
    }
}

#[test]
fn line_map_when_empty_program_then_source_file_table_still_populated_with_zero_hash_for_no_bytes()
{
    // Compile with an empty lookup — no bytes mean an all-zero hash
    // (the spec's "drift check unavailable" sentinel) and no
    // line-map entries (the emitter skips set_source_position when
    // no source bytes are cached).
    let file_id = FileId::from_string("nobytes.st");
    let source = "PROGRAM main\n  VAR x : DINT; END_VAR\n  x := 1;\nEND_PROGRAM\n";
    let options = CompilerOptions::default();
    let library = parse_program(source, &file_id, &options).unwrap();
    let (analyzed, ctx) = resolve_types(&[&library], &options).unwrap();

    let container = compile(
        &analyzed,
        &ctx,
        &CodegenOptions::default(),
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let debug = container.debug_section.as_ref().expect("debug section");

    // Source file is still registered (so file_ids resolve) but the
    // hash is all-zero.
    assert_eq!(debug.source_files.len(), 1);
    assert_eq!(
        debug.source_files[0].content_hash,
        [0u8; SOURCE_FILE_HASH_LEN]
    );
    // No line-map entries — without source bytes we can't compute
    // (line, column), so we don't record anything.
    assert!(
        debug.line_map.is_empty(),
        "expected empty line_map for EmptyLookup, got {:?}",
        debug.line_map
    );
}

#[test]
fn line_map_when_optimizer_runs_then_bytecode_offsets_are_on_instruction_boundaries() {
    // Use a program whose peephole optimizer will visibly fold
    // instructions. `x := x` (self-assignment) is one of the
    // patterns the optimizer collapses; verify any remaining line-map
    // entries point at valid instruction starts in the optimized
    // bytecode.
    let source = "PROGRAM main\n  VAR x : DINT; END_VAR\n  x := 10;\n  x := x + 5;\nEND_PROGRAM\n";
    let (container, _) = compile_with_source(source);
    let debug = container
        .debug_section
        .as_ref()
        .expect("debug section present");

    for entry in &debug.line_map {
        let bytecode = container
            .code
            .get_function_bytecode(entry.function_id)
            .expect("function bytecode present");
        let offset = entry.bytecode_offset as usize;
        assert!(
            offset < bytecode.len(),
            "line_map offset {offset} is past end of function bytecode ({} bytes)",
            bytecode.len(),
        );
        // Walk the bytecode from the start and confirm `offset` is
        // one of the instruction starts. This is a coarse check —
        // we just decode opcode arg-counts and skip — but it catches
        // off-by-one remap bugs that would put offsets in the middle
        // of multi-byte instructions.
        let mut pc = 0usize;
        let mut hit = false;
        while pc < bytecode.len() {
            if pc == offset {
                hit = true;
                break;
            }
            let op = bytecode[pc];
            pc += opcode::instruction_size(op);
        }
        assert!(
            hit,
            "line_map offset {offset} (function {:?}) does not land on an instruction \
             boundary in the optimized bytecode {:02X?}",
            entry.function_id, bytecode,
        );
    }
}
