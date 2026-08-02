//! Build-script helper that generates `spec_requirements.rs` from requirement
//! markers in design spec markdown files.
//!
//! Markers are bold inline IDs of the form `**REQ-<AREA>-<crate-slug>-<NNN>**`
//! (e.g. `**REQ-EN-codegen-001**`). Every ID carries a mandatory lowercase
//! crate slug between the uppercase area code and the trailing number; the
//! slug names the crate that owns the requirement's conformance test. The
//! unslugged form (`**REQ-EN-001**`) is rejected at build time — a listed doc
//! containing one panics this generator.
//!
//! The generator is *crate-aware*: it derives the current crate's slug from
//! `CARGO_PKG_NAME` and is accountable only for the requirements whose slug
//! matches. Several crates may therefore list the same `.md`, each owning and
//! testing a disjoint slugged subset. The generated file contains one constant
//! per owned requirement, an `ALL` slice of the owned requirements, and an
//! `UNTESTED` slice of owned requirements that have no corresponding
//! `#[spec_test]` attribute in this crate's source tree (`src/` and `tests/`).
//!
//! See `specs/design/cross-crate-spec-conformance.md`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// A parsed requirement ID split into its three fields.
///
/// `REQ-VC-vm-cli-001` → area `VC`, slug `vm-cli`, number `001`. The slug may
/// itself contain hyphens (`vm-cli`), so parsing anchors on the ends: the
/// leading uppercase run is the area, the trailing digit run is the number,
/// and the lowercase text in between is the crate slug.
#[derive(Debug, PartialEq, Eq)]
struct ReqId {
    area: String,
    slug: String,
    number: String,
}

/// Parses a raw requirement ID (`REQ-…`) into `(area, slug, number)`.
///
/// Returns `None` when the ID is malformed or — the case that matters most —
/// carries no crate slug (`REQ-CF-001`), so callers can reject it.
fn parse_req_id(raw: &str) -> Option<ReqId> {
    let body = raw.strip_prefix("REQ-")?;
    let parts: Vec<&str> = body.split('-').collect();
    // area + at least one slug segment + number.
    if parts.len() < 3 {
        return None;
    }
    let area = parts[0];
    let number = parts[parts.len() - 1];
    let slug_parts = &parts[1..parts.len() - 1];

    // Area: a non-empty run of uppercase letters/digits.
    if area.is_empty()
        || !area
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return None;
    }
    // Number: a non-empty run of digits.
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Slug: one or more non-empty lowercase (or digit) segments.
    if slug_parts.iter().any(|p| {
        p.is_empty()
            || !p
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    }) {
        return None;
    }

    Some(ReqId {
        area: area.to_string(),
        slug: slug_parts.join("-"),
        number: number.to_string(),
    })
}

/// The crate slug that owns a requirement's test, i.e. the slug embedded in its
/// ID. There is no fallback: an ID that does not parse has no owner.
fn owner(raw: &str) -> Option<String> {
    parse_req_id(raw).map(|r| r.slug)
}

/// Generates `spec_requirements.rs` in `OUT_DIR` from the given list of spec
/// markdown filenames (relative to `specs/design/`).
///
/// Intended to be the sole call from a crate's `build.rs`:
///
/// ```ignore
/// fn main() {
///     ironplc_spec_requirements_gen::generate(&["my-spec.md"]);
/// }
/// ```
///
/// # Panics
///
/// Panics if a listed document contains a `**REQ-…**` marker with no crate
/// slug (the unslugged form is no longer valid).
pub fn generate(spec_files: &[&str]) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let specs_dir = Path::new(&manifest_dir).join("../../specs/design");

    // The current crate's slug — `ironplc-vm-cli` → `vm-cli`. This is the set
    // of requirements this crate is accountable for.
    let pkg_name = env::var("CARGO_PKG_NAME").unwrap();
    let my_slug = pkg_name
        .strip_prefix("ironplc-")
        .unwrap_or(&pkg_name)
        .to_string();

    let mut requirements = BTreeSet::new();

    for filename in spec_files {
        let path = specs_dir.join(filename);
        println!("cargo:rerun-if-changed={}", path.display());

        if let Ok(content) = fs::read_to_string(&path) {
            extract_requirements(&content, filename, &mut requirements);
        }
    }

    let mut tested = BTreeSet::new();
    for dir in ["src", "tests"] {
        let scan_dir = Path::new(&manifest_dir).join(dir);
        for path in collect_rs_files(&scan_dir) {
            println!("cargo:rerun-if-changed={}", path.display());
            if let Ok(content) = fs::read_to_string(&path) {
                extract_tested_requirements(&content, &mut tested);
            }
        }
    }

    // This crate owns only the requirements whose slug matches its own.
    let owned: Vec<&String> = requirements
        .iter()
        .filter(|r| owner(r).as_deref() == Some(my_slug.as_str()))
        .collect();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("spec_requirements.rs");

    let mut code = String::from("// Auto-generated from specs/design/*.md — do not edit.\n\n");

    for req in &owned {
        let ident = req.replace('-', "_");
        // The ident carries the lowercase crate slug (e.g. `REQ_EN_codegen_001`),
        // so silence `non_upper_case_globals` in addition to `dead_code`.
        code.push_str(&format!(
            "#[allow(dead_code, non_upper_case_globals)] pub const {ident}: &str = \"{req}\";\n"
        ));
    }

    code.push('\n');
    code.push_str("#[allow(dead_code)]\npub const ALL: &[&str] = &[\n");
    for req in &owned {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    let untested: Vec<&&String> = owned
        .iter()
        .filter(|r| {
            let ident = r.replace('-', "_");
            !tested.contains(&ident)
        })
        .collect();

    code.push('\n');
    code.push_str("#[allow(dead_code)]\npub const UNTESTED: &[&str] = &[\n");
    for req in &untested {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    fs::write(&dest, code).unwrap();
}

/// Collects all `.rs` files under a directory, recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Extracts all `**REQ-<AREA>-<slug>-<NNN>**` bold markers from markdown
/// content, validating that each carries a crate slug.
///
/// # Panics
///
/// Panics — naming the offending marker and `filename` — if a `**REQ-…**`
/// marker does not parse to a slugged ID. This is the mechanism that removes
/// the single-crate (unslugged) form: an unslugged requirement fails the build
/// of any crate that lists its document.
fn extract_requirements(content: &str, filename: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    while let Some(start) = rest.find("**REQ-") {
        let after_open = &rest[start + 2..]; // skip opening **
        if let Some(end) = after_open.find("**") {
            let id = &after_open[..end];
            if id.starts_with("REQ-") {
                if parse_req_id(id).is_none() {
                    panic!(
                        "spec requirement `**{id}**` in `{filename}` has no crate slug. \
                         Requirement IDs must be `**REQ-<AREA>-<crate-slug>-<NNN>**` \
                         (e.g. `**REQ-EN-codegen-001**`); the unslugged form is no longer \
                         valid. See specs/design/cross-crate-spec-conformance.md."
                    );
                }
                out.insert(id.to_string());
            }
            rest = &after_open[end + 2..];
        } else {
            break;
        }
    }
}

/// Extracts requirement identifiers from `#[spec_test(REQ_XX_slug_NNN)]`
/// attributes in Rust source. Collects the underscore-form identifiers
/// (e.g. `REQ_EN_codegen_001`).
fn extract_tested_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    let needle = "spec_test(REQ_";
    while let Some(start) = rest.find(needle) {
        let after = &rest[start + "spec_test(".len()..];
        if let Some(end) = after.find(')') {
            let ident = &after[..end];
            if ident.starts_with("REQ_") && ident.len() >= 8 {
                out.insert(ident.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_req_id_when_single_word_slug_then_splits_fields() {
        let parsed = parse_req_id("REQ-EN-codegen-001").unwrap();
        assert_eq!(parsed.area, "EN");
        assert_eq!(parsed.slug, "codegen");
        assert_eq!(parsed.number, "001");
    }

    #[test]
    fn parse_req_id_when_hyphenated_slug_then_slug_keeps_hyphen() {
        let parsed = parse_req_id("REQ-VC-vm-cli-001").unwrap();
        assert_eq!(parsed.area, "VC");
        assert_eq!(parsed.slug, "vm-cli");
        assert_eq!(parsed.number, "001");
    }

    #[test]
    fn parse_req_id_when_unslugged_then_none() {
        // No lowercase middle segment — the removed single-crate form.
        assert_eq!(parse_req_id("REQ-CF-001"), None);
    }

    #[test]
    fn parse_req_id_when_malformed_then_none() {
        assert_eq!(parse_req_id("REQ-"), None);
        assert_eq!(parse_req_id("REQ-EN-codegen-"), None); // empty number
        assert_eq!(parse_req_id("REQ-EN--001"), None); // empty slug segment
        assert_eq!(parse_req_id("NOTREQ-EN-codegen-001"), None);
    }

    #[test]
    fn owner_is_the_requirement_slug() {
        assert_eq!(owner("REQ-EN-codegen-001").as_deref(), Some("codegen"));
        assert_eq!(owner("REQ-VC-vm-cli-001").as_deref(), Some("vm-cli"));
        assert_eq!(owner("REQ-CF-001"), None);
    }

    #[test]
    fn extract_requirements_when_slugged_then_collected() {
        let md = "**REQ-CF-container-001** The header is 256 bytes.\n\
                  **REQ-CF-container-002** Magic.";
        let mut reqs = BTreeSet::new();
        extract_requirements(md, "bytecode-container-format.md", &mut reqs);
        assert_eq!(reqs.len(), 2);
        assert!(reqs.contains("REQ-CF-container-001"));
        assert!(reqs.contains("REQ-CF-container-002"));
    }

    #[test]
    fn extract_requirements_ignores_non_req_bold() {
        let md = "This is **bold text** and **REQ-CF-container-010** is a req.";
        let mut reqs = BTreeSet::new();
        extract_requirements(md, "doc.md", &mut reqs);
        assert_eq!(reqs.len(), 1);
        assert!(reqs.contains("REQ-CF-container-010"));
    }

    #[test]
    #[should_panic(expected = "no crate slug")]
    fn extract_requirements_when_unslugged_marker_then_panics() {
        let md = "**REQ-CF-001** An unslugged marker must be rejected.";
        let mut reqs = BTreeSet::new();
        extract_requirements(md, "bytecode-container-format.md", &mut reqs);
    }

    /// A requirement owned by another crate is excluded from this crate's owned
    /// set (and therefore from `UNTESTED`), so several crates can list one doc.
    #[test]
    fn owned_filter_excludes_other_crates_requirements() {
        let reqs: BTreeSet<String> = ["REQ-EN-codegen-001", "REQ-EN-container-061"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let owned: Vec<&String> = reqs
            .iter()
            .filter(|r| owner(r).as_deref() == Some("codegen"))
            .collect();
        assert_eq!(owned, vec![&"REQ-EN-codegen-001".to_string()]);
    }

    /// A slugged requirement owned here but untested here lands in `UNTESTED`;
    /// one that is tested does not.
    #[test]
    fn untested_contains_owned_requirements_without_a_test() {
        let reqs: BTreeSet<String> = ["REQ-EN-codegen-001", "REQ-EN-codegen-002"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut tested = BTreeSet::new();
        tested.insert("REQ_EN_codegen_001".to_string());

        let untested: Vec<&String> = reqs
            .iter()
            .filter(|r| owner(r).as_deref() == Some("codegen"))
            .filter(|r| !tested.contains(&r.replace('-', "_")))
            .collect();
        assert_eq!(untested, vec![&"REQ-EN-codegen-002".to_string()]);
    }

    #[test]
    fn extract_tested_requirements_collects_idents() {
        let rs = r#"
            #[spec_test(REQ_CF_container_001)]
            fn some_test() {}

            #[spec_test(REQ_IS_container_010)]
            #[ignore]
            fn another_test() {}
        "#;
        let mut tested = BTreeSet::new();
        extract_tested_requirements(rs, &mut tested);
        assert_eq!(tested.len(), 2);
        assert!(tested.contains("REQ_CF_container_001"));
        assert!(tested.contains("REQ_IS_container_010"));
    }
}
