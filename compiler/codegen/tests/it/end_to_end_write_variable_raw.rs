//! End-to-end tests for embedder-supplied 64-bit variable values.
//!
//! These exercise the full pipeline — parse -> compile -> VM execution —
//! for the two embedder use cases that `VmRunning::write_variable_raw`
//! enables:
//!
//! - **Fieldbus input mapping**: a host writes a 64-bit process input
//!   (`LINT`, `LREAL`, `LWORD`) into a program variable before the scan, and
//!   the ST body computes on the full-width value.
//! - **RETAIN persistence**: a host reads a 64-bit slot with
//!   `read_variable_raw` at shutdown and restores it with
//!   `write_variable_raw` on the next start, so an accumulator resumes where
//!   it left off.
//!
//! Every test here fails without a raw write path: the pre-existing
//! `write_variable` takes an `i32` and stores through `Slot::from_i32`, which
//! truncates each of these values to its low 32 bits. The truncation itself is
//! pinned by
//! [`end_to_end_when_lint_input_written_as_i32_then_program_sees_truncated_value`].

use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::test_support::load_and_start;

use crate::common::{parse_and_compile, parse_and_run_rounds, VmBuffers};

/// Doubles a 64-bit fieldbus input. `fieldbus_in` is var 0, `scaled` is var 1.
const SCALE_LINT_INPUT: &str = "
PROGRAM main
  VAR
    fieldbus_in : LINT;
    scaled : LINT;
  END_VAR
  scaled := fieldbus_in * 2;
END_PROGRAM
";

/// A 64-bit process input that does not survive a trip through `i32`.
const LINT_INPUT: i64 = 5_000_000_000;

#[test]
fn end_to_end_when_lint_input_written_raw_then_program_sees_full_64_bits() {
    parse_and_run_rounds(SCALE_LINT_INPUT, &CompilerOptions::default(), |vm| {
        vm.write_variable_raw(VarIndex::new(0), LINT_INPUT as u64)
            .unwrap();
        vm.run_round(0).unwrap();

        assert_eq!(vm.read_variable_i64(VarIndex::new(0)).unwrap(), LINT_INPUT);
        assert_eq!(
            vm.read_variable_i64(VarIndex::new(1)).unwrap(),
            LINT_INPUT * 2
        );
    });
}

/// Pins the truncation that made `write_variable_raw` necessary: the `i32`
/// write path drops the upper 32 bits, so the program computes on a different
/// number than the embedder supplied.
#[test]
fn end_to_end_when_lint_input_written_as_i32_then_program_sees_truncated_value() {
    parse_and_run_rounds(SCALE_LINT_INPUT, &CompilerOptions::default(), |vm| {
        vm.write_variable(VarIndex::new(0), LINT_INPUT as i32)
            .unwrap();
        vm.run_round(0).unwrap();

        let truncated = LINT_INPUT as i32 as i64;
        assert_ne!(truncated, LINT_INPUT);
        assert_eq!(
            vm.read_variable_i64(VarIndex::new(1)).unwrap(),
            truncated * 2
        );
    });
}

#[test]
fn end_to_end_when_lreal_input_written_raw_then_program_keeps_full_precision() {
    let source = "
PROGRAM main
  VAR
    sensor : LREAL;
    doubled : LREAL;
  END_VAR
  doubled := sensor * 2.0;
END_PROGRAM
";
    // A value whose low 32 bits alone say nothing about the number.
    let sensor = std::f64::consts::PI;

    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.write_variable_raw(VarIndex::new(0), sensor.to_bits())
            .unwrap();
        vm.run_round(0).unwrap();

        let doubled = f64::from_bits(vm.read_variable_raw(VarIndex::new(1)).unwrap());
        assert_eq!(doubled, sensor * 2.0);
    });
}

#[test]
fn end_to_end_when_lword_input_written_raw_then_high_word_bits_survive() {
    let source = "
PROGRAM main
  VAR
    status : LWORD;
    high_bits : LWORD;
  END_VAR
  high_bits := status AND LWORD#16#FFFF_FFFF_0000_0000;
END_PROGRAM
";
    let status = 0xDEAD_BEEF_0BAD_F00D_u64;

    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.write_variable_raw(VarIndex::new(0), status).unwrap();
        vm.run_round(0).unwrap();

        assert_eq!(vm.read_variable_raw(VarIndex::new(0)).unwrap(), status);
        assert_eq!(
            vm.read_variable_raw(VarIndex::new(1)).unwrap(),
            0xDEAD_BEEF_0000_0000_u64
        );
    });
}

/// RETAIN round-trip across a restart: run the accumulator past `i32` range,
/// snapshot the raw slot as a host would at shutdown, then restore it into a
/// freshly started VM and confirm the totals continue rather than restart.
#[test]
fn end_to_end_when_retained_lint_restored_raw_then_accumulator_resumes() {
    let source = "
PROGRAM main
  VAR
    total : LINT;
    step : LINT;
  END_VAR
  step := 1000000000;
  total := total + step;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // First run: five scans accumulate 5e9, which exceeds i32 range.
    let mut bufs = VmBuffers::from_container(&container);
    let snapshot = {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        for round in 0..5 {
            vm.run_round(round).unwrap();
        }
        assert_eq!(
            vm.read_variable_i64(VarIndex::new(0)).unwrap(),
            5_000_000_000
        );
        vm.read_variable_raw(VarIndex::new(0)).unwrap()
    };

    // Restart: a fresh VM zeroes the accumulator during init, and the host
    // restores the retained slot before the first scan.
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    assert_eq!(vm.read_variable_i64(VarIndex::new(0)).unwrap(), 0);

    vm.write_variable_raw(VarIndex::new(0), snapshot).unwrap();
    for round in 0..2 {
        vm.run_round(round).unwrap();
    }

    assert_eq!(
        vm.read_variable_i64(VarIndex::new(0)).unwrap(),
        7_000_000_000
    );
}
