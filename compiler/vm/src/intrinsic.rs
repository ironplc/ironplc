//! Native intrinsic implementations for standard function blocks.

use crate::error::Trap;

/// Field byte size (all fields are 8-byte aligned slots).
const FIELD_SIZE: usize = 8;

/// Reads an i32 from an FB instance field.
fn read_i32(instance: &[u8], field: usize) -> i32 {
    let offset = field * FIELD_SIZE;
    let bytes: [u8; 4] = instance[offset..offset + 4].try_into().unwrap();
    i32::from_le_bytes(bytes)
}

/// Writes an i32 to an FB instance field.
fn write_i32(instance: &mut [u8], field: usize, value: i32) {
    let offset = field * FIELD_SIZE;
    instance[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    // Zero upper 4 bytes for slot consistency.
    instance[offset + 4..offset + 8].copy_from_slice(&[0, 0, 0, 0]);
}

/// Reads an i64 from an FB instance field.
fn read_i64(instance: &[u8], field: usize) -> i64 {
    let offset = field * FIELD_SIZE;
    let bytes: [u8; 8] = instance[offset..offset + 8].try_into().unwrap();
    i64::from_le_bytes(bytes)
}

/// Writes an i64 to an FB instance field.
fn write_i64(instance: &mut [u8], field: usize, value: i64) {
    let offset = field * FIELD_SIZE;
    instance[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Shared field indices for timer FBs (TON, TOF, TP).
/// All timer FBs use the same 6-field layout.
const TIMER_IN: usize = 0;
const TIMER_PT: usize = 1;
const TIMER_Q: usize = 2;
const TIMER_ET: usize = 3;
const TIMER_START_TIME: usize = 4; // hidden
const TIMER_RUNNING: usize = 5; // hidden

/// Number of fields (including hidden) for a timer FB instance.
pub const TIMER_INSTANCE_FIELDS: usize = 6;

/// Executes one scan of the TON (on-delay timer) intrinsic.
///
/// # Arguments
/// * `instance` - Mutable slice of the FB instance memory (6 fields * 8 bytes = 48 bytes).
/// * `cycle_time` - Current scan cycle time in microseconds.
///
/// # TON behavior (IEC 61131-3 section 2.5.2.3.1):
/// - When IN rises (FALSE->TRUE): start timing, ET=0, Q=FALSE
/// - While IN is TRUE: ET increments up to PT. When ET >= PT, Q becomes TRUE.
/// - When IN falls (TRUE->FALSE): Q=FALSE, ET=0, stop timing.
///
/// PT and ET are i32 milliseconds. The hidden start_time is i64 microseconds
/// (matching cycle_time). Elapsed time is converted from microseconds to
/// milliseconds for comparison with PT and assignment to ET.
pub fn ton(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i32(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if in_val {
        if !running {
            // Rising edge: start timing
            write_i64(instance, TIMER_START_TIME, cycle_time);
            write_i32(instance, TIMER_RUNNING, 1);
            write_i32(instance, TIMER_ET, 0);
            write_i32(instance, TIMER_Q, 0);
        } else {
            // Timing in progress
            let start_time = read_i64(instance, TIMER_START_TIME);
            let elapsed_ms = ((cycle_time - start_time) / 1000) as i32;
            let et = if elapsed_ms > pt { pt } else { elapsed_ms };
            write_i32(instance, TIMER_ET, et);
            if et >= pt {
                write_i32(instance, TIMER_Q, 1);
            }
        }
    } else {
        // IN is FALSE: reset
        write_i32(instance, TIMER_Q, 0);
        write_i32(instance, TIMER_ET, 0);
        write_i32(instance, TIMER_RUNNING, 0);
    }
    Ok(())
}

/// Executes one scan of the TOF (off-delay timer) intrinsic.
///
/// # Arguments
/// * `instance` - Mutable slice of the FB instance memory (6 fields * 8 bytes = 48 bytes).
/// * `cycle_time` - Current scan cycle time in microseconds.
///
/// # TOF behavior (IEC 61131-3 section 2.5.2.3.2):
/// - When IN is TRUE: Q=TRUE, ET=0, stop timing.
/// - When IN falls (TRUE->FALSE): start timing, ET=0, Q=TRUE.
/// - While IN is FALSE and timing: ET increments up to PT. When ET >= PT, Q becomes FALSE.
/// - If IN returns to TRUE while timing: reset.
///
/// PT and ET are i32 milliseconds. The hidden start_time is i64 microseconds.
pub fn tof(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i32(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if in_val {
        // IN is TRUE: Q=TRUE, ET=0, stop any timing
        write_i32(instance, TIMER_Q, 1);
        write_i32(instance, TIMER_ET, 0);
        write_i32(instance, TIMER_RUNNING, 0);
    } else if !running {
        // Falling edge: start timing
        write_i64(instance, TIMER_START_TIME, cycle_time);
        write_i32(instance, TIMER_RUNNING, 1);
        write_i32(instance, TIMER_ET, 0);
        // Q stays TRUE during timing
        write_i32(instance, TIMER_Q, 1);
    } else {
        // Timing in progress (IN is FALSE)
        let start_time = read_i64(instance, TIMER_START_TIME);
        let elapsed_ms = ((cycle_time - start_time) / 1000) as i32;
        let et = if elapsed_ms > pt { pt } else { elapsed_ms };
        write_i32(instance, TIMER_ET, et);
        if et >= pt {
            write_i32(instance, TIMER_Q, 0);
        }
    }
    Ok(())
}

/// Executes one scan of the TP (pulse timer) intrinsic.
///
/// # Arguments
/// * `instance` - Mutable slice of the FB instance memory (6 fields * 8 bytes = 48 bytes).
/// * `cycle_time` - Current scan cycle time in microseconds.
///
/// # TP behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When IN rises (FALSE->TRUE) and not already pulsing: Q=TRUE, start timing, ET=0.
/// - While pulsing: ET increments up to PT. When ET >= PT, Q becomes FALSE, pulse ends.
/// - Changes to IN during the pulse are ignored; the pulse always runs for full duration PT.
///
/// PT and ET are i32 milliseconds. The hidden start_time is i64 microseconds.
pub fn tp(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i32(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if running {
        // Pulse in progress — ignore IN changes
        let start_time = read_i64(instance, TIMER_START_TIME);
        let elapsed_ms = ((cycle_time - start_time) / 1000) as i32;
        let et = if elapsed_ms > pt { pt } else { elapsed_ms };
        write_i32(instance, TIMER_ET, et);
        if et >= pt {
            // Pulse complete
            write_i32(instance, TIMER_Q, 0);
            write_i32(instance, TIMER_RUNNING, 0);
        }
    } else if in_val {
        // Rising edge: start pulse
        write_i64(instance, TIMER_START_TIME, cycle_time);
        write_i32(instance, TIMER_RUNNING, 1);
        write_i32(instance, TIMER_ET, 0);
        write_i32(instance, TIMER_Q, 1);
    }
    // else: not running and IN is FALSE — no action, Q stays as-is
    Ok(())
}

// =============================================================================
// Bistable Function Blocks (IEC 61131-3 Section 2.5.2.3.1)
// =============================================================================

// --- SR (Set-Reset, Set dominant) field layout ---
const SR_S1: usize = 0;
const SR_R: usize = 1;
const SR_Q1: usize = 2;

/// Number of fields for an SR FB instance.
pub const SR_INSTANCE_FIELDS: usize = 3;

/// Executes one scan of the SR (set-reset, set dominant) intrinsic.
///
/// # SR behavior (IEC 61131-3 section 2.5.2.3.1):
/// Q1 := S1 OR (NOT R AND Q1)
/// Set (S1) dominates: if both S1 and R are TRUE, Q1 is TRUE.
pub fn sr(instance: &mut [u8]) -> Result<(), Trap> {
    let s1 = read_i32(instance, SR_S1) != 0;
    let r = read_i32(instance, SR_R) != 0;
    let q1 = read_i32(instance, SR_Q1) != 0;

    let new_q1 = s1 || (!r && q1);
    write_i32(instance, SR_Q1, i32::from(new_q1));

    Ok(())
}

// --- RS (Reset-Set, Reset dominant) field layout ---
const RS_S: usize = 0;
const RS_R1: usize = 1;
const RS_Q1: usize = 2;

/// Number of fields for an RS FB instance.
pub const RS_INSTANCE_FIELDS: usize = 3;

/// Executes one scan of the RS (reset-set, reset dominant) intrinsic.
///
/// # RS behavior (IEC 61131-3 section 2.5.2.3.1):
/// Q1 := NOT R1 AND (S OR Q1)
/// Reset (R1) dominates: if both S and R1 are TRUE, Q1 is FALSE.
pub fn rs(instance: &mut [u8]) -> Result<(), Trap> {
    let s = read_i32(instance, RS_S) != 0;
    let r1 = read_i32(instance, RS_R1) != 0;
    let q1 = read_i32(instance, RS_Q1) != 0;

    let new_q1 = !r1 && (s || q1);
    write_i32(instance, RS_Q1, i32::from(new_q1));

    Ok(())
}

// =============================================================================
// Counter Function Blocks (IEC 61131-3 Section 2.5.2.3.3)
// =============================================================================

/// The integer a counter counts in: the type of its `PV` and `CV`.
///
/// Each width of a counter (`CTU_UDINT`, `CTU_LINT`, ...) has its own
/// `FB_CALL` type id and counts in its own type, so a `CTU_LINT` counts past
/// 32 bits and a `CTU_UDINT` reads a preset above 2,147,483,647 as positive.
/// `CTU`, `CTU_INT` and `CTU_DINT` count in a signed 32-bit integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterWidth {
    I32,
    U32,
    I64,
    U64,
}

impl CounterWidth {
    /// The width the counter with `FB_CALL` type id `type_id` counts in.
    pub fn of(type_id: u16) -> CounterWidth {
        use ironplc_container::opcode::fb_type;
        match type_id {
            fb_type::CTU_UDINT | fb_type::CTD_UDINT | fb_type::CTUD_UDINT => CounterWidth::U32,
            fb_type::CTU_LINT | fb_type::CTD_LINT | fb_type::CTUD_LINT => CounterWidth::I64,
            fb_type::CTU_ULINT | fb_type::CTD_ULINT | fb_type::CTUD_ULINT => CounterWidth::U64,
            _ => CounterWidth::I32,
        }
    }
}

/// A value a counter counts in, read from and written to an instance field.
///
/// Counting saturates at the bounds of the type, as IEC 61131-3 counts only
/// while `CV` is below the type's maximum or above its minimum.
trait CounterValue: Copy + PartialOrd {
    const ZERO: Self;
    fn read(instance: &[u8], field: usize) -> Self;
    fn write(self, instance: &mut [u8], field: usize);
    fn up(self) -> Self;
    fn down(self) -> Self;
}

macro_rules! counter_value {
    ($t:ty, $bytes:literal) => {
        impl CounterValue for $t {
            const ZERO: Self = 0;

            fn read(instance: &[u8], field: usize) -> Self {
                let offset = field * FIELD_SIZE;
                let bytes: [u8; $bytes] = instance[offset..offset + $bytes].try_into().unwrap();
                <$t>::from_le_bytes(bytes)
            }

            /// Writes the value, zeroing the rest of the 8-byte slot.
            fn write(self, instance: &mut [u8], field: usize) {
                let offset = field * FIELD_SIZE;
                instance[offset..offset + FIELD_SIZE].fill(0);
                instance[offset..offset + $bytes].copy_from_slice(&self.to_le_bytes());
            }

            fn up(self) -> Self {
                self.saturating_add(1)
            }

            fn down(self) -> Self {
                self.saturating_sub(1)
            }
        }
    };
}

counter_value!(i32, 4);
counter_value!(u32, 4);
counter_value!(i64, 8);
counter_value!(u64, 8);

/// Runs `count` at the type `width` names.
macro_rules! at_width {
    ($width:expr, $count:ident, $instance:expr) => {
        match $width {
            CounterWidth::I32 => $count::<i32>($instance),
            CounterWidth::U32 => $count::<u32>($instance),
            CounterWidth::I64 => $count::<i64>($instance),
            CounterWidth::U64 => $count::<u64>($instance),
        }
    };
}

// --- CTU (Count Up) field layout ---
const CTU_CU: usize = 0;
const CTU_R: usize = 1;
const CTU_PV: usize = 2;
const CTU_Q: usize = 3;
const CTU_CV: usize = 4;
const CTU_PREV_CU: usize = 5; // hidden

/// Number of fields (including hidden) for a CTU FB instance.
pub const CTU_INSTANCE_FIELDS: usize = 6;

/// Executes one scan of the CTU (count up) intrinsic, counting in `width`.
///
/// # CTU behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When R is TRUE: CV = 0 (reset takes priority)
/// - On rising edge of CU (and R is FALSE): CV increments by 1
/// - Q = (CV >= PV)
pub fn ctu(instance: &mut [u8], width: CounterWidth) -> Result<(), Trap> {
    at_width!(width, ctu_at, instance);
    Ok(())
}

fn ctu_at<T: CounterValue>(instance: &mut [u8]) {
    let cu = read_i32(instance, CTU_CU) != 0;
    let r = read_i32(instance, CTU_R) != 0;
    let pv = T::read(instance, CTU_PV);
    let prev_cu = read_i32(instance, CTU_PREV_CU) != 0;

    let mut cv = T::read(instance, CTU_CV);

    if r {
        cv = T::ZERO;
    } else if cu && !prev_cu {
        cv = cv.up();
    }

    cv.write(instance, CTU_CV);
    write_i32(instance, CTU_Q, i32::from(cv >= pv));
    write_i32(instance, CTU_PREV_CU, i32::from(cu));
}

// --- CTD (Count Down) field layout ---
const CTD_CD: usize = 0;
const CTD_LD: usize = 1;
const CTD_PV: usize = 2;
const CTD_Q: usize = 3;
const CTD_CV: usize = 4;
const CTD_PREV_CD: usize = 5; // hidden

/// Number of fields (including hidden) for a CTD FB instance.
pub const CTD_INSTANCE_FIELDS: usize = 6;

/// Executes one scan of the CTD (count down) intrinsic, counting in `width`.
///
/// # CTD behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When LD is TRUE: CV = PV (load takes priority)
/// - On rising edge of CD (and LD is FALSE): CV decrements by 1
/// - Q = (CV <= 0)
pub fn ctd(instance: &mut [u8], width: CounterWidth) -> Result<(), Trap> {
    at_width!(width, ctd_at, instance);
    Ok(())
}

fn ctd_at<T: CounterValue>(instance: &mut [u8]) {
    let cd = read_i32(instance, CTD_CD) != 0;
    let ld = read_i32(instance, CTD_LD) != 0;
    let pv = T::read(instance, CTD_PV);
    let prev_cd = read_i32(instance, CTD_PREV_CD) != 0;

    let mut cv = T::read(instance, CTD_CV);

    if ld {
        cv = pv;
    } else if cd && !prev_cd {
        cv = cv.down();
    }

    cv.write(instance, CTD_CV);
    write_i32(instance, CTD_Q, i32::from(cv <= T::ZERO));
    write_i32(instance, CTD_PREV_CD, i32::from(cd));
}

// --- CTUD (Count Up/Down) field layout ---
const CTUD_CU: usize = 0;
const CTUD_CD: usize = 1;
const CTUD_R: usize = 2;
const CTUD_LD: usize = 3;
const CTUD_PV: usize = 4;
const CTUD_QU: usize = 5;
const CTUD_QD: usize = 6;
const CTUD_CV: usize = 7;
const CTUD_PREV_CU: usize = 8; // hidden
const CTUD_PREV_CD: usize = 9; // hidden

/// Number of fields (including hidden) for a CTUD FB instance.
pub const CTUD_INSTANCE_FIELDS: usize = 10;

/// Executes one scan of the CTUD (count up/down) intrinsic, counting in
/// `width`.
///
/// # CTUD behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When R is TRUE: CV = 0 (reset takes priority)
/// - When LD is TRUE (and R is FALSE): CV = PV (load takes priority over counting)
/// - On rising edge of CU: CV increments by 1
/// - On rising edge of CD: CV decrements by 1
/// - QU = (CV >= PV), QD = (CV <= 0)
pub fn ctud(instance: &mut [u8], width: CounterWidth) -> Result<(), Trap> {
    at_width!(width, ctud_at, instance);
    Ok(())
}

fn ctud_at<T: CounterValue>(instance: &mut [u8]) {
    let cu = read_i32(instance, CTUD_CU) != 0;
    let cd = read_i32(instance, CTUD_CD) != 0;
    let r = read_i32(instance, CTUD_R) != 0;
    let ld = read_i32(instance, CTUD_LD) != 0;
    let pv = T::read(instance, CTUD_PV);
    let prev_cu = read_i32(instance, CTUD_PREV_CU) != 0;
    let prev_cd = read_i32(instance, CTUD_PREV_CD) != 0;

    let mut cv = T::read(instance, CTUD_CV);

    if r {
        cv = T::ZERO;
    } else if ld {
        cv = pv;
    } else {
        if cu && !prev_cu {
            cv = cv.up();
        }
        if cd && !prev_cd {
            cv = cv.down();
        }
    }

    cv.write(instance, CTUD_CV);
    write_i32(instance, CTUD_QU, i32::from(cv >= pv));
    write_i32(instance, CTUD_QD, i32::from(cv <= T::ZERO));
    write_i32(instance, CTUD_PREV_CU, i32::from(cu));
    write_i32(instance, CTUD_PREV_CD, i32::from(cd));
}

// =============================================================================
// R_TRIG (Rising Edge Detector)
// =============================================================================

/// Field indices for R_TRIG instances.
const R_TRIG_CLK: usize = 0;
const R_TRIG_Q: usize = 1;
const R_TRIG_M: usize = 2; // hidden: previous CLK value

/// Total fields per R_TRIG instance (including hidden).
pub const R_TRIG_INSTANCE_FIELDS: usize = 3;

/// Rising edge detector: Q is TRUE for one scan when CLK transitions FALSE→TRUE.
pub fn r_trig(instance: &mut [u8]) -> Result<(), Trap> {
    let clk = read_i32(instance, R_TRIG_CLK) != 0;
    let m = read_i32(instance, R_TRIG_M) != 0;

    let q = clk && !m;

    write_i32(instance, R_TRIG_Q, i32::from(q));
    write_i32(instance, R_TRIG_M, i32::from(clk));

    Ok(())
}

// =============================================================================
// F_TRIG (Falling Edge Detector)
// =============================================================================

/// Field indices for F_TRIG instances.
const F_TRIG_CLK: usize = 0;
const F_TRIG_Q: usize = 1;
const F_TRIG_M: usize = 2; // hidden: previous CLK value

/// Total fields per F_TRIG instance (including hidden).
pub const F_TRIG_INSTANCE_FIELDS: usize = 3;

/// Falling edge detector: Q is TRUE for one scan when CLK transitions TRUE→FALSE.
pub fn f_trig(instance: &mut [u8]) -> Result<(), Trap> {
    let clk = read_i32(instance, F_TRIG_CLK) != 0;
    let m = read_i32(instance, F_TRIG_M) != 0;

    let q = !clk && m;

    write_i32(instance, F_TRIG_Q, i32::from(q));
    write_i32(instance, F_TRIG_M, i32::from(clk));

    Ok(())
}

#[cfg(test)]
mod counter_tests {
    use super::*;
    use ironplc_container::opcode::fb_type;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// An instance of `fields` fields, all zero but the `CV` field
    /// `cv_field`, which holds `cv`.
    fn counter_at<T: CounterValue>(fields: usize, cv_field: usize, cv: T) -> Vec<u8> {
        let mut instance = vec![0u8; fields * FIELD_SIZE];
        cv.write(&mut instance, cv_field);
        instance
    }

    #[spec_test(REQ_CW_vm_001)]
    #[rstest]
    #[case::int(fb_type::CTU, CounterWidth::I32)]
    #[case::udint(fb_type::CTD_UDINT, CounterWidth::U32)]
    #[case::lint(fb_type::CTUD_LINT, CounterWidth::I64)]
    #[case::ulint(fb_type::CTU_ULINT, CounterWidth::U64)]
    fn counter_width_of_when_counter_type_then_its_width(
        #[case] type_id: u16,
        #[case] expected: CounterWidth,
    ) {
        assert_eq!(CounterWidth::of(type_id), expected);
    }

    #[spec_test(REQ_CW_vm_002)]
    #[test]
    fn ctu_when_lint_cv_at_largest_dint_then_counts_past_it() {
        let mut instance = counter_at(CTU_INSTANCE_FIELDS, CTU_CV, i64::from(i32::MAX));
        write_i32(&mut instance, CTU_CU, 1);

        ctu(&mut instance, CounterWidth::I64).unwrap();

        assert_eq!(i64::read(&instance, CTU_CV), i64::from(i32::MAX) + 1);
    }

    #[spec_test(REQ_CW_vm_002)]
    #[test]
    fn ctu_when_udint_preset_above_largest_dint_then_q_false() {
        let mut instance = vec![0u8; CTU_INSTANCE_FIELDS * FIELD_SIZE];
        3_000_000_000u32.write(&mut instance, CTU_PV);

        ctu(&mut instance, CounterWidth::U32).unwrap();

        assert_eq!(read_i32(&instance, CTU_Q), 0);
    }

    #[spec_test(REQ_CW_vm_003)]
    #[test]
    fn ctu_when_udint_cv_at_maximum_then_stays_there() {
        let mut instance = counter_at(CTU_INSTANCE_FIELDS, CTU_CV, u32::MAX);
        write_i32(&mut instance, CTU_CU, 1);

        ctu(&mut instance, CounterWidth::U32).unwrap();

        assert_eq!(u32::read(&instance, CTU_CV), u32::MAX);
    }

    #[spec_test(REQ_CW_vm_003)]
    #[test]
    fn ctd_when_ulint_cv_at_zero_then_stays_zero_and_q_true() {
        let mut instance = counter_at(CTD_INSTANCE_FIELDS, CTD_CV, 0u64);
        write_i32(&mut instance, CTD_CD, 1);

        ctd(&mut instance, CounterWidth::U64).unwrap();

        assert_eq!(u64::read(&instance, CTD_CV), 0);
        assert_eq!(read_i32(&instance, CTD_Q), 1);
    }

    #[spec_test(REQ_CW_vm_002)]
    #[test]
    fn ctud_when_ulint_loads_preset_above_largest_lint_then_cv_holds_it() {
        let mut instance = vec![0u8; CTUD_INSTANCE_FIELDS * FIELD_SIZE];
        write_i32(&mut instance, CTUD_LD, 1);
        u64::MAX.write(&mut instance, CTUD_PV);

        ctud(&mut instance, CounterWidth::U64).unwrap();

        assert_eq!(u64::read(&instance, CTUD_CV), u64::MAX);
        assert_eq!(read_i32(&instance, CTUD_QU), 1);
        assert_eq!(read_i32(&instance, CTUD_QD), 0);
    }
}
