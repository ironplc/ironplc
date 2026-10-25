//! The standard library as this backend sees it: the conversion between two
//! operation types, and the field layout of each standard function block,
//! which runs in the VM.

use std::collections::HashMap;

use ironplc_container::{opcode, FbTypeId};
use ironplc_ir::lowered::{CounterWidth, PouId, Program, StandardFunctionBlock};
use ironplc_ir::Diagnostic;

use super::compile::{OpWidth, Signedness, VarTypeInfo};
use crate::emit::Emitter;

/// Emits the appropriate conversion BUILTIN opcode for the source->target
/// type transition. Does nothing for same-domain integer conversions that
/// are handled by the Slot's sign-extension and truncation.
pub(crate) fn emit_conversion_opcode(
    emitter: &mut Emitter,
    source: &VarTypeInfo,
    target: &VarTypeInfo,
) -> Result<(), Diagnostic> {
    use opcode::builtin::*;
    use OpWidth::*;
    use Signedness::*;

    let func_id = match (
        source.op_width,
        source.signedness,
        target.op_width,
        target.signedness,
    ) {
        // Same OpWidth: no conversion needed (truncation handles sub-width)
        (W32, _, W32, _) | (W64, _, W64, _) => None,

        // W32 signed -> W64: sign extension already in Slot, no-op
        (W32, Signed, W64, _) => None,

        // W32 unsigned -> W64: need zero-extension
        (W32, Unsigned, W64, _) => Some(CONV_U32_TO_I64),

        // W64 -> W32: as_i32() truncation at store time, no-op
        (W64, _, W32, _) => None,

        // Integer -> Float
        (W32, Signed, F32, _) => Some(CONV_I32_TO_F32),
        (W32, Signed, F64, _) => Some(CONV_I32_TO_F64),
        (W64, Signed, F32, _) => Some(CONV_I64_TO_F32),
        (W64, Signed, F64, _) => Some(CONV_I64_TO_F64),
        (W32, Unsigned, F32, _) => Some(CONV_U32_TO_F32),
        (W32, Unsigned, F64, _) => Some(CONV_U32_TO_F64),
        (W64, Unsigned, F32, _) => Some(CONV_U64_TO_F32),
        (W64, Unsigned, F64, _) => Some(CONV_U64_TO_F64),

        // Float -> Integer
        (F32, _, W32, Signed) => Some(CONV_F32_TO_I32),
        (F32, _, W64, Signed) => Some(CONV_F32_TO_I64),
        (F64, _, W32, Signed) => Some(CONV_F64_TO_I32),
        (F64, _, W64, Signed) => Some(CONV_F64_TO_I64),
        (F32, _, W32, Unsigned) => Some(CONV_F32_TO_U32),
        (F32, _, W64, Unsigned) => Some(CONV_F32_TO_U64),
        (F64, _, W32, Unsigned) => Some(CONV_F64_TO_U32),
        (F64, _, W64, Unsigned) => Some(CONV_F64_TO_U64),

        // Float -> Float
        (F32, _, F64, _) => Some(CONV_F32_TO_F64),
        (F64, _, F32, _) => Some(CONV_F64_TO_F32),

        // Same float width (shouldn't happen, but handle gracefully)
        (F32, _, F32, _) | (F64, _, F64, _) => None,
    };
    match func_id {
        Some(func_id) => emitter.emit_builtin(func_id),
        None => Ok(()),
    }
}

// --- FB type helpers and string conversion (moved from compile.rs) ---

/// The VM block that runs the standard function block `block`, as
/// `(type_id, total_num_fields, field_name -> index)`.
///
/// The analyzer records which standard block a type is on the type itself,
/// so a user-defined block named like one never reaches this table.
pub(crate) fn standard_fb_layout(
    block: StandardFunctionBlock,
) -> (FbTypeId, usize, HashMap<String, u8>) {
    use CounterWidth::*;
    use StandardFunctionBlock::*;
    let (type_id, fields, field_map) = match block {
        Ton => (opcode::fb_type::TON, 6, timer_fb_fields()),
        Tof => (opcode::fb_type::TOF, 6, timer_fb_fields()),
        Tp => (opcode::fb_type::TP, 6, timer_fb_fields()),
        // Each width of a counter counts in its own type: `INT` and `DINT`
        // in a signed 32-bit integer, the others in their own.
        Ctu(Int | Dint) => (opcode::fb_type::CTU_DINT, 6, ctu_fb_fields()),
        Ctu(Udint) => (opcode::fb_type::CTU_UDINT, 6, ctu_fb_fields()),
        Ctu(Lint) => (opcode::fb_type::CTU_LINT, 6, ctu_fb_fields()),
        Ctu(Ulint) => (opcode::fb_type::CTU_ULINT, 6, ctu_fb_fields()),
        Ctd(Int | Dint) => (opcode::fb_type::CTD_DINT, 6, ctd_fb_fields()),
        Ctd(Udint) => (opcode::fb_type::CTD_UDINT, 6, ctd_fb_fields()),
        Ctd(Lint) => (opcode::fb_type::CTD_LINT, 6, ctd_fb_fields()),
        Ctd(Ulint) => (opcode::fb_type::CTD_ULINT, 6, ctd_fb_fields()),
        Ctud(Int | Dint) => (opcode::fb_type::CTUD_DINT, 10, ctud_fb_fields()),
        Ctud(Udint) => (opcode::fb_type::CTUD_UDINT, 10, ctud_fb_fields()),
        Ctud(Lint) => (opcode::fb_type::CTUD_LINT, 10, ctud_fb_fields()),
        Ctud(Ulint) => (opcode::fb_type::CTUD_ULINT, 10, ctud_fb_fields()),
        Sr => (opcode::fb_type::SR, 3, sr_fb_fields()),
        Rs => (opcode::fb_type::RS, 3, rs_fb_fields()),
        RTrig => (opcode::fb_type::R_TRIG, 3, edge_trig_fb_fields()),
        FTrig => (opcode::fb_type::F_TRIG, 3, edge_trig_fb_fields()),
    };
    (FbTypeId::new(type_id), fields, field_map)
}

/// The standard function block each POU of `program` stands for, which
/// runs in the VM: the block whose VM block [`standard_fb_layout`] gives a
/// lowered call of the POU.
pub(crate) fn standard_blocks(program: &Program) -> HashMap<PouId, StandardFunctionBlock> {
    let pous = program.pous();
    pous.ids()
        .filter_map(|pou| Some((pou, pous.standard_block(pou)?)))
        .collect()
}

/// Returns the shared field map for timer FBs (TON, TOF, TP).
/// Fields 4-5 are hidden (start_time, running) and not included.
fn timer_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("in".to_string(), 0);
    fields.insert("pt".to_string(), 1);
    fields.insert("q".to_string(), 2);
    fields.insert("et".to_string(), 3);
    fields
}

/// Returns the field map for CTU (count up) FBs.
/// Field 5 is hidden (prev_cu) and not included.
fn ctu_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTD (count down) FBs.
/// Field 5 is hidden (prev_cd) and not included.
fn ctd_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cd".to_string(), 0);
    fields.insert("ld".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTUD (count up/down) FBs.
/// Fields 8-9 are hidden (prev_cu, prev_cd) and not included.
fn ctud_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("cd".to_string(), 1);
    fields.insert("r".to_string(), 2);
    fields.insert("ld".to_string(), 3);
    fields.insert("pv".to_string(), 4);
    fields.insert("qu".to_string(), 5);
    fields.insert("qd".to_string(), 6);
    fields.insert("cv".to_string(), 7);
    fields
}

/// Returns the field map for SR (set-reset) FBs.
fn sr_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s1".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for RS (reset-set) FBs.
fn rs_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s".to_string(), 0);
    fields.insert("r1".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for edge trigger FBs (R_TRIG, F_TRIG).
/// Field 2 is hidden (M / previous CLK) and not included.
fn edge_trig_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("clk".to_string(), 0);
    fields.insert("q".to_string(), 1);
    fields
}
