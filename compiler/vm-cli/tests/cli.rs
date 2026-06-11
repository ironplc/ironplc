use std::path::{Path, PathBuf};

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use ironplc_container::debug_section::{iec_type_tag, VarNameEntry};
use ironplc_container::{
    ContainerBuilder, FunctionId, InstanceId, ProgramInstanceEntry, TaskEntry, TaskId, TaskType,
    VarIndex,
};
use predicates::prelude::*;
use spec_test_macro::spec_test;
use std::process::Command;
use tempfile::TempDir;

/// Spec-conformance requirements generated from `specs/design/vm-cli.md`.
/// Referenced by `#[spec_test(REQ_VC_NNN)]`. See vm-cli/build.rs.
#[allow(dead_code)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}

/// Meta-test: every requirement in `specs/design/vm-cli.md` has a
/// `#[spec_test(REQ_VC_NNN)]` somewhere in src/ or tests/. The build script
/// populates `UNTESTED` from files it scans.
#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        spec_requirements::UNTESTED
    );
}

/// One-time generator for golden test files. Run with:
/// cargo test -p ironplc-vm-cli --test cli generate_golden -- --ignored --nocapture
///
/// `steel_thread.iplc` is intentionally **not** regenerated here — it
/// is a frozen artifact that exercises the container reader end-to-end.
/// Adding a new entry to this generator is fine; if you ever need to
/// refresh the steel-thread golden, do it from a throwaway script with
/// full awareness of what the format change is. It was last refreshed for
/// the format_version 2 -> 3 string-header/constant-pool encoding bump
/// (ADR-0035); the reader only accepts the current `FORMAT_VERSION`.
#[test]
#[ignore]
fn generate_golden_files() {
    let path = path_to_golden_resource("debug_source_file_table.iplc");
    write_debug_source_file_table_container(&path);
    eprintln!("Generated golden file: {}", path.display());
}

fn path_to_golden_resource(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources");
    path.push("test");
    path.push(name);
    path
}

/// Builds the steel thread container (x := 10; y := x + 32) and writes it to
/// the given path.
fn write_steel_thread_container(path: &Path) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
        0x0C, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
        0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
        0x20,                   // ADD_I32                  (10 + 32)
        0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
        0x8C,                   // RET_VOID
    ];

    use ironplc_container::debug_section::{iec_type_tag, var_section, VarNameEntry};
    use ironplc_container::id_types::{FunctionId, VarIndex};

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(10)
        .add_i32_constant(32)
        .add_function(ironplc_container::FunctionId::new(0), &bytecode, 2, 2, 0)
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(0),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: "x".to_string(),
            type_name: "DINT".to_string(),
        })
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(1),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: "y".to_string(),
            type_name: "DINT".to_string(),
        })
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// Builds a container exercising the new debug section features added in
/// `specs/plans/2026-05-22-debug-source-file-table.md`:
///
/// - `SOURCE_FILE_TABLE` (tag 6) with two entries (`main.st`, `lib.st`)
///   whose `content_hash` fields are real BLAKE3 digests of synthetic
///   source bytes.
/// - A `LINE_MAP` (tag 1) whose entries reference both files via
///   `file_id`, including one entry at the same `(line, column)` as
///   another but with a different `file_id` to exercise the post-PR
///   wire layout.
///
/// The bytecode itself is the same `x := 10; y := x + 32` steel thread,
/// so the example runs end-to-end under `ironplcvm`. Loading the file
/// also exercises the BLAKE3-vs-SHA-256 spec change (`header.source_hash`
/// is gone; bytes 40-71 must be zero), and the backwards-compat read
/// of the original `steel_thread.iplc` is verified by the other vm-cli
/// tests.
fn write_debug_source_file_table_container(path: &Path) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
        0x0C, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
        0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
        0x20,                   // ADD_I32
        0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
        0x8C,                   // RET_VOID
    ];

    use ironplc_container::debug_section::{
        iec_type_tag, var_section, LineMapEntry, SourceFileEntry, VarNameEntry,
    };
    use ironplc_container::id_types::{
        FunctionId, SourceColumn, SourceFileId, SourceLine, VarIndex,
    };

    let main_source =
        b"PROGRAM main\nVAR x, y : DINT; END_VAR\nx := 10;\ny := lib_add(x, 32);\nEND_PROGRAM\n";
    let lib_source = b"FUNCTION lib_add : DINT\nVAR_INPUT a, b : DINT; END_VAR\nlib_add := a + b;\nEND_FUNCTION\n";

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(10)
        .add_i32_constant(32)
        .add_function(ironplc_container::FunctionId::new(0), &bytecode, 2, 2, 0)
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(0),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: "x".to_string(),
            type_name: "DINT".to_string(),
        })
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(1),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: "y".to_string(),
            type_name: "DINT".to_string(),
        })
        // file_id = 0 is the program file, file_id = 1 the library.
        .add_source_file(SourceFileEntry {
            path: "src/main.st".into(),
            content_hash: *blake3::hash(main_source).as_bytes(),
        })
        .add_source_file(SourceFileEntry {
            path: "src/lib.st".into(),
            content_hash: *blake3::hash(lib_source).as_bytes(),
        })
        // LineMap mixing file_ids: the assignment lines are in main.st,
        // the addition is "inlined" from lib.st. The exact line/column
        // numbers are illustrative.
        .add_line_map_entry(LineMapEntry {
            function_id: FunctionId::SCAN,
            bytecode_offset: 0,
            file_id: SourceFileId::new(0),
            source_line: SourceLine::new(3),
            source_column: SourceColumn::new(1),
        })
        .add_line_map_entry(LineMapEntry {
            function_id: FunctionId::SCAN,
            bytecode_offset: 9,
            file_id: SourceFileId::new(1),
            source_line: SourceLine::new(3),
            source_column: SourceColumn::new(1),
        })
        .add_line_map_entry(LineMapEntry {
            function_id: FunctionId::SCAN,
            bytecode_offset: 13,
            file_id: SourceFileId::new(0),
            source_line: SourceLine::new(4),
            source_column: SourceColumn::new(1),
        })
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-003: `run --scans N` runs exactly N rounds then exits 0.
#[spec_test(REQ_VC_003)]
fn run_when_valid_container_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&container_path).arg("--scans").arg("1");
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// REQ-VC-005: `run --dump-vars <PATH>` writes variable values to a file.
#[spec_test(REQ_VC_005)]
fn run_when_valid_container_file_and_dump_vars_then_writes_variables(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    let dump_path = dir.path().join("vars.txt");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&container_path)
        .arg("--dump-vars")
        .arg(&dump_path)
        .arg("--scans")
        .arg("1");
    cmd.assert().success();

    let contents = std::fs::read_to_string(&dump_path)?;
    assert_eq!(contents, "x: 10\ny: 42\n");

    Ok(())
}

/// REQ-VC-001: a missing container file yields V6001 exit 2.
#[spec_test(REQ_VC_001)]
fn run_when_file_not_found_then_exit_2_and_v6001() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg("test/file/doesnt/exist.iplc");
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6001"));

    Ok(())
}

/// REQ-VC-002: a malformed container yields V6002 exit 2.
#[spec_test(REQ_VC_002)]
fn run_when_invalid_file_then_exit_2_and_v6002() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let bad_path = dir.path().join("bad.iplc");
    std::fs::write(&bad_path, "this is not a container file")?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&bad_path);
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6002"));

    Ok(())
}

#[test]
fn run_when_golden_container_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let golden_path = path_to_golden_resource("steel_thread.iplc");
    let dir = TempDir::new()?;
    let dump_path = dir.path().join("vars.txt");

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&golden_path)
        .arg("--dump-vars")
        .arg(&dump_path)
        .arg("--scans")
        .arg("1");
    cmd.assert().success();

    let contents = std::fs::read_to_string(&dump_path)?;
    // Golden file pre-dates this PR's header revisions; the fact that it
    // still loads and runs is itself the backwards-compatibility check —
    // bytes 40-71 (formerly `source_hash`) are silently accepted by the
    // new reader as `reserved_hash_slot`, and every other field offset is
    // unchanged. See `specs/plans/2026-05-22-debug-source-file-table.md`.
    assert_eq!(contents, "x: 10\ny: 42\n");

    Ok(())
}

/// Loads the debug-source-file-table example container (generated by the
/// `generate_golden_files` ignored test) and asserts the new debug
/// section features round-trip through the reader.
#[test]
fn read_when_debug_source_file_table_golden_then_decodes_new_debug_fields() {
    use ironplc_container::id_types::FunctionId;
    use ironplc_container::Container;
    use std::io::Cursor;

    let bytes = std::fs::read(path_to_golden_resource("debug_source_file_table.iplc"))
        .expect("debug_source_file_table.iplc must exist; regenerate via generate_golden_files");
    let container = Container::read_from(&mut Cursor::new(&bytes)).unwrap();

    let debug = container
        .debug_section
        .as_ref()
        .expect("debug section present");

    // SOURCE_FILE_TABLE round-trip
    assert_eq!(debug.source_files.len(), 2);
    assert_eq!(debug.source_files[0].path, "src/main.st");
    assert_eq!(debug.source_files[1].path, "src/lib.st");
    let expected_main = *blake3::hash(
        b"PROGRAM main\nVAR x, y : DINT; END_VAR\nx := 10;\ny := lib_add(x, 32);\nEND_PROGRAM\n",
    )
    .as_bytes();
    let expected_lib =
        *blake3::hash(b"FUNCTION lib_add : DINT\nVAR_INPUT a, b : DINT; END_VAR\nlib_add := a + b;\nEND_FUNCTION\n")
            .as_bytes();
    assert_eq!(debug.source_files[0].content_hash, expected_main);
    assert_eq!(debug.source_files[1].content_hash, expected_lib);

    // LineMap mixes file_ids
    let lm = &debug.line_map;
    assert_eq!(lm.len(), 3);
    assert!(lm.iter().all(|e| e.function_id == FunctionId::SCAN));
    assert!(lm.iter().any(|e| e.file_id.raw() == 0));
    assert!(lm.iter().any(|e| e.file_id.raw() == 1));

    // The new `header.source_hash` is gone; the wire slot must be zero.
    assert_eq!(container.header.reserved_hash_slot, [0u8; 32]);
}

/// REQ-VC-013: `benchmark` prints a JSON object with `scan_us` stats and tasks.
#[spec_test(REQ_VC_013)]
fn benchmark_when_valid_container_then_outputs_json_with_scan_us(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark")
        .arg(&container_path)
        .arg("--cycles")
        .arg("100")
        .arg("--warmup")
        .arg("10");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("scan_us"))
        .stdout(predicate::str::contains("mean"))
        .stdout(predicate::str::contains("stddev"))
        .stdout(predicate::str::contains("p99"))
        .stdout(predicate::str::contains("tasks"));

    Ok(())
}

/// REQ-VC-016: `benchmark` surfaces file-open errors as V6001 exit 2.
#[spec_test(REQ_VC_016)]
fn benchmark_when_file_not_found_then_exit_2_and_v6001() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark").arg("nonexistent.iplc");
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6001"));

    Ok(())
}

/// Builds a container whose program divides by zero: 10 / 0.
fn write_divide_by_zero_container(path: &Path) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (0)
        0x30,                   // DIV_I32                  (10 / 0 → trap)
        0x8C,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(0)
        .add_i32_constant(10)
        .add_i32_constant(0)
        .add_function(ironplc_container::FunctionId::new(0), &bytecode, 2, 0, 0)
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-004: a runtime trap exits 1 with the trap's V-code on stderr.
#[spec_test(REQ_VC_004)]
fn run_when_divide_by_zero_then_exit_1_and_v4001() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("div_zero.iplc");
    write_divide_by_zero_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&container_path).arg("--scans").arg("1");
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("V4001"))
        .stderr(predicate::str::contains("divide by zero"));

    Ok(())
}

#[test]
fn version_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("version");
    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("ironplcvm version "));

    Ok(())
}

/// Builds a container with debug info: two BOOL variables named Button and Buzzer.
/// Program logic: Buzzer := NOT Button (Button defaults to 0/FALSE, so Buzzer = TRUE).
fn write_doorbell_container(path: &Path) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x0C, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push Button)
        0x7B,                   // BOOL_NOT                 (NOT Button)
        0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (Buzzer := result)
        0x8C,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_function(FunctionId::new(0), &bytecode, 1, 2, 0)
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(0),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: 0,
            iec_type_tag: iec_type_tag::BOOL,
            name: "Button".into(),
            type_name: "BOOL".into(),
        })
        .add_var_name(VarNameEntry {
            var_index: VarIndex::new(1),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: 0,
            iec_type_tag: iec_type_tag::BOOL,
            name: "Buzzer".into(),
            type_name: "BOOL".into(),
        })
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-008: with debug info, the dump uses named variables.
#[spec_test(REQ_VC_008)]
fn run_when_debug_info_and_dump_vars_then_shows_named_variables(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("doorbell.iplc");
    let dump_path = dir.path().join("vars.txt");
    write_doorbell_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&container_path)
        .arg("--dump-vars")
        .arg(&dump_path)
        .arg("--scans")
        .arg("1");
    cmd.assert().success();

    let contents = std::fs::read_to_string(&dump_path)?;
    assert_eq!(contents, "Button: FALSE\nBuzzer: TRUE\n");

    Ok(())
}

/// REQ-VC-006: `--dump-vars` without a path writes the dump to stdout.
#[spec_test(REQ_VC_006)]
fn run_when_dump_vars_without_path_then_prints_to_stdout() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&container_path)
        .arg("--scans")
        .arg("1")
        .arg("--dump-vars");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("x: 10"))
        .stdout(predicate::str::contains("y: 42"));

    Ok(())
}

/// Builds a container with a no-op init function and a scan function that
/// assigns `x := 10` then divides by zero. The init runs cleanly under
/// `Vm::start()`; the fault happens inside `run_round`, so the pre-fault
/// variable state is observable via `--dump-vars`.
fn write_fault_with_vars_container(path: &Path) {
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        0x8C,                   // RET_VOID — init is a no-op.
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (0)
        0x30,                   // DIV_I32                  (10 / 0 → trap)
        0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (unreached)
        0x8C,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(10)
        .add_i32_constant(0)
        .add_function(FunctionId::new(0), &init_bytecode, 0, 0, 0)
        .add_function(FunctionId::new(1), &scan_bytecode, 2, 2, 0)
        .init_function_id(FunctionId::new(0))
        .entry_function_id(FunctionId::new(1))
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-007: a runtime trap with `--dump-vars` writes the current variable
/// state before exiting with code 1.
#[spec_test(REQ_VC_007)]
fn run_when_fault_and_dump_vars_then_writes_variables_and_exits_1(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("fault.iplc");
    let dump_path = dir.path().join("vars.txt");
    write_fault_with_vars_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&container_path)
        .arg("--dump-vars")
        .arg(&dump_path)
        .arg("--scans")
        .arg("1");
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("V4001"));

    let contents = std::fs::read_to_string(&dump_path)?;
    // x was stored before the fault; y was never stored.
    assert_eq!(contents, "var[0]: 10\nvar[1]: 0\n");

    Ok(())
}

/// REQ-VC-010: an unreachable dump path returns V6004 with exit code 2.
#[spec_test(REQ_VC_010)]
fn run_when_dump_path_in_nonexistent_directory_then_exit_2_and_v6004(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    // A parent directory that doesn't exist → File::create fails.
    let dump_path = dir.path().join("no_such_subdir").join("vars.txt");

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run")
        .arg(&container_path)
        .arg("--dump-vars")
        .arg(&dump_path)
        .arg("--scans")
        .arg("1");
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6004"));

    Ok(())
}

/// REQ-VC-016: `benchmark` surfaces malformed-container errors as V6002/exit 2.
#[spec_test(REQ_VC_016)]
fn benchmark_when_invalid_file_then_exit_2_and_v6002() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let bad_path = dir.path().join("bad.iplc");
    std::fs::write(&bad_path, "this is not a container file")?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark").arg(&bad_path);
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6002"));

    Ok(())
}

/// Builds a container whose init is a no-op but whose scan function divides
/// by zero. The fault therefore occurs inside `run_round`, not `start()`,
/// which is the path used by `benchmark`'s warmup and measured loops.
fn write_scan_divide_by_zero_container(path: &Path) {
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![0x8C]; // RET_VOID
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (0)
        0x30,                   // DIV_I32                  (10 / 0 → trap)
        0x8C,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(0)
        .add_i32_constant(10)
        .add_i32_constant(0)
        .add_function(FunctionId::new(0), &init_bytecode, 0, 0, 0)
        .add_function(FunctionId::new(1), &scan_bytecode, 2, 0, 0)
        .init_function_id(FunctionId::new(0))
        .entry_function_id(FunctionId::new(1))
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-017: a trap during the benchmark warmup phase exits 1 with the trap's V-code.
#[spec_test(REQ_VC_017)]
fn benchmark_when_fault_during_warmup_then_exit_1() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("scan_div_zero.iplc");
    write_scan_divide_by_zero_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark")
        .arg(&container_path)
        .arg("--warmup")
        .arg("5")
        .arg("--cycles")
        .arg("10");
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("V4001"));

    Ok(())
}

/// REQ-VC-017: a trap during the measured phase (warmup=0) exits 1 with the trap's V-code.
#[spec_test(REQ_VC_017)]
fn benchmark_when_fault_during_measured_then_exit_1() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("scan_div_zero.iplc");
    write_scan_divide_by_zero_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark")
        .arg(&container_path)
        .arg("--warmup")
        .arg("0")
        .arg("--cycles")
        .arg("5");
    cmd.assert()
        .code(1)
        .stderr(predicate::str::contains("V4001"));

    Ok(())
}

/// REQ-VC-014: with `--cycles 0 --warmup 0`, `benchmark` still emits valid
/// JSON — `scan_us` stats are zero and no samples were measured.
#[spec_test(REQ_VC_014)]
fn benchmark_when_zero_cycles_then_outputs_zero_stats() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark")
        .arg(&container_path)
        .arg("--warmup")
        .arg("0")
        .arg("--cycles")
        .arg("0");
    let output = cmd.assert().success().get_output().stdout.clone();
    let json: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(json["cycles"], 0);
    assert_eq!(json["warmup"], 0);
    // With zero samples, max and p99 come from `unwrap_or(0.0)` / the empty
    // percentile guard. mean and stddev are NaN (serialised as null).
    assert_eq!(json["scan_us"]["p99"], 0.0);
    assert_eq!(json["scan_us"]["max"], 0.0);

    Ok(())
}

/// Builds a container with an explicit cyclic task at `interval_us`.
/// The program is a no-op (RET_VOID) so run_round is cheap.
fn write_cyclic_task_container(path: &Path, interval_us: u64) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x8C,                   // RET_VOID
    ];

    let task = TaskEntry {
        task_id: TaskId::DEFAULT,
        priority: 0,
        task_type: TaskType::Cyclic,
        flags: 0x01, // enabled
        interval_us,
        single_var_index: VarIndex::NO_SINGLE_VAR,
        watchdog_us: 0,
        input_image_offset: 0,
        output_image_offset: 0,
        reserved: [0; 4],
    };
    let program = ProgramInstanceEntry {
        instance_id: InstanceId::DEFAULT,
        task_id: TaskId::DEFAULT,
        entry_function_id: FunctionId::new(0),
        var_table_offset: 0,
        var_table_count: 0,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        init_function_id: FunctionId::new(0),
    };

    let container = ContainerBuilder::new()
        .num_variables(0)
        .add_function(FunctionId::new(0), &bytecode, 0, 0, 0)
        .add_task(task)
        .add_program_instance(program)
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

/// REQ-VC-015: `benchmark` emits per-cyclic-task `budget_pct` when the task's
/// interval is non-zero.
#[spec_test(REQ_VC_015)]
fn benchmark_when_cyclic_task_then_budget_pct_in_output() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = TempDir::new()?;
    let container_path = dir.path().join("cyclic.iplc");
    // 10 ms interval — non-zero so the budget_pct branch fires.
    write_cyclic_task_container(&container_path, 10_000);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("benchmark")
        .arg(&container_path)
        .arg("--warmup")
        .arg("1")
        .arg("--cycles")
        .arg("5");
    let output = cmd.assert().success().get_output().stdout.clone();
    let json: serde_json::Value = serde_json::from_slice(&output)?;
    let tasks = json["tasks"]
        .as_array()
        .expect("tasks must be a JSON array");
    assert!(!tasks.is_empty(), "expected at least one task entry");
    let task = &tasks[0];
    assert_eq!(task["task_type"], "Cyclic");
    let budget = &task["budget_pct"];
    assert!(budget.is_object(), "expected budget_pct object: {task}");
    assert!(budget["mean"].is_number());
    assert!(budget["p99"].is_number());
    assert!(budget["max"].is_number());

    Ok(())
}

/// REQ-VC-012: `run` sleeps between rounds for a cyclic task — two rounds with
/// a 20 ms interval must take at least one interval of wall-clock time.
#[spec_test(REQ_VC_012)]
fn run_when_cyclic_task_then_sleeps_between_rounds() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("cyclic.iplc");
    let interval_us: u64 = 20_000; // 20 ms
    write_cyclic_task_container(&container_path, interval_us);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&container_path).arg("--scans").arg("2");
    let start = std::time::Instant::now();
    cmd.assert().success();
    let elapsed = start.elapsed();

    // Spawning a cargo binary has non-trivial overhead, so the exact wall-clock
    // depends on the host. We just assert the run took at least a single
    // interval — proof that `next_due_us`-driven sleep was exercised at least
    // once. A busy-loop would finish in microseconds.
    let interval = std::time::Duration::from_micros(interval_us);
    assert!(
        elapsed >= interval,
        "expected at least one cyclic interval ({interval:?}) of wall-clock, got {elapsed:?}"
    );

    Ok(())
}

/// REQ-VC-011: without `--scans`, `run` loops until SIGINT then exits 0.
/// Unix-only: we send SIGINT via `kill(2)` after giving the child time to
/// install the ctrlc handler and enter the main loop.
#[cfg(unix)]
#[spec_test(REQ_VC_011)]
fn run_without_scans_then_stops_on_sigint() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::{Command as ProcessCommand, Stdio};
    use std::time::Duration;

    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    // Use a cyclic container so the loop sleeps between rounds — that gives the
    // kill(2) call a deterministic window to be observed.
    write_cyclic_task_container(&container_path, 10_000);

    let mut child = ProcessCommand::new(cargo::cargo_bin!("ironplcvm"))
        .arg("run")
        .arg(&container_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Give the child time to install its ctrlc handler and enter the scan loop.
    std::thread::sleep(Duration::from_millis(300));

    let pid = child.id();
    let kill_status = ProcessCommand::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()?;
    assert!(kill_status.success(), "failed to signal child");

    let status = child.wait()?;
    assert!(
        status.success(),
        "expected clean exit 0 after SIGINT, got {status:?}"
    );

    Ok(())
}
