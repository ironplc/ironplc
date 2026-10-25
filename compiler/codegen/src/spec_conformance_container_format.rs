//! Spec conformance tests for what the compiler writes into the container
//! header (`REQ-CF-codegen-*`).
//!
//! The container crate owns the rest of `bytecode-container-format.md`; the
//! requirements here are claims about compiler output rather than about the
//! format, so they are verified where the output is produced.
//!
//! See `specs/design/spec-conformance-testing.md` for the mechanism.

use std::io::Cursor;

use ironplc_container::{integrity, FileHeader, HEADER_SIZE};
use ironplc_ir::FileId;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

/// Compiles `source` and returns the serialized bytes with the header parsed
/// back out of them, so every assertion is about what reaches the file.
fn compiled(source: &str) -> (Vec<u8>, FileHeader) {
    try_compiled(source, &CompilerOptions::default()).unwrap()
}

/// Compiles `source` under `options` as [`compiled`] does, or gives the
/// code of the problem the compiler reports.
fn try_compiled(source: &str, options: &CompilerOptions) -> Result<(Vec<u8>, FileHeader), String> {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::analyze(&[&library], options).unwrap();
    let container = crate::test_support::compile_analyzed(&analyzed, &ctx)
        .map_err(|diagnostic| diagnostic.code)?;
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    let header = FileHeader::read_from(&mut Cursor::new(&buf[..HEADER_SIZE])).unwrap();
    Ok((buf, header))
}

const ASSIGNMENT_PROGRAM: &str = "PROGRAM main
         VAR
             x : DINT;
         END_VAR
             x := 1;
         END_PROGRAM";

/// The bytes of one section, as the header's directory locates it.
fn section(buf: &[u8], offset: u32, size: u32) -> &[u8] {
    &buf[offset as usize..(offset + size) as usize]
}

/// REQ-CF-codegen-025: layout_hash is written as zeros — nothing computes
/// it yet.
#[spec_test(REQ_CF_codegen_025)]
fn container_spec_req_cf_025_layout_hash_is_written_as_zeros() {
    let (_, header) = compiled(ASSIGNMENT_PROGRAM);
    assert_eq!(header.layout_hash, integrity::NO_HASH);
}

/// REQ-CF-codegen-026: the compiler writes content_hash and debug_hash,
/// each reproducible from the section bytes it wrote.
#[spec_test(REQ_CF_codegen_026)]
fn container_spec_req_cf_026_content_and_debug_hashes_match_written_sections() {
    let (buf, header) = compiled(ASSIGNMENT_PROGRAM);

    let expected_content = integrity::content_hash(&integrity::Content {
        header: buf[..HEADER_SIZE].try_into().unwrap(),
        task_table: section(&buf, header.task_section_offset, header.task_section_size),
        type_section: section(&buf, header.type_section_offset, header.type_section_size),
        const_section: section(&buf, header.const_section_offset, header.const_section_size),
        code_section: section(&buf, header.code_section_offset, header.code_section_size),
    });
    assert_ne!(header.content_hash, integrity::NO_HASH);
    assert_eq!(header.content_hash, expected_content);

    let debug = section(&buf, header.debug_section_offset, header.debug_section_size);
    assert_ne!(debug.len(), 0);
    assert_ne!(header.debug_hash, integrity::NO_HASH);
    assert_eq!(header.debug_hash, integrity::debug_hash(debug));
}

/// The most variable-table slots one generated function takes. Its prologue
/// sets each local, so a function of many more would outgrow the bytecode a
/// line map entry can address before the table filled.
const FUNCTION_SLOTS: usize = 4096;

/// Functions that take `slots` variable-table slots between them, each its
/// `INT` locals and its result, and the statements of `main` that call
/// them, which lay each one out.
fn functions_taking(slots: usize) -> (String, String) {
    let mut functions = String::new();
    let mut calls = String::new();
    let mut left = slots;
    let mut function = 0;
    while left > 0 {
        let taken = left.min(FUNCTION_SLOTS);
        let locals: String = (1..taken).map(|i| format!("x{i} : INT; ")).collect();
        let vars = if locals.is_empty() {
            String::new()
        } else {
            format!("VAR {locals}END_VAR")
        };
        functions.push_str(&format!(
            "FUNCTION f{function} : INT {vars} f{function} := 1; END_FUNCTION\n"
        ));
        calls.push_str(&format!("y := f{function}();\n"));
        left -= taken;
        function += 1;
    }
    (functions, calls)
}

/// A program whose variables take `slots` variable-table slots: `main`'s
/// `y`, then the functions it calls.
fn program_taking(slots: usize) -> String {
    let (functions, calls) = functions_taking(slots - 1);
    format!("{functions}PROGRAM main VAR y : INT; END_VAR {calls} END_PROGRAM")
}

/// REQ-CF-codegen-035: a program that takes every slot of the variable
/// table compiles, and `num_variables` counts them all.
#[spec_test(REQ_CF_codegen_035)]
fn container_spec_req_cf_035_when_program_takes_65535_slots_then_num_variables_counts_them() {
    let (_, header) = compiled(&program_taking(65_535));

    assert_eq!(header.num_variables, 65_535);
}

/// REQ-CF-codegen-035: a program that takes one slot more than the
/// variable table holds is P9997. Before, `num_variables` and every index
/// were taken modulo 65,536, so the container's variables aliased.
#[spec_test(REQ_CF_codegen_035)]
fn container_spec_req_cf_035_when_program_takes_65536_slots_then_p9997() {
    let result = try_compiled(&program_taking(65_536), &CompilerOptions::default());

    assert_eq!(result.err().as_deref(), Some("P9997"));
}

/// A program whose functions take `slots` variable-table slots, after
/// `main`'s `y` and `inst`, then the copy slot of the block `Fb`'s one
/// field, then the result slot of its method `m`.
fn program_with_method_after(slots: usize) -> String {
    let (functions, calls) = functions_taking(slots);
    format!(
        "{functions}
         FUNCTION_BLOCK Fb
           VAR a : INT; END_VAR
           METHOD m : INT m := a; END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR y : INT; inst : Fb; END_VAR {calls} y := inst.m(); END_PROGRAM"
    )
}

/// REQ-CF-codegen-035: a function block's copy slots and its methods'
/// slots count toward the table: a method's result slot that is past the
/// last slot of the table is P9997, and one at most at the last compiles.
#[spec_test(REQ_CF_codegen_035)]
fn container_spec_req_cf_035_when_method_result_slot_past_table_then_p9997() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };

    // `a` takes slot 65,532 and `m`'s result slot 65,533.
    assert!(try_compiled(&program_with_method_after(65_530), &options).is_ok());
    // `a` takes slot 65,534, the last, and `m`'s result has none.
    assert_eq!(
        try_compiled(&program_with_method_after(65_532), &options)
            .err()
            .as_deref(),
        Some("P9997")
    );
}
