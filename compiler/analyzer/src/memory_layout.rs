//! Memory layout analysis for PLC variables.
//!
//! This module analyzes the symbol table and generates memory layout information
//! for statically allocated variables, including offsets and alignment.

use crate::symbol_environment::{ScopeKind, SymbolEnvironment, SymbolInfo, SymbolKind};
use ironplc_dsl::core::Id;

/// Memory layout information for a statically allocated variable
#[derive(Debug, Clone)]
pub struct VariableMemoryLayout {
    /// The symbol ID
    pub symbol_id: Id,
    /// The scope where this variable is declared
    pub scope: ScopeKind,
    /// Offset in bytes from the start of the scope's memory region
    pub offset: usize,
    /// Size of the variable in bytes
    pub size: usize,
    /// Alignment requirement in bytes (for proper memory layout)
    pub alignment: usize,
    /// Whether this variable is a constant (affects memory placement)
    pub is_constant: bool,
    /// The data type of the variable
    pub data_type: Option<String>,
}

/// Memory layout for a complete scope
#[derive(Debug, Clone)]
pub struct ScopeMemoryLayout {
    /// The scope this layout represents
    pub scope: ScopeKind,
    /// Total memory size required for this scope
    pub total_size: usize,
    /// Variables in this scope with their memory layout
    pub variables: Vec<VariableMemoryLayout>,
}

/// Complete memory layout for the entire program
#[derive(Debug, Clone)]
pub struct ProgramMemoryLayout {
    /// Global scope memory layout
    pub global: ScopeMemoryLayout,
    /// All other scoped memory layouts
    pub scopes: Vec<ScopeMemoryLayout>,
    /// Total memory required across all scopes
    pub total_memory: usize,
}

impl ProgramMemoryLayout {
    /// Create a new program memory layout
    pub fn new() -> Self {
        Self {
            global: ScopeMemoryLayout {
                scope: ScopeKind::Global,
                total_size: 0,
                variables: Vec::new(),
            },
            scopes: Vec::new(),
            total_memory: 0,
        }
    }

    /// Get the memory layout for a specific scope
    pub fn get_scope_layout(&self, scope: &ScopeKind) -> Option<&ScopeMemoryLayout> {
        match scope {
            ScopeKind::Global => Some(&self.global),
            ScopeKind::Named(_) => self.scopes.iter().find(|s| s.scope == *scope),
        }
    }

    /// Get the memory layout for a specific variable
    pub fn get_variable_layout(
        &self,
        symbol_id: &Id,
        scope: &ScopeKind,
    ) -> Option<&VariableMemoryLayout> {
        let scope_layout = self.get_scope_layout(scope)?;
        scope_layout
            .variables
            .iter()
            .find(|v| v.symbol_id == *symbol_id)
    }
}

/// Calculate memory layout for all variables in the symbol environment
pub fn calculate_memory_layout(symbol_environment: &SymbolEnvironment) -> ProgramMemoryLayout {
    let mut program_layout = ProgramMemoryLayout::new();

    // Calculate global scope layout
    program_layout.global =
        calculate_scope_memory_layout(&ScopeKind::Global, &symbol_environment.get_global_symbols());

    // Calculate each scoped layout
    for (scope, symbols) in symbol_environment.get_scoped_symbols() {
        let scope_layout = calculate_scope_memory_layout(scope, symbols);
        program_layout.scopes.push(scope_layout);
    }

    // Calculate total memory
    program_layout.total_memory = program_layout.global.total_size
        + program_layout
            .scopes
            .iter()
            .map(|s| s.total_size)
            .sum::<usize>();

    program_layout
}

/// Calculate memory layout for variables in a specific scope
fn calculate_scope_memory_layout(
    scope: &ScopeKind,
    symbols: &std::collections::HashMap<Id, SymbolInfo>,
) -> ScopeMemoryLayout {
    let mut variables = Vec::new();
    let mut current_offset = 0;

    // Collect all variable symbols with their type information
    let mut variable_info: Vec<(Id, SymbolInfo, usize, usize)> = symbols
        .iter()
        .filter_map(|(id, symbol)| {
            if matches!(symbol.kind, SymbolKind::Variable) {
                let (size, alignment) = calculate_variable_size_and_alignment(symbol);
                Some((id.clone(), symbol.clone(), size, alignment))
            } else {
                None
            }
        })
        .collect();

    // Sort variables by alignment requirements (largest alignment first for proper packing)
    variable_info.sort_by(|(_, _, _, a_align), (_, _, _, b_align)| b_align.cmp(a_align));

    // Assign offsets
    for (id, symbol, size, alignment) in variable_info {
        // Align the current offset to the variable's alignment requirement
        current_offset = align_offset(current_offset, alignment);

        let memory_layout = VariableMemoryLayout {
            symbol_id: id,
            scope: scope.clone(),
            offset: current_offset,
            size,
            alignment,
            is_constant: false, // TODO: Determine constant status from symbol info
            data_type: symbol.data_type.clone(),
        };

        variables.push(memory_layout);
        current_offset += size;
    }

    ScopeMemoryLayout {
        scope: scope.clone(),
        total_size: current_offset,
        variables,
    }
}

/// Calculate the size and alignment requirements for a variable based on its type
fn calculate_variable_size_and_alignment(symbol: &SymbolInfo) -> (usize, usize) {
    // For now, use simple size calculations
    // In a real implementation, this would analyze the actual type information
    let size = match symbol.data_type.as_deref() {
        Some("bool") => 1,
        Some("sint") | Some("usint") | Some("byte") => 1,
        Some("int") | Some("uint") | Some("word") => 2,
        Some("dint") | Some("udint") | Some("dword") => 4,
        Some("lint") | Some("ulint") | Some("lword") => 8,
        Some("real") => 4,
        Some("lreal") => 8,
        Some("time")
        | Some("date")
        | Some("time_of_day")
        | Some("tod")
        | Some("date_and_time")
        | Some("dt") => 8,
        Some("string") | Some("wstring") => 0, // Variable length, handled separately
        _ => 4,                                // Default to 4 bytes for unknown types
    };

    // Alignment is typically the same as size for simple types
    let alignment = size.max(1);

    (size, alignment)
}

/// Align an offset to the specified alignment boundary
fn align_offset(offset: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return offset;
    }
    let remainder = offset % alignment;
    if remainder == 0 {
        offset
    } else {
        offset + (alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::SourceSpan;

    #[test]
    fn test_align_offset() {
        assert_eq!(align_offset(0, 4), 0);
        assert_eq!(align_offset(1, 4), 4);
        assert_eq!(align_offset(4, 4), 4);
        assert_eq!(align_offset(5, 4), 8);
        assert_eq!(align_offset(8, 4), 8);
    }

    #[test]
    fn test_calculate_variable_size_and_alignment() {
        let symbol = SymbolInfo::new(
            SymbolKind::Variable,
            ScopeKind::Global,
            SourceSpan::default(),
        )
        .with_data_type("int".to_string());

        let (size, alignment) = calculate_variable_size_and_alignment(&symbol);
        assert_eq!(size, 2);
        assert_eq!(alignment, 2);
    }

    #[test]
    fn test_program_memory_layout() {
        let layout = ProgramMemoryLayout::new();
        assert_eq!(layout.total_memory, 0);
        assert_eq!(layout.scopes.len(), 0);
    }
}
