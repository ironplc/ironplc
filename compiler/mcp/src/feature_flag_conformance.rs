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
//! For each flag we keep a [`FlagFixture`]: a single source snippet whose
//! accept/reject outcome is decided by that flag. Same input, one variable
//! changed (the flag), so the outcome is attributable to the flag and nothing
//! else — the strongest form of "this example is supported, this other code is
//! not".
//!
//! Two meta-tests keep the table honest and decoupled:
//! - [`every_feature_flag_has_a_behavioral_fixture`] fails when a new flag is
//!   added without a fixture (the reminder lives in the suite, not a reviewer's
//!   head), mirroring `spec_conformance::all_spec_requirements_have_tests`.
//! - [`every_fixture_references_a_real_feature_flag`] fails when a fixture's key
//!   or prerequisite is a typo or names a removed flag.
//!
//! Adding a flag therefore means adding *your own* fixture row (cohesion), not
//! editing a shared count or a neighbor's assertions (coupling).

use ironplc_parser::options::CompilerOptions;

use crate::tools;
use crate::tools::common::SourceInput;

/// What the fixture's source is expected to do as its flag is toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expectation {
    /// The flag gates real behavior: the source is rejected with the flag off
    /// and accepted with it on. The normal, desired case.
    GatesBehavior,
    /// The flag is declared and surfaced by `list_options`, but the compiler
    /// does not yet enforce it — the construct compiles whether the flag is on
    /// or off. Recorded here so the gap is *visible and tested* rather than
    /// silently assumed to work. If enforcement is later added, the `off` case
    /// starts rejecting, this fixture fails, and it should move to
    /// [`Expectation::GatesBehavior`].
    NotYetEnforced,
}

/// A source snippet that isolates a single feature flag's effect.
struct FlagFixture {
    /// The `option_key` (== `CompilerOptions` field name) this fixture exercises.
    key: &'static str,
    /// Flags that must also be on merely to *reach* the behavior under test —
    /// e.g. `allow_ref_to` before `allow_ref_arithmetic`, whose snippet cannot
    /// parse without `REF_TO` recognized. Held constant across the off/on runs
    /// so that toggling `key` is the only difference.
    prereqs: &'static [&'static str],
    /// Expected accept/reject behavior as `key` toggles.
    expectation: Expectation,
    /// The source under test. Snippets are minimal and analysis-clean so the
    /// only reason the `on` case could fail is the feature itself.
    source: &'static str,
}

use Expectation::{GatesBehavior, NotYetEnforced};

/// One fixture per vendor-extension flag. Order mirrors `FEATURE_DESCRIPTORS`
/// for readability; the suite does not depend on ordering. Snippets are adapted
/// from the compiler's own positive/negative tests (parser and analyzer) so
/// they exercise the real enforcement path.
const FLAG_FIXTURES: &[FlagFixture] = &[
    // `//` line comment is a syntax error under strict IEC; the flag permits it.
    FlagFixture {
        key: "allow_c_style_comments",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nx : INT; // a comment\nEND_VAR\nx := 1;\nEND_PROGRAM",
    },
    // The missing `;` after END_IF is inserted only when the flag is on.
    FlagFixture {
        key: "allow_missing_semicolon",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nx : BOOL;\nEND_VAR\nIF x THEN\nx := FALSE;\nEND_IF\nEND_PROGRAM",
    },
    // Declared-but-not-enforced: reserved problem code P4028 is emitted nowhere,
    // and the grammar accepts a top-level VAR_GLOBAL unconditionally.
    FlagFixture {
        key: "allow_top_level_var_global",
        prereqs: &[],
        expectation: NotYetEnforced,
        source: "VAR_GLOBAL CONSTANT\nX : INT := 250;\nEND_VAR\nPROGRAM p\nEND_PROGRAM",
    },
    // Declared-but-not-enforced: reserved problem code P4029 is emitted nowhere,
    // and the parser accepts a constant in a type parameter unconditionally.
    FlagFixture {
        key: "allow_constant_type_params",
        prereqs: &[],
        expectation: NotYetEnforced,
        source: "VAR_GLOBAL CONSTANT\nSTRING_LENGTH : INT := 250;\nEND_VAR\nFUNCTION_BLOCK fb1\nVAR_INPUT\nSTR : STRING[STRING_LENGTH];\nEND_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nEND_PROGRAM",
    },
    // An empty VAR block is rejected unless the flag is on.
    FlagFixture {
        key: "allow_empty_var_blocks",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nEND_VAR\nEND_PROGRAM",
    },
    // TIME is a type keyword; using it as a function name needs the flag.
    FlagFixture {
        key: "allow_time_as_function_name",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "FUNCTION TIME : INT\nVAR_INPUT x : INT; END_VAR\nTIME := x;\nEND_FUNCTION\nPROGRAM p\nEND_PROGRAM",
    },
    // REF_TO / REF() without full Edition 3. With the flag off, REF_TO is
    // demoted to an identifier and the declaration fails to parse.
    FlagFixture {
        key: "allow_ref_to",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nx : REF_TO INT;\nEND_VAR\nEND_PROGRAM",
    },
    // Arithmetic on a REF_TO type (P2033). Needs REF_TO to parse at all.
    FlagFixture {
        key: "allow_ref_arithmetic",
        prereqs: &["allow_ref_to"],
        expectation: GatesBehavior,
        source: "PROGRAM Main\nVAR\nx : INT;\nr : REF_TO INT := REF(x);\ny : INT;\nEND_VAR\ny := r + 1;\nEND_PROGRAM",
    },
    // REF() on a stack-allocated (function VAR_INPUT) variable (P2029).
    FlagFixture {
        key: "allow_ref_stack_variables",
        prereqs: &["allow_ref_to"],
        expectation: GatesBehavior,
        source: "FUNCTION MyFunc : INT\nVAR_INPUT\ninVal : INT;\nEND_VAR\nVAR\nr : REF_TO INT;\nEND_VAR\nr := REF(inVal);\nMyFunc := 0;\nEND_FUNCTION\nPROGRAM p\nEND_PROGRAM",
    },
    // Assigning between REF_TO of different base types (P2032, type punning).
    FlagFixture {
        key: "allow_ref_type_punning",
        prereqs: &["allow_ref_to"],
        expectation: GatesBehavior,
        source: "PROGRAM Main\nVAR\nx : REAL;\nr : REF_TO INT;\nEND_VAR\nr := REF(x);\nEND_PROGRAM",
    },
    // Integer literal as a BOOL initializer.
    FlagFixture {
        key: "allow_int_to_bool_initializer",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nx : BOOL := 1;\nEND_VAR\nEND_PROGRAM",
    },
    // SIZEOF() operator — registered as a builtin only when the flag is on.
    FlagFixture {
        key: "allow_sizeof",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nx : INT;\ns : DINT;\nEND_VAR\ns := SIZEOF(x);\nEND_PROGRAM",
    },
    // Implicit __SYSTEM_UP_TIME global, seeded only when the flag is on (P4007).
    FlagFixture {
        key: "allow_system_uptime_global",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nt : TIME;\nEND_VAR\nt := __SYSTEM_UP_TIME;\nEND_PROGRAM",
    },
    // Implicit widening across bit-string/integer families: literal 0 -> BYTE
    // arg (P4026) is only allowed when the flag is on.
    FlagFixture {
        key: "allow_cross_family_widening",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "FUNCTION TAKES_BYTE : BYTE\nVAR_INPUT\nx : BYTE;\nEND_VAR\nTAKES_BYTE := x;\nEND_FUNCTION\nPROGRAM main\nVAR\nresult : BYTE;\nEND_VAR\nresult := TAKES_BYTE(0);\nEND_PROGRAM",
    },
    // Partial-access bit syntax `.%Xn` as an alias for `.n` (P4033).
    FlagFixture {
        key: "allow_partial_access_syntax",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "PROGRAM main\nVAR\nb : BYTE;\nr : BOOL;\nEND_VAR\nr := b.%X0;\nEND_PROGRAM",
    },
    // Curly-brace pragma skipped as trivia.
    FlagFixture {
        key: "allow_pragmas",
        prereqs: &[],
        expectation: GatesBehavior,
        source: "{attribute 'qualified_only'}\nPROGRAM p\nEND_PROGRAM",
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

/// Completeness: every feature flag must have a behavioral fixture. Adding a
/// flag with no fixture fails here rather than passing silently.
#[test]
fn every_feature_flag_has_a_behavioral_fixture() {
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        assert!(
            FLAG_FIXTURES.iter().any(|f| f.key == fd.option_key),
            "feature flag `{}` has no fixture in FLAG_FIXTURES. Add a source snippet whose \
             accept/reject outcome is decided by the flag, so the flag is proven to gate real \
             compiler behavior (not just set a struct field).",
            fd.option_key
        );
    }
}

/// Reverse guard: every fixture key/prereq must name a real flag, catching
/// typos and fixtures left behind after a flag is renamed or removed.
#[test]
fn every_fixture_references_a_real_feature_flag() {
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
}

/// Behavior: with prerequisites held constant, toggling each flag produces the
/// expected accept/reject outcome. This is what replaces the count/state
/// coupling — proof the flag gates real behavior (or a recorded gap where it
/// does not yet).
#[test]
fn each_feature_flag_gates_its_example_source() {
    for fx in FLAG_FIXTURES {
        let mut on_flags = fx.prereqs.to_vec();
        on_flags.push(fx.key);

        let off = tools::check::build_response(&sources(fx.source), &ed2_with(fx.prereqs));
        let on = tools::check::build_response(&sources(fx.source), &ed2_with(&on_flags));

        // With the flag on, the construct must always compile — otherwise the
        // fixture is not isolating the feature (some unrelated error leaks in).
        assert!(
            on.ok,
            "flag `{}`: source expected to be ACCEPTED with the flag on but got diagnostics: \
             {:?}\nsource:\n{}",
            fx.key, on.diagnostics, fx.source
        );

        match fx.expectation {
            GatesBehavior => assert!(
                !off.ok,
                "flag `{}`: source expected to be REJECTED with the flag off (prereqs {:?}) but \
                 it compiled cleanly. The fixture no longer isolates the flag.\nsource:\n{}",
                fx.key, fx.prereqs, fx.source
            ),
            NotYetEnforced => assert!(
                off.ok,
                "flag `{}` is recorded as NotYetEnforced, but its source was REJECTED with the \
                 flag off — enforcement appears to have been added. Move this fixture to \
                 `Expectation::GatesBehavior`. diagnostics: {:?}",
                fx.key, off.diagnostics
            ),
        }
    }
}
