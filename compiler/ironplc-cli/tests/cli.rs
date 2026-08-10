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

/// The contributor scenario behind the `Tc2_Math` library: a real TwinCAT
/// project layout whose `.plcproj` references `Tc2_Math`, with source calling
/// `LTRUNC`/`LMOD`/`MODABS`/`FRAC`. Activation comes from the project file
/// alone (`REQ-CL-sources-001`) — no `--library` flag — and both `check` and
/// `compile` succeed, so the intrinsic-bound functions work on the paved path.
#[test]
fn compile_when_plcproj_references_tc2_math_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    std::fs::write(
        temp.path().join("MAIN.st"),
        "PROGRAM MAIN
VAR
    angle : LREAL := 400.56;
    wrapped : LREAL;
    truncated : LREAL;
    fraction : LREAL;
END_VAR
    wrapped := MODABS(angle, 360.0);
    truncated := LTRUNC(angle);
    fraction := FRAC(angle);
END_PROGRAM
",
    )?;
    std::fs::write(
        temp.path().join("project.plcproj"),
        r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="MAIN.st" />
    <PlaceholderReference Include="Tc2_Math">
      <DefaultResolution>Tc2_Math, * (Beckhoff Automation GmbH)</DefaultResolution>
      <Namespace>Tc2_Math</Namespace>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
    )?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check").arg(temp.path());
    cmd.assert().success();

    let output = NamedTempFile::with_suffix(".iplc")?;
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("compile")
        .arg(temp.path())
        .arg("--output")
        .arg(output.path());
    cmd.assert().success();
    assert!(output.path().metadata()?.len() > 0);

    Ok(())
}

/// The check/compile split for a declare-only library function
/// (`REQ-CL-analyzer-007` vs `REQ-CL-codegen-002` at the CLI level):
/// `Tc2_Utilities`' `LREAL_TO_FMTSTR` is declared but unimplemented, so
/// `check` passes — the corpus-check use case — while `compile` of a call
/// fails with P4046 rather than generating wrong code.
#[test]
fn check_passes_but_compile_fails_p4046_when_declare_only_function_called(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = NamedTempFile::with_suffix(".st")?;
    use std::io::Write;
    write!(
        source,
        "PROGRAM main
VAR
    s : STRING[255];
END_VAR
    s := LREAL_TO_FMTSTR(1.5, 2, TRUE);
END_PROGRAM
"
    )?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check")
        .arg("--library")
        .arg("Tc2_Utilities")
        .arg(source.path());
    cmd.assert().success();

    let output = NamedTempFile::with_suffix(".iplc")?;
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("compile")
        .arg("--library")
        .arg("Tc2_Utilities")
        .arg(source.path())
        .arg("--output")
        .arg(output.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("P4046").and(predicate::str::contains("LREAL_TO_FMTSTR")));

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
