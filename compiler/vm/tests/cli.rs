use std::path::{Path, PathBuf};

use assert_cmd::cargo;
use assert_cmd::prelude::*;
use ironplc_container::ContainerBuilder;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// One-time generator for golden test files. Run with:
/// cargo test -p ironplc-vm --test cli generate_golden -- --ignored --nocapture
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
        .add_function(0, &bytecode, 2, 2)
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
fn run_when_file_not_found_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg("test/file/doesnt/exist.iplc");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Unable")));

    Ok(())
}

#[test]
fn run_when_invalid_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let bad_path = dir.path().join("bad.iplc");
    std::fs::write(&bad_path, "this is not a container file")?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("run").arg(&bad_path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unable").or(predicate::str::contains("error")));

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
fn version_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcvm"));
    cmd.arg("version");
    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("ironplcvm version "));

    Ok(())
}
