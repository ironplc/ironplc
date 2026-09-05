//! The single Layer-1-coupled corner of the DAP server.
//!
//! Everything that maps between the debugger's `(FunctionId, bytecode_offset)`
//! space and *source* coordinates — source line → offset for breakpoints,
//! frame → name/source location for stack traces, and variable slot →
//! name/type/value for inspection — lives here and nowhere else. The rest of
//! the server speaks only in resolved values, so the debug section (line map,
//! VAR_NAME, FUNC_NAME, STRING layouts, source file table, `debug_format`) is
//! a dependency of exactly one module.

use ironplc_container::debug_format::VariableRenderer;
use ironplc_container::debug_section::{DebugSection, SourceFileEntry};
use ironplc_container::{FunctionId, SourceColumn, SourceFileId, SourceLine};

use super::types::Variable;

/// A source breakpoint resolved against the line map.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedBreakpoint {
    /// The source line the breakpoint actually bound to — the requested
    /// line, or the next executable line when the requested one has no code
    /// (the standard "snap forward" debugger behavior).
    pub line: SourceLine,
    /// The bytecode locations to arm: for each function with code on the
    /// bound line, the smallest bytecode offset on that line.
    pub locations: Vec<(FunctionId, usize)>,
}

/// A stack frame resolved against FUNC_NAME and the line map.
#[derive(Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// The POU name from FUNC_NAME, or `function {id}` when unnamed.
    pub name: String,
    /// Source line of the paused instruction; `SourceLine(0)` ("unknown")
    /// when the line map has no entry for it.
    pub line: SourceLine,
    /// Source column; `SourceColumn(0)` when unknown.
    pub column: SourceColumn,
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
    requested: SourceLine,
) -> Option<ResolvedBreakpoint> {
    let debug = debug?;

    // Restrict to entries from the requested file. A container without a
    // source file table predates per-file tracking: every entry is eligible.
    let file_ids: Option<Vec<SourceFileId>> = if debug.source_files.is_empty() {
        None
    } else {
        let ids: Vec<SourceFileId> = debug
            .source_files
            .iter()
            .enumerate()
            .filter(|(_, sf)| file_matches(&sf.path, source_path))
            .filter_map(|(index, _)| u16::try_from(index).ok().map(SourceFileId::new))
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
                && e.source_line.raw() >= requested.raw()
                && file_ids.as_ref().is_none_or(|ids| ids.contains(&e.file_id))
        })
        .collect();

    // Snap forward to the nearest executable line.
    let bound = candidates
        .iter()
        .map(|e| e.source_line)
        .min_by_key(|line| line.raw())?;

    // Arm each function's first offset on the bound line (statement start).
    let mut locations: Vec<(FunctionId, usize)> = Vec::new();
    for entry in candidates.iter().filter(|e| e.source_line == bound) {
        let offset = usize::from(entry.bytecode_offset);
        match locations
            .iter_mut()
            .find(|(function, _)| *function == entry.function_id)
        {
            Some((_, existing)) => *existing = (*existing).min(offset),
            None => locations.push((entry.function_id, offset)),
        }
    }

    Some(ResolvedBreakpoint {
        line: bound,
        locations,
    })
}

/// Whether a recorded source path refers to the same file as a requested
/// path. Exact match first; otherwise the file names are compared to absorb
/// absolute-vs-relative differences between what the compiler recorded and
/// what the editor sends.
///
/// Deliberately string-based rather than [`std::path::Path`]: `Path` parses
/// and compares by the rules of the *host* platform, but `recorded` comes
/// from wherever the container was compiled. A Windows-recorded
/// `C:\work\demo.st` is a single component on a Unix host, so
/// `Path::file_name` yields the whole string. `Path` comparison is also
/// always case-sensitive (even on Windows) and does not fold a leading
/// `./`, so it decides neither half of this question. True identity via
/// `fs::canonicalize` needs both paths to exist on this machine, which a
/// container built elsewhere does not guarantee.
fn file_matches(recorded: &str, requested: &str) -> bool {
    recorded == requested || file_names_match(file_name(recorded), file_name(requested))
}

/// Compares two file names using the host filesystem's case rules.
///
/// Windows and macOS default to case-insensitive filesystems, so `Demo.st`
/// and `demo.st` name the same file there. Linux and other Unixes are
/// case-sensitive, where they are genuinely *different* files — folding case
/// would bind a breakpoint to the wrong source. (The insensitive form folds
/// ASCII only; Windows applies full Unicode case rules, which matters solely
/// for non-ASCII file names that differ only by case.)
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn file_names_match(recorded: &str, requested: &str) -> bool {
    recorded.eq_ignore_ascii_case(requested)
}

/// See the case-insensitive counterpart for why this is host-conditional.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn file_names_match(recorded: &str, requested: &str) -> bool {
    recorded == requested
}

/// The final path component, treating both `/` and `\` as separators —
/// `recorded` may come from a container compiled on another platform, so
/// host-specific splitting is not enough.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Resolve a paused frame to its POU name and source location.
///
/// The line map lookup returns the entry enclosing `pc` (largest offset
/// `<= pc`). A frame with no FUNC_NAME entry keeps the `function {id}`
/// fallback; one with no line map hit reports the "unknown" line and no
/// source, which clients render as a name-only frame. A `pc` beyond `u16`
/// likewise resolves to unknown — the line map cannot describe an offset it
/// has no room to store.
pub fn resolve_frame(
    debug: Option<&DebugSection>,
    function_id: FunctionId,
    pc: usize,
) -> FrameInfo {
    let name = debug
        .and_then(|d| d.func_names.iter().find(|f| f.function_id == function_id))
        .map(|f| f.name.clone())
        .unwrap_or_else(|| format!("function {}", function_id.raw()));

    let location = u16::try_from(pc)
        .ok()
        .and_then(|offset| debug?.lookup_source_location(function_id, offset))
        .filter(|e| e.source_line.raw() != 0);

    match location {
        Some(entry) => FrameInfo {
            name,
            line: entry.source_line,
            column: entry.source_column,
            source: debug
                .and_then(|d| d.source_files.get(usize::from(entry.file_id.raw())))
                .map(|sf: &SourceFileEntry| (file_name(&sf.path).to_string(), sf.path.clone())),
        },
        None => FrameInfo {
            name,
            line: SourceLine::default(),
            column: SourceColumn::default(),
            source: None,
        },
    }
}

/// Render a run of variable slots for a `variables` response.
///
/// `values[i]` is the raw 64-bit slot for variable index `i`; `data_region`
/// backs STRING and WSTRING reads.
///
/// Naming and value rendering both come from
/// [`VariableRenderer`](ironplc_container::debug_format::VariableRenderer) —
/// the one place that formats a variable for display
/// (`specs/design/variable-value-rendering.md`) — so the debugger pane agrees
/// with `--dump-vars` and the playground. A slot with no VAR_NAME entry keeps
/// the `var[i]` / signed-decimal fallback, so the pane never goes blank.
pub fn render_variables(
    debug: Option<&DebugSection>,
    values: &[u64],
    data_region: &[u8],
) -> Vec<Variable> {
    let renderer = VariableRenderer::from_debug_section(debug);
    values
        .iter()
        .enumerate()
        .map(|(i, &raw)| {
            let index = i as u16;
            Variable {
                name: renderer.name(index),
                value: renderer.render(index, raw, data_region).text,
                type_name: renderer.var(index).map(|info| info.type_name.clone()),
                variables_reference: 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{
        iec_type_tag, var_section, FuncNameEntry, LineMapEntry, SourceFileEntry, StringLayoutEntry,
        VarNameEntry,
    };
    use ironplc_container::{
        SourceColumn, SourceFileId, SourceLine, VarIndex, VALUE_INVALID, VALUE_UNAVAILABLE,
    };

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
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", SourceLine::new(10)).unwrap();
        assert_eq!(resolved.line.raw(), 10);
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 0)]);
    }

    #[test]
    fn resolve_breakpoint_when_line_has_no_code_then_snaps_forward() {
        let debug = a_debug_section();
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", SourceLine::new(11)).unwrap();
        assert_eq!(resolved.line.raw(), 12);
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 6)]);
    }

    #[test]
    fn resolve_breakpoint_when_line_past_end_then_none() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "demo.st", SourceLine::new(13)).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_no_debug_or_no_line_map_then_none() {
        assert!(resolve_breakpoint(None, "demo.st", SourceLine::new(10)).is_none());
        let empty = DebugSection::default();
        assert!(resolve_breakpoint(Some(&empty), "demo.st", SourceLine::new(10)).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_path_differs_by_directory_then_matches_by_file_name() {
        let debug = a_debug_section();
        let resolved =
            resolve_breakpoint(Some(&debug), "/work/project/demo.st", SourceLine::new(10)).unwrap();
        assert_eq!(resolved.line.raw(), 10);
        // A Windows-style separator also splits, so a container compiled on
        // Windows still matches when debugged elsewhere.
        assert!(
            resolve_breakpoint(Some(&debug), "C:\\work\\demo.st", SourceLine::new(10)).is_some()
        );
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn resolve_breakpoint_when_case_differs_on_case_insensitive_host_then_matches() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "/work/Demo.st", SourceLine::new(10)).is_some());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn resolve_breakpoint_when_case_differs_on_case_sensitive_host_then_none() {
        // On a case-sensitive filesystem `Demo.st` is a different file from
        // `demo.st`; folding case would bind the breakpoint to the wrong one.
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "/work/Demo.st", SourceLine::new(10)).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_path_is_different_file_then_none() {
        let debug = a_debug_section();
        assert!(resolve_breakpoint(Some(&debug), "/work/other.st", SourceLine::new(10)).is_none());
    }

    #[test]
    fn resolve_breakpoint_when_no_source_file_table_then_path_is_not_checked() {
        let mut debug = a_debug_section();
        debug.source_files.clear();
        let resolved =
            resolve_breakpoint(Some(&debug), "anything.st", SourceLine::new(10)).unwrap();
        assert_eq!(resolved.line.raw(), 10);
    }

    #[test]
    fn resolve_breakpoint_when_multiple_offsets_on_line_then_arms_statement_start() {
        let mut debug = a_debug_section();
        // A second, later offset on line 10: the earlier one is the start.
        debug.line_map.push(line_entry(FunctionId::SCAN, 3, 0, 10));
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", SourceLine::new(10)).unwrap();
        assert_eq!(resolved.locations, vec![(FunctionId::SCAN, 0)]);
    }

    #[test]
    fn resolve_breakpoint_when_two_functions_on_line_then_arms_both() {
        let mut debug = a_debug_section();
        debug.line_map.push(line_entry(FunctionId::INIT, 4, 0, 10));
        let resolved = resolve_breakpoint(Some(&debug), "demo.st", SourceLine::new(10)).unwrap();
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
        assert_eq!(info.line.raw(), 12);
        assert_eq!(info.column.raw(), 1);
        assert_eq!(info.source, Some(("demo.st".into(), "demo.st".into())));
    }

    #[test]
    fn resolve_frame_when_unnamed_function_then_id_fallback() {
        let debug = a_debug_section();
        let info = resolve_frame(Some(&debug), FunctionId::new(7), 0);
        assert_eq!(info.name, "function 7");
        assert_eq!(info.line.raw(), 0);
        assert!(info.source.is_none());
    }

    #[test]
    fn resolve_frame_when_no_debug_section_then_fallback_frame() {
        let info = resolve_frame(None, FunctionId::SCAN, 3);
        assert_eq!(info.name, "function 1");
        assert_eq!(info.line.raw(), 0);
        assert_eq!(info.column.raw(), 0);
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
        assert_eq!(vars[0].value, VALUE_INVALID);
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
        assert_eq!(vars[0].value, VALUE_INVALID);
    }

    #[test]
    fn render_variables_when_string_has_no_layout_then_placeholder() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::STRING, "msg", "STRING")],
            ..DebugSection::default()
        };
        let vars = render_variables(Some(&debug), &[0], &[]);
        assert_eq!(vars[0].value, VALUE_UNAVAILABLE);
    }

    #[test]
    fn render_variables_when_wstring_then_reads_data_region() {
        let debug = DebugSection {
            var_names: vec![var_name(0, iec_type_tag::WSTRING, "wmsg", "WSTRING")],
            string_layouts: vec![StringLayoutEntry {
                var_index: VarIndex::new(0),
                data_offset: 0,
                max_length: 8,
            }],
            ..DebugSection::default()
        };
        // [max_len=8][cur_len=2][char_width=2] then "hi" as UTF-16LE.
        let data = vec![8, 0, 2, 0, 2, 0, b'h', 0, b'i', 0];
        let vars = render_variables(Some(&debug), &[0], &data);
        assert_eq!(vars[0].value, "\"hi\"");
    }

    #[test]
    fn render_variables_when_no_slots_then_empty() {
        assert!(render_variables(None, &[], &[]).is_empty());
    }
}
