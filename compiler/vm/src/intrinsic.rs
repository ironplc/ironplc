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
