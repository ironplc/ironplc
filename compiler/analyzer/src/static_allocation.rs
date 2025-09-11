//! Static allocation analysis for PLC variables.
//!
//! This module analyzes variables in a PLC library and generates information
//! about their static memory allocation requirements, including sizes, types,
//! and other relevant details needed for actual system allocation.

use crate::memory_layout::calculate_memory_layout;
use crate::symbol_environment::{SymbolEnvironment, SymbolInfo, SymbolKind};
use ironplc_dsl::common::Type;
use ironplc_dsl::core::Id;

/// Information about a statically allocated variable
#[derive(Debug)]
pub struct StaticVariableInfo {
    pub name: String,
    pub scope: String,
    pub variable_type: String,
    pub data_type: String,
    pub size_bytes: Option<usize>,
    pub offset_bytes: Option<usize>,
    pub is_constant: bool,
    pub is_external: bool,
    pub location: Option<String>,
}

/// Generate static allocation information for all variables in the library
pub fn generate_static_allocation_info(symbol_environment: &SymbolEnvironment) -> String {
    let mut output = String::new();

    // Collect all variables from the symbol table instead of parsing the library
    let mut static_variables = Vec::new();

    // Get global variables
    for (id, symbol) in symbol_environment.get_global_symbols() {
        if matches!(symbol.kind, SymbolKind::Variable) {
            if let Some(var_info) = create_static_variable_info_from_symbol(id, symbol, "Global") {
                static_variables.push(var_info);
            }
        }
    }

    // Get scoped variables
    for (scope, symbols) in symbol_environment.get_scoped_symbols() {
        for (id, symbol) in symbols {
            if matches!(symbol.kind, SymbolKind::Variable) {
                let scope_name = format!("{:?}", scope);
                if let Some(var_info) =
                    create_static_variable_info_from_symbol(id, symbol, &scope_name)
                {
                    static_variables.push(var_info);
                }
            }
        }
    }

    // Calculate memory layout using the symbol environment
    let memory_layout = calculate_memory_layout(symbol_environment);

    // Generate summary
    output.push_str(&format!(
        "Total statically allocated variables: {}\n\n",
        static_variables.len()
    ));

    // Group variables by scope
    let mut scope_groups: std::collections::HashMap<String, Vec<&StaticVariableInfo>> =
        std::collections::HashMap::new();
    for var in &static_variables {
        scope_groups.entry(var.scope.clone()).or_default().push(var);
    }

    // Output variables grouped by scope with memory layout information
    for (scope, vars) in scope_groups {
        output.push_str(&format!("=== {} ===\n", scope));

        // Get memory layout for this scope
        let scope_layout = if scope == "Global" {
            Some(&memory_layout.global)
        } else {
            memory_layout
                .scopes
                .iter()
                .find(|s| format!("{:?}", s.scope).contains(&scope))
        };

        for var in vars {
            let mut var_info = format_variable_info(var);

            // Add memory layout information if available
            if let Some(layout) = scope_layout {
                if let Some(var_layout) = layout
                    .variables
                    .iter()
                    .find(|v| v.symbol_id.original() == var.name.as_str())
                {
                    var_info.push_str(&format!(" | Offset: {} bytes", var_layout.offset));
                }
            }

            output.push_str(&format!("  {}\n", var_info));
        }
        output.push_str("\n");
    }

    // Calculate total memory requirements
    let total_size: usize = static_variables.iter().filter_map(|v| v.size_bytes).sum();

    output.push_str(&format!("Total memory required: {} bytes\n", total_size));
    output.push_str(&format!(
        "Memory layout total: {} bytes\n",
        memory_layout.total_memory
    ));

    output
}

/// Calculate the size of a basic type
fn calculate_type_size(type_name: &Type) -> Option<usize> {
    let type_str = type_name.to_string().to_lowercase();
    match type_str.as_str() {
        "bool" => Some(1),
        "sint" | "usint" | "byte" => Some(1),
        "int" | "uint" | "word" => Some(2),
        "dint" | "udint" | "dword" => Some(4),
        "lint" | "ulint" | "lword" => Some(8),
        "real" => Some(4),
        "lreal" => Some(8),
        "time" => Some(8),
        "date" => Some(8),
        "time_of_day" | "tod" => Some(8),
        "date_and_time" | "dt" => Some(8),
        "string" => None,  // Variable length
        "wstring" => None, // Variable length
        _ => None,         // Unknown type
    }
}

/// Format variable information for output
fn format_variable_info(var: &StaticVariableInfo) -> String {
    let size_info = var
        .size_bytes
        .map(|size| format!("{} bytes", size))
        .unwrap_or_else(|| "unknown size".to_string());

    let const_info = if var.is_constant { " CONST" } else { "" };
    let external_info = if var.is_external { " EXTERNAL" } else { "" };
    let location_info = var
        .location
        .as_ref()
        .map(|loc| format!(" @ {}", loc))
        .unwrap_or_else(String::new);

    format!(
        "{:<20} | {:<15} | {:<20} | {:<15} | {}{}{}{}",
        var.name,
        var.variable_type,
        var.data_type,
        size_info,
        const_info,
        external_info,
        location_info,
        if var.scope != "Global" {
            format!(" (in {})", var.scope)
        } else {
            String::new()
        }
    )
}

/// Create static variable information directly from a symbol in the symbol table
fn create_static_variable_info_from_symbol(
    id: &Id,
    symbol: &SymbolInfo,
    scope: &str,
) -> Option<StaticVariableInfo> {
    // Extract type and size information from the symbol
    let (variable_type, data_type, size_bytes) = extract_type_and_size_from_symbol(symbol);

    Some(StaticVariableInfo {
        name: id.original().to_string(),
        scope: scope.to_string(),
        variable_type,
        data_type,
        size_bytes,
        offset_bytes: None, // Will be filled later by memory layout
        is_constant: false, // TODO: Determine constant status from symbol info
        is_external: symbol.is_external,
        location: None,
    })
}

/// Extract type and size information from a symbol
fn extract_type_and_size_from_symbol(symbol: &SymbolInfo) -> (String, String, Option<usize>) {
    let data_type = symbol
        .data_type
        .clone()
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let size = calculate_type_size(&Type::from(data_type.as_str()));

    (data_type.clone(), data_type, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_environment::{ScopeKind, SymbolInfo, SymbolKind};
    use ironplc_dsl::core::SourceSpan;
    use ironplc_dsl::{common::Type, core::Id};

    #[test]
    fn test_calculate_type_size() {
        assert_eq!(calculate_type_size(&Type::from("bool")), Some(1));
        assert_eq!(calculate_type_size(&Type::from("int")), Some(2));
        assert_eq!(calculate_type_size(&Type::from("dint")), Some(4));
        assert_eq!(calculate_type_size(&Type::from("real")), Some(4));
        assert_eq!(calculate_type_size(&Type::from("lreal")), Some(8));
        assert_eq!(calculate_type_size(&Type::from("unknown")), None);
    }

    #[test]
    fn test_create_static_variable_info_from_symbol() {
        let symbol = SymbolInfo::new(
            SymbolKind::Variable,
            ScopeKind::Global,
            SourceSpan::default(),
        )
        .with_data_type("int".to_string())
        .with_external(false);

        let var_info =
            create_static_variable_info_from_symbol(&Id::from("test_var"), &symbol, "test_scope")
                .unwrap();
        assert_eq!(var_info.name, "test_var");
        assert_eq!(var_info.scope, "test_scope");
        assert_eq!(var_info.data_type, "int");
        assert_eq!(var_info.size_bytes, Some(2));
        assert_eq!(var_info.is_constant, false);
        assert_eq!(var_info.is_external, false);
    }

    #[test]
    fn test_create_static_variable_info_from_symbol_unknown_type() {
        let symbol = SymbolInfo::new(
            SymbolKind::Variable,
            ScopeKind::Global,
            SourceSpan::default(),
        )
        .with_external(true);

        let var_info = create_static_variable_info_from_symbol(
            &Id::from("unknown_var"),
            &symbol,
            "test_scope",
        )
        .unwrap();
        assert_eq!(var_info.name, "unknown_var");
        assert_eq!(var_info.scope, "test_scope");
        assert_eq!(var_info.data_type, "UNKNOWN");
        assert_eq!(var_info.size_bytes, None);
        assert_eq!(var_info.is_external, true);
    }

    #[test]
    fn test_extract_type_and_size_from_symbol() {
        let symbol = SymbolInfo::new(
            SymbolKind::Variable,
            ScopeKind::Global,
            SourceSpan::default(),
        )
        .with_data_type("dint".to_string());

        let (variable_type, data_type, size_bytes) = extract_type_and_size_from_symbol(&symbol);
        assert_eq!(variable_type, "dint");
        assert_eq!(data_type, "dint");
        assert_eq!(size_bytes, Some(4));
    }

    #[test]
    fn test_format_variable_info() {
        let var_info = StaticVariableInfo {
            name: "test_var".to_string(),
            scope: "test_scope".to_string(),
            variable_type: "Var".to_string(),
            data_type: "INT".to_string(),
            size_bytes: Some(2),
            offset_bytes: None,
            is_constant: false,
            is_external: false,
            location: None,
        };

        let formatted = format_variable_info(&var_info);
        assert!(formatted.contains("test_var"));
        assert!(formatted.contains("Var"));
        assert!(formatted.contains("INT"));
        assert!(formatted.contains("2 bytes"));
        assert!(formatted.contains("(in test_scope)"));
    }

    #[test]
    fn test_format_variable_info_with_offset() {
        let var_info = StaticVariableInfo {
            name: "test_var".to_string(),
            scope: "test_scope".to_string(),
            variable_type: "Var".to_string(),
            data_type: "INT".to_string(),
            size_bytes: Some(2),
            offset_bytes: Some(4),
            is_constant: false,
            is_external: false,
            location: None,
        };

        let formatted = format_variable_info(&var_info);
        assert!(formatted.contains("test_var"));
        assert!(formatted.contains("2 bytes"));
        assert!(formatted.contains("(in test_scope)"));
    }
}
