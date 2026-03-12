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
pub fn ton(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i64(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if in_val {
        if !running {
            // Rising edge: start timing
            write_i64(instance, TIMER_START_TIME, cycle_time);
            write_i32(instance, TIMER_RUNNING, 1);
            write_i64(instance, TIMER_ET, 0);
            write_i32(instance, TIMER_Q, 0);
        } else {
            // Timing in progress
            let start_time = read_i64(instance, TIMER_START_TIME);
            let elapsed = cycle_time - start_time;
            let et = if elapsed > pt { pt } else { elapsed };
            write_i64(instance, TIMER_ET, et);
            if et >= pt {
                write_i32(instance, TIMER_Q, 1);
            }
        }
    } else {
        // IN is FALSE: reset
        write_i32(instance, TIMER_Q, 0);
        write_i64(instance, TIMER_ET, 0);
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
pub fn tof(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i64(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if in_val {
        // IN is TRUE: Q=TRUE, ET=0, stop any timing
        write_i32(instance, TIMER_Q, 1);
        write_i64(instance, TIMER_ET, 0);
        write_i32(instance, TIMER_RUNNING, 0);
    } else if !running {
        // Falling edge: start timing
        write_i64(instance, TIMER_START_TIME, cycle_time);
        write_i32(instance, TIMER_RUNNING, 1);
        write_i64(instance, TIMER_ET, 0);
        // Q stays TRUE during timing
        write_i32(instance, TIMER_Q, 1);
    } else {
        // Timing in progress (IN is FALSE)
        let start_time = read_i64(instance, TIMER_START_TIME);
        let elapsed = cycle_time - start_time;
        let et = if elapsed > pt { pt } else { elapsed };
        write_i64(instance, TIMER_ET, et);
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
pub fn tp(instance: &mut [u8], cycle_time: i64) -> Result<(), Trap> {
    let in_val = read_i32(instance, TIMER_IN) != 0;
    let pt = read_i64(instance, TIMER_PT);
    let running = read_i32(instance, TIMER_RUNNING) != 0;

    if running {
        // Pulse in progress — ignore IN changes
        let start_time = read_i64(instance, TIMER_START_TIME);
        let elapsed = cycle_time - start_time;
        let et = if elapsed > pt { pt } else { elapsed };
        write_i64(instance, TIMER_ET, et);
        if et >= pt {
            // Pulse complete
            write_i32(instance, TIMER_Q, 0);
            write_i32(instance, TIMER_RUNNING, 0);
        }
    } else if in_val {
        // Rising edge: start pulse
        write_i64(instance, TIMER_START_TIME, cycle_time);
        write_i32(instance, TIMER_RUNNING, 1);
        write_i64(instance, TIMER_ET, 0);
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

// --- CTU (Count Up) field layout ---
const CTU_CU: usize = 0;
const CTU_R: usize = 1;
const CTU_PV: usize = 2;
const CTU_Q: usize = 3;
const CTU_CV: usize = 4;
const CTU_PREV_CU: usize = 5; // hidden

/// Number of fields (including hidden) for a CTU FB instance.
pub const CTU_INSTANCE_FIELDS: usize = 6;

/// Executes one scan of the CTU (count up) intrinsic.
///
/// # CTU behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When R is TRUE: CV = 0 (reset takes priority)
/// - On rising edge of CU (and R is FALSE): CV increments by 1
/// - Q = (CV >= PV)
pub fn ctu(instance: &mut [u8]) -> Result<(), Trap> {
    let cu = read_i32(instance, CTU_CU) != 0;
    let r = read_i32(instance, CTU_R) != 0;
    let pv = read_i32(instance, CTU_PV);
    let prev_cu = read_i32(instance, CTU_PREV_CU) != 0;

    let mut cv = read_i32(instance, CTU_CV);

    if r {
        cv = 0;
    } else if cu && !prev_cu {
        cv = cv.saturating_add(1);
    }

    write_i32(instance, CTU_CV, cv);
    write_i32(instance, CTU_Q, if cv >= pv { 1 } else { 0 });
    write_i32(instance, CTU_PREV_CU, i32::from(cu));

    Ok(())
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

/// Executes one scan of the CTD (count down) intrinsic.
///
/// # CTD behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When LD is TRUE: CV = PV (load takes priority)
/// - On rising edge of CD (and LD is FALSE): CV decrements by 1
/// - Q = (CV <= 0)
pub fn ctd(instance: &mut [u8]) -> Result<(), Trap> {
    let cd = read_i32(instance, CTD_CD) != 0;
    let ld = read_i32(instance, CTD_LD) != 0;
    let pv = read_i32(instance, CTD_PV);
    let prev_cd = read_i32(instance, CTD_PREV_CD) != 0;

    let mut cv = read_i32(instance, CTD_CV);

    if ld {
        cv = pv;
    } else if cd && !prev_cd {
        cv = cv.saturating_sub(1);
    }

    write_i32(instance, CTD_CV, cv);
    write_i32(instance, CTD_Q, if cv <= 0 { 1 } else { 0 });
    write_i32(instance, CTD_PREV_CD, i32::from(cd));

    Ok(())
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

/// Executes one scan of the CTUD (count up/down) intrinsic.
///
/// # CTUD behavior (IEC 61131-3 section 2.5.2.3.3):
/// - When R is TRUE: CV = 0 (reset takes priority)
/// - When LD is TRUE (and R is FALSE): CV = PV (load takes priority over counting)
/// - On rising edge of CU: CV increments by 1
/// - On rising edge of CD: CV decrements by 1
/// - QU = (CV >= PV), QD = (CV <= 0)
pub fn ctud(instance: &mut [u8]) -> Result<(), Trap> {
    let cu = read_i32(instance, CTUD_CU) != 0;
    let cd = read_i32(instance, CTUD_CD) != 0;
    let r = read_i32(instance, CTUD_R) != 0;
    let ld = read_i32(instance, CTUD_LD) != 0;
    let pv = read_i32(instance, CTUD_PV);
    let prev_cu = read_i32(instance, CTUD_PREV_CU) != 0;
    let prev_cd = read_i32(instance, CTUD_PREV_CD) != 0;

    let mut cv = read_i32(instance, CTUD_CV);

    if r {
        cv = 0;
    } else if ld {
        cv = pv;
    } else {
        if cu && !prev_cu {
            cv = cv.saturating_add(1);
        }
        if cd && !prev_cd {
            cv = cv.saturating_sub(1);
        }
    }

    write_i32(instance, CTUD_CV, cv);
    write_i32(instance, CTUD_QU, if cv >= pv { 1 } else { 0 });
    write_i32(instance, CTUD_QD, if cv <= 0 { 1 } else { 0 });
    write_i32(instance, CTUD_PREV_CU, i32::from(cu));
    write_i32(instance, CTUD_PREV_CD, i32::from(cd));

    Ok(())
}
