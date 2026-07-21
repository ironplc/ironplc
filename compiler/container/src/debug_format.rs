//! Shared helpers for rendering VM variable values from a [`Container`].
//!
//! These were previously duplicated across `ironplc-cli` (the LSP runner),
//! `vm-cli`, and the playground WASM crate. The format produced by
//! [`format_variable_value`] follows REQ-VC-vm-cli-009 in
//! `specs/design/vm-cli.md` so the CLI dump remains spec-compliant.
//! Tools such as the playground that need richer rendering (decimal
//! seconds, `D#YYYY-MM-DD`, enum value names) can still wrap or replace
//! these helpers locally.

use std::collections::HashMap;
use std::format;
use std::string::String;

use crate::debug_section::iec_type_tag;
use crate::Container;

/// Debug metadata for a single variable, extracted from the container's
/// debug section.
pub struct VarDebugInfo {
    pub name: String,
    pub type_name: String,
    pub iec_type_tag: u8,
}

/// Builds a lookup map from variable index to [`VarDebugInfo`] from the
/// container's debug section. Returns an empty map when the container
/// carries no debug section.
pub fn build_var_debug_map(container: &Container) -> HashMap<u16, VarDebugInfo> {
    let mut map = HashMap::new();
    if let Some(debug) = &container.debug_section {
        for entry in &debug.var_names {
            map.insert(
                entry.var_index.raw(),
                VarDebugInfo {
                    name: entry.name.clone(),
                    type_name: entry.type_name.clone(),
                    iec_type_tag: entry.iec_type_tag,
                },
            );
        }
    }
    map
}

/// Formats a raw 64-bit slot value for display, interpreting it according
/// to the supplied IEC type tag. Unknown tags fall back to a signed 32-bit
/// decimal so display never panics.
pub fn format_variable_value(raw: u64, tag: u8) -> String {
    match tag {
        iec_type_tag::BOOL => {
            if (raw as i32) != 0 {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        iec_type_tag::SINT => format!("{}", raw as i32 as i8),
        iec_type_tag::INT => format!("{}", raw as i32 as i16),
        iec_type_tag::DINT => format!("{}", raw as i32),
        iec_type_tag::LINT => format!("{}", raw as i64),
        iec_type_tag::USINT => format!("{}", raw as u8),
        iec_type_tag::UINT => format!("{}", raw as u16),
        iec_type_tag::UDINT => format!("{}", raw as u32),
        iec_type_tag::ULINT => format!("{raw}"),
        iec_type_tag::REAL => format!("{}", f32::from_bits(raw as u32)),
        iec_type_tag::LREAL => format!("{}", f64::from_bits(raw)),
        iec_type_tag::BYTE => format!("16#{:02X}", raw as u8),
        iec_type_tag::WORD => format!("16#{:04X}", raw as u16),
        iec_type_tag::DWORD => format!("16#{:08X}", raw as u32),
        iec_type_tag::LWORD => format!("16#{:016X}", raw),
        iec_type_tag::TIME => format!("T#{}ms", raw as i32),
        iec_type_tag::LTIME => format!("LTIME#{}ms", raw as i64),
        _ => format!("{}", raw as i32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    use crate::debug_section::{function_id, iec_type_tag, var_section, VarNameEntry};
    use crate::id_types::VarIndex;
    use crate::ContainerBuilder;

    fn container_with_debug(entries: Vec<VarNameEntry>) -> Container {
        let mut builder = ContainerBuilder::new();
        for entry in entries {
            builder = builder.add_var_name(entry);
        }
        builder.build()
    }

    #[test]
    fn build_var_debug_map_when_no_debug_section_then_empty() {
        let container = ContainerBuilder::new().build();
        let map = build_var_debug_map(&container);
        assert!(map.is_empty());
    }

    #[test]
    fn build_var_debug_map_when_entries_present_then_indexed_by_var_index() {
        let container = container_with_debug(vec![
            VarNameEntry {
                var_index: VarIndex::new(0),
                function_id: function_id::GLOBAL_SCOPE,
                var_section: var_section::VAR,
                iec_type_tag: iec_type_tag::DINT,
                name: "counter".into(),
                type_name: "DINT".into(),
            },
            VarNameEntry {
                var_index: VarIndex::new(2),
                function_id: function_id::GLOBAL_SCOPE,
                var_section: var_section::VAR,
                iec_type_tag: iec_type_tag::BOOL,
                name: "flag".into(),
                type_name: "BOOL".into(),
            },
        ]);

        let map = build_var_debug_map(&container);
        assert_eq!(map.len(), 2);
        let counter = map.get(&0).expect("var index 0 present");
        assert_eq!(counter.name, "counter");
        assert_eq!(counter.type_name, "DINT");
        assert_eq!(counter.iec_type_tag, iec_type_tag::DINT);
        let flag = map.get(&2).expect("var index 2 present");
        assert_eq!(flag.name, "flag");
        assert_eq!(flag.type_name, "BOOL");
        assert_eq!(flag.iec_type_tag, iec_type_tag::BOOL);
    }

    #[test]
    fn format_variable_value_when_bool_then_true_or_false() {
        assert_eq!(format_variable_value(0, iec_type_tag::BOOL), "FALSE");
        assert_eq!(format_variable_value(1, iec_type_tag::BOOL), "TRUE");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::BOOL),
            "TRUE"
        );
    }

    #[test]
    fn format_variable_value_when_signed_int_then_signed_decimal() {
        assert_eq!(format_variable_value(0xFF, iec_type_tag::SINT), "-1");
        assert_eq!(format_variable_value(0xFFFF, iec_type_tag::INT), "-1");
        assert_eq!(format_variable_value(0xFFFF_FFFF, iec_type_tag::DINT), "-1");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_FFFF_FFFF, iec_type_tag::LINT),
            "-1"
        );
    }

    #[test]
    fn format_variable_value_when_unsigned_int_then_unsigned_decimal() {
        assert_eq!(format_variable_value(0xFF, iec_type_tag::USINT), "255");
        assert_eq!(format_variable_value(0xFFFF, iec_type_tag::UINT), "65535");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::UDINT),
            "4294967295"
        );
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_FFFF_FFFF, iec_type_tag::ULINT),
            "18446744073709551615"
        );
    }

    #[test]
    fn format_variable_value_when_real_then_float_decimal() {
        let raw32 = 1.5_f32.to_bits() as u64;
        assert_eq!(format_variable_value(raw32, iec_type_tag::REAL), "1.5");
        let raw64 = 2.25_f64.to_bits();
        assert_eq!(format_variable_value(raw64, iec_type_tag::LREAL), "2.25");
    }

    #[test]
    fn format_variable_value_when_bit_string_then_iec_hex() {
        assert_eq!(format_variable_value(0xAB, iec_type_tag::BYTE), "16#AB");
        assert_eq!(format_variable_value(0x0F, iec_type_tag::BYTE), "16#0F");
        assert_eq!(format_variable_value(0xABCD, iec_type_tag::WORD), "16#ABCD");
        assert_eq!(
            format_variable_value(0xDEAD_BEEF, iec_type_tag::DWORD),
            "16#DEADBEEF"
        );
        assert_eq!(
            format_variable_value(0x0000_0000_DEAD_BEEF, iec_type_tag::LWORD),
            "16#00000000DEADBEEF"
        );
    }

    #[test]
    fn format_variable_value_when_time_then_iec_duration() {
        assert_eq!(format_variable_value(250, iec_type_tag::TIME), "T#250ms");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::TIME),
            "T#-1ms"
        );
        assert_eq!(
            format_variable_value(10_000, iec_type_tag::LTIME),
            "LTIME#10000ms"
        );
    }

    #[test]
    fn format_variable_value_when_unknown_tag_then_signed_i32_fallback() {
        assert_eq!(format_variable_value(42, iec_type_tag::OTHER), "42");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::OTHER),
            "-1"
        );
    }
}
