//! Behavioral conformance tests for compiler feature flags.
//!
//! ## Why this exists
//!
//! The `list_options` tool exposes one entry per vendor-extension flag in
//! [`CompilerOptions::FEATURE_DESCRIPTORS`]. It is tempting to test that surface
//! by *counting* flags (`assert_eq!(flags.len(), 16)`) or by asserting a flag's
//! boolean is set. Both couple the test suite to every feature commit — the
//! count must be bumped, or a state assertion added — and neither proves the
//! flag actually *does* anything: a flag can be wired into `CompilerOptions` and
//! `list_options` yet connected to no parser/analyzer rule (a dead flag), and a
//! count/state test passes anyway.
//!
//! ## What this tests instead
//!
//! For each flag we keep a [`FlagFixture`]: a single source snippet that is
//! rejected with the flag off and accepted with the flag on. Same input, one
//! variable changed (the flag), so the accept/reject flip is attributable to
//! that flag and nothing else — the strongest form of "this example is
//! supported, this other code is not".
//!
//! A few flags are declared and surfaced by `list_options` but are not yet
//! enforced by the compiler — the construct they gate compiles whether the flag
//! is on or off, so no off-rejects/on-accepts fixture can exist. Those are
//! listed in [`UNENFORCED`], each tracked by an issue. The gap lives in the
//! issue tracker, not in a special test expectation.
//!
//! Two meta-tests keep the tables honest and decoupled:
//! - [`every_feature_flag_has_a_fixture_or_is_tracked_unenforced`] fails when a
//!   new flag is added without either a fixture or an `UNENFORCED` entry (the
//!   reminder lives in the suite, not a reviewer's head), mirroring
//!   `spec_conformance::all_spec_requirements_have_tests`.
//! - [`fixture_and_unenforced_tables_are_consistent`] fails when an entry names
//!   a flag that does not exist, or when a flag is listed as both fixtured and
//!   unenforced (e.g. enforcement was added but `UNENFORCED` was not updated).
//!
//! Adding a flag therefore means adding *your own* fixture row (cohesion), not
//! editing a shared count or a neighbor's assertions (coupling).

use ironplc_parser::options::CompilerOptions;

use crate::tools;
use crate::tools::common::SourceInput;

/// A source snippet that isolates a single feature flag's effect.
struct FlagFixture {
    /// The `option_key` (== `CompilerOptions` field name) this fixture exercises.
    key: &'static str,
    /// Flags that must also be on merely to *reach* the behavior under test —
    /// e.g. `allow_ref_to` before `allow_ref_arithmetic`, whose snippet cannot
    /// parse without `REF_TO` recognized. Held constant across the off/on runs
    /// so that toggling `key` is the only difference.
    prereqs: &'static [&'static str],
    /// Source rejected with `key` off (prereqs on) and accepted with `key` on.
    source: &'static str,
}

/// Vendor flags that are declared and surfaced by `list_options` but that the
/// compiler does not yet enforce: the construct they gate compiles whether the
/// flag is on or off, so no off-rejects/on-accepts fixture can exist. Each is
/// tracked by an issue. When enforcement lands, add a fixture to
/// [`FLAG_FIXTURES`] and remove the entry here.
///
/// - `allow_constant_type_params`  → ironplc/ironplc#1234
const UNENFORCED: &[&str] = &["allow_constant_type_params"];

/// One fixture per *enforced* vendor-extension flag. Order mirrors
/// `FEATURE_DESCRIPTORS` for readability; the suite does not depend on ordering.
/// Snippets are adapted from the compiler's own positive/negative tests (parser
/// and analyzer) so they exercise the real enforcement path.
const FLAG_FIXTURES: &[FlagFixture] = &[
    // `//` line comment is a syntax error under strict IEC; the flag permits it.
    FlagFixture {
        key: "allow_c_style_comments",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : INT; // a comment\nEND_VAR\nx := 1;\nEND_PROGRAM",
    },
    // The missing `;` after END_IF is inserted only when the flag is on.
    FlagFixture {
        key: "allow_missing_semicolon",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : BOOL;\nEND_VAR\nIF x THEN\nx := FALSE;\nEND_IF\nEND_PROGRAM",
    },
    // An empty VAR block is rejected unless the flag is on.
    FlagFixture {
        key: "allow_empty_var_blocks",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nEND_VAR\nEND_PROGRAM",
    },
    // A VAR_GLOBAL block at the top level (outside CONFIGURATION) is rejected
    // with P4028 unless the flag is on.
    FlagFixture {
        key: "allow_top_level_var_global",
        prereqs: &[],
        source: "VAR_GLOBAL CONSTANT\nX : INT := 250;\nEND_VAR\nPROGRAM p\nEND_PROGRAM",
    },
    // TIME is a type keyword; using it as a function name needs the flag.
    FlagFixture {
        key: "allow_time_as_function_name",
        prereqs: &[],
        source: "FUNCTION TIME : INT\nVAR_INPUT x : INT; END_VAR\nTIME := x;\nEND_FUNCTION\nPROGRAM p\nEND_PROGRAM",
    },
    // REF_TO / REF() without full Edition 3. With the flag off, REF_TO is
    // demoted to an identifier and the declaration fails to parse.
    FlagFixture {
        key: "allow_ref_to",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : REF_TO INT;\nEND_VAR\nEND_PROGRAM",
    },
    // Arithmetic on a REF_TO type (P2033). Needs REF_TO to parse at all.
    FlagFixture {
        key: "allow_ref_arithmetic",
        prereqs: &["allow_ref_to"],
        source: "PROGRAM Main\nVAR\nx : INT;\nr : REF_TO INT := REF(x);\ny : INT;\nEND_VAR\ny := r + 1;\nEND_PROGRAM",
    },
    // REF() on a stack-allocated (function VAR_INPUT) variable (P2029).
    FlagFixture {
        key: "allow_ref_stack_variables",
        prereqs: &["allow_ref_to"],
        source: "FUNCTION MyFunc : INT\nVAR_INPUT\ninVal : INT;\nEND_VAR\nVAR\nr : REF_TO INT;\nEND_VAR\nr := REF(inVal);\nMyFunc := 0;\nEND_FUNCTION\nPROGRAM p\nEND_PROGRAM",
    },
    // Assigning between REF_TO of different base types (P2032, type punning).
    FlagFixture {
        key: "allow_ref_type_punning",
        prereqs: &["allow_ref_to"],
        source: "PROGRAM Main\nVAR\nx : REAL;\nr : REF_TO INT;\nEND_VAR\nr := REF(x);\nEND_PROGRAM",
    },
    // Integer literal as a BOOL initializer.
    FlagFixture {
        key: "allow_int_to_bool_initializer",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : BOOL := 1;\nEND_VAR\nEND_PROGRAM",
    },
    // SIZEOF() operator — registered as a builtin only when the flag is on.
    FlagFixture {
        key: "allow_sizeof",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : INT;\ns : DINT;\nEND_VAR\ns := SIZEOF(x);\nEND_PROGRAM",
    },
    // Implicit __SYSTEM_UP_TIME global, seeded only when the flag is on (P4007).
    FlagFixture {
        key: "allow_system_uptime_global",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nt : TIME;\nEND_VAR\nt := __SYSTEM_UP_TIME;\nEND_PROGRAM",
    },
    // Implicit widening across bit-string/integer families: literal 0 -> BYTE
    // arg (P4026) is only allowed when the flag is on.
    FlagFixture {
        key: "allow_cross_family_widening",
        prereqs: &[],
        source: "FUNCTION TAKES_BYTE : BYTE\nVAR_INPUT\nx : BYTE;\nEND_VAR\nTAKES_BYTE := x;\nEND_FUNCTION\nPROGRAM main\nVAR\nresult : BYTE;\nEND_VAR\nresult := TAKES_BYTE(0);\nEND_PROGRAM",
    },
    // Partial-access bit syntax `.%Xn` as an alias for `.n` (P4033).
    FlagFixture {
        key: "allow_partial_access_syntax",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nb : BYTE;\nr : BOOL;\nEND_VAR\nr := b.%X0;\nEND_PROGRAM",
    },
    // Curly-brace pragma skipped as trivia.
    FlagFixture {
        key: "allow_pragmas",
        prereqs: &[],
        source: "{attribute 'qualified_only'}\nPROGRAM p\nEND_PROGRAM",
    },
    // AND_THEN short-circuit operator. With the flag off, AND_THEN is demoted
    // to an identifier and the expression fails to parse.
    FlagFixture {
        key: "allow_short_circuit_operators",
        prereqs: &[],
        source: "FUNCTION_BLOCK FB_Example\nVAR\na : BOOL;\nb : BOOL;\nresult : BOOL;\nEND_VAR\nresult := a AND_THEN b;\nEND_FUNCTION_BLOCK\nPROGRAM p\nEND_PROGRAM",
    },
    // AT-located variable mixed with a plain variable in one VAR block (P4036).
    FlagFixture {
        key: "allow_mixed_located_var_declarations",
        prereqs: &[],
        source: "FUNCTION_BLOCK FB_Example\nVAR\ntempSensor AT%I* : INT;\nfbComm : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nEND_PROGRAM",
    },
];

/// Wraps snippet text as the single-source input the tools expect.
fn sources(content: &str) -> Vec<SourceInput> {
    vec![SourceInput {
        name: "main.st".into(),
        content: content.into(),
    }]
}

/// Builds an ed2 options object with the given flags enabled.
fn ed2_with(flags: &[&str]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "dialect".into(),
        serde_json::Value::String("iec61131-3-ed2".into()),
    );
    for flag in flags {
        map.insert((*flag).to_string(), serde_json::Value::Bool(true));
    }
    serde_json::Value::Object(map)
}

/// Completeness: every feature flag must have a behavioral fixture, or be listed
/// in `UNENFORCED`. Adding a flag with neither fails here rather than passing
/// silently.
#[test]
fn every_feature_flag_has_a_fixture_or_is_tracked_unenforced() {
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        let has_fixture = FLAG_FIXTURES.iter().any(|f| f.key == fd.option_key);
        let is_unenforced = UNENFORCED.contains(&fd.option_key);
        assert!(
            has_fixture || is_unenforced,
            "feature flag `{}` has no behavioral fixture. Add a source snippet to FLAG_FIXTURES \
             that is rejected with the flag off and accepted with it on. If the flag is declared \
             but not yet enforced, add it to UNENFORCED with a tracking issue instead.",
            fd.option_key
        );
    }
}

/// Consistency guard for both tables: every key/prereq must name a real flag,
/// and a flag must not be both fixtured and listed as unenforced (which happens
/// if enforcement is added but `UNENFORCED` is not updated).
#[test]
fn fixture_and_unenforced_tables_are_consistent() {
    let is_flag = |key: &str| {
        CompilerOptions::FEATURE_DESCRIPTORS
            .iter()
            .any(|fd| fd.option_key == key)
    };

    for fx in FLAG_FIXTURES {
        assert!(
            is_flag(fx.key),
            "fixture key `{}` matches no FEATURE_DESCRIPTOR (typo or removed flag?)",
            fx.key
        );
        for prereq in fx.prereqs {
            assert!(
                is_flag(prereq),
                "fixture `{}` lists unknown prerequisite `{}`",
                fx.key,
                prereq
            );
        }
    }

    for key in UNENFORCED {
        assert!(
            is_flag(key),
            "UNENFORCED lists `{key}`, which matches no FEATURE_DESCRIPTOR (typo or removed flag?)"
        );
        assert!(
            !FLAG_FIXTURES.iter().any(|f| f.key == *key),
            "`{key}` is listed in UNENFORCED but also has a behavioral fixture. If it is now \
             enforced, remove it from UNENFORCED; otherwise remove the fixture."
        );
    }
}

/// Behavior: with prerequisites held constant, each flag flips its example
/// source from rejected (off) to accepted (on). This is what replaces the
/// count/state coupling — proof the flag gates real behavior.
#[test]
fn each_feature_flag_gates_its_example_source_off_then_on() {
    for fx in FLAG_FIXTURES {
        let mut on_flags = fx.prereqs.to_vec();
        on_flags.push(fx.key);

        let off = tools::check::build_response(&sources(fx.source), &ed2_with(fx.prereqs));
        assert!(
            !off.ok,
            "flag `{}`: source expected to be REJECTED with the flag off (prereqs {:?}) but it \
             compiled cleanly. The fixture no longer isolates the flag.\nsource:\n{}",
            fx.key, fx.prereqs, fx.source
        );

        let on = tools::check::build_response(&sources(fx.source), &ed2_with(&on_flags));
        assert!(
            on.ok,
            "flag `{}`: source expected to be ACCEPTED with the flag on but got diagnostics: \
             {:?}\nsource:\n{}",
            fx.key, on.diagnostics, fx.source
        );
    }
}
