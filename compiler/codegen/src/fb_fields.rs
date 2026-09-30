//! The field layout of a function block type, as calls and member accesses
//! see it: which data-region field each named input, output or variable of
//! an instance occupies.
//!
//! A user-defined block's layout is built from its declaration by
//! `compile::compile_program_with_functions`; a standard-library block's
//! comes from [`resolve_fb_type`], and must agree with the field order its VM
//! intrinsic reads.
//!
//! The layout also keeps the block's `VAR_INPUT` fields in declaration
//! order: a non-formal call, `inst(1, 2)`, binds its arguments to them by
//! position (IEC 61131-3 non-formal call). The analyzer binds and counts the
//! same list (`call_assignment_check::bind_inputs`, P4003).

use std::collections::HashMap;

use ironplc_container::opcode;

/// The fields of a function block type that a call or a member access can
/// name, each with its index in an instance's data region.
///
/// Names are lowercase: IEC 61131-3 identifiers are case-insensitive.
#[derive(Clone, Debug, Default)]
pub(crate) struct FbFields {
    indices: HashMap<String, u8>,
    /// The `VAR_INPUT` fields, name and index, in declaration order.
    inputs: Vec<(String, u8)>,
}

impl FbFields {
    /// The layout of a standard-library block: its inputs at indices 0, 1,
    /// ... in declaration order, then its outputs. Hidden state the
    /// intrinsic keeps after the outputs has no name and is not listed.
    fn declared(inputs: &[&str], outputs: &[&str]) -> Self {
        let mut fields = FbFields::default();
        // The names lead each `zip`: it stops on the first iterator that
        // runs out, so an index taken past the last input would be lost.
        let mut indices = 0_u8..;
        for (name, index) in inputs.iter().zip(indices.by_ref()) {
            fields.insert_input(name.to_string(), index);
        }
        for (name, index) in outputs.iter().zip(indices) {
            fields.insert(name.to_string(), index);
        }
        fields
    }

    /// Records that the field `name` (lowercase) is at `index`.
    pub(crate) fn insert(&mut self, name: String, index: u8) {
        self.indices.insert(name, index);
    }

    /// Records that the `VAR_INPUT` field `name` (lowercase) is at `index`.
    /// Inputs are recorded in declaration order: the next one recorded is
    /// the next position of a non-formal call.
    pub(crate) fn insert_input(&mut self, name: String, index: u8) {
        self.inputs.push((name.clone(), index));
        self.insert(name, index);
    }

    /// The `VAR_INPUT` fields, name and index, in declaration order: the
    /// fields the arguments of a non-formal call bind to, one by one.
    pub(crate) fn inputs(&self) -> impl Iterator<Item = (&str, u8)> {
        self.inputs
            .iter()
            .map(|(name, index)| (name.as_str(), *index))
    }

    /// The index of the field `name` (lowercase), if the block has one.
    pub(crate) fn index_of(&self, name: &str) -> Option<u8> {
        self.indices.get(name).copied()
    }
}

/// Resolves a standard FB type name to its (type_id, total_num_fields,
/// field layout). Returns None for unknown FB types.
pub(crate) fn resolve_fb_type(name: &str) -> Option<(u16, usize, FbFields)> {
    match name {
        "TON" => Some((opcode::fb_type::TON, 6, timer_fb_fields())),
        "TOF" => Some((opcode::fb_type::TOF, 6, timer_fb_fields())),
        "TP" => Some((opcode::fb_type::TP, 6, timer_fb_fields())),
        "CTU" | "CTU_INT" | "CTU_DINT" | "CTU_LINT" | "CTU_UDINT" | "CTU_ULINT" => {
            Some((opcode::fb_type::CTU, 6, ctu_fb_fields()))
        }
        "CTD" | "CTD_INT" | "CTD_DINT" | "CTD_LINT" | "CTD_UDINT" | "CTD_ULINT" => {
            Some((opcode::fb_type::CTD, 6, ctd_fb_fields()))
        }
        "CTUD" | "CTUD_INT" | "CTUD_DINT" | "CTUD_LINT" | "CTUD_UDINT" | "CTUD_ULINT" => {
            Some((opcode::fb_type::CTUD, 10, ctud_fb_fields()))
        }
        "SR" => Some((opcode::fb_type::SR, 3, sr_fb_fields())),
        "RS" => Some((opcode::fb_type::RS, 3, rs_fb_fields())),
        "R_TRIG" => Some((opcode::fb_type::R_TRIG, 3, edge_trig_fb_fields())),
        "F_TRIG" => Some((opcode::fb_type::F_TRIG, 3, edge_trig_fb_fields())),
        _ => None,
    }
}

/// Returns the shared field layout for timer FBs (TON, TOF, TP).
/// Fields 4-5 are hidden (start_time, running) and not included.
fn timer_fb_fields() -> FbFields {
    FbFields::declared(&["in", "pt"], &["q", "et"])
}

/// Returns the field layout for CTU (count up) FBs.
/// Field 5 is hidden (prev_cu) and not included.
fn ctu_fb_fields() -> FbFields {
    FbFields::declared(&["cu", "r", "pv"], &["q", "cv"])
}

/// Returns the field layout for CTD (count down) FBs.
/// Field 5 is hidden (prev_cd) and not included.
fn ctd_fb_fields() -> FbFields {
    FbFields::declared(&["cd", "ld", "pv"], &["q", "cv"])
}

/// Returns the field layout for CTUD (count up/down) FBs.
/// Fields 8-9 are hidden (prev_cu, prev_cd) and not included.
fn ctud_fb_fields() -> FbFields {
    FbFields::declared(&["cu", "cd", "r", "ld", "pv"], &["qu", "qd", "cv"])
}

/// Returns the field layout for SR (set-reset) FBs.
fn sr_fb_fields() -> FbFields {
    FbFields::declared(&["s1", "r"], &["q1"])
}

/// Returns the field layout for RS (reset-set) FBs.
fn rs_fb_fields() -> FbFields {
    FbFields::declared(&["s", "r1"], &["q1"])
}

/// Returns the field layout for edge trigger FBs (R_TRIG, F_TRIG).
/// Field 2 is hidden (M / previous CLK) and not included.
fn edge_trig_fb_fields() -> FbFields {
    FbFields::declared(&["clk"], &["q"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_fb_type_when_ctud_then_inputs_precede_outputs() {
        let (_, _, fields) = resolve_fb_type("CTUD").unwrap();
        assert_eq!(fields.index_of("cu"), Some(0));
        assert_eq!(fields.index_of("pv"), Some(4));
        assert_eq!(fields.index_of("qu"), Some(5));
        assert_eq!(fields.index_of("cv"), Some(7));
    }

    #[test]
    fn resolve_fb_type_when_sr_then_output_follows_last_input() {
        let (_, _, fields) = resolve_fb_type("SR").unwrap();
        assert_eq!(fields.index_of("r"), Some(1));
        assert_eq!(fields.index_of("q1"), Some(2));
    }

    #[test]
    fn inputs_when_ctu_then_lists_inputs_in_declaration_order() {
        let (_, _, fields) = resolve_fb_type("CTU").unwrap();
        let inputs: Vec<(&str, u8)> = fields.inputs().collect();
        assert_eq!(vec![("cu", 0), ("r", 1), ("pv", 2)], inputs);
    }

    #[test]
    fn resolve_fb_type_when_unknown_then_none() {
        assert!(resolve_fb_type("NOT_A_BLOCK").is_none());
    }

    #[test]
    fn index_of_when_field_not_declared_then_none() {
        let (_, _, fields) = resolve_fb_type("TON").unwrap();
        assert_eq!(fields.index_of("start_time"), None);
    }
}
