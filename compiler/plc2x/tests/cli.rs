use assert_cmd::cargo;
use assert_cmd::prelude::*;
use ironplc_test::shared_resource_path;
use predicates::prelude::*;
use std::{path::PathBuf, process::Command};
use tempfile::NamedTempFile;

pub fn path_to_test_resource(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources");
    path.push("test");
    path.push(name);
    path
}

#[test]
fn check_when_not_a_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check").arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    Ok(())
}

#[test]
fn check_when_trace_log_and_not_a_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("-v")
        .arg("-v")
        .arg("-v")
        .arg("-v")
        .arg("check")
        .arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    Ok(())
}

#[test]
fn check_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check").arg(shared_resource_path("first_steps.st"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn check_when_valid_file_8859_encoded_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check")
        .arg(path_to_test_resource("first_steps_8859.st"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn check_when_binary_encoded_then_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check")
        .arg(path_to_test_resource("binary_file.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error during analysis"));

    Ok(())
}

#[test]
fn check_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Syntax error"));

    Ok(())
}

#[test]
fn check_when_semantic_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Enumeration uses value"));

    Ok(())
}

#[test]
fn echo_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("echo").arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn echo_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("echo")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("Syntax error"))
        .stderr(predicate::str::contains("Expected"));

    Ok(())
}

#[test]
fn echo_when_semantic_error_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    // For echo, we are only asking if we could parse, not if it is semantically
    // valid, so a semantic problem should not be an error.
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("echo")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn tokenize_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("tokenize")
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert().success().stdout(predicate::str::contains(
        "Type: EndConfiguration, Value: \'END_CONFIGURATION\', At: Ln 175,Col 0",
    ));

    Ok(())
}

#[test]
fn compile_when_valid_file_then_creates_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = NamedTempFile::new()?;
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("compile")
        .arg(shared_resource_path("steel_thread.st"))
        .arg("--output")
        .arg(output.path());
    cmd.assert().success().stdout(predicate::str::is_empty());

    assert!(output.path().metadata()?.len() > 0);

    Ok(())
}

#[test]
fn compile_when_short_output_flag_then_creates_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = NamedTempFile::new()?;
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("compile")
        .arg(shared_resource_path("steel_thread.st"))
        .arg("-o")
        .arg(output.path());
    cmd.assert().success().stdout(predicate::str::is_empty());

    assert!(output.path().metadata()?.len() > 0);

    Ok(())
}

#[test]
fn compile_when_syntax_error_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let output = NamedTempFile::new()?;
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("compile")
        .arg(shared_resource_path("first_steps_syntax_error.st"))
        .arg("--output")
        .arg(output.path());
    cmd.assert().failure();

    Ok(())
}

#[test]
fn compile_when_missing_output_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("compile")
        .arg(shared_resource_path("steel_thread.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--output"));

    Ok(())
}

#[test]
fn version_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!());

    cmd.arg("version");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("ironplcc version "));

    Ok(())
}
