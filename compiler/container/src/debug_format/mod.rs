//! The one place that turns a VM variable into display text.
//!
//! Four surfaces show variable values — `ironplcvm run --dump-vars`, the DAP
//! server's variables pane, the LSP run panel, and the playground — and every
//! one of them goes through [`VariableRenderer`]. That is the module's whole
//! purpose: the rendering rules are a matter of policy (which literal syntax,
//! which unit, what to show when a value cannot be read), and policy split
//! across four copies drifts. It has: the same `STRING` once printed as `0` in
//! the CLI dump and as its content in the playground, and the same `TIME` once
//! printed in two different units.
//!
//! Only three type families can be rendered from the raw slot alone, so the
//! renderer — not a free function over `(raw, tag)` — is the entry point:
//!
//! - `STRING`/`WSTRING` slots are unused; the content lives in the data region
//!   at the offset recorded in the debug section's STRING layout sub-table.
//! - An enumeration's slot holds an ordinal; the value name lives in the
//!   ENUM_DEF sub-table, keyed by the variable's declared type name.
//! - Everything else renders from the slot and its IEC type tag.
//!
//! The rendering rules are specified in
//! `specs/design/variable-value-rendering.md` (`REQ-VR-container-*`).

mod datetime;
mod string_value;

use std::collections::HashMap;
use std::format;
use std::string::{String, ToString};

use crate::debug_section::{iec_type_tag, DebugSection};
use crate::Container;

use string_value::read_string_value;

/// Shown for a string variable whose debug section carries no STRING layout
/// entry, so there is no data-region offset to read from.
pub const VALUE_UNAVAILABLE: &str = "<unavailable>";

/// Shown for a string variable whose recorded layout does not fit the data
/// region — a corrupt or mismatched container.
pub const VALUE_INVALID: &str = "<invalid>";

/// Shown for an aggregate whose declared type name is not recorded, so not
/// even `<TYPE_NAME>` can be produced.
pub const VALUE_AGGREGATE: &str = "<aggregate>";

/// Debug metadata for a single variable, extracted from the container's
/// debug section.
pub struct VarDebugInfo {
    pub name: String,
    pub type_name: String,
    pub iec_type_tag: u8,
}

/// A variable value rendered for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedValue {
    /// The display text.
    pub text: String,
    /// `false` when `text` is a placeholder ([`VALUE_UNAVAILABLE`],
    /// [`VALUE_INVALID`]) rather than the variable's value.
    ///
    /// A surface that can style values distinguishes the two so a placeholder
    /// never reads as content — which is exactly how a `STRING` rendered as
    /// its unused slot came to look like a real `0`.
    pub valid: bool,
}

impl RenderedValue {
    fn value(text: String) -> Self {
        RenderedValue { text, valid: true }
    }

    fn placeholder(text: &str) -> Self {
        RenderedValue {
            text: text.to_string(),
            valid: false,
        }
    }
}

/// Renders variable values for display, using a container's debug section.
///
/// Built once per container and reused across every variable and every scan:
/// it holds the three debug sub-tables a rendering needs (VAR_NAME, STRING
/// layouts, ENUM_DEF) as lookups rather than re-scanning the vectors per slot.
///
/// A container with no debug section yields a renderer that still works — every
/// variable falls back to `var[i]` and a signed decimal — so a surface needs no
/// separate no-debug-info path.
pub struct VariableRenderer {
    vars: HashMap<u16, VarDebugInfo>,
    string_offsets: HashMap<u16, u32>,
    enum_values: HashMap<(String, i32), String>,
}

impl VariableRenderer {
    /// Builds a renderer from a container's debug section.
    pub fn new(container: &Container) -> Self {
        Self::from_debug_section(container.debug_section.as_ref())
    }

    /// Builds a renderer from a debug section directly, for a caller that
    /// holds one without the surrounding [`Container`].
    pub fn from_debug_section(debug: Option<&DebugSection>) -> Self {
        let mut vars = HashMap::new();
        let mut string_offsets = HashMap::new();
        let mut enum_values = HashMap::new();

        if let Some(debug) = debug {
            for entry in &debug.var_names {
                vars.insert(
                    entry.var_index.raw(),
                    VarDebugInfo {
                        name: entry.name.clone(),
                        type_name: entry.type_name.clone(),
                        iec_type_tag: entry.iec_type_tag,
                    },
                );
            }
            for layout in &debug.string_layouts {
                string_offsets.insert(layout.var_index.raw(), layout.data_offset);
            }
            for entry in &debug.enum_defs {
                for (ordinal, value_name) in entry.values.iter().enumerate() {
                    enum_values.insert(
                        (entry.type_name.clone(), ordinal as i32),
                        value_name.clone(),
                    );
                }
            }
        }

        VariableRenderer {
            vars,
            string_offsets,
            enum_values,
        }
    }

    /// The debug metadata for a variable, or `None` when the container's debug
    /// section does not name it.
    pub fn var(&self, index: u16) -> Option<&VarDebugInfo> {
        self.vars.get(&index)
    }

    /// The variable's source name, or `var[<index>]` when it has none.
    pub fn name(&self, index: u16) -> String {
        match self.vars.get(&index) {
            Some(info) => info.name.clone(),
            None => format!("var[{index}]"),
        }
    }

    /// Renders the variable's value.
    ///
    /// `raw` is the variable table slot; `data_region` is the VM's data region,
    /// which backs `STRING` and `WSTRING` content. Pass an empty slice when the
    /// caller has no data region — string variables then render as
    /// [`VALUE_INVALID`] rather than as a wrong value.
    pub fn render(&self, index: u16, raw: u64, data_region: &[u8]) -> RenderedValue {
        let Some(info) = self.vars.get(&index) else {
            // No debug entry: nothing identifies the type, so show the slot.
            return RenderedValue::value(format!("{}", raw as i32));
        };

        match info.iec_type_tag {
            iec_type_tag::STRING | iec_type_tag::WSTRING => self.render_string(index, data_region),
            iec_type_tag::STRUCT | iec_type_tag::ARRAY | iec_type_tag::FB_INSTANCE => {
                render_aggregate(&info.type_name)
            }
            tag => match self.enum_value_name(&info.type_name, raw) {
                Some(text) => RenderedValue::value(text),
                None => RenderedValue::value(format_slot_value(raw, tag)),
            },
        }
    }

    /// Renders one variable as a `<name>: <value>` line.
    pub fn line(&self, index: u16, raw: u64, data_region: &[u8]) -> String {
        format!(
            "{}: {}",
            self.name(index),
            self.render(index, raw, data_region).text
        )
    }

    /// Reads a string variable's content out of the data region. The encoding
    /// comes from the value's own header, so `STRING` and `WSTRING` take the
    /// same path.
    fn render_string(&self, index: u16, data_region: &[u8]) -> RenderedValue {
        let Some(&offset) = self.string_offsets.get(&index) else {
            return RenderedValue::placeholder(VALUE_UNAVAILABLE);
        };
        match read_string_value(data_region, offset) {
            Ok(text) => RenderedValue::value(text),
            Err(_) => RenderedValue::placeholder(VALUE_INVALID),
        }
    }

    /// The `NAME (ordinal)` rendering for a slot holding an enumeration value,
    /// or `None` when the declared type is not a known enumeration or the
    /// ordinal is out of its range.
    fn enum_value_name(&self, type_name: &str, raw: u64) -> Option<String> {
        if type_name.is_empty() {
            return None;
        }
        let ordinal = raw as i32;
        let value_name = self.enum_values.get(&(type_name.to_string(), ordinal))?;
        Some(format!("{value_name} ({ordinal})"))
    }
}

/// Names an aggregate whose contents this renderer cannot reach.
///
/// A structure, array or function-block instance keeps its contents in the
/// data region and its slot holds the byte offset of them. Rendering that slot
/// publishes an internal layout detail as if it were program data — and a
/// convincing one, since the offset moves when an unrelated declaration
/// changes size. So the value is reported as absent, named by its declared
/// type, and marked invalid so a surface that styles values shows it as the
/// placeholder it is.
///
/// Rendering the contents themselves needs a debug sub-table describing field
/// and element layout, which the container does not yet carry.
fn render_aggregate(type_name: &str) -> RenderedValue {
    if type_name.is_empty() {
        return RenderedValue::placeholder(VALUE_AGGREGATE);
    }
    RenderedValue {
        text: format!("<{type_name}>"),
        valid: false,
    }
}

/// Formats a raw 64-bit slot according to its IEC type tag.
///
/// Private on purpose: it can only render the types whose whole value fits in
/// the slot, and a caller reaching past [`VariableRenderer`] for the ones that
/// do not is how `STRING` came to print as `0`. An unknown tag falls back to a
/// signed 32-bit decimal so display never panics.
fn format_slot_value(raw: u64, tag: u8) -> String {
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
        iec_type_tag::LWORD => format!("16#{raw:016X}"),
        iec_type_tag::TIME => datetime::format_duration("T", raw as i32 as i64),
        iec_type_tag::LTIME => datetime::format_duration("LTIME", raw as i64),
        iec_type_tag::DATE => datetime::format_date("D", raw as u32 as u64),
        iec_type_tag::LDATE => datetime::format_date("LDATE", raw),
        iec_type_tag::TIME_OF_DAY => datetime::format_time_of_day("TOD", raw as u32 as u64),
        iec_type_tag::LTOD => datetime::format_time_of_day("LTOD", raw),
        iec_type_tag::DATE_AND_TIME => datetime::format_date_and_time("DT", raw as u32 as u64),
        iec_type_tag::LDT => datetime::format_date_and_time("LDT", raw),
        // STRING and WSTRING never reach here — `VariableRenderer::render`
        // routes them to the data region, whose content their slot lacks.
        _ => format!("{}", raw as i32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    use spec_test_macro::spec_test;

    use crate::debug_section::{function_id, var_section, EnumDefEntry, VarNameEntry};
    use crate::id_types::VarIndex;
    use crate::{ContainerBuilder, StringLayoutEntry};

    fn var(index: u16, tag: u8, name: &str, type_name: &str) -> VarNameEntry {
        VarNameEntry {
            var_index: VarIndex::new(index),
            function_id: function_id::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: tag,
            name: name.into(),
            type_name: type_name.into(),
        }
    }

    /// A renderer over a debug section holding exactly the given variables.
    fn renderer_for(vars: Vec<VarNameEntry>) -> VariableRenderer {
        let mut builder = ContainerBuilder::new();
        for entry in vars {
            builder = builder.add_var_name(entry);
        }
        VariableRenderer::new(&builder.build())
    }

    /// A renderer over one string variable at data-region offset `offset`.
    fn string_renderer(tag: u8, offset: u32) -> VariableRenderer {
        let container = ContainerBuilder::new()
            .add_var_name(var(0, tag, "msg", "STRING"))
            .add_string_layout(StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: offset,
                max_length: 20,
            })
            .build();
        VariableRenderer::new(&container)
    }

    /// A data region holding one narrow string value at offset 0.
    fn narrow_region(text: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&(text.len() as u16).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(text);
        data
    }

    /// The rendered text of the only variable, given a raw slot value.
    fn text(renderer: &VariableRenderer, raw: u64) -> String {
        renderer.render(0, raw, &[]).text
    }

    /// The rendered text of a variable of the given type tag.
    fn tagged(tag: u8, raw: u64) -> String {
        text(&renderer_for(vec![var(0, tag, "v", "")]), raw)
    }

    /// REQ-VR-container-001: no debug section — indexed name, decimal value.
    #[spec_test(REQ_VR_container_001)]
    fn render_when_no_debug_section_then_indexed_name_and_decimal_value() {
        let renderer = VariableRenderer::from_debug_section(None);
        assert_eq!(renderer.name(3), "var[3]");
        assert_eq!(renderer.render(3, 42, &[]).text, "42");
        assert_eq!(renderer.render(3, 0xFFFF_FFFF, &[]).text, "-1");
    }

    /// REQ-VR-container-002: a named variable renders with its source name.
    #[spec_test(REQ_VR_container_002)]
    fn name_when_variable_named_then_source_name_else_indexed() {
        let renderer = renderer_for(vec![var(0, iec_type_tag::DINT, "counter", "DINT")]);
        assert_eq!(renderer.name(0), "counter");
        assert_eq!(renderer.name(1), "var[1]");
    }

    /// REQ-VR-container-003: the line rendering is `<name>: <value>`.
    #[spec_test(REQ_VR_container_003)]
    fn line_when_named_variable_then_name_colon_value() {
        let renderer = renderer_for(vec![var(0, iec_type_tag::DINT, "counter", "DINT")]);
        assert_eq!(renderer.line(0, 42, &[]), "counter: 42");
        assert_eq!(renderer.line(1, 7, &[]), "var[1]: 7");
    }

    /// REQ-VR-container-010: the type tag selects the rendering.
    #[spec_test(REQ_VR_container_010)]
    fn render_when_same_slot_and_different_tags_then_different_text() {
        let raw = 1;
        assert_eq!(tagged(iec_type_tag::DINT, raw), "1");
        assert_eq!(tagged(iec_type_tag::BOOL, raw), "TRUE");
        assert_eq!(tagged(iec_type_tag::BYTE, raw), "16#01");
        assert_eq!(tagged(iec_type_tag::TIME, raw), "T#1ms");
    }

    /// REQ-VR-container-011: BOOL renders as TRUE/FALSE.
    #[spec_test(REQ_VR_container_011)]
    fn render_when_bool_then_true_or_false() {
        assert_eq!(tagged(iec_type_tag::BOOL, 0), "FALSE");
        assert_eq!(tagged(iec_type_tag::BOOL, 1), "TRUE");
        assert_eq!(tagged(iec_type_tag::BOOL, 0xFFFF_FFFF), "TRUE");
    }

    /// REQ-VR-container-012: signed integers render as signed decimals.
    #[spec_test(REQ_VR_container_012)]
    fn render_when_signed_integer_then_signed_decimal() {
        assert_eq!(tagged(iec_type_tag::SINT, 0xFF), "-1");
        assert_eq!(tagged(iec_type_tag::INT, 0xFFFF), "-1");
        assert_eq!(tagged(iec_type_tag::DINT, 0xFFFF_FFFF), "-1");
        assert_eq!(tagged(iec_type_tag::LINT, 0xFFFF_FFFF_FFFF_FFFF), "-1");
        assert_eq!(tagged(iec_type_tag::DINT, 42), "42");
    }

    /// REQ-VR-container-013: unsigned integers render as unsigned decimals.
    #[spec_test(REQ_VR_container_013)]
    fn render_when_unsigned_integer_then_unsigned_decimal() {
        assert_eq!(tagged(iec_type_tag::USINT, 0xFF), "255");
        assert_eq!(tagged(iec_type_tag::UINT, 0xFFFF), "65535");
        assert_eq!(tagged(iec_type_tag::UDINT, 0xFFFF_FFFF), "4294967295");
        assert_eq!(
            tagged(iec_type_tag::ULINT, 0xFFFF_FFFF_FFFF_FFFF),
            "18446744073709551615"
        );
    }

    /// REQ-VR-container-014: REAL and LREAL render as decimals.
    #[spec_test(REQ_VR_container_014)]
    fn render_when_float_then_decimal() {
        assert_eq!(tagged(iec_type_tag::REAL, 1.5_f32.to_bits() as u64), "1.5");
        assert_eq!(tagged(iec_type_tag::LREAL, 2.25_f64.to_bits()), "2.25");
    }

    /// REQ-VR-container-015: bit strings render as IEC hex.
    #[spec_test(REQ_VR_container_015)]
    fn render_when_bit_string_then_iec_hex() {
        assert_eq!(tagged(iec_type_tag::BYTE, 0x0F), "16#0F");
        assert_eq!(tagged(iec_type_tag::WORD, 0xABCD), "16#ABCD");
        assert_eq!(tagged(iec_type_tag::DWORD, 0xDEAD_BEEF), "16#DEADBEEF");
        assert_eq!(
            tagged(iec_type_tag::LWORD, 0xDEAD_BEEF),
            "16#00000000DEADBEEF"
        );
    }

    /// REQ-VR-container-016: durations render in milliseconds.
    #[spec_test(REQ_VR_container_016)]
    fn render_when_duration_then_milliseconds() {
        assert_eq!(tagged(iec_type_tag::TIME, 1500), "T#1500ms");
        assert_eq!(tagged(iec_type_tag::LTIME, 10_000), "LTIME#10000ms");
    }

    /// REQ-VR-container-017: dates render as calendar dates.
    #[spec_test(REQ_VR_container_017)]
    fn render_when_date_then_calendar_date() {
        assert_eq!(tagged(iec_type_tag::DATE, 1_705_276_800), "D#2024-01-15");
        assert_eq!(
            tagged(iec_type_tag::LDATE, 1_705_276_800),
            "LDATE#2024-01-15"
        );
    }

    /// REQ-VR-container-018: times of day render as clock times.
    #[spec_test(REQ_VR_container_018)]
    fn render_when_time_of_day_then_clock_time() {
        assert_eq!(
            tagged(iec_type_tag::TIME_OF_DAY, 52_200_000),
            "TOD#14:30:00"
        );
        assert_eq!(tagged(iec_type_tag::LTOD, 86_399_999), "LTOD#23:59:59.999");
    }

    /// REQ-VR-container-019: dates and times render as both.
    #[spec_test(REQ_VR_container_019)]
    fn render_when_date_and_time_then_calendar_date_and_clock_time() {
        assert_eq!(
            tagged(iec_type_tag::DATE_AND_TIME, 1_705_329_000),
            "DT#2024-01-15-14:30:00"
        );
        assert_eq!(
            tagged(iec_type_tag::LDT, 1_705_329_000),
            "LDT#2024-01-15-14:30:00"
        );
    }

    /// REQ-VR-container-020: an unknown tag falls back to a signed decimal.
    #[spec_test(REQ_VR_container_020)]
    fn render_when_unknown_tag_then_signed_decimal_fallback() {
        assert_eq!(tagged(iec_type_tag::OTHER, 42), "42");
        assert_eq!(tagged(iec_type_tag::OTHER, 0xFFFF_FFFF), "-1");
    }

    /// REQ-VR-container-021: a duration's sign follows the `#`.
    #[spec_test(REQ_VR_container_021)]
    fn render_when_negative_duration_then_sign_after_hash() {
        assert_eq!(tagged(iec_type_tag::TIME, 0xFFFF_FFFF), "T#-1ms");
        assert_eq!(
            tagged(iec_type_tag::LTIME, 0xFFFF_FFFF_FFFF_FFFF),
            "LTIME#-1ms"
        );
    }

    /// REQ-VR-container-030: STRING content comes from the data region, not
    /// the slot. This is the defect the module exists to prevent: the slot of
    /// the string below holds 0, which used to be what the CLI dump printed.
    #[spec_test(REQ_VR_container_030)]
    fn render_when_string_then_reads_data_region_not_slot() {
        let renderer = string_renderer(iec_type_tag::STRING, 0);
        let data = narrow_region(b"hello");
        assert_eq!(renderer.render(0, 0, &data).text, "'hello'");
    }

    /// REQ-VR-container-031: the header's own `char_width` sets the byte span
    /// and encoding — a WSTRING is two bytes per code unit.
    #[spec_test(REQ_VR_container_031)]
    fn render_when_wide_string_then_two_bytes_per_code_unit() {
        let renderer = string_renderer(iec_type_tag::WSTRING, 0);
        let mut data = Vec::new();
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&[b'h', 0, b'i', 0]);
        assert_eq!(renderer.render(0, 0, &data).text, "\"hi\"");
    }

    /// REQ-VR-container-032: narrow values are single-quoted, wide values
    /// double-quoted, both with `$` escapes.
    #[spec_test(REQ_VR_container_032)]
    fn render_when_string_then_quoted_iec_literal_with_escapes() {
        let renderer = string_renderer(iec_type_tag::STRING, 0);
        let data = narrow_region(b"it's $5\t");
        assert_eq!(renderer.render(0, 0, &data).text, "'it$'s $$5$T'");
    }

    /// REQ-VR-container-040: a string with no layout entry is unavailable.
    #[spec_test(REQ_VR_container_040)]
    fn render_when_string_without_layout_then_unavailable_placeholder() {
        let renderer = renderer_for(vec![var(0, iec_type_tag::STRING, "msg", "STRING")]);
        let rendered = renderer.render(0, 0, &narrow_region(b"hello"));
        assert_eq!(rendered.text, VALUE_UNAVAILABLE);
        assert!(!rendered.valid);
    }

    /// REQ-VR-container-041: a layout that does not fit the data region, or a
    /// bad `char_width`, is invalid.
    #[spec_test(REQ_VR_container_041)]
    fn render_when_string_layout_does_not_fit_then_invalid_placeholder() {
        let renderer = string_renderer(iec_type_tag::STRING, 64);
        let rendered = renderer.render(0, 0, &narrow_region(b"hello"));
        assert_eq!(rendered.text, VALUE_INVALID);
        assert!(!rendered.valid);

        let mut bad_width = narrow_region(b"hello");
        bad_width[4..6].copy_from_slice(&7u16.to_le_bytes());
        let renderer = string_renderer(iec_type_tag::STRING, 0);
        assert_eq!(renderer.render(0, 0, &bad_width).text, VALUE_INVALID);
    }

    /// REQ-VR-container-042: only placeholders are marked invalid.
    #[spec_test(REQ_VR_container_042)]
    fn render_when_value_is_real_content_then_marked_valid() {
        let renderer = string_renderer(iec_type_tag::STRING, 0);
        assert!(renderer.render(0, 0, &narrow_region(b"hello")).valid);
        assert!(renderer.render(0, 0, &narrow_region(b"")).valid);
        assert!(tagged_rendered(iec_type_tag::DINT, 0).valid);
    }

    fn tagged_rendered(tag: u8, raw: u64) -> RenderedValue {
        renderer_for(vec![var(0, tag, "v", "")]).render(0, raw, &[])
    }

    /// REQ-VR-container-043: an aggregate names its type instead of showing
    /// the data-region offset its slot holds. The raw values below are real
    /// offsets, and each would otherwise print as a convincing integer.
    #[spec_test(REQ_VR_container_043)]
    fn render_when_aggregate_then_type_name_placeholder() {
        let struct_var = renderer_for(vec![var(0, iec_type_tag::STRUCT, "origin", "POINT")]);
        assert_eq!(struct_var.render(0, 0, &[]).text, "<POINT>");
        assert!(!struct_var.render(0, 0, &[]).valid);

        let array_var = renderer_for(vec![var(0, iec_type_tag::ARRAY, "counts", "ARRAY OF DINT")]);
        assert_eq!(array_var.render(0, 16, &[]).text, "<ARRAY OF DINT>");
        assert!(!array_var.render(0, 16, &[]).valid);

        let fb_var = renderer_for(vec![var(0, iec_type_tag::FB_INSTANCE, "timer", "TON")]);
        assert_eq!(fb_var.render(0, 56, &[]).text, "<TON>");
        assert!(!fb_var.render(0, 56, &[]).valid);
    }

    /// REQ-VR-container-043: with no type name recorded there is still no
    /// value to show, so a generic placeholder stands in.
    #[spec_test(REQ_VR_container_043)]
    fn render_when_aggregate_without_type_name_then_generic_placeholder() {
        let renderer = renderer_for(vec![var(0, iec_type_tag::STRUCT, "anon", "")]);
        let rendered = renderer.render(0, 8, &[]);
        assert_eq!(rendered.text, VALUE_AGGREGATE);
        assert!(!rendered.valid);
    }

    /// The aggregate tags are distinct from `OTHER` precisely so that a named
    /// subrange, which does hold its value in the slot, keeps rendering it.
    #[test]
    fn render_when_other_tag_with_type_name_then_still_shows_the_value() {
        let renderer = renderer_for(vec![var(0, iec_type_tag::OTHER, "lvl", "LEVEL")]);
        let rendered = renderer.render(0, 75, &[]);
        assert_eq!(rendered.text, "75");
        assert!(rendered.valid);
    }

    /// A renderer over one enumeration variable of type `COLOR`.
    fn enum_renderer(tag: u8) -> VariableRenderer {
        let container = ContainerBuilder::new()
            .add_var_name(var(0, tag, "shade", "COLOR"))
            .add_enum_def(EnumDefEntry {
                type_name: "COLOR".into(),
                values: vec!["RED".into(), "GREEN".into(), "BLUE".into()],
            })
            .build();
        VariableRenderer::new(&container)
    }

    /// REQ-VR-container-050: an enumeration slot renders its value name.
    #[spec_test(REQ_VR_container_050)]
    fn render_when_enumeration_ordinal_then_value_name_and_ordinal() {
        let renderer = enum_renderer(iec_type_tag::OTHER);
        assert_eq!(renderer.render(0, 0, &[]).text, "RED (0)");
        assert_eq!(renderer.render(0, 2, &[]).text, "BLUE (2)");
    }

    /// REQ-VR-container-051: an ordinal with no name falls back to the tag.
    #[spec_test(REQ_VR_container_051)]
    fn render_when_enumeration_ordinal_out_of_range_then_tag_rendering() {
        let renderer = enum_renderer(iec_type_tag::DINT);
        assert_eq!(renderer.render(0, 9, &[]).text, "9");
    }

    #[test]
    fn var_when_variable_named_then_returns_its_debug_info() {
        let renderer = renderer_for(vec![var(2, iec_type_tag::BOOL, "flag", "BOOL")]);
        let info = renderer.var(2).unwrap();
        assert_eq!(info.name, "flag");
        assert_eq!(info.type_name, "BOOL");
        assert_eq!(info.iec_type_tag, iec_type_tag::BOOL);
        assert!(renderer.var(0).is_none());
    }

    /// A surface with no data region to hand (the DAP renders a slot list on
    /// its own) must not be shown a wrong string value.
    #[test]
    fn render_when_string_and_empty_data_region_then_invalid_placeholder() {
        let renderer = string_renderer(iec_type_tag::STRING, 0);
        assert_eq!(renderer.render(0, 0, &[]).text, VALUE_INVALID);
    }
}
