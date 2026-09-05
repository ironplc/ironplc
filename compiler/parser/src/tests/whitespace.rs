//! Whitespace invariance: IEC 61131-3 is free-format, so wherever the grammar
//! permits optional whitespace the spelling with it must parse to the *same
//! AST* as the spelling without.
//!
//! Asserting that is a matrix -- constructs x positions within each construct
//! x kinds of filler -- and one `#[test]` per cell would be hundreds of
//! near-identical functions. Instead each snippet is authored **once** with a
//! [`GAP`] marker at every position where whitespace is legal, and
//! [`assert_gaps_accepted`] expands it into every variant.
//!
//! Two facts make this work:
//!
//! 1. `SourceSpan` compares equal unconditionally (`dsl::core`), so inserting
//!    whitespace shifts every span in the tree without changing AST equality.
//!    That makes "same AST as the tight spelling" a legal assertion, and a far
//!    stronger one than `is_ok()`.
//! 2. `parser.rs` defines `_ = (whitespace() / comment() / pragma())*`, where
//!    `whitespace()` is `Whitespace / Newline`. Every legal gap therefore
//!    accepts a space, a newline *and* a comment -- see [`FILLERS`].
//!
//! [`assert_gaps_rejected`] is the same machinery inverted, for adjacencies
//! that sit *inside* a single lexical unit (`INT#16`, `T#100ms`,
//! `2024-01-20`), where free-format does not apply and a gap must stay a parse
//! error. That table is the tripwire: if a row starts passing, a grammar
//! change widened the accepted language too far.
//!
//! The accepted table has two halves: rows that pin optional-whitespace rules
//! the grammar has always had, and rows added with the fix for
//! <https://github.com/ironplc/ironplc/issues/1437>, each a spelling that
//! returned P0002 before the variable-reference chain and the dotted qualified
//! paths gained their `_`.

use super::common::*;

/// Marks a position where IEC 61131-3 permits optional whitespace. Chosen
/// because it cannot appear in ST source outside a string literal, so it is
/// unambiguous in a template.
const GAP: char = '·';

/// The strings substituted at a gap. `parser.rs` defines
/// `_ = (whitespace() / comment() / pragma())*`, so a legal gap accepts all of
/// these. Pragmas are excluded because `TokenType::Pragma` exists only when
/// `allow_pragmas` is set (see `xform_collapse_pragmas`).
const FILLERS: [&str; 3] = [" ", "\n", "(* gap *)"];

/// The canonical tight spelling: every marker removed.
fn tight(source: &str) -> String {
    source.replace(GAP, "")
}

/// Replaces only the gap at `index`, removing the rest.
fn fill_one(source: &str, index: usize, filler: &str) -> String {
    let mut seen = 0;
    let mut out = String::with_capacity(source.len() + filler.len());
    for ch in source.chars() {
        if ch == GAP {
            if seen == index {
                out.push_str(filler);
            }
            seen += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Every whitespace variant of `source`: each gap filled on its own with each
/// filler, then all gaps filled at once with each filler. A template with `n`
/// gaps yields `3n + 3` variants.
fn gap_variants(source: &str) -> Vec<String> {
    let count = source.matches(GAP).count();
    assert!(count > 0, "template has no `{GAP}` marker:\n{source}");

    let mut variants = Vec::with_capacity(FILLERS.len() * (count + 1));
    for filler in FILLERS {
        for index in 0..count {
            variants.push(fill_one(source, index, filler));
        }
        variants.push(source.replace(GAP, filler));
    }
    variants
}

/// Asserts every whitespace variant of `template` parses to the same AST as
/// its tight spelling.
fn assert_gaps_accepted(template: &str, wrap: fn(&str) -> String, options: &CompilerOptions) {
    let wrapped = wrap(template);
    let canonical = tight(&wrapped);
    let baseline = parse_program(&canonical, &FileId::default(), options);
    assert!(
        baseline.is_ok(),
        "the tight spelling must parse before widening means anything:\n{canonical}\n{:?}",
        baseline.err()
    );
    let baseline = baseline.unwrap();

    for variant in gap_variants(&wrapped) {
        let actual = parse_program(&variant, &FileId::default(), options);
        assert!(
            actual.is_ok(),
            "whitespace is legal here but the widened spelling was rejected:\n{variant}\n{:?}",
            actual.err()
        );
        assert_eq!(
            actual.unwrap(),
            baseline,
            "the widened spelling parsed to a different AST:\n{variant}"
        );
    }
}

/// Asserts the tight spelling of `template` parses but every widened variant
/// is rejected -- the tokens either side of a gap belong to one lexical unit.
fn assert_gaps_rejected(template: &str, wrap: fn(&str) -> String, options: &CompilerOptions) {
    let wrapped = wrap(template);
    let canonical = tight(&wrapped);
    let baseline = parse_program(&canonical, &FileId::default(), options);
    assert!(
        baseline.is_ok(),
        "the tight spelling must parse for the rejection to be about whitespace:\n{canonical}\n{:?}",
        baseline.err()
    );

    for variant in gap_variants(&wrapped) {
        let actual = parse_program(&variant, &FileId::default(), options);
        assert!(
            actual.is_err(),
            "these tokens are one lexical unit, so a gap must be rejected:\n{variant}"
        );
        assert_eq!(
            actual.unwrap_err().code,
            "P0002",
            "the rejection must be the syntax error for the gap, not an unrelated failure:\n{variant}"
        );
    }
}

// ---------------------------------------------------------------------
// Wrappers. A `#[case]` row carries the construct and nothing else; the
// wrapper supplies whatever POU the construct has to live in.
// ---------------------------------------------------------------------

/// For rows that are already a whole library (`TYPE`, `CONFIGURATION`).
fn verbatim(source: &str) -> String {
    source.to_owned()
}

/// A statement in a program body. Reuses the shared fixture declarations.
fn in_program(body: &str) -> String {
    wrap_program(body)
}

/// A statement in a method body, where `THIS^`/`SUPER^` are meaningful. Same
/// shape as `parse_in_method` in `this_super.rs`.
fn in_method(body: &str) -> String {
    format!(
        "FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    count : INT;
END_VAR
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
    )
}

/// A `VAR_CONFIG` entry. The block is only legal in a `CONFIGURATION`, and
/// only after at least one `RESOURCE`, so the wrapper supplies both.
fn in_var_config(entry: &str) -> String {
    format!(
        "CONFIGURATION config
RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : plc_prg;
END_RESOURCE
VAR_CONFIG
{entry}
END_VAR
END_CONFIGURATION"
    )
}

// ---------------------------------------------------------------------
// Gaps the grammar already permits.
// ---------------------------------------------------------------------

/// Regression cover for the optional-whitespace rules that exist today. Each
/// row names the grammar rule whose `_` it pins, so a rule that loses its `_`
/// fails here rather than silently narrowing the accepted language.
#[rstest]
#[case::array_specification(
    "TYPE A : ARRAY·[·0·..·3·]·OF INT; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::array_dimensions(
    "TYPE A : ARRAY[0..3·,·0..3] OF INT; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::string_length("TYPE S : STRING·[·10·]; END_TYPE", verbatim, CompilerOptions::default)]
#[case::array_element_string_length(
    "TYPE A : ARRAY[0..3] OF STRING·[·10·]; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::array_element_string_paren_length(
    "TYPE A : ARRAY[0..3] OF STRING·(·10·); END_TYPE",
    verbatim,
    opts_with_paren_string_length
)]
#[case::enumeration_values(
    "TYPE E : (·RED·,·GREEN·); END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::subscript_interior("v := grid[·1·,·2·];", in_program, CompilerOptions::default)]
#[case::function_arguments("v := ADD(·1·,·2·);", in_program, CompilerOptions::default)]
#[case::assignment("v·:=·1;", in_program, CompilerOptions::default)]
#[case::unary_operator("v := -·10;", in_program, CompilerOptions::default)]
#[case::binary_operator("v := 1·+·2;", in_program, CompilerOptions::default)]
#[case::statement_separator("v := 1·;·b := 2;", in_program, CompilerOptions::default)]
#[case::if_statement(
    "IF r· THEN v := 1;· ELSE v := 2;· END_IF;",
    in_program,
    CompilerOptions::default
)]
#[case::for_statement(
    "FOR v·:=·0· TO 3· DO b := 1;· END_FOR;",
    in_program,
    CompilerOptions::default
)]
#[case::case_statement(
    "CASE v· OF 1·:·b := 1;· ELSE b := 2;· END_CASE;",
    in_program,
    CompilerOptions::default
)]
#[case::var_declaration(
    "PROGRAM main VAR x·:·INT·:=·1·; END_VAR x := 2; END_PROGRAM",
    verbatim,
    CompilerOptions::default
)]
// `this_super.rs` also spells out THIS/space/comment before the caret, but
// asserts the AST *shape* (that the head is a SelfRef). This row asserts
// invariance instead. It is the one gap in the #1437 family that `self_ref`
// already permitted -- `self_ref_chain` below covers the rest of the same
// construct.
#[case::self_ref_caret("THIS·^.count := 1;", in_method, opts_with_fb_inheritance)]
// ---------------------------------------------------------------------
// Gaps issue #1437 reported as rejected. Each row is a spelling that
// returned P0002 before the grammar was widened.
// ---------------------------------------------------------------------
#[case::structured_field("v := s·.·x;", in_program, CompilerOptions::default)]
#[case::subscript_read("v := arr·[0];", in_program, CompilerOptions::default)]
#[case::subscript_assign("arr·[0] := 5;", in_program, CompilerOptions::default)]
#[case::deref_operator("v := myRef·^;", in_program, CompilerOptions::default)]
#[case::deref_then_field("v := myRef·^·.·field;", in_program, CompilerOptions::default)]
#[case::deref_then_subscript("v := myRef·^·[0];", in_program, CompilerOptions::default)]
#[case::bit_access("v := b·.·0;", in_program, CompilerOptions::default)]
#[case::partial_access("v := b·.·%X0;", in_program, opts_with_partial_access)]
#[case::mixed_chain("v := s·.·inner·[0]·.·field;", in_program, CompilerOptions::default)]
#[case::self_ref_chain("THIS·^·.·count := 1;", in_method, opts_with_fb_inheritance)]
#[case::super_ref_chain("SUPER·^·.·count := 1;", in_method, opts_with_fb_inheritance)]
#[case::access_path(
    "PROGRAM main VAR_ACCESS p : VarName·.·Path : BOOL READ_ONLY; END_VAR v := 1; END_PROGRAM",
    verbatim,
    CompilerOptions::default
)]
#[case::var_config_located(
    "Some·.·Located·.·Item·.·Path AT %QB1 : BYTE;",
    in_var_config,
    CompilerOptions::default
)]
#[case::var_config_fb_init(
    "Some·.·Block·.·Item·.·Path : FB_TYPE := (ELEM := VAL);",
    in_var_config,
    CompilerOptions::default
)]
fn parse_when_gap_filled_then_same_ast(
    #[case] template: &'static str,
    #[case] wrap: fn(&str) -> String,
    #[case] options: fn() -> CompilerOptions,
) {
    assert_gaps_accepted(template, wrap, &options());
}

// ---------------------------------------------------------------------
// Adjacencies inside a single lexical unit -- the tripwire.
// ---------------------------------------------------------------------

/// These tokens spell one lexical unit between them, so free-format does not
/// apply and a gap must stay a parse error. Issue #1437 lists them explicitly
/// as adjacencies a whitespace fix must *not* widen; a row that starts passing
/// is the signal that one did.
///
/// `REF=` is deliberately not a row here. It is a single token, so #1437 could
/// not have widened it, and `tests/reference_to.rs` already owns that
/// assertion -- a row here would only duplicate it.
#[rstest]
#[case::typed_integer("v := INT·#·16;", in_program, CompilerOptions::default)]
#[case::typed_real("v := REAL·#·1.5;", in_program, CompilerOptions::default)]
#[case::typed_bit_string("v := WORD·#·16#FFFF;", in_program, CompilerOptions::default)]
#[case::typed_string(
    "TYPE S : STRING := STRING·#·'a'; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::duration_prefix(
    "TYPE T1 : TIME := T·#·100ms; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::date_prefix(
    "TYPE D1 : DATE := DATE·#·2024-01-20; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::date_internals(
    "TYPE D2 : DATE := DATE#2024·-·01·-·20; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::daytime_internals(
    "TYPE T2 : TOD := TOD#14·:·30·:·20; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::enumerated_value(
    "TYPE E2 : (RED, GREEN) := E2·#·RED; END_TYPE",
    verbatim,
    CompilerOptions::default
)]
#[case::negative_subrange_bound(
    "TYPE R1 : INT (-·10..10); END_TYPE",
    verbatim,
    CompilerOptions::default
)]
fn parse_when_gap_filled_then_rejected(
    #[case] template: &'static str,
    #[case] wrap: fn(&str) -> String,
    #[case] options: fn() -> CompilerOptions,
) {
    assert_gaps_rejected(template, wrap, &options());
}
