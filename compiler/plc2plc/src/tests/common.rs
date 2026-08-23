//! Shared imports and the round-trip assertions every renderer test uses.
//!
//! Rendering alone does not prove the output is usable: a golden-file
//! comparison cannot tell correct output from output the parser rejects. So
//! every test here re-parses what it rendered, and is one of:
//!
//! 1. [`assert_round_trips`] — parse → render → re-parse, same AST; or
//! 2. [`assert_resource_renders_to`] — the same round trip, plus the
//!    rendered text pinned against a committed `*_rendered.st` golden file.
//!
//! [`assert_round_trips_idempotently`] is the escape hatch for renderings
//! that deliberately normalize to a different AST spelling. It still
//! re-parses; it just compares text-to-text instead of AST-to-AST.
//!
//! All three return the rendered text. Add a `contains` assertion on top
//! only for what AST equality cannot see -- identifier casing (`Id` compares
//! case-insensitively) or a spelling the AST does not record.

pub(crate) use std::fs;
pub(crate) use std::path::PathBuf;

pub(crate) use dsl::core::FileId;

pub(crate) use ironplc_parser::options::{CompilerOptions, Dialect};
pub(crate) use ironplc_parser::parse_program;
pub(crate) use ironplc_test::read_shared_resource;

pub(crate) use crate::write_to_string;

pub(crate) fn read_resource(name: &'static str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);

    fs::read_to_string(path.clone()).unwrap_or_else(|_| panic!("Unable to read file {path:?}"))
}

/// The edition-3 dialect options, the most common non-default set.
pub(crate) fn edition3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

/// Parses `source`, renders it, and re-parses the rendering, requiring the
/// re-parse to succeed and to produce the same AST. Returns the rendered
/// text.
///
/// This is the default assertion for a renderer test. The rendering is
/// re-parsed with the *same* options as the source: a rendering that needs a
/// laxer dialect than its source did is a renderer bug.
pub(crate) fn assert_round_trips(source: &str, options: &CompilerOptions) -> String {
    let library_original = parse_program(source, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Source did not parse: {e:?}\n{source}"));
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    assert_eq!(
        library_original, library_rendered,
        "Round trip changed the AST. Rendered:\n{rendered}"
    );

    rendered
}

/// Like [`assert_round_trips`], but for renderings that deliberately
/// normalize to a spelling that re-parses as a *different* AST — a
/// bit-string literal that decimalizes to a `SignedInteger`, or a mixed
/// `VAR` block that renders as one block per declaration.
///
/// The rendering must still re-parse; rendering the re-parsed library must
/// then reproduce byte-identical text, so the normalization is a fixed
/// point rather than a drift. Returns the rendered text.
///
/// Prefer [`assert_round_trips`] — reach for this only when the AST
/// difference is understood and documented at the call site.
pub(crate) fn assert_round_trips_idempotently(source: &str, options: &CompilerOptions) -> String {
    let library_original = parse_program(source, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Source did not parse: {e:?}\n{source}"));
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    let rendered_again = write_to_string(&library_rendered).unwrap();
    assert_eq!(
        rendered, rendered_again,
        "Rendering is not idempotent -- a second pass changed the text."
    );

    rendered
}

/// Round-trips a shared corpus source (per [`assert_round_trips`]) and
/// additionally pins the rendered text against the committed golden file
/// `rendered_name` under `plc2plc/resources/test`.
///
/// Use this when the exact rendered layout is worth freezing. The round trip
/// proves the golden file is *valid* input; the comparison proves it has not
/// changed.
pub(crate) fn assert_resource_renders_to(
    source_name: &'static str,
    rendered_name: &'static str,
    options: &CompilerOptions,
) -> String {
    let source = read_shared_resource(source_name);
    let rendered = assert_round_trips(&source, options);
    assert_eq!(rendered, read_resource(rendered_name));
    rendered
}

/// Renders an in-memory library (one built by hand rather than parsed),
/// requiring the rendering to parse and to survive a further render
/// unchanged. Returns the rendered text.
///
/// A hand-built library has no source to compare against, so this is the
/// round trip's other half: proof that what the renderer emits for that AST
/// shape is text the parser accepts.
pub(crate) fn assert_library_renders_to_parseable_text(
    library: &dsl::common::Library,
    options: &CompilerOptions,
) -> String {
    let rendered = write_to_string(library).unwrap();

    let reparsed = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    let rendered_again = write_to_string(&reparsed).unwrap();
    assert_eq!(
        rendered, rendered_again,
        "Rendering is not idempotent -- a second pass changed the text."
    );

    rendered
}
