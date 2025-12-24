use assert_cmd::prelude::*;
use ironplc_test::shared_resource_path;
use predicates::prelude::*;
use std::{path::PathBuf, process::Command};

pub fn path_to_test_resource(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources");
    path.push("test");
    path.push(name);
    path
}

#[test]
fn check_when_not_a_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check").arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    Ok(())
}

#[test]
fn check_when_trace_log_and_not_a_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

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
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check").arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK"));

    Ok(())
}

#[test]
fn check_when_valid_file_8859_encoded_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check")
        .arg(path_to_test_resource("first_steps_8859.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK"));

    Ok(())
}

#[test]
fn check_when_binary_encoded_then_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check")
        .arg(path_to_test_resource("binary_file.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error during analysis"));

    Ok(())
}

#[test]
fn check_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Syntax error"));

    Ok(())
}

#[test]
fn check_when_semantic_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Enumeration uses value"));

    Ok(())
}

#[test]
fn echo_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("echo").arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn echo_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

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
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("echo")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn tokenize_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("tokenize")
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert().success().stdout(predicate::str::contains(
        "Type: EndConfiguration, Value: \'END_CONFIGURATION\', At: Ln 175,Col 0",
    ));

    Ok(())
}

#[test]
fn version_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("version");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("ironplcc version "));

    Ok(())
}

// JSON Export Tests

#[test]
fn export_json_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json").arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("schema_version"))
        .stdout(predicate::str::contains("metadata"))
        .stdout(predicate::str::contains("library"));

    Ok(())
}

#[test]
fn export_json_when_pretty_print_then_formatted() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--pretty")
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("{\n  \"schema_version\""));

    Ok(())
}

#[test]
fn export_json_when_include_comments_then_option_set() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--include-comments")
        .arg("--pretty")
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"include_comments\": true"));

    Ok(())
}

#[test]
fn export_json_when_output_file_then_creates_file() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use tempfile::tempdir;

    let temp_dir = tempdir()?;
    let output_path = temp_dir.path().join("test_output.json");

    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--output")
        .arg(&output_path)
        .arg("--pretty")
        .arg(shared_resource_path("first_steps.st"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!("JSON exported to: {}", output_path.display())));

    // Verify file was created and contains valid JSON
    let content = fs::read_to_string(&output_path)?;
    assert!(content.contains("schema_version"));
    assert!(content.contains("metadata"));
    assert!(content.contains("library"));

    Ok(())
}

#[test]
fn export_json_when_nested_output_dir_then_creates_directories() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use tempfile::tempdir;

    let temp_dir = tempdir()?;
    let output_path = temp_dir.path().join("nested").join("dir").join("test_output.json");

    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--output")
        .arg(&output_path)
        .arg(shared_resource_path("first_steps.st"));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!("JSON exported to: {}", output_path.display())));

    // Verify file was created
    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path)?;
    assert!(content.contains("schema_version"));

    Ok(())
}

#[test]
fn export_json_when_multiple_files_then_combines() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--pretty")
        .arg(shared_resource_path("first_steps.st"))
        .arg(shared_resource_path("first_steps.st")); // Use same file twice for simplicity
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("schema_version"))
        .stdout(predicate::str::contains("elements"));

    Ok(())
}

#[test]
fn export_json_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse library"));

    Ok(())
}

#[test]
fn export_json_when_nonexistent_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json").arg("nonexistent_file.st");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    Ok(())
}

#[test]
fn export_json_when_invalid_output_path_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--output")
        .arg("/root/invalid_path.json") // Should fail on most systems
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to create"));

    Ok(())
}

#[test]
fn export_json_when_location_info_disabled_then_option_set() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("ironplcc")?;

    cmd.arg("export-json")
        .arg("--include-locations=false")
        .arg("--pretty")
        .arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"include_locations\": false"));

    Ok(())
}
