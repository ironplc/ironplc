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
//! Every feature flag must have a fixture — there is no escape hatch for
//! "declared but not yet enforced". A flag that is surfaced by `list_options`
//! but gates no behavior is a dead flag, and the suite fails until it either
//! gains a fixture (enforcement lands) or is removed.
//!
//! Two meta-tests keep the table honest and decoupled:
//! - [`every_feature_flag_has_a_fixture`] fails when a new flag is added without
//!   a fixture (the reminder lives in the suite, not a reviewer's head),
//!   mirroring `spec_conformance::all_spec_requirements_have_tests`.
//! - [`fixture_keys_name_real_flags`] fails when a fixture entry names a flag or
//!   prerequisite that does not exist (typo or removed flag).
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

/// One fixture per vendor-extension flag. Order mirrors
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
    // A constant reference in a type parameter (STRING length) is rejected with
    // P4029 unless the flag is on, which resolves it to the constant's value. A
    // POU-local VAR CONSTANT is used so the fixture needs no top-level-var-global
    // prerequisite.
    FlagFixture {
        key: "allow_constant_type_params",
        prereqs: &[],
        source: "FUNCTION_BLOCK fb1\nVAR CONSTANT\nSTRING_LENGTH : INT := 250;\nEND_VAR\nVAR_INPUT\nSTR : STRING[STRING_LENGTH];\nEND_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nEND_PROGRAM",
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
    // TwinCAT REFERENCE TO: without the flag, REFERENCE is a demoted identifier
    // and the declaration is a parse error; with it, the type parses.
    FlagFixture {
        key: "allow_reference_to",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : REFERENCE TO INT;\nEND_VAR\nEND_PROGRAM",
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
    // A VAR initializer that is a constant expression (not a bare literal).
    // The parser accepts this unconditionally; the flag gates a later
    // semantic fold pass (P4037 when off, even though the expression is
    // foldable).
    FlagFixture {
        key: "allow_constant_initializer_expressions",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nd2r : LREAL := 4.25/180.0;\nEND_VAR\nEND_PROGRAM",
    },
    // A hex/binary/octal bit-string literal used as a CASE label. The parser
    // accepts it unconditionally; the flag gates a semantic rule (P4041 when
    // off).
    FlagFixture {
        key: "allow_bit_string_case_labels",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nx : DINT;\ny : DINT;\nEND_VAR\nCASE x OF\n16#D012: y := 1;\nEND_CASE;\nEND_PROGRAM",
    },
    // The STRING(n) parenthesis length delimiter. The grammar accepts it
    // unconditionally; a token-stream rule rejects it (P4042) when the flag
    // is off. The standard STRING[n] bracket form is always accepted.
    FlagFixture {
        key: "allow_paren_string_length",
        prereqs: &[],
        source: "PROGRAM main\nVAR\nhostName : STRING(255);\nEND_VAR\nEND_PROGRAM",
    },
    // A general expression (pointer deref + member access) as a struct/FB
    // initializer value. The parser accepts it unconditionally; a semantic
    // rule rejects it (P4043) when the flag is off. Requires allow_ref_to for
    // the REF_TO deref in the value expression.
    FlagFixture {
        key: "allow_struct_initializer_expressions",
        prereqs: &["allow_ref_to"],
        source: "FUNCTION_BLOCK FB_Device\nVAR_INPUT\nDelta : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\nTYPE MyStruct :\nSTRUCT\nx : INT;\nEND_STRUCT;\nEND_TYPE\nPROGRAM main\nVAR\npDevice : REF_TO FB_Device;\ns : MyStruct := (x := pDevice^.Delta);\nEND_VAR\nEND_PROGRAM",
    },
    // The CODESYS/TwinCAT EXTENDS clause on a FUNCTION_BLOCK header. With the
    // flag off, EXTENDS demotes to a plain identifier, so two consecutive
    // identifiers after the FB name is a parse error. With the flag on, it
    // parses as the inheritance clause.
    FlagFixture {
        key: "allow_oop_extensions",
        prereqs: &[],
        source: "FUNCTION_BLOCK FB_Base\nVAR\nx : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\nFUNCTION_BLOCK FB_Derived EXTENDS FB_Base\nEND_FUNCTION_BLOCK",
    },
    // With the flag off, PI is not a declared symbol, so the statement-context
    // reference fails to resolve. With the flag on, the compiler injects PI
    // as an implicit LREAL global constant and the reference resolves.
    FlagFixture {
        key: "allow_math_constants",
        prereqs: &[],
        source: "FUNCTION_BLOCK FB_Example\nVAR\nd2r : LREAL;\nEND_VAR\nd2r := PI/180.0;\nEND_FUNCTION_BLOCK",
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
/// flag without one fails here rather than passing silently — there is no
/// "not yet enforced" escape hatch.
#[test]
fn every_feature_flag_has_a_fixture() {
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        let has_fixture = FLAG_FIXTURES.iter().any(|f| f.key == fd.option_key);
        assert!(
            has_fixture,
            "feature flag `{}` has no behavioral fixture. Add a source snippet to FLAG_FIXTURES \
             that is rejected with the flag off and accepted with it on. Every flag must gate \
             real behavior; a flag that gates nothing is a dead flag and must be removed instead.",
            fd.option_key
        );
    }
}

/// Consistency guard: every fixture key and prerequisite must name a real flag.
#[test]
fn fixture_keys_name_real_flags() {
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
