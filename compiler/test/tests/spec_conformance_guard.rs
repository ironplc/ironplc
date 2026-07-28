//! Workspace-level orphan guard for spec-conformance requirement ownership.
//!
//! Per-crate meta-tests (`all_spec_requirements_have_tests`) each check only
//! the requirements their own crate owns. That leaves one gap: a requirement
//! slugged for a crate that does **not** list the requirement's design doc
//! would be owned by nobody and checked by no meta-test. This guard closes it.
//!
//! Both sides of the check are recovered from files already in the tree — there
//! is no separate manifest to keep in sync:
//!
//! * **Requirements**: every enforced `specs/design/*.md` (a doc listed by some
//!   `build.rs`) is parsed for `**REQ-…**` markers, recovering each
//!   `(doc, slug)`.
//! * **Listings**: every `compiler/*/build.rs` is parsed for the `.md`
//!   filenames it passes to `generate(&[…])`, recovering the owning slug from
//!   the `build.rs` directory name, giving the `(slug, doc)` listing set.
//!
//! The guard asserts that every requirement is slugged and that every
//! `(slug, doc)` a requirement uses is claimed by a listing crate.
//!
//! See `specs/design/cross-crate-spec-conformance.md`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Parsers (pure — fixture-tested below)
// ---------------------------------------------------------------------------

/// Extracts the `.md` filenames listed in `generate(&[ … ])` calls in a
/// `build.rs` source. Only quoted string literals ending in `.md` that appear
/// inside a `generate(&[` … `]` span are returned, so `.md` filenames mentioned
/// in comments are ignored.
fn listed_specs(build_rs: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = build_rs;
    let needle = "generate(&[";
    while let Some(idx) = rest.find(needle) {
        let after = &rest[idx + needle.len()..];
        let end = after.find(']').unwrap_or(after.len());
        let span = &after[..end];

        let mut s = span;
        while let Some(q1) = s.find('"') {
            let after_q = &s[q1 + 1..];
            if let Some(q2) = after_q.find('"') {
                let lit = &after_q[..q2];
                if lit.ends_with(".md") {
                    out.push(lit.to_string());
                }
                s = &after_q[q2 + 1..];
            } else {
                break;
            }
        }
        rest = &after[end..];
    }
    out
}

/// Parses the crate slug out of a requirement ID, anchoring on the ends so a
/// hyphenated slug like `vm-cli` is preserved. Returns `None` for a malformed
/// or unslugged ID (`REQ-CF-001`).
fn req_slug(id: &str) -> Option<String> {
    let body = id.strip_prefix("REQ-")?;
    let parts: Vec<&str> = body.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let area = parts[0];
    let number = parts[parts.len() - 1];
    let slug_parts = &parts[1..parts.len() - 1];

    if area.is_empty()
        || !area
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return None;
    }
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if slug_parts.iter().any(|p| {
        p.is_empty()
            || !p
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    }) {
        return None;
    }
    Some(slug_parts.join("-"))
}

/// Extracts every `**REQ-…**` bold marker (raw ID) from markdown content.
fn req_markers(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = md;
    while let Some(start) = rest.find("**REQ-") {
        let after_open = &rest[start + 2..];
        if let Some(end) = after_open.find("**") {
            let id = &after_open[..end];
            if id.starts_with("REQ-") {
                out.push(id.to_string());
            }
            rest = &after_open[end + 2..];
        } else {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Audit (pure — fixture-tested below)
// ---------------------------------------------------------------------------

/// Checks every requirement against the crate listings. `doc_markers` maps a
/// design-doc filename to the raw requirement IDs it contains; `listings` is
/// the set of `(slug, doc)` pairs a `build.rs` claims. Returns one message per
/// problem (empty when the tree is consistent).
fn audit(
    listings: &BTreeSet<(String, String)>,
    doc_markers: &[(String, Vec<String>)],
) -> Vec<String> {
    let mut problems = Vec::new();
    for (doc, ids) in doc_markers {
        for id in ids {
            match req_slug(id) {
                None => problems.push(format!(
                    "requirement `{id}` in `{doc}` has no crate slug \
                     (expected `REQ-<AREA>-<crate-slug>-<NNN>`)"
                )),
                Some(slug) => {
                    if !listings.contains(&(slug.clone(), doc.clone())) {
                        problems.push(format!(
                            "requirement `{id}` in `{doc}` is owned by slug `{slug}`, \
                             but no crate `{slug}` lists `{doc}` in its build.rs"
                        ));
                    }
                }
            }
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// Live guard over the actual repository
// ---------------------------------------------------------------------------

/// Every enforced requirement is slugged, and every `(slug, doc)` a requirement
/// uses is claimed by a crate that lists the doc.
#[test]
fn every_requirement_slug_is_claimed_by_a_listing_crate() {
    let compiler_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let specs_dir = compiler_dir.join("../specs/design");

    // Listings: (slug, doc) for every compiler/<crate>/build.rs, where the slug
    // is the crate directory name (which equals CARGO_PKG_NAME minus `ironplc-`).
    let mut listings: BTreeSet<(String, String)> = BTreeSet::new();
    let mut listed_docs: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(&compiler_dir)
        .expect("read compiler dir")
        .flatten()
    {
        let dir = entry.path();
        let build_rs = dir.join("build.rs");
        if !build_rs.is_file() {
            continue;
        }
        let slug = dir
            .file_name()
            .expect("crate dir name")
            .to_string_lossy()
            .to_string();
        let src = fs::read_to_string(&build_rs).expect("read build.rs");
        for md in listed_specs(&src) {
            listings.insert((slug.clone(), md.clone()));
            listed_docs.insert(md);
        }
    }

    assert!(
        !listings.is_empty(),
        "orphan guard found no spec listings — did the build.rs layout change?"
    );

    // Requirements: markers in every enforced doc.
    let doc_markers: Vec<(String, Vec<String>)> = listed_docs
        .iter()
        .map(|doc| {
            let content = fs::read_to_string(specs_dir.join(doc)).unwrap_or_default();
            (doc.clone(), req_markers(&content))
        })
        .collect();

    let problems = audit(&listings, &doc_markers);
    assert!(
        problems.is_empty(),
        "spec-conformance orphan guard failed:\n  {}",
        problems.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Fixture-based unit tests (no dependency on live repo state)
// ---------------------------------------------------------------------------

#[test]
fn listed_specs_when_multiline_generate_then_extracts_all() {
    let build_rs = r#"
        fn main() {
            // See specs/design/spec-conformance-testing.md for the design.
            ironplc_spec_requirements_gen::generate(&[
                "bytecode-container-format.md",
                "enumeration-codegen.md",
            ]);
        }
    "#;
    let specs = listed_specs(build_rs);
    assert_eq!(
        specs,
        vec![
            "bytecode-container-format.md".to_string(),
            "enumeration-codegen.md".to_string(),
        ]
    );
}

#[test]
fn listed_specs_when_single_line_then_extracts_one() {
    let build_rs = r#"fn main() { some::generate(&["vm-cli.md"]); }"#;
    assert_eq!(listed_specs(build_rs), vec!["vm-cli.md".to_string()]);
}

#[test]
fn listed_specs_ignores_md_in_comments() {
    // A `.md` filename in a comment is not inside a generate(&[…]) span.
    let build_rs = r#"
        //! See `specs/design/spec-conformance-testing.md`.
        fn main() { generate_other_things(); }
    "#;
    assert!(listed_specs(build_rs).is_empty());
}

#[test]
fn req_slug_parses_single_and_hyphenated_slugs() {
    assert_eq!(req_slug("REQ-EN-codegen-001").as_deref(), Some("codegen"));
    assert_eq!(req_slug("REQ-VC-vm-cli-001").as_deref(), Some("vm-cli"));
}

#[test]
fn req_slug_rejects_unslugged_and_malformed() {
    assert_eq!(req_slug("REQ-CF-001"), None);
    assert_eq!(req_slug("REQ-EN--001"), None);
    assert_eq!(req_slug("REQ-EN-codegen-"), None);
    assert_eq!(req_slug("not-an-id"), None);
}

#[test]
fn req_markers_extracts_bold_ids_only() {
    let md = "Prose **bold** then **REQ-CF-container-001** and **REQ-EN-codegen-010**.";
    assert_eq!(
        req_markers(md),
        vec![
            "REQ-CF-container-001".to_string(),
            "REQ-EN-codegen-010".to_string(),
        ]
    );
}

#[test]
fn audit_passes_when_every_slug_is_listed() {
    let mut listings = BTreeSet::new();
    listings.insert(("codegen".to_string(), "enumeration-codegen.md".to_string()));
    listings.insert((
        "container".to_string(),
        "enumeration-codegen.md".to_string(),
    ));
    let doc_markers = vec![(
        "enumeration-codegen.md".to_string(),
        vec![
            "REQ-EN-codegen-001".to_string(),
            "REQ-EN-container-061".to_string(),
        ],
    )];
    assert!(audit(&listings, &doc_markers).is_empty());
}

#[test]
fn audit_flags_requirement_slug_with_no_listing_crate() {
    // `foo` owns a requirement but no crate lists the doc for `foo`.
    let listings = BTreeSet::new();
    let doc_markers = vec![(
        "enumeration-codegen.md".to_string(),
        vec!["REQ-EN-foo-001".to_string()],
    )];
    let problems = audit(&listings, &doc_markers);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("slug `foo`"), "{}", problems[0]);
}

#[test]
fn audit_flags_unslugged_requirement() {
    let mut listings = BTreeSet::new();
    listings.insert(("codegen".to_string(), "enumeration-codegen.md".to_string()));
    let doc_markers = vec![(
        "enumeration-codegen.md".to_string(),
        vec!["REQ-EN-001".to_string()],
    )];
    let problems = audit(&listings, &doc_markers);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("no crate slug"), "{}", problems[0]);
}
