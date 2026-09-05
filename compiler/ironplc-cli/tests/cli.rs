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
        .stderr(predicate::str::contains("Check failed"));

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

    // A source that fails to parse contributes nothing to the echoed
    // stream -- its diagnostics go to stderr like every other command's.
    cmd.arg("echo")
        .arg(shared_resource_path("first_steps_syntax_error.st"));
    cmd.assert()
        .failure()
        .stdout(predicate::str::is_empty())
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

/// Implicit-library activation end to end (`REQ-CL-sources-008`): a realistic
/// TwinCAT solution whose `.plcproj` declares **no** library references, whose
/// `MAIN` calls `BOOL_TO_STRING` — a name TwinCAT provides to every project as
/// a built-in conversion operator. Discovering the `.plcproj` activates the
/// implicit `Tc2_BuiltIns` library, so the check passes with no `--library`
/// flag and no reference in the project file.
#[test]
fn check_when_twincat_solution_has_no_references_then_builtins_resolve(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(path_to_test_resource("twincat_builtins_solution"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Negative control for the test above, and the explicit-activation channel:
/// the same call as bare source fails (libraries are dormant by default,
/// `REQ-CL-analyzer-001` — no project context means no implicit activation),
/// then passes with `--library Tc2_BuiltIns` (`REQ-CL-sources-006`).
#[test]
fn check_when_bare_source_calls_bool_to_string_then_requires_library_flag(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("main.st");
    std::fs::write(
        &source,
        "PROGRAM main VAR s : STRING; END_VAR s := BOOL_TO_STRING(TRUE); END_PROGRAM",
    )?;

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check").arg(&source);
    cmd.assert().failure();

    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));
    cmd.arg("check")
        .arg("--library")
        .arg("Tc2_BuiltIns")
        .arg(&source);
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// `Tc2_Math` activation from the project file alone: a realistic TwinCAT
/// solution whose `.plcproj` declares a `<PlaceholderReference>` to
/// `Tc2_Math`, whose `MAIN` calls all four library functions (`LTRUNC`,
/// `LMOD`, `MODABS`, `FRAC`). The check passes with no `--library` flag —
/// activation comes from the reference, matching how real TwinCAT projects
/// state their `Tc2_Math` dependency.
#[test]
fn check_when_twincat_solution_references_tc2_math_then_ok(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(path_to_test_resource("twincat_tc2_math_solution"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Negative control for the test above: the identical solution with the
/// `<PlaceholderReference>` removed must fail — `Tc2_Math` is
/// reference-activated only (dormant by default, never implicit), so the
/// four function names no longer resolve.
#[test]
fn check_when_twincat_solution_tc2_math_reference_removed_then_undefined(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    copy_dir_recursive(
        &path_to_test_resource("twincat_tc2_math_solution"),
        temp.path(),
    )?;

    let plcproj = temp
        .path()
        .join("AxisSolution")
        .join("PlcAxis")
        .join("PlcAxis.plcproj");
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
    cmd.assert().failure();

    Ok(())
}

/// End-to-end coverage for TwinCAT `<Method>` elements: a realistic solution
/// whose `FB_Motor.TcPOU` declares `SetSpeed` and `Stop` as sibling
/// `<Method>` elements, and whose `MAIN` calls both. Before the method
/// elements were read, this reported P4046 against a method declared in the
/// very file being checked.
#[test]
fn check_when_twincat_solution_declares_pou_methods_then_ok(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(path_to_test_resource("twincat_method_solution"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// `Tc2_Utilities` activation from the project file alone: a realistic
/// TwinCAT solution whose `.plcproj` declares a `<PlaceholderReference>` to
/// `Tc2_Utilities`, whose `MAIN` calls `LREAL_TO_FMTSTR`. The check passes
/// with no `--library` flag — activation comes from the reference, matching
/// how real TwinCAT projects state their `Tc2_Utilities` dependency.
#[test]
fn check_when_twincat_solution_references_tc2_utilities_then_ok(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(cargo::cargo_bin!("ironplcc"));

    cmd.arg("check")
        .arg("--dialect")
        .arg("twincat")
        .arg(path_to_test_resource("twincat_tc2_utilities_solution"));
    cmd.assert().success().stdout(predicate::str::is_empty());

    Ok(())
}

/// Negative control for the test above: the identical solution with the
/// `<PlaceholderReference>` removed must fail — `Tc2_Utilities` is
/// reference-activated only (dormant by default, never implicit), so
/// `LREAL_TO_FMTSTR` no longer resolves.
#[test]
fn check_when_twincat_solution_tc2_utilities_reference_removed_then_undefined(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    copy_dir_recursive(
        &path_to_test_resource("twincat_tc2_utilities_solution"),
        temp.path(),
    )?;

    let plcproj = temp
        .path()
        .join("FormatSolution")
        .join("PlcFormat")
        .join("PlcFormat.plcproj");
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
    cmd.assert().failure();

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
