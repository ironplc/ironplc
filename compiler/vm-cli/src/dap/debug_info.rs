//! The single Layer-1-coupled corner of the DAP server.
//!
//! Everything that maps between the debugger's `(FunctionId, bytecode_offset)`
//! space and *source* coordinates — source line → offset for breakpoints,
//! frame → name/source location for stack traces, and variable slot →
//! name/type/value for inspection — lives here and nowhere else. The rest of
//! the server speaks only in resolved values, so the debug section (line map,
//! VAR_NAME, FUNC_NAME, STRING layouts, source file table, `debug_format`) is
//! a dependency of exactly one module.

use ironplc_container::debug_format::format_variable_value;
use ironplc_container::debug_section::{iec_type_tag, DebugSection, SourceFileEntry};
use ironplc_container::{FunctionId, STRING_HEADER_BYTES};

use super::types::Variable;

/// Placeholder value for a variable whose bytes cannot be read (corrupt
/// STRING layout) or whose rendering is not yet supported (WSTRING).
const VALUE_NOT_AVAILABLE: &str = "<not available>";

/// A source breakpoint resolved against the line map.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedBreakpoint {
    /// The 1-based source line the breakpoint actually bound to — the
    /// requested line, or the next executable line when the requested one
    /// has no code (the standard "snap forward" debugger behavior).
    pub line: i64,
    /// The bytecode locations to arm: for each function with code on the
    /// bound line, the smallest bytecode offset on that line.
    pub locations: Vec<(FunctionId, usize)>,
}

/// A stack frame resolved against FUNC_NAME and the line map.
#[derive(Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// The POU name from FUNC_NAME, or `function {id}` when unnamed.
    pub name: String,
    /// 1-based source line of the paused instruction; `0` when unknown.
    pub line: i64,
    /// 1-based source column; `0` when unknown.
    pub column: i64,
    /// `(file name, recorded path)` from the source file table, when the
    /// line map hit carries a resolvable `file_id`.
    pub source: Option<(String, String)>,
}

/// Resolve a source breakpoint to the line it binds to and the
/// `(function, bytecode offset)` locations that should be armed for it.
///
/// Returns `None` (an unverified breakpoint) when the container has no line
/// map, the requested path does not match any recorded source file, or no
/// executable line exists at or after the requested line.
pub fn resolve_breakpoint(
    debug: Option<&DebugSection>,
    source_path: &str,
    line: i64,
) -> Option<ResolvedBreakpoint> {
    let debug = debug?;
    if line < 0 {
        // Never produced by a conformant client (DAP lines are 1-based).
        return None;
    }

    // Restrict to entries from the requested file. A container without a
    // source file table predates per-file tracking: every entry is eligible.
    let file_ids: Option<Vec<u16>> = if debug.source_files.is_empty() {
        None
    } else {
        let ids: Vec<u16> = debug
            .source_files
            .iter()
            .enumerate()
            .filter(|(_, sf)| file_matches(&sf.path, source_path))
            .map(|(id, _)| id as u16)
            .collect();
        if ids.is_empty() {
            // The breakpoint is in a file this container was not built from.
            return None;
        }
        Some(ids)
    };

    let candidates: Vec<_> = debug
        .line_map
        .iter()
        .filter(|e| {
            e.source_line.raw() != 0 // 0 = unknown line, never a bind target
                && (e.source_line.raw() as i64) >= line
                && file_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&e.file_id.raw()))
        })
        .collect();

    // Snap forward to the nearest executable line.
    let bound = candidates.iter().map(|e| e.source_line.raw()).min()?;

    // Arm each function's first offset on the bound line (statement start).
    let mut locations: Vec<(FunctionId, usize)> = Vec::new();
    for entry in candidates.iter().filter(|e| e.source_line.raw() == bound) {
        let offset = entry.bytecode_offset as usize;
        match locations
            .iter_mut()
            .find(|(function, _)| *function == entry.function_id)
        {
            Some((_, existing)) => *existing = (*existing).min(offset),
            None => locations.push((entry.function_id, offset)),
        }
    }

    Some(ResolvedBreakpoint {
        line: bound as i64,
        locations,
    })
}

/// Whether a recorded source path refers to the same file as a requested
/// path. Exact match first; otherwise the file names are compared (ASCII
/// case-insensitively, for Windows) to absorb absolute-vs-relative and
/// separator differences between what the compiler recorded and what the
/// editor sends.
fn file_matches(recorded: &str, requested: &str) -> bool {
    recorded == requested || file_name(recorded).eq_ignore_ascii_case(file_name(requested))
}

/// The final path component, treating both `/` and `\` as separators.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Resolve a paused frame to its POU name and source location.
///
/// The line map lookup returns the entry enclosing `pc` (largest offset
/// `<= pc`). A frame with no FUNC_NAME entry keeps the `function {id}`
/// fallback; one with no line map hit reports line `0` and no source, which
/// clients render as a name-only frame.
pub fn resolve_frame(
    debug: Option<&DebugSection>,
    function_id: FunctionId,
    pc: usize,
) -> FrameInfo {
    let name = debug
        .and_then(|d| d.func_names.iter().find(|f| f.function_id == function_id))
        .map(|f| f.name.clone())
        .unwrap_or_else(|| format!("function {}", function_id.raw()));

    let location = debug
        .and_then(|d| d.lookup_source_location(function_id, u16::try_from(pc).unwrap_or(u16::MAX)))
        .filter(|e| e.source_line.raw() != 0);

    match location {
        Some(entry) => FrameInfo {
            name,
            line: entry.source_line.raw() as i64,
            column: entry.source_column.raw() as i64,
            source: debug
                .and_then(|d| d.source_files.get(entry.file_id.raw() as usize))
                .map(|sf: &SourceFileEntry| (file_name(&sf.path).to_string(), sf.path.clone())),
        },
        None => FrameInfo {
            name,
            line: 0,
            column: 0,
            source: None,
        },
    }
}

/// Render a run of variable slots for a `variables` response.
///
/// `values[i]` is the raw 64-bit slot for variable index `i`; `data_region`
/// backs STRING reads. A slot with a VAR_NAME entry renders with its source
/// name, declared type, and a value formatted per its IEC type tag; a slot
/// without one (or a container without VAR_NAME) keeps the `var[i]` /
/// signed-decimal fallback so the pane never goes blank.
pub fn render_variables(
    debug: Option<&DebugSection>,
    values: &[u64],
    data_region: &[u8],
) -> Vec<Variable> {
    let entries: std::collections::HashMap<usize, &_> = debug
        .map(|d| {
            d.var_names
                .iter()
                .map(|entry| (entry.var_index.raw() as usize, entry))
                .collect()
        })
        .unwrap_or_default();
    values
        .iter()
        .enumerate()
        .map(|(i, &raw)| {
            let entry = entries.get(&i);
            match entry {
                Some(entry) => Variable {
                    name: entry.name.clone(),
                    value: variable_value(debug, entry.iec_type_tag, i, raw, data_region),
                    type_name: Some(entry.type_name.clone()),
                    variables_reference: 0,
                },
                None => Variable {
                    name: format!("var[{i}]"),
                    value: (raw as i32).to_string(),
                    type_name: None,
                    variables_reference: 0,
                },
            }
        })
        .collect()
}

/// Format one variable's value per its IEC type tag. STRING values live in
/// the data region (the slot is unused); everything else renders from the
/// raw slot via the shared `debug_format` helper.
fn variable_value(
    debug: Option<&DebugSection>,
    tag: u8,
    var_index: usize,
    raw: u64,
    data_region: &[u8],
) -> String {
    match tag {
        iec_type_tag::STRING => debug
            .and_then(|d| {
                d.string_layouts
                    .iter()
                    .find(|layout| layout.var_index.raw() as usize == var_index)
            })
            .and_then(|layout| read_string_value(data_region, layout.data_offset))
            .unwrap_or_else(|| VALUE_NOT_AVAILABLE.to_string()),
        // WSTRING rendering is a v1 cut (as in the playground).
        iec_type_tag::WSTRING => VALUE_NOT_AVAILABLE.to_string(),
        _ => format_variable_value(raw, tag),
    }
}

/// Reads a STRING value from the data region at the given offset and renders
/// it as a single-quoted IEC literal. The layout (ADR-0035) is
/// `[max_len: u16][cur_len: u16][char_width: u16][bytes…]`. Returns `None`
/// when the recorded offset or length would read past the end of the region
/// (corrupt debug info must degrade to a placeholder, not a panic).
fn read_string_value(data_region: &[u8], data_offset: u32) -> Option<String> {
    let off = data_offset as usize;
    if off + STRING_HEADER_BYTES > data_region.len() {
        return None;
    }
    let cur_len = u16::from_le_bytes([data_region[off + 2], data_region[off + 3]]) as usize;
    let start = off + STRING_HEADER_BYTES;
    let end = start + cur_len;
    if end > data_region.len() {
        return None;
    }
    Some(format_iec_string_literal(&data_region[start..end]))
}

/// Renders raw STRING bytes as an IEC 61131-3 single-quoted string literal:
/// printable ASCII passes through, the named escapes (`$T`, `$L`, `$P`,
/// `$R`, `$$`, `$'`) cover the common control characters, and anything else
/// becomes a `$XX` two-digit hex escape.
fn format_iec_string_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('\'');
    for &b in bytes {
        match b {
            b'$' => out.push_str("$$"),
            b'\'' => out.push_str("$'"),
            0x09 => out.push_str("$T"),
            0x0A => out.push_str("$L"),
            0x0C => out.push_str("$P"),
            0x0D => out.push_str("$R"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("${b:02X}")),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{
        var_section, FuncNameEntry, LineMapEntry, SourceFileEntry, StringLayoutEntry, VarNameEntry,
    };
    use ironplc_container::{SourceColumn, SourceFileId, SourceLine, VarIndex};

    fn line_entry(function_id: FunctionId, offset: u16, file: u16, line: u16) -> LineMapEntry {
        LineMapEntry {
            function_id,
            bytecode_offset: offset,
            file_id: SourceFileId::new(file),
            source_line: SourceLine::new(line),
            source_column: SourceColumn::new(1),
        }
    }

    fn source_file(path: &str) -> SourceFileEntry {
        SourceFileEntry {
            path: path.into(),
            content_hash: [0u8; ironplc_container::debug_section::SOURCE_FILE_HASH_LEN],
        }
    }

    fn var_name(index: u16, tag: u8, name: &str, type_name: &str) -> VarNameEntry {
        VarNameEntry {
            var_index: VarIndex::new(index),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: tag,
            name: name.into(),
            type_name: type_name.into(),
        }
    }

    /// A debug section for one scan function: statements on lines 10 and 12
    /// (offsets 0 and 6) of `demo.st`.
    fn a_debug_section() -> DebugSection {
        DebugSection {
            func_names: vec![FuncNameEntry {
                function_id: FunctionId::SCAN,
                name: "MAIN".into(),
            }],
            line_map: vec![
                line_entry(FunctionId::SCAN, 0, 0, 10),
                line_entry(FunctionId::SCAN, 6, 0, 12),
            ],
            source_files: vec![source_file("demo.st")],
            ..DebugSection::default()
        }
    }

    #[test]
    fn resolve_breakpoint_when_line_has_code_then_binds_exactly() {
        let debug = a_debug_section();
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", 10).unwrap();
        assert_eq!(resolved.line, 10);
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 0)]);
    }

    #[test]
    fn resolve_breakpoint_when_line_has_no_code_then_snaps_forward() {
        let debug = a_debug_section();
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", 11).unwrap();
        assert_eq!(resolved.line, 12);
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 6)]);
    }

    #[test]
    fn resolve_breakpoint_when_line_past_end_then_none() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "demo.st", 13).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_line_negative_then_none() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "demo.st", -1).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_no_debug_or_no_line_map_then_none() {
        assert!(resolve_breakpoint(None, "demo.st", 10).is_none());
        let empty = DebugSection::default();
        assert!(resolve_breakpoint(Some(&empty), "demo.st", 10).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_path_differs_by_directory_then_matches_by_file_name() {
        let debug = a_debug_section();
        let resolved = resolve_breakpoint(Some(&debug), "/work/project/demo.st", 10).unwrap();
        assert_eq!(resolved.line, 10);
        // Windows-style requested path also matches on the file name.
        assert!(resolve_breakpoint(Some(&debug), "C:\\work\\Demo.st", 10).is_some());
    }

    #[test]
    fn resolve_breakpoint_when_path_is_different_file_then_none() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "/work/other.st", 10).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_no_source_file_table_then_path_is_not_checked() {
        let mut debug = a_debug_section();
        debug.source_files.clear();
        let resolved = resolve_breakpoint(Some(&debug), "anything.st", 10).unwrap();
        assert_eq!(resolved.line, 10);
    }

    #[test]
    fn resolve_breakpoint_when_multiple_offsets_on_line_then_arms_statement_start() {
        let mut debug = a_debug_section();
        // A second, later offset on line 10: the earlier one is the start.
        debug.line_map.push(line_entry(FunctionId::SCAN, 3, 0, 10));
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", 10).unwrap();
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 0)]);
    }

    #[test]
    fn resolve_breakpoint_when_two_functions_on_line_then_arms_both() {
        let mut debug = a_debug_section();
        debug.line_map.push(line_entry(FunctionId::INIT, 4, 0, 10));
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", 10).unwrap();
        assert_eq!(resolved.locations.len(), 2);
        assert!(resolved.locations.contains(&(FunctionId::SCAN, 0)));
        assert!(resolved.locations.contains(&(FunctionId::INIT, 4)));
    }

    #[test]
    fn resolve_frame_when_named_and_mapped_then_full_frame_info() {
        let debug = a_debug_section();
        // pc 7 is inside the statement starting at offset 6 (line 12).
        let info = resolve_frame(Some(&debug), FunctionId::SCAN, 7);
        assert_eq!(info.name, "MAIN");
        assert_eq!(info.line, 12);
        assert_eq!(info.column, 1);
        assert_eq!(info.source, Some(("demo.st".into(), "demo.st".into())));
    }

    #[test]
    fn resolve_frame_when_unnamed_function_then_id_fallback() {
        let debug = a_debug_section();
        let info = resolve_frame(Some(&debug), FunctionId::new(7), 0);
        assert_eq!(info.name, "function 7");
        assert_eq!(info.line, 0);
        assert!(info.source.is_none());
    }

    #[test]
    fn resolve_frame_when_no_debug_section_then_fallback_frame() {
        let info = resolve_frame(None, FunctionId::SCAN, 3);
        assert_eq!(info.name, "function 1");
        assert_eq!(info.line, 0);
        assert_eq!(info.column, 0);
        assert!(info.source.is_none());
    }

    #[test]
    fn render_variables_when_entries_present_then_named_and_typed() {
        let debug = DebugSection {
            var_names: vec![
                var_name(0, iec_type_tag::DINT, "counter", "DINT"),
                var_name(1, iec_type_tag::BOOL, "running", "BOOL"),
                var_name(2, iec_type_tag::REAL, "ratio", "REAL"),
            ],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[42, 1, 1.5f32.to_bits() as u64], &[]);
        assert_eq!(vars[0].name, "counter");
        assert_eq!(vars[0].value, "42");
        assert_eq!(vars[0].type_name.as_deref(), Some("DINT"));
        assert_eq!(vars[1].name, "running");
        assert_eq!(vars[1].value, "TRUE");
        assert_eq!(vars[2].name, "ratio");
        assert_eq!(vars[2].value, "1.5");
    }

    #[test]
    fn render_variables_when_slot_has_no_entry_then_indexed_fallback() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::DINT, "counter", "DINT")],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[7, 0xFFFF_FFFF], &[]);
        assert_eq!(vars[0].name, "counter");
        // Slot 1 has no VAR_NAME entry: passthrough name and i32 rendering.
        assert_eq!(vars[1].name, "var[1]");
        assert_eq!(vars[1].value, "-1");
        assert!(vars[1].type_name.is_none());
    }

    #[test]
    fn render_variables_when_no_debug_then_all_indexed_fallback() {
        let vars = render_variables(None, &[10], &[]);
        assert_eq!(vars[0].name, "var[0]");
        assert_eq!(vars[0].value, "10");
    }

    #[test]
    fn render_variables_when_string_then_reads_data_region() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::STRING, "msg", "STRING")],
            string_layouts: vec![StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: 0,
                max_length: 8,
            }],
            ..DebugSection::default()
        };
        // [max_len=8][cur_len=2][char_width=1]"hi" + unused capacity.
        let mut data = vec![8, 0, 2, 0, 1, 0];
        data.extend_from_slice(b"hi");
        data.extend_from_slice(&[0; 6]);
        let vars = render_variables(Some(&debug), &[0], &data);
        assert_eq!(vars[0].value, "'hi'");
    }

    #[test]
    fn render_variables_when_string_layout_out_of_bounds_then_placeholder() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::STRING, "msg", "STRING")],
            string_layouts: vec![StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: 100, // past the end of the 4-byte region below
                max_length: 8,
            }],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[0], &[0, 0, 0, 0]);
        assert_eq!(vars[0].value, VALUE_NOT_AVAILABLE);
    }

    #[test]
    fn render_variables_when_string_length_out_of_bounds_then_placeholder() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::STRING, "msg", "STRING")],
            string_layouts: vec![StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: 0,
                max_length: 8,
            }],
            ..DebugSection::default()
        };
        // cur_len (40) reads past the end of the region.
        let vars = render_variables(Some(&debug), &[0], &[8, 0, 40, 0, 1, 0, b'h', b'i']);
        assert_eq!(vars[0].value, VALUE_NOT_AVAILABLE);
    }

    #[test]
    fn render_variables_when_string_has_no_layout_then_placeholder() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::STRING, "msg", "STRING")],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[0], &[]);
        assert_eq!(vars[0].value, VALUE_NOT_AVAILABLE);
    }

    #[test]
    fn render_variables_when_wstring_then_placeholder() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::WSTRING, "wmsg", "WSTRING")],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[0], &[]);
        assert_eq!(vars[0].value, VALUE_NOT_AVAILABLE);
    }

    #[test]
    fn format_iec_string_literal_when_escapes_needed_then_dollar_escaped() {
        assert_eq!(format_iec_string_literal(b"a$b"), "'a$$b'");
        assert_eq!(format_iec_string_literal(b"it's"), "'it$'s'");
        assert_eq!(format_iec_string_literal(b"a\tb\n"), "'a$Tb$L'");
        assert_eq!(format_iec_string_literal(&[0x01]), "'$01'");
    }

    #[test]
    fn render_variables_when_no_slots_then_empty() {
        assert!(render_variables(None, &[], &[]).is_empty());
    }
}
