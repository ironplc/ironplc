//! The single Layer-1-coupled corner of the DAP server.
//!
//! Everything that maps between the debugger's `(FunctionId, bytecode_offset)`
//! space and *source* coordinates — source line → offset for breakpoints, and
//! variable slot → name/type for inspection — lives here and nowhere else. The
//! rest of the server speaks only in offsets and raw slot values, so the debug
//! section (line map, VAR_NAME, `debug_format`) is a dependency of exactly one
//! module.
//!
//! This first cut ships **passthrough** resolvers (see the plan,
//! `specs/plans/2026-06-25-dap-server-scaffold.md` §"Debug-info coupling,
//! isolated"): the DAP `line` is treated as a raw bytecode offset and slots are
//! rendered by index with no names. The two function signatures are the stable
//! seam — commit 5 swaps real line-map / `debug_format` lookups in behind them
//! without touching any other module.
//!
//! Both resolvers are consumed by the run/stop loop that lands in commit 4
//! (`setBreakpoints` and `variables`); until then they are exercised only by
//! the unit tests below, hence the module-level `dead_code` allowance.
#![allow(dead_code)]

use ironplc_container::debug_section::DebugSection;
use ironplc_container::FunctionId;

use super::types::Variable;

/// Resolve a source breakpoint to the `(function, bytecode offset)` locations
/// that should be armed for it.
///
/// **Passthrough:** the DAP `line` is interpreted directly as a bytecode offset
/// in the scan function, ignoring `debug` and `source_path`. A negative line
/// (never produced by a conformant client) resolves to nothing. Commit 5
/// replaces the body with a real line-map + SOURCE_FILE lookup keyed off
/// `source_path`; this signature stays fixed.
pub fn resolve_breakpoint(
    _debug: Option<&DebugSection>,
    _source_path: &str,
    line: i64,
) -> Vec<(FunctionId, usize)> {
    if line < 0 {
        return Vec::new();
    }
    vec![(FunctionId::SCAN, line as usize)]
}

/// Render a run of variable slots for a `variables` response.
///
/// `values[i]` is the raw 64-bit slot for variable index `i`. **Passthrough:**
/// each slot is named `var[i]` and rendered as a signed 32-bit decimal, with no
/// type — `debug` is ignored. Commit 5 replaces the body with VAR_NAME lookups
/// for names/types and `debug_format::format_variable_value` for the value;
/// this signature stays fixed.
pub fn render_variables(_debug: Option<&DebugSection>, values: &[u64]) -> Vec<Variable> {
    values
        .iter()
        .enumerate()
        .map(|(i, &raw)| Variable {
            name: format!("var[{i}]"),
            value: (raw as i32).to_string(),
            type_name: None,
            variables_reference: 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_breakpoint_when_line_nonnegative_then_offset_passthrough_in_scan() {
        // The passthrough treats the line as a raw offset in the scan function.
        let locations = resolve_breakpoint(None, "any.st", 6);
        assert_eq!(locations, vec![(FunctionId::SCAN, 6)]);
    }

    #[test]
    fn resolve_breakpoint_when_line_negative_then_no_locations() {
        assert!(resolve_breakpoint(None, "any.st", -1).is_empty());
    }

    #[test]
    fn render_variables_when_slots_given_then_indexed_names_and_i32_values() {
        let vars = render_variables(None, &[10, 0xFFFF_FFFF, 42]);
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0].name, "var[0]");
        assert_eq!(vars[0].value, "10");
        assert!(vars[0].type_name.is_none());
        assert_eq!(vars[0].variables_reference, 0);
        // Low 32 bits interpreted as signed.
        assert_eq!(vars[1].name, "var[1]");
        assert_eq!(vars[1].value, "-1");
        assert_eq!(vars[2].value, "42");
    }

    #[test]
    fn render_variables_when_no_slots_then_empty() {
        assert!(render_variables(None, &[]).is_empty());
    }
}
