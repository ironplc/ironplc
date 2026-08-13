//! End-to-end tests for the `Tc2_Utilities` compatibility library.
//!
//! Pins every test vector from the clean-room behavior specification
//! (`specs/design/library-interfaces/tc2-utilities.md`): the bundled library
//! is activated through the real registry, merged ahead of user source,
//! analyzed, compiled, and run on the VM. Every row asserts exact string
//! equality — the spec's fixed-point formatting is deterministic, digit for
//! digit.

use ironplc_analyzer::stages::analyze;
use ironplc_codegen::compile;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_sources::libraries::{remove_shadowed_functions, LibraryName, LibraryRegistry};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;

/// Activates the bundled `Tc2_Utilities`, analyzes it merged ahead of
/// `source`, compiles, and runs one scan cycle. The full activate → analyze →
/// codegen → VM-run path the acceptance criteria require.
fn run_with_tc2_utilities(source: &str) -> VmBuffers {
    let options = CompilerOptions::default();
    let compat = LibraryRegistry::bundled()
        .load(&LibraryName::from("Tc2_Utilities"))
        .expect("bundled Tc2_Utilities must load")
        .library;
    let user = parse_program(source, &FileId::default(), &options).unwrap();
    // The same user-shadowing filter the project pipeline applies.
    let compat = remove_shadowed_functions(vec![compat], &[&user]);
    let analyze_input: Vec<&Library> = compat.iter().chain(std::iter::once(&user)).collect();
    let (analyzed, context) = analyze(&analyze_input, &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );

    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: false,
    };
    let container = compile(
        &analyzed,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).expect("VM load must not trap");
        vm.run_round(0).expect("VM run must not trap");
    }
    bufs
}

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Formats one call `LREAL_TO_FMTSTR(x, <precision>, <round>)` where `x` is
/// an LREAL assigned from `value_expr`, and returns the resulting string
/// (`s`, the first and only STRING variable, at data offset 0). Routing the
/// input through an LREAL variable keeps the value in binary64 — a bare real
/// literal in argument position would resolve as REAL (binary32) — and lets
/// a vector construct its input arithmetically (infinity, NaN).
fn fmt(value_expr: &str, precision: i32, round: bool) -> String {
    let round = if round { "TRUE" } else { "FALSE" };
    let source = format!(
        "PROGRAM main
VAR
    s : STRING;
    x : LREAL;
END_VAR
    x := {value_expr};
    s := LREAL_TO_FMTSTR(x, {precision}, {round});
END_PROGRAM
"
    );
    let bufs = run_with_tc2_utilities(&source);
    read_string(&bufs.data_region, 0)
}

// ---------------------------------------------------------------------------
// Sign, rounding mode, and truncation.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_round_then_rounds_at_last_place() {
    assert_eq!(fmt("123.456", 2, true), "123.46");
}

#[test]
fn end_to_end_when_truncate_then_drops_beyond_last_place() {
    assert_eq!(fmt("123.456", 2, false), "123.45");
}

#[test]
fn end_to_end_when_negative_then_signed() {
    assert_eq!(fmt("-123.456", 2, true), "-123.46");
}

#[test]
fn end_to_end_when_half_then_rounds_away_from_zero_not_bankers() {
    assert_eq!(fmt("2.5", 0, true), "3");
    assert_eq!(fmt("-2.5", 0, true), "-3");
}

#[test]
fn end_to_end_when_half_truncated_then_drops_fraction() {
    assert_eq!(fmt("2.5", 0, false), "2");
}

#[test]
fn end_to_end_when_negative_truncated_then_toward_zero() {
    assert_eq!(fmt("-2.8", 0, false), "-2");
}

// ---------------------------------------------------------------------------
// Carry, padding, and zero.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_fraction_rounds_all_the_way_up_then_carries_into_integer() {
    assert_eq!(fmt("0.996", 2, true), "1.00");
}

#[test]
fn end_to_end_when_negative_truncated_fraction_then_no_carry() {
    assert_eq!(fmt("-0.996", 2, false), "-0.99");
}

#[test]
fn end_to_end_when_fraction_has_leading_zero_then_left_padded() {
    assert_eq!(fmt("1.05", 2, true), "1.05");
}

#[test]
fn end_to_end_when_fraction_shorter_than_precision_then_zero_filled() {
    assert_eq!(fmt("1.5", 3, false), "1.500");
}

#[test]
fn end_to_end_when_zero_then_zero_digits() {
    assert_eq!(fmt("0.0", 2, true), "0.00");
}

#[test]
fn end_to_end_when_negative_zero_then_unsigned() {
    // -0.0 < 0.0 is FALSE, so negative zero renders without a sign. The
    // input is negated from a variable so no constant folding can drop the
    // sign of zero before the call.
    let source = "PROGRAM main
VAR
    s : STRING;
    zero : LREAL;
END_VAR
    zero := 0.0;
    s := LREAL_TO_FMTSTR(-zero, 2, TRUE);
END_PROGRAM
";
    let bufs = run_with_tc2_utilities(source);
    assert_eq!(read_string(&bufs.data_region, 0), "0.00");
}

// ---------------------------------------------------------------------------
// Precision clamping.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_negative_precision_then_clamped_to_zero() {
    assert_eq!(fmt("1.5", -3, true), "2");
}

#[test]
fn end_to_end_when_precision_above_fifteen_then_clamped_to_fifteen() {
    assert_eq!(fmt("1.5", 100, false), "1.500000000000000");
}

// ---------------------------------------------------------------------------
// Domain boundaries.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_literal_not_representable_then_stored_value_renders() {
    // The literal is not representable in binary64; the stored value — 2^53 —
    // renders exactly.
    assert_eq!(fmt("9007199254740993.0", 0, true), "9007199254740992");
}

#[test]
fn end_to_end_when_large_integral_value_then_exact_digits() {
    // In domain; binary64 values >= 2^52 are integral, so the fraction digits
    // are genuinely zero.
    assert_eq!(fmt("9.0E18", 0, true), "9000000000000000000");
}

#[test]
fn end_to_end_when_at_or_beyond_two_pow_63_then_empty() {
    assert_eq!(fmt("1.0E19", 0, true), "");
    assert_eq!(fmt("-1.0E19", 2, true), "");
}

#[test]
fn end_to_end_when_infinite_then_empty() {
    // Constructed arithmetically: 1.0E308 * 10.0 overflows to +infinity.
    assert_eq!(fmt("1.0E308 * 10.0", 2, true), "");
    assert_eq!(fmt("-1.0E308 * 10.0", 2, true), "");
}

#[test]
fn end_to_end_when_nan_then_empty() {
    // Constructed arithmetically: __MOD(1.5, 0.0) is NaN. NaN fails every
    // comparison, so this exercises the deliberate `in = in` domain test.
    assert_eq!(fmt("__MOD(1.5, 0.0)", 2, true), "");
    assert_eq!(fmt("__MOD(1.5, 0.0)", 0, false), "");
}

// ---------------------------------------------------------------------------
// Formal (named) argument passing — the vendor's documented parameter names.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_formal_arguments_then_vendor_names_resolve() {
    let source = "PROGRAM main
VAR
    s : STRING;
    x : LREAL;
END_VAR
    x := 123.456;
    s := LREAL_TO_FMTSTR(in := x, iPrecision := 2, bRound := TRUE);
END_PROGRAM
";
    let bufs = run_with_tc2_utilities(source);
    assert_eq!(read_string(&bufs.data_region, 0), "123.46");
}

// ---------------------------------------------------------------------------
// Shadowing — a user-defined LREAL_TO_FMTSTR takes precedence.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_when_user_function_shadows_lreal_to_fmtstr_then_user_body_runs() {
    let bufs = run_with_tc2_utilities(
        "FUNCTION LREAL_TO_FMTSTR : STRING
VAR_INPUT
    in : LREAL;
    iPrecision : INT;
    bRound : BOOL;
END_VAR
    LREAL_TO_FMTSTR := 'shadowed';
END_FUNCTION
PROGRAM main
VAR
    s : STRING;
    x : LREAL;
END_VAR
    x := 123.456;
    s := LREAL_TO_FMTSTR(x, 2, TRUE);
END_PROGRAM
",
    );
    // The user's body (the constant), not the library's formatter.
    assert_eq!(read_string(&bufs.data_region, 0), "shadowed");
}
