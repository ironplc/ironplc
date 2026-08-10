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
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check").arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));

    Ok(())
}

#[test]
fn check_when_trace_log_and_not_a_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

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
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check").arg(shared_resource_path("first_steps.st"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn check_when_valid_file_8859_encoded_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg(path_to_test_resource("first_steps_8859.st"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn check_when_binary_encoded_then_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg(path_to_test_resource("binary_file.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error during analysis"));

    Ok(())
}

#[test]
fn check_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Syntax error"));

    Ok(())
}

#[test]
fn check_when_semantic_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Enumeration uses value"));

    Ok(())
}

#[test]
fn echo_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("echo").arg(shared_resource_path("first_steps.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn echo_when_syntax_error_file_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

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
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("echo")
        .arg(shared_resource_path("first_steps_semantic_error.st"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("END_CONFIGURATION"));

    Ok(())
}

#[test]
fn tokenize_when_valid_file_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

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
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

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
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("compile")
        .arg(shared_resource_path("steel_thread.st"))
        .arg("-o")
        .arg(output.path());
    cmd.assert().success().stdout(predicate::str::is_empty());

    assert!(output.path().metadata()?.len() > 0);

    Ok(())
}

#[test]
fn compile_when_output_is_input_then_fails_without_modifying_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("input.st");
    let original = std::fs::read(shared_resource_path("steel_thread.st"))?;
    std::fs::write(&source, &original)?;
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("compile").arg(&source).arg("--output").arg(&source);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("P6009"));

    // The source must be preserved byte-for-byte when the write is rejected.
    assert_eq!(std::fs::read(&source)?, original);

    Ok(())
}

#[test]
fn compile_when_output_is_input_via_relative_path_then_fails_without_modifying_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("input.st");
    let original = std::fs::read(shared_resource_path("steel_thread.st"))?;
    std::fs::write(&source, &original)?;
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    // Input is a relative path and output is the absolute path to the same
    // file. Canonicalization must recognize these as the same file.
    cmd.current_dir(temp.path())
        .arg("compile")
        .arg("input.st")
        .arg("--output")
        .arg(&source);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("P6009"));

    assert_eq!(std::fs::read(&source)?, original);

    Ok(())
}

#[test]
fn compile_when_syntax_error_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let output = NamedTempFile::new()?;
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("compile")
        .arg(shared_resource_path("first_steps_syntax_error.st"))
        .arg("--output")
        .arg(output.path());
    cmd.assert().failure();

    Ok(())
}

#[test]
fn compile_when_missing_output_then_err() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("compile")
        .arg(shared_resource_path("steel_thread.st"));
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--output"));

    Ok(())
}

/// Copies a directory tree, used to derive a modified variant of a checked-in
/// fixture without mutating the fixture itself.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// End-to-end validation of the compatibility-library mechanism against a
/// realistic TwinCAT solution layout (`.sln` -> `.tsproj` -> `.plcproj` ->
/// `POUs/*.TcPOU`), modeled on the minimal real project provided on issue
/// #1199 (same structure and `VAR CONSTANT ... := PI/180.0` shape,
/// independently authored logic). The `.plcproj` references `Tc2_System`
/// (`REQ-CL-sources-001`), so `PI` must resolve and fold in the constant
/// initializer (`REQ-CL-analyzer-003`) with no `--library` flag — activation
/// comes from the project file alone.
#[test]
fn check_when_twincat_solution_references_tc2_system_then_ok(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(path_to_test_resource("twincat_tc2_system_solution"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Negative control for the test above: the identical solution with the
/// `<PlaceholderReference>` removed from the `.plcproj` must fail — the
/// `PI/180.0` initializer no longer reduces to a constant (P4038) because
/// `PI` is unknown. Libraries are dormant by default
/// (`REQ-CL-analyzer-001`), so this proves the passing test passes *because
/// of* the library reference, not because `PI` is a compiler builtin.
#[test]
fn check_when_twincat_solution_library_reference_removed_then_pi_undefined(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    copy_dir_recursive(
        &path_to_test_resource("twincat_tc2_system_solution"),
        temp.path(),
    )?;

    let plcproj = temp
        .path()
        .join("TurntableSolution")
        .join("PlcTurntable")
        .join("PlcTurntable.plcproj");
    let content = std::fs::read_to_string(&plcproj)?;
    let start = content
        .find("<PlaceholderReference")
        .expect("fixture .plcproj must contain a PlaceholderReference element");
    let end_tag = "</PlaceholderReference>";
    let end = content
        .find(end_tag)
        .expect("fixture .plcproj must close the PlaceholderReference element")
        + end_tag.len();
    std::fs::write(
        &plcproj,
        format!("{}{}", &content[..start], &content[end..]),
    )?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(temp.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("P4038").and(predicate::str::contains("PI")));

    Ok(())
}

/// A source using TwinCAT's built-in `BOOL_TO_STRING` conversion operator,
/// which IronPLC serves from the bundled `Tc2_BuiltIns` library (never from
/// the compiler tables — ADR-0042 rule 1).
const BOOL_TO_STRING_SOURCE: &str = "PROGRAM main
VAR
    flag : BOOL;
    s : STRING;
END_VAR
    s := BOOL_TO_STRING(flag);
END_PROGRAM
";

/// Discovering a `.plcproj` — the project's explicit statement of the TwinCAT
/// target — auto-activates `Tc2_BuiltIns`, even though the `.plcproj` carries
/// no library reference at all (in TwinCAT the built-in operators belong to
/// no library, so there is nothing to reference).
#[test]
fn check_when_plcproj_discovered_then_tc2_builtins_auto_activates(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("main.st"), BOOL_TO_STRING_SOURCE)?;
    std::fs::write(
        dir.path().join("project.plcproj"),
        r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="main.st" />
  </ItemGroup>
</Project>"#,
    )?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check").arg(dir.path());
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Negative control: the identical source compiled as a bare `.st` file (no
/// `.plcproj`, no `--library`) fails — `Tc2_BuiltIns` is dormant by default,
/// so `BOOL_TO_STRING` does not resolve. This proves the test above passes
/// *because of* the `.plcproj` discovery, not because `BOOL_TO_STRING` is a
/// compiler builtin.
#[test]
fn check_when_bare_st_file_then_bool_to_string_dormant() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("main.st");
    std::fs::write(&source, BOOL_TO_STRING_SOURCE)?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check").arg(&source);
    // P4017: FunctionCallUndeclared -- BOOL_TO_STRING is not in scope.
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("P4017"));

    Ok(())
}

/// Explicit activation for source with no project context: `--library
/// Tc2_BuiltIns` brings the operator surface into scope for `check`.
#[test]
fn check_when_library_flag_tc2_builtins_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("main.st");
    std::fs::write(&source, BOOL_TO_STRING_SOURCE)?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check")
        .arg("--library")
        .arg("Tc2_BuiltIns")
        .arg(&source);
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Explicit activation also carries through `compile`: the library's ST body
/// is compiled like user code into the output container.
#[test]
fn compile_when_library_flag_tc2_builtins_then_creates_output(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("main.st");
    std::fs::write(&source, BOOL_TO_STRING_SOURCE)?;
    let output = NamedTempFile::new()?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("compile")
        .arg("--library")
        .arg("Tc2_BuiltIns")
        .arg(&source)
        .arg("--output")
        .arg(output.path());
    cmd.assert().success().stdout(predicate::str::is_empty());

    assert!(output.path().metadata()?.len() > 0);

    Ok(())
}

#[test]
fn version_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("version");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("ironplcc version "));

    Ok(())
}
