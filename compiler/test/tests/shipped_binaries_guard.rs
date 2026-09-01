//! Workspace-level guard that every binary we build is a binary we ship.
//!
//! A binary that is built but not packaged fails no compiler test: it surfaces
//! only as the program missing from an installed IronPLC. This guard catches
//! that in CI instead.
//!
//! Like `spec_conformance_guard.rs`, both sides are recovered from files already
//! in the tree, so there is no new manifest to keep in sync:
//!
//! * **Built**: the `[[bin]]` names declared by each `members` entry of
//!   `compiler/Cargo.toml`.
//! * **Unix packages**: the `binaries := "…"` list in `compiler/justfile`, which
//!   both tar recipes interpolate.
//! * **Windows installer**: `compiler/setup.nsi`, resolved from its `File`
//!   directives *through* its `!define`s — a define that is never installed does
//!   not count as shipped.
//! * **Homebrew**: `libexec.install` and `bin.install_symlink` in
//!   `compiler/homebrew/Formula/ironplc.rb`, checked separately, because a
//!   binary that is installed but not symlinked never reaches the PATH.
//! * **`curl … | sh`**: the `BINARIES="…"` list in `compiler/install.sh`, which
//!   is the only documented Linux install path. A binary the script does not
//!   name is extracted from the archive and then deleted with the temp
//!   directory, so omission here is silent at both build and install time.
//!
//! The guard asserts all five agree. Adding a binary therefore fails here until
//! every installer carries it.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Payloads that ship alongside the executables but are not executables, so the
/// Homebrew install list is not purely a binary list.
const NON_BINARY_PAYLOADS: &[&str] = &["resources", "bom.cdx.json"];

// ---------------------------------------------------------------------------
// Parsers (pure — fixture-tested below)
// ---------------------------------------------------------------------------

/// The string literals inside the first `[…]` list that follows `key` in
/// `text`. Used for TOML arrays and Ruby argument lists alike, both of which
/// quote every element.
fn quoted_items_after(text: &str, key: &str) -> Vec<String> {
    let Some(idx) = text.find(key) else {
        return Vec::new();
    };
    let after = &text[idx + key.len()..];
    let end = after.find(']').unwrap_or(after.len());
    quoted_strings(&after[..end])
}

/// Every double-quoted string literal in `span`, in order.
fn quoted_strings(span: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = span;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else {
            break;
        };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// True when a manifest line is a whole-line comment for `prefix` syntax.
fn is_comment(line: &str, prefix: &str) -> bool {
    line.trim_start().starts_with(prefix)
}

/// The `members` list of a workspace `Cargo.toml`.
fn workspace_members(cargo_toml: &str) -> Vec<String> {
    quoted_items_after(cargo_toml, "members = [")
}

/// The `name` of every `[[bin]]` target declared in a crate's `Cargo.toml`.
/// Comment lines are ignored so the commentary above a `[[bin]]` block cannot
/// contribute a phantom target.
fn bin_names(cargo_toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_bin = false;
    for line in cargo_toml.lines() {
        if is_comment(line, "#") {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_bin = trimmed == "[[bin]]";
            continue;
        }
        if in_bin && trimmed.starts_with("name") {
            if let Some(name) = quoted_strings(trimmed).into_iter().next() {
                out.push(name);
            }
        }
    }
    out
}

/// The whitespace-separated names inside the quoted value assigned to `name` on
/// the first non-comment line that assigns it.
///
/// Both packaging lists have this shape — the justfile's `binaries := "a b c"`
/// and the install script's `BINARIES="a b c"` — so one parser reads both. The
/// assignment operator has to follow the name, so a longer identifier that ends
/// in it (`LEGACY_OPTIONAL_BINARIES`) is not mistaken for it.
fn whitespace_list_assignment(text: &str, name: &str) -> Vec<String> {
    for line in text.lines() {
        if is_comment(line, "#") {
            continue;
        }
        let Some(after_name) = line.trim_start().strip_prefix(name) else {
            continue;
        };
        let operator = after_name.trim_start();
        if !operator.starts_with(":=") && !operator.starts_with('=') {
            continue;
        }
        if let Some(list) = quoted_strings(line).into_iter().next() {
            return list.split_whitespace().map(str::to_string).collect();
        }
    }
    Vec::new()
}

/// The `binaries := "…"` packaging list from the justfile.
fn just_binaries(justfile: &str) -> Vec<String> {
    whitespace_list_assignment(justfile, "binaries")
}

/// The `BINARIES="…"` install list from `install.sh`.
fn install_sh_binaries(install_sh: &str) -> Vec<String> {
    whitespace_list_assignment(install_sh, "BINARIES")
}

/// The `LEGACY_OPTIONAL_BINARIES="…"` list from `install.sh`: the binaries a
/// published release is allowed to predate, whose absence from the downloaded
/// archive warns instead of failing the install.
fn install_sh_legacy_optional_binaries(install_sh: &str) -> Vec<String> {
    whitespace_list_assignment(install_sh, "LEGACY_OPTIONAL_BINARIES")
}

/// The name a `${VAR}` reference points at, if `token` contains exactly one.
fn interpolated_name(token: &str) -> Option<&str> {
    let start = token.find("${")?;
    let after = &token[start + 2..];
    let end = after.find('}')?;
    Some(&after[..end])
}

/// The binaries an NSIS script actually installs: each `File` directive is
/// resolved through the script's `!define`s, then stripped of the
/// `${EXTENSION}` suffix that carries the platform's `.exe`.
fn nsis_installed_binaries(nsi: &str) -> Vec<String> {
    // `!define NAME "value"` → NAME: value
    let mut defines: Vec<(String, String)> = Vec::new();
    for line in nsi.lines() {
        if is_comment(line, ";") {
            continue;
        }
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("!define ") else {
            continue;
        };
        let Some(name) = rest.split_whitespace().next() else {
            continue;
        };
        if let Some(value) = quoted_strings(rest).into_iter().next() {
            defines.push((name.to_string(), value));
        }
    }

    let mut out = Vec::new();
    for line in nsi.lines() {
        if is_comment(line, ";") {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with("File ") {
            continue;
        }
        let Some(arg) = quoted_strings(trimmed).into_iter().next() else {
            continue;
        };
        // Only artifacts from the build output are binaries; LICENSE and the
        // resources tree come from elsewhere in the source tree.
        if !arg.contains("${ARTIFACTSDIR}") {
            continue;
        }
        // The file component carries the `${SOMEFILE}` reference.
        let Some(file_part) = arg.rsplit('\\').next() else {
            continue;
        };
        let Some(var) = interpolated_name(file_part) else {
            continue;
        };
        let Some((_, value)) = defines.iter().find(|(name, _)| name == var) else {
            continue;
        };
        out.push(value.replace("${EXTENSION}", ""));
    }
    out
}

/// The executables a Homebrew formula copies into `libexec`, excluding the
/// non-binary payloads that ship in the same tarball.
fn formula_installed_binaries(formula: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in formula.lines() {
        if is_comment(line, "#") {
            continue;
        }
        if line.contains("libexec.install") {
            out.extend(
                quoted_strings(line)
                    .into_iter()
                    .filter(|item| !NON_BINARY_PAYLOADS.contains(&item.as_str())),
            );
        }
    }
    out
}

/// The executables a Homebrew formula symlinks onto the PATH.
fn formula_symlinked_binaries(formula: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in formula.lines() {
        if is_comment(line, "#") {
            continue;
        }
        if line.contains("bin.install_symlink") {
            out.extend(quoted_strings(line));
        }
    }
    out
}

/// The names in `expected` that `actual` is missing, and vice versa, formatted
/// as a problem line for `label` (empty when the two agree).
fn compare(label: &str, expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let missing: Vec<&String> = expected.difference(actual).collect();
    if !missing.is_empty() {
        problems.push(format!("{label} does not ship: {missing:?}"));
    }
    let extra: Vec<&String> = actual.difference(expected).collect();
    if !extra.is_empty() {
        problems.push(format!(
            "{label} ships binaries that are not built: {extra:?}"
        ));
    }
    problems
}

// ---------------------------------------------------------------------------
// Live guard over the actual repository
// ---------------------------------------------------------------------------

/// The `compiler/` directory of the live repository.
fn compiler_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Every `[[bin]]` name declared by a workspace member of `compiler/Cargo.toml`.
fn built_binaries(compiler_dir: &Path) -> BTreeSet<String> {
    let workspace = fs::read_to_string(compiler_dir.join("Cargo.toml")).unwrap();
    let mut built: BTreeSet<String> = BTreeSet::new();
    for member in workspace_members(&workspace) {
        let manifest = compiler_dir.join(&member).join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        built.extend(bin_names(&text));
    }

    assert!(
        !built.is_empty(),
        "shipped-binaries guard found no [[bin]] targets — did the workspace layout change?"
    );
    built
}

/// Every `[[bin]]` target in the workspace is shipped by every packaging path.
#[test]
fn every_built_binary_is_shipped_by_every_installer() {
    let compiler_dir = compiler_dir();
    let built = built_binaries(&compiler_dir);

    let justfile = fs::read_to_string(compiler_dir.join("justfile")).unwrap();
    let nsi = fs::read_to_string(compiler_dir.join("setup.nsi")).unwrap();
    let formula = fs::read_to_string(compiler_dir.join("homebrew/Formula/ironplc.rb")).unwrap();
    let install_sh = fs::read_to_string(compiler_dir.join("install.sh")).unwrap();

    let unix: BTreeSet<String> = just_binaries(&justfile).into_iter().collect();
    let windows: BTreeSet<String> = nsis_installed_binaries(&nsi).into_iter().collect();
    let brew_install: BTreeSet<String> = formula_installed_binaries(&formula).into_iter().collect();
    let brew_path: BTreeSet<String> = formula_symlinked_binaries(&formula).into_iter().collect();
    let curl_sh: BTreeSet<String> = install_sh_binaries(&install_sh).into_iter().collect();

    let mut problems = Vec::new();
    problems.extend(compare(
        "justfile `binaries` (macOS/Linux tarballs)",
        &built,
        &unix,
    ));
    problems.extend(compare("setup.nsi (Windows installer)", &built, &windows));
    problems.extend(compare("Homebrew libexec.install", &built, &brew_install));
    problems.extend(compare("Homebrew bin.install_symlink", &built, &brew_path));
    problems.extend(compare(
        "install.sh `BINARIES` (curl | sh)",
        &built,
        &curl_sh,
    ));

    // A legacy-optional name is one `install.sh` tolerates being absent from an
    // older release's archive. It only reaches the install loop if `BINARIES`
    // also names it, so one that does not is dead configuration that silently
    // grants nothing.
    let legacy: BTreeSet<String> = install_sh_legacy_optional_binaries(&install_sh)
        .into_iter()
        .collect();
    let stray: Vec<&String> = legacy.difference(&curl_sh).collect();
    if !stray.is_empty() {
        problems.push(format!(
            "install.sh `LEGACY_OPTIONAL_BINARIES` names binaries absent from `BINARIES`: {stray:?}"
        ));
    }

    assert!(
        problems.is_empty(),
        "shipped-binaries guard failed (built: {built:?}):\n  {}",
        problems.join("\n  ")
    );
}

/// The reference pages under `docs/reference/`, by file stem.
///
/// The pages live in per-area directories (`compiler/`, `runtime/`, `mcp/`),
/// so the stem — not the path — is what has to match the binary name.
fn reference_page_stems(docs_reference: &Path) -> BTreeSet<String> {
    let mut stems = BTreeSet::new();
    let mut dirs = vec![docs_reference.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rst") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    stems.insert(stem.to_string());
                }
            }
        }
    }
    stems
}

/// Every shipped binary has a reference page named after it.
///
/// The binary name is also a documentation URL — `ironplcvmd` ships as
/// `docs/reference/runtime/ironplcvmd.rst`. Nothing else connects the two, so
/// a rename that updates the `[[bin]]` target and the installers but leaves the
/// page at its old slug produces docs that describe a program no longer
/// installed under that name. This guard makes the page part of the rename.
#[test]
fn every_built_binary_has_a_reference_page() {
    let compiler_dir = compiler_dir();
    let built = built_binaries(&compiler_dir);
    let docs_reference = compiler_dir.join("../docs/reference");

    let stems = reference_page_stems(&docs_reference);
    assert!(
        !stems.is_empty(),
        "reference-page guard found no .rst pages under {} — did the docs layout change?",
        docs_reference.display()
    );

    let missing: Vec<&String> = built.difference(&stems).collect();
    assert!(
        missing.is_empty(),
        "shipped binaries with no docs/reference/**/<name>.rst page: {missing:?}"
    );
}

// ---------------------------------------------------------------------------
// Fixture-based unit tests (no dependency on live repo state)
// ---------------------------------------------------------------------------

#[test]
fn workspace_members_when_multiline_list_then_extracts_all() {
    let toml = r#"
[workspace]
members = [
    "analyzer",
    "vm-cli",
]
resolver = "2"
"#;
    assert_eq!(workspace_members(toml), vec!["analyzer", "vm-cli"]);
}

#[test]
fn bin_names_when_multiple_bins_then_extracts_each() {
    let toml = r#"
[[bin]]
name = "ironplcvm"
path = "src/main.rs"

[[bin]]
name = "ironplcvmd"
path = "src/dap_main.rs"

[dependencies]
name = "not-a-binary"
"#;
    assert_eq!(bin_names(toml), vec!["ironplcvm", "ironplcvmd"]);
}

#[test]
fn bin_names_when_commented_bin_then_ignores_it() {
    let toml = r#"
# [[bin]]
# name = "ghost"
[[bin]]
name = "real"
"#;
    assert_eq!(bin_names(toml), vec!["real"]);
}

#[test]
fn bin_names_when_no_bins_then_empty() {
    assert!(bin_names("[package]\nname = \"lib-only\"\n").is_empty());
}

#[test]
fn just_binaries_when_assignment_present_then_splits_on_whitespace() {
    let justfile = "# binaries := \"commented\"\nbinaries := \"ironplcc ironplcvm\"\n";
    assert_eq!(just_binaries(justfile), vec!["ironplcc", "ironplcvm"]);
}

#[test]
fn just_binaries_when_assignment_missing_then_empty() {
    assert!(just_binaries("compile:\n  cargo build\n").is_empty());
}

#[test]
fn install_sh_binaries_when_assignment_present_then_splits_on_whitespace() {
    let script = "# BINARIES=\"commented\"\nBINARIES=\"ironplcc ironplcvmd\"\n";
    assert_eq!(install_sh_binaries(script), vec!["ironplcc", "ironplcvmd"]);
}

#[test]
fn install_sh_binaries_when_legacy_list_precedes_it_then_reads_the_right_one() {
    // `LEGACY_OPTIONAL_BINARIES` ends in `BINARIES`; a prefix match that did not
    // require the `=` to follow the name would read the legacy list as the
    // install list, and the guard would pass on a script that installs nothing.
    let script = "LEGACY_OPTIONAL_BINARIES=\"ironplcvmd\"\nBINARIES=\"ironplcc ironplcvmd\"\n";
    assert_eq!(install_sh_binaries(script), vec!["ironplcc", "ironplcvmd"]);
    assert_eq!(
        install_sh_legacy_optional_binaries(script),
        vec!["ironplcvmd"]
    );
}

#[test]
fn install_sh_legacy_optional_binaries_when_absent_then_empty() {
    assert!(install_sh_legacy_optional_binaries("BINARIES=\"ironplcc\"\n").is_empty());
}

#[test]
fn nsis_installed_binaries_when_defines_resolve_then_strips_extension() {
    let nsi = r#"
!define APPFILE "ironplcc${EXTENSION}"
!define VMDFILE "ironplcvmd${EXTENSION}"
Section "Program files"
    File "..\LICENSE"
    File "${ARTIFACTSDIR}\${APPFILE}"
    File "${ARTIFACTSDIR}\${VMDFILE}"
SectionEnd
"#;
    assert_eq!(nsis_installed_binaries(nsi), vec!["ironplcc", "ironplcvmd"]);
}

#[test]
fn nsis_installed_binaries_when_define_never_installed_then_omits_it() {
    // A binary that is defined but has no File directive is not shipped, and
    // must not be counted as though it were.
    let nsi = r#"
!define APPFILE "ironplcc${EXTENSION}"
!define VMDFILE "ironplcvmd${EXTENSION}"
    File "${ARTIFACTSDIR}\${APPFILE}"
"#;
    assert_eq!(nsis_installed_binaries(nsi), vec!["ironplcc"]);
}

#[test]
fn nsis_installed_binaries_when_comment_then_ignores_it() {
    let nsi = r#"
!define APPFILE "ironplcc${EXTENSION}"
    ; File "${ARTIFACTSDIR}\${APPFILE}"
"#;
    assert!(nsis_installed_binaries(nsi).is_empty());
}

#[test]
fn formula_installed_binaries_when_resources_present_then_excluded() {
    let formula = r#"
      libexec.install "ironplcc", "ironplcvm", "resources"
      bin.install_symlink libexec/"ironplcc"
"#;
    assert_eq!(
        formula_installed_binaries(formula),
        vec!["ironplcc", "ironplcvm"]
    );
}

#[test]
fn formula_symlinked_binaries_when_several_then_extracts_each() {
    let formula = r#"
      libexec.install "ironplcc", "resources"
      bin.install_symlink libexec/"ironplcc"
      bin.install_symlink libexec/"ironplcvmd"
"#;
    assert_eq!(
        formula_symlinked_binaries(formula),
        vec!["ironplcc", "ironplcvmd"]
    );
}

#[test]
fn compare_when_sets_agree_then_no_problems() {
    let a: BTreeSet<String> = ["x".to_string()].into_iter().collect();
    assert!(compare("label", &a, &a).is_empty());
}

#[test]
fn compare_when_binary_unshipped_then_reports_it() {
    let built: BTreeSet<String> = ["x".to_string(), "y".to_string()].into_iter().collect();
    let shipped: BTreeSet<String> = ["x".to_string()].into_iter().collect();
    let problems = compare("installer", &built, &shipped);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("does not ship"));
    assert!(problems[0].contains('y'));
}

#[test]
fn compare_when_extra_shipped_then_reports_it() {
    let built: BTreeSet<String> = ["x".to_string()].into_iter().collect();
    let shipped: BTreeSet<String> = ["x".to_string(), "z".to_string()].into_iter().collect();
    let problems = compare("installer", &built, &shipped);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("not built"));
}

#[test]
fn interpolated_name_when_no_reference_then_none() {
    assert_eq!(interpolated_name("plain.exe"), None);
    assert_eq!(interpolated_name("${VMFILE}"), Some("VMFILE"));
}
