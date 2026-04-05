use std::path::{Path, PathBuf};

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use ironplc_container::debug_section::{iec_type_tag, VarNameEntry};
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// One-time generator for golden test files. Run with:
/// cargo test -p ironplc-vm-cli --test cli generate_golden -- --ignored --nocapture
#[test]
#[ignore]
fn generate_golden_files() {
    let path = path_to_golden_resource("steel_thread.iplc");
    write_steel_thread_container(&path);
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
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
        0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
        0x30,                   // ADD_I32                  (10 + 32)
        0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
        0xB5,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(10)
        .add_i32_constant(32)
        .add_function(ironplc_container::FunctionId::new(0), &bytecode, 2, 2, 0)
        .build();

    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    std::fs::write(path, &buf).unwrap();
}

#[test]
fn run_when_valid_container_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let container_path = dir.path().join("test.iplc");
    write_steel_thread_container(&container_path);

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&container_path).arg("--scans").arg("1");
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
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
    assert_eq!(contents, "var[0]: 10\nvar[1]: 42\n");

    Ok(())
}

#[test]
fn run_when_file_not_found_then_exit_2_and_v6001() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg("test/file/doesnt/exist.iplc");
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("V6001"));

    Ok(())
}

#[test]
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
    assert_eq!(contents, "var[0]: 10\nvar[1]: 42\n");

    Ok(())
}

#[test]
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

#[test]
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
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (0)
        0x33,                   // DIV_I32                  (10 / 0 → trap)
        0xB5,                   // RET_VOID
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

#[test]
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
        0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push Button)
        0x57,                   // BOOL_NOT                 (NOT Button)
        0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (Buzzer := result)
        0xB5,                   // RET_VOID
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

#[test]
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

#[test]
fn run_when_dump_vars_without_path_then_prints_to_stdout(
) -> Result<(), Box<dyn std::error::Error>> {
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
        .stdout(predicate::str::contains("var[0]: 10"))
        .stdout(predicate::str::contains("var[1]: 42"));

    Ok(())
}
