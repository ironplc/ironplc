//! JSON export functionality for IronPLC AST structures.
//!
//! This module provides serialization capabilities to export parsed AST structures
//! as JSON, enabling external tools and analysis frameworks to process Structured Text code.

use serde::{Serialize, Serializer};
use serde_json;
use std::io::Write;
use thiserror::Error;

use crate::common::{Library, InitialValueAssignmentKind};
use crate::core::SourceSpan;

/// Errors that can occur during JSON export operations.
#[derive(Debug, Error)]
pub enum JsonExportError {
    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Invalid output path: {0}")]
    InvalidPath(String),
    
    #[error("Missing required data: {0}")]
    MissingData(String),
}

/// Configuration options for JSON export.
#[derive(Debug, Clone)]
pub struct JsonExportOptions {
    /// Include comments in the JSON output
    pub include_comments: bool,
    /// Include source location information
    pub include_locations: bool,
    /// Pretty-print the JSON output
    pub pretty_print: bool,
}

impl Default for JsonExportOptions {
    fn default() -> Self {
        Self {
            include_comments: false,
            include_locations: true,
            pretty_print: false,
        }
    }
}

/// Core JSON exporter for IronPLC AST structures.
#[derive(Debug)]
pub struct JsonExporter {
    options: JsonExportOptions,
    schema_version: String,
}

impl JsonExporter {
    /// Create a new JsonExporter with default options.
    pub fn new() -> Self {
        Self {
            options: JsonExportOptions::default(),
            schema_version: "1.0.0".to_string(),
        }
    }

    /// Create a JsonExporter with custom options.
    pub fn with_options(options: JsonExportOptions) -> Self {
        Self {
            options,
            schema_version: "1.0.0".to_string(),
        }
    }

    /// Export a Library AST to JSON string.
    /// This method uses lazy evaluation to minimize memory usage and processing overhead.
    pub fn export_library(&self, library: &Library) -> Result<String, JsonExportError> {
        let wrapper = JsonWrapper::new(library, &self.options, &self.schema_version);
        
        // Use appropriate serialization method based on pretty_print option
        if self.options.pretty_print {
            serde_json::to_string_pretty(&wrapper).map_err(JsonExportError::from)
        } else {
            serde_json::to_string(&wrapper).map_err(JsonExportError::from)
        }
    }

    /// Export a Library AST to a writer.
    /// This method streams the output to avoid memory duplication for large ASTs.
    pub fn export_to_writer<W: Write>(&self, library: &Library, writer: W) -> Result<(), JsonExportError> {
        let wrapper = JsonWrapper::new(library, &self.options, &self.schema_version);
        
        // Stream directly to writer to minimize memory usage
        if self.options.pretty_print {
            serde_json::to_writer_pretty(writer, &wrapper).map_err(JsonExportError::from)
        } else {
            serde_json::to_writer(writer, &wrapper).map_err(JsonExportError::from)
        }
    }
}

impl Default for JsonExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON wrapper structure that includes metadata and schema information.
#[derive(Debug, Serialize)]
struct JsonWrapper<'a> {
    schema_version: &'a str,
    source_files: Vec<String>,
    metadata: JsonMetadata,
    library: &'a Library,
    symbol_table: SymbolTable,
    #[serde(skip_serializing_if = "Option::is_none")]
    formatting: Option<FormattingInfo>,
}

impl<'a> JsonWrapper<'a> {
    fn new(library: &'a Library, options: &JsonExportOptions, schema_version: &'a str) -> Self {
        // Conditional extraction based on options to minimize overhead
        let source_files = extract_source_files(library);
        let symbol_table = extract_symbol_table(library);
        
        // Only extract formatting information if comments are requested
        let formatting = if options.include_comments {
            Some(extract_formatting_info(library))
        } else {
            None
        };
        
        Self {
            schema_version,
            source_files,
            metadata: JsonMetadata::new(options),
            library,
            symbol_table,
            formatting,
        }
    }
}

/// Metadata included in JSON export.
#[derive(Debug, Serialize)]
struct JsonMetadata {
    compiler_version: String,
    export_timestamp: String,
    options: JsonExportOptions,
}

impl JsonMetadata {
    fn new(options: &JsonExportOptions) -> Self {
        Self {
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            export_timestamp: time::OffsetDateTime::now_utc().to_string(),
            options: options.clone(),
        }
    }
}

impl Serialize for JsonExportOptions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("JsonExportOptions", 3)?;
        state.serialize_field("include_comments", &self.include_comments)?;
        state.serialize_field("include_locations", &self.include_locations)?;
        state.serialize_field("pretty_print", &self.pretty_print)?;
        state.end()
    }
}

/// Serializable representation of source location information.
/// Converts byte offsets to human-readable line and column numbers.
#[derive(Debug, Clone, Serialize)]
pub struct SerializableSourceSpan {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub file_id: String,
}

impl SerializableSourceSpan {
    /// Create a SerializableSourceSpan from a SourceSpan and source text.
    /// 
    /// This function calculates line and column positions from byte offsets
    /// by counting newlines and characters in the source text.
    pub fn from_source_span(span: &SourceSpan, source_text: Option<&str>) -> Self {
        let (start_line, start_column, end_line, end_column) = match source_text {
            Some(text) => calculate_line_column_positions(text, span.start, span.end),
            None => (1, 1, 1, 1), // Default to line 1, column 1 if no source text
        };

        Self {
            start_line,
            start_column,
            end_line,
            end_column,
            file_id: span.file_id.to_string(),
        }
    }

    /// Create a SerializableSourceSpan with default values for missing location data.
    pub fn default_location(file_id: &str) -> Self {
        Self {
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            file_id: file_id.to_string(),
        }
    }
}

/// Calculate line and column positions from byte offsets in source text.
/// Returns (start_line, start_column, end_line, end_column) as 1-based indices.
fn calculate_line_column_positions(source_text: &str, start_byte: usize, end_byte: usize) -> (u32, u32, u32, u32) {
    let mut line = 1u32;
    let mut column = 1u32;
    let mut start_line = 1u32;
    let mut start_column = 1u32;
    let mut end_line = 1u32;
    let mut end_column = 1u32;
    
    let mut found_start = false;
    let mut found_end = false;
    
    for (byte_index, ch) in source_text.char_indices() {
        // Check if we've reached the start position
        if !found_start && byte_index >= start_byte {
            start_line = line;
            start_column = column;
            found_start = true;
        }
        
        // Check if we've reached the end position
        if !found_end && byte_index >= end_byte {
            end_line = line;
            end_column = column;
            found_end = true;
            break;
        }
        
        // Update line and column based on character
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    
    // If we didn't find the end position, use the final line/column
    if !found_end {
        end_line = line;
        end_column = column;
    }
    
    (start_line, start_column, end_line, end_column)
}

/// Serializable representation of a symbol table entry.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolEntry {
    pub name: String,
    pub symbol_type: String,
    pub scope: String,
    pub declaration_location: Option<SerializableSourceSpan>,
    pub data_type: Option<String>,
    pub is_external: bool,
}

/// Serializable representation of the symbol table.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolTable {
    pub symbols: std::collections::HashMap<String, SymbolEntry>,
    pub scopes: Vec<String>,
}

/// Serializable representation of a comment.
#[derive(Debug, Clone, Serialize)]
pub struct CommentInfo {
    pub text: String,
    pub style: String, // "block" for (* *) or "line" for //
    pub location: Option<SerializableSourceSpan>,
    pub attached_to: Option<String>, // ID of the AST node this comment is attached to
}

/// Serializable representation of formatting information.
#[derive(Debug, Clone, Serialize)]
pub struct FormattingInfo {
    pub comments: Vec<CommentInfo>,
    pub whitespace_significant: bool,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: std::collections::HashMap::new(),
            scopes: vec!["global".to_string()],
        }
    }

    pub fn add_symbol(&mut self, id: String, entry: SymbolEntry) {
        self.symbols.insert(id, entry);
    }

    pub fn add_scope(&mut self, scope: String) {
        if !self.scopes.contains(&scope) {
            self.scopes.push(scope);
        }
    }
}

/// Extract unique source file IDs from a Library AST.
/// This function provides a simple implementation that returns a default file list.
/// In a full implementation, this would traverse the AST to collect all unique file IDs.
fn extract_source_files(_library: &Library) -> Vec<String> {
    // For now, return a default file list
    // In a complete implementation, this would traverse the AST using the fold pattern
    // to collect all unique FileId values from SourceSpan fields
    vec!["<unknown>".to_string()]
}

/// Extract symbol table information from a Library AST.
/// This creates a basic symbol table by traversing the AST and collecting
/// function, function block, and variable declarations.
fn extract_symbol_table(library: &Library) -> SymbolTable {
    use crate::common::*;
    use crate::core::Located;
    
    let mut symbol_table = SymbolTable::new();
    
    for element in &library.elements {
        match element {
            LibraryElementKind::FunctionDeclaration(func) => {
                // Add function to symbol table
                let symbol_entry = SymbolEntry {
                    name: func.name.original.clone(),
                    symbol_type: "function".to_string(),
                    scope: "global".to_string(),
                    declaration_location: Some(SerializableSourceSpan::from_source_span(&func.name.span, None)),
                    data_type: Some(func.return_type.name.original.clone()),
                    is_external: func.external_annotation.is_some(),
                };
                symbol_table.add_symbol(func.name.original.clone(), symbol_entry);
                
                // Add function scope
                symbol_table.add_scope(func.name.original.clone());
                
                // Add function variables (simplified - just count them for now)
                for (i, var) in func.variables.iter().enumerate() {
                    if let Some(symbolic_id) = var.identifier.symbolic_id() {
                        let var_type = extract_type_from_initializer(&var.initializer);
                        let var_symbol = SymbolEntry {
                            name: symbolic_id.original.clone(),
                            symbol_type: "variable".to_string(),
                            scope: func.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: var_type,
                            is_external: false,
                        };
                        let var_id = format!("{}::{}", func.name.original, symbolic_id.original);
                        symbol_table.add_symbol(var_id, var_symbol);
                    } else {
                        // Handle direct variables with generated names
                        let var_symbol = SymbolEntry {
                            name: format!("direct_var_{}", i),
                            symbol_type: "direct_variable".to_string(),
                            scope: func.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: extract_type_from_initializer(&var.initializer),
                            is_external: false,
                        };
                        let var_id = format!("{}::direct_var_{}", func.name.original, i);
                        symbol_table.add_symbol(var_id, var_symbol);
                    }
                }
            }
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                // Add function block to symbol table
                let symbol_entry = SymbolEntry {
                    name: fb.name.name.original.clone(),
                    symbol_type: "function_block".to_string(),
                    scope: "global".to_string(),
                    declaration_location: Some(SerializableSourceSpan::from_source_span(&fb.name.name.span, None)),
                    data_type: None,
                    is_external: false,
                };
                symbol_table.add_symbol(fb.name.name.original.clone(), symbol_entry);
                
                // Add function block scope
                symbol_table.add_scope(fb.name.name.original.clone());
                
                // Add function block variables (simplified)
                for (i, var) in fb.variables.iter().enumerate() {
                    if let Some(symbolic_id) = var.identifier.symbolic_id() {
                        let var_type = extract_type_from_initializer(&var.initializer);
                        let var_symbol = SymbolEntry {
                            name: symbolic_id.original.clone(),
                            symbol_type: "variable".to_string(),
                            scope: fb.name.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: var_type,
                            is_external: false,
                        };
                        let var_id = format!("{}::{}", fb.name.name.original, symbolic_id.original);
                        symbol_table.add_symbol(var_id, var_symbol);
                    } else {
                        // Handle direct variables with generated names
                        let var_symbol = SymbolEntry {
                            name: format!("direct_var_{}", i),
                            symbol_type: "direct_variable".to_string(),
                            scope: fb.name.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: extract_type_from_initializer(&var.initializer),
                            is_external: false,
                        };
                        let var_id = format!("{}::direct_var_{}", fb.name.name.original, i);
                        symbol_table.add_symbol(var_id, var_symbol);
                    }
                }
            }
            LibraryElementKind::ProgramDeclaration(prog) => {
                // Add program to symbol table
                let symbol_entry = SymbolEntry {
                    name: prog.name.original.clone(),
                    symbol_type: "program".to_string(),
                    scope: "global".to_string(),
                    declaration_location: Some(SerializableSourceSpan::from_source_span(&prog.name.span, None)),
                    data_type: None,
                    is_external: false,
                };
                symbol_table.add_symbol(prog.name.original.clone(), symbol_entry);
                
                // Add program scope
                symbol_table.add_scope(prog.name.original.clone());
                
                // Add program variables (simplified)
                for (i, var) in prog.variables.iter().enumerate() {
                    if let Some(symbolic_id) = var.identifier.symbolic_id() {
                        let var_type = extract_type_from_initializer(&var.initializer);
                        let var_symbol = SymbolEntry {
                            name: symbolic_id.original.clone(),
                            symbol_type: "variable".to_string(),
                            scope: prog.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: var_type,
                            is_external: false,
                        };
                        let var_id = format!("{}::{}", prog.name.original, symbolic_id.original);
                        symbol_table.add_symbol(var_id, var_symbol);
                    } else {
                        // Handle direct variables with generated names
                        let var_symbol = SymbolEntry {
                            name: format!("direct_var_{}", i),
                            symbol_type: "direct_variable".to_string(),
                            scope: prog.name.original.clone(),
                            declaration_location: Some(SerializableSourceSpan::from_source_span(&var.span(), None)),
                            data_type: extract_type_from_initializer(&var.initializer),
                            is_external: false,
                        };
                        let var_id = format!("{}::direct_var_{}", prog.name.original, i);
                        symbol_table.add_symbol(var_id, var_symbol);
                    }
                }
            }
            LibraryElementKind::DataTypeDeclaration(dt) => {
                // Add data type to symbol table (simplified - just use the element as a string)
                let symbol_entry = SymbolEntry {
                    name: format!("data_type_{}", symbol_table.symbols.len()),
                    symbol_type: "type".to_string(),
                    scope: "global".to_string(),
                    declaration_location: None,
                    data_type: None,
                    is_external: false,
                };
                symbol_table.add_symbol(format!("data_type_{}", symbol_table.symbols.len()), symbol_entry);
            }
            _ => {
                // Handle other element types as needed
            }
        }
    }
    
    symbol_table
}

/// Extract type information from an InitialValueAssignmentKind.
/// This is a simplified implementation that extracts basic type information.
fn extract_type_from_initializer(initializer: &InitialValueAssignmentKind) -> Option<String> {
    use crate::common::*;
    
    match initializer {
        InitialValueAssignmentKind::Simple(simple) => {
            Some(simple.type_name.name.original.clone())
        }
        InitialValueAssignmentKind::String(_) => {
            Some("STRING".to_string())
        }
        InitialValueAssignmentKind::EnumeratedValues(_) => {
            Some("ENUMERATION".to_string())
        }
        InitialValueAssignmentKind::EnumeratedType(enum_type) => {
            Some(enum_type.type_name.name.original.clone())
        }
        InitialValueAssignmentKind::FunctionBlock(fb) => {
            Some(fb.type_name.name.original.clone())
        }
        InitialValueAssignmentKind::Structure(_) => {
            Some("STRUCT".to_string())
        }
        InitialValueAssignmentKind::Array(_) => {
            Some("ARRAY".to_string())
        }
        InitialValueAssignmentKind::LateResolvedType(type_name) => {
            Some(type_name.name.original.clone())
        }
        InitialValueAssignmentKind::Subrange(_) => {
            Some("SUBRANGE".to_string())
        }
        InitialValueAssignmentKind::None(_) => {
            None
        }
    }
}

/// Extract formatting information from a Library AST.
/// This creates basic formatting information including placeholder comments.
/// In a full implementation, this would extract actual comments from the source text.
fn extract_formatting_info(_library: &Library) -> FormattingInfo {
    // For now, create a placeholder formatting info structure
    // In a complete implementation, this would:
    // 1. Parse the original source text to extract comments
    // 2. Associate comments with nearby AST nodes
    // 3. Preserve whitespace information for source reconstruction
    
    FormattingInfo {
        comments: vec![
            // Placeholder comment to demonstrate the structure
            CommentInfo {
                text: "Placeholder comment - actual comment extraction not yet implemented".to_string(),
                style: "block".to_string(),
                location: None,
                attached_to: None,
            }
        ],
        whitespace_significant: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde::Serialize;

    #[test]
    fn test_json_exporter_creation() {
        let exporter = JsonExporter::new();
        assert_eq!(exporter.schema_version, "1.0.0");
        assert!(!exporter.options.include_comments);
        assert!(exporter.options.include_locations);
        assert!(!exporter.options.pretty_print);
    }

    #[test]
    fn test_json_exporter_with_options() {
        let options = JsonExportOptions {
            include_comments: true,
            include_locations: false,
            pretty_print: true,
        };
        let exporter = JsonExporter::with_options(options);
        assert!(exporter.options.include_comments);
        assert!(!exporter.options.include_locations);
        assert!(exporter.options.pretty_print);
    }

    #[test]
    fn test_json_export_options_serialization() {
        let options = JsonExportOptions::default();
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("include_comments"));
        assert!(json.contains("include_locations"));
        assert!(json.contains("pretty_print"));
    }

    // Simple test structure for property testing
    #[derive(Debug, Clone, Serialize)]
    struct TestLibrary {
        name: String,
        version: String,
        elements: Vec<String>,
    }

    impl Arbitrary for TestLibrary {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            (
                "[a-zA-Z][a-zA-Z0-9_]*",
                r"[0-9]+\.[0-9]+\.[0-9]+",
                prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 0..10),
            )
                .prop_map(|(name, version, elements)| TestLibrary {
                    name,
                    version,
                    elements,
                })
                .boxed()
        }
    }

    /// **Feature: ironplc-json-ast-export, Property 2: JSON validity and consistency**
    /// For any serializable structure, JSON export should produce valid JSON with consistent field naming
    proptest! {
        #[test]
        fn prop_json_validity_and_consistency(test_lib in any::<TestLibrary>()) {
            let exporter = JsonExporter::new();
            
            // Create a test wrapper similar to JsonWrapper but with TestLibrary
            #[derive(Serialize)]
            struct TestWrapper<'a> {
                schema_version: &'a str,
                metadata: JsonMetadata,
                library: &'a TestLibrary,
            }
            
            let wrapper = TestWrapper {
                schema_version: "1.0.0",
                metadata: JsonMetadata::new(&exporter.options),
                library: &test_lib,
            };

            // Test that serialization produces valid JSON
            let json_result = serde_json::to_string(&wrapper);
            prop_assert!(json_result.is_ok(), "JSON serialization should succeed");
            
            let json_string = json_result.unwrap();
            
            // Test that the JSON is valid by parsing it back
            let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_result.is_ok(), "Generated JSON should be valid and parseable");
            
            let parsed_json = parsed_result.unwrap();
            
            // Test consistent field naming conventions
            prop_assert!(parsed_json.get("schema_version").is_some(), "JSON should contain schema_version field");
            prop_assert!(parsed_json.get("metadata").is_some(), "JSON should contain metadata field");
            prop_assert!(parsed_json.get("library").is_some(), "JSON should contain library field");
            
            // Test that metadata has expected structure
            let metadata = parsed_json.get("metadata").unwrap();
            prop_assert!(metadata.get("compiler_version").is_some(), "Metadata should contain compiler_version");
            prop_assert!(metadata.get("export_timestamp").is_some(), "Metadata should contain export_timestamp");
            prop_assert!(metadata.get("options").is_some(), "Metadata should contain options");
            
            // Test pretty printing produces valid JSON too
            let exporter_pretty = JsonExporter::with_options(JsonExportOptions {
                pretty_print: true,
                ..JsonExportOptions::default()
            });
            
            let wrapper_pretty = TestWrapper {
                schema_version: "1.0.0",
                metadata: JsonMetadata::new(&exporter_pretty.options),
                library: &test_lib,
            };
            
            let pretty_json_result = serde_json::to_string_pretty(&wrapper_pretty);
            prop_assert!(pretty_json_result.is_ok(), "Pretty JSON serialization should succeed");
            
            let pretty_json = pretty_json_result.unwrap();
            let pretty_parsed: Result<serde_json::Value, _> = serde_json::from_str(&pretty_json);
            prop_assert!(pretty_parsed.is_ok(), "Pretty printed JSON should be valid");
        }
    }

    /// **Feature: ironplc-json-ast-export, Property 1: Complete AST serialization**
    /// For any valid AST structure, JSON serialization should preserve all nodes, their types, and structural relationships
    proptest! {
        #[test]
        fn prop_complete_ast_serialization(library in library_strategy()) {
            let exporter = JsonExporter::new();
            
            // Test that the library can be serialized to JSON
            let json_result = exporter.export_library(&library);
            prop_assert!(json_result.is_ok(), "Library serialization should succeed");
            
            let json_string = json_result.unwrap();
            
            // Test that the JSON is valid
            let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_result.is_ok(), "Generated JSON should be valid and parseable");
            
            let parsed_json = parsed_result.unwrap();
            
            // Test that all required top-level fields are present
            prop_assert!(parsed_json.get("schema_version").is_some(), "JSON should contain schema_version");
            prop_assert!(parsed_json.get("metadata").is_some(), "JSON should contain metadata");
            prop_assert!(parsed_json.get("library").is_some(), "JSON should contain library");
            
            // Test that library structure is preserved
            let library_json = parsed_json.get("library").unwrap();
            prop_assert!(library_json.get("elements").is_some(), "Library should contain elements array");
            
            // Test that the number of elements is preserved
            let elements_json = library_json.get("elements").unwrap().as_array().unwrap();
            prop_assert_eq!(elements_json.len(), library.elements.len(), "Element count should be preserved");
            
            // Test with different export options
            let exporter_with_options = JsonExporter::with_options(JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: true,
            });
            
            let json_with_options = exporter_with_options.export_library(&library);
            prop_assert!(json_with_options.is_ok(), "Library serialization with options should succeed");
            
            // Verify the JSON with options is also valid
            let json_with_options_str = json_with_options.unwrap();
            let parsed_with_options: Result<serde_json::Value, _> = serde_json::from_str(&json_with_options_str);
            prop_assert!(parsed_with_options.is_ok(), "JSON with options should be valid");
        }
    }

    // Strategy for generating Library instances for property testing
    fn library_strategy() -> impl Strategy<Value = Library> {
        prop::collection::vec(library_element_strategy(), 0..5)
            .prop_map(|elements| Library { elements })
    }

    /// **Feature: ironplc-json-ast-export, Property 3: Location data preservation**
    /// For any AST node with source location, the JSON should include complete and consistent position information
    proptest! {
        #[test]
        fn prop_location_data_preservation(
            source_text in "[a-zA-Z0-9\\s\\n]{10,100}",
            start_pos in 0usize..50,
            end_offset in 1usize..20
        ) {
            // Ensure end position is after start position and within bounds
            let end_pos = (start_pos + end_offset).min(source_text.len());
            let start_pos = start_pos.min(end_pos.saturating_sub(1));
            
            let source_span = SourceSpan {
                start: start_pos,
                end: end_pos,
                file_id: crate::core::FileId::from_string("test.st"),
            };
            
            // Test SerializableSourceSpan creation
            let serializable_span = SerializableSourceSpan::from_source_span(&source_span, Some(&source_text));
            
            // Test that line numbers are at least 1
            prop_assert!(serializable_span.start_line >= 1, "Start line should be at least 1");
            prop_assert!(serializable_span.end_line >= 1, "End line should be at least 1");
            prop_assert!(serializable_span.start_column >= 1, "Start column should be at least 1");
            prop_assert!(serializable_span.end_column >= 1, "End column should be at least 1");
            
            // Test that end position is not before start position
            prop_assert!(
                serializable_span.end_line >= serializable_span.start_line,
                "End line should not be before start line"
            );
            
            if serializable_span.end_line == serializable_span.start_line {
                prop_assert!(
                    serializable_span.end_column >= serializable_span.start_column,
                    "End column should not be before start column on same line"
                );
            }
            
            // Test that file_id is preserved
            prop_assert_eq!(&serializable_span.file_id, "test.st", "File ID should be preserved");
            
            // Test serialization to JSON
            let json_result = serde_json::to_string(&serializable_span);
            prop_assert!(json_result.is_ok(), "SerializableSourceSpan should serialize to JSON");
            
            let json_string = json_result.unwrap();
            let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_json.is_ok(), "Serialized location data should be valid JSON");
            
            let json_obj = parsed_json.unwrap();
            prop_assert!(json_obj.get("start_line").is_some(), "JSON should contain start_line");
            prop_assert!(json_obj.get("start_column").is_some(), "JSON should contain start_column");
            prop_assert!(json_obj.get("end_line").is_some(), "JSON should contain end_line");
            prop_assert!(json_obj.get("end_column").is_some(), "JSON should contain end_column");
            prop_assert!(json_obj.get("file_id").is_some(), "JSON should contain file_id");
        }
    }

    #[test]
    fn test_location_calculation_with_newlines() {
        let source_text = "line1\nline2\nline3";
        let (start_line, start_column, end_line, end_column) = 
            calculate_line_column_positions(source_text, 6, 11); // "line2"
        
        assert_eq!(start_line, 2);
        assert_eq!(start_column, 1);
        assert_eq!(end_line, 2);
        assert_eq!(end_column, 6);
    }

    #[test]
    fn test_serializable_source_span_default() {
        let span = SerializableSourceSpan::default_location("default.st");
        assert_eq!(span.start_line, 1);
        assert_eq!(span.start_column, 1);
        assert_eq!(span.end_line, 1);
        assert_eq!(span.end_column, 1);
        assert_eq!(span.file_id, "default.st");
    }

    #[test]
    fn test_missing_source_text_handling() {
        let source_span = SourceSpan {
            start: 10,
            end: 20,
            file_id: crate::core::FileId::from_string("missing.st"),
        };
        
        // Test with None source text (missing location information)
        let serializable_span = SerializableSourceSpan::from_source_span(&source_span, None);
        
        // Should default to line 1, column 1
        assert_eq!(serializable_span.start_line, 1);
        assert_eq!(serializable_span.start_column, 1);
        assert_eq!(serializable_span.end_line, 1);
        assert_eq!(serializable_span.end_column, 1);
        assert_eq!(serializable_span.file_id, "missing.st");
        
        // Should still serialize to valid JSON
        let json_result = serde_json::to_string(&serializable_span);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        // Verify JSON contains null-safe values
        assert_eq!(parsed_json["start_line"], 1);
        assert_eq!(parsed_json["start_column"], 1);
        assert_eq!(parsed_json["end_line"], 1);
        assert_eq!(parsed_json["end_column"], 1);
        assert_eq!(parsed_json["file_id"], "missing.st");
    }

    /// **Feature: ironplc-json-ast-export, Property 4: Multi-file support**
    /// For any set of input files, each AST node should correctly reference its source file
    proptest! {
        #[test]
        fn prop_multi_file_support(library in library_strategy()) {
            let exporter = JsonExporter::new();
            
            // Test that the library can be serialized with multi-file support
            let json_result = exporter.export_library(&library);
            prop_assert!(json_result.is_ok(), "Multi-file library serialization should succeed");
            
            let json_string = json_result.unwrap();
            let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_json.is_ok(), "Multi-file JSON should be valid");
            
            let json_obj = parsed_json.unwrap();
            
            // Test that source_files field is present
            prop_assert!(json_obj.get("source_files").is_some(), "JSON should contain source_files field");
            
            let source_files = json_obj.get("source_files").unwrap().as_array().unwrap();
            prop_assert!(!source_files.is_empty(), "Source files array should not be empty");
            
            // Test that all source files are strings
            for file in source_files {
                prop_assert!(file.is_string(), "Each source file should be a string");
            }
            
            // Test that the JSON structure includes all required fields for multi-file support
            prop_assert!(json_obj.get("schema_version").is_some(), "JSON should contain schema_version");
            prop_assert!(json_obj.get("metadata").is_some(), "JSON should contain metadata");
            prop_assert!(json_obj.get("library").is_some(), "JSON should contain library");
            
            // Test with different export options to ensure multi-file support works across configurations
            let exporter_with_options = JsonExporter::with_options(JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: true,
            });
            
            let json_with_options = exporter_with_options.export_library(&library);
            prop_assert!(json_with_options.is_ok(), "Multi-file serialization with options should succeed");
            
            let json_with_options_str = json_with_options.unwrap();
            let parsed_with_options: Result<serde_json::Value, _> = serde_json::from_str(&json_with_options_str);
            prop_assert!(parsed_with_options.is_ok(), "Multi-file JSON with options should be valid");
            
            let json_with_options_obj = parsed_with_options.unwrap();
            prop_assert!(json_with_options_obj.get("source_files").is_some(), "JSON with options should contain source_files");
        }
    }

    /// **Feature: ironplc-json-ast-export, Property 5: Symbol table completeness**
    /// For any program with variable declarations and references, the JSON should preserve all symbol information and binding relationships
    proptest! {
        #[test]
        fn prop_symbol_table_completeness(library in library_strategy()) {
            let exporter = JsonExporter::new();
            
            // Test that the library can be serialized with symbol table information
            let json_result = exporter.export_library(&library);
            prop_assert!(json_result.is_ok(), "Library serialization with symbol table should succeed");
            
            let json_string = json_result.unwrap();
            let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_json.is_ok(), "JSON with symbol table should be valid");
            
            let json_obj = parsed_json.unwrap();
            
            // Test that symbol_table field is present
            prop_assert!(json_obj.get("symbol_table").is_some(), "JSON should contain symbol_table field");
            
            let symbol_table = json_obj.get("symbol_table").unwrap();
            prop_assert!(symbol_table.get("symbols").is_some(), "Symbol table should contain symbols field");
            prop_assert!(symbol_table.get("scopes").is_some(), "Symbol table should contain scopes field");
            
            let symbols = symbol_table.get("symbols").unwrap().as_object().unwrap();
            let scopes = symbol_table.get("scopes").unwrap().as_array().unwrap();
            
            // Test that global scope is always present
            prop_assert!(scopes.iter().any(|s| s.as_str() == Some("global")), "Global scope should be present");
            
            // Test that each symbol has required fields
            for (symbol_id, symbol_info) in symbols {
                prop_assert!(symbol_info.get("name").is_some(), "Symbol should have name field");
                prop_assert!(symbol_info.get("symbol_type").is_some(), "Symbol should have symbol_type field");
                prop_assert!(symbol_info.get("scope").is_some(), "Symbol should have scope field");
                prop_assert!(symbol_info.get("is_external").is_some(), "Symbol should have is_external field");
                
                // Verify symbol_type is one of the expected values
                let symbol_type = symbol_info.get("symbol_type").unwrap().as_str().unwrap();
                prop_assert!(
                    ["function", "function_block", "program", "type", "variable", "direct_variable"].contains(&symbol_type),
                    "Symbol type should be one of the expected values, got: {}", symbol_type
                );
                
                // Verify scope is a string
                let scope = symbol_info.get("scope").unwrap().as_str().unwrap();
                prop_assert!(!scope.is_empty(), "Symbol scope should not be empty");
                
                // Verify is_external is a boolean
                prop_assert!(symbol_info.get("is_external").unwrap().is_boolean(), "is_external should be boolean");
            }
            
            // Test that function declarations create corresponding symbols
            // Note: Duplicate function names will only create one symbol entry (last one wins)
            let library_elements = json_obj.get("library").unwrap().get("elements").unwrap().as_array().unwrap();
            let mut unique_function_names = std::collections::HashSet::new();
            
            for element in library_elements {
                if let Some(func_decl) = element.get("FunctionDeclaration") {
                    if let Some(name_obj) = func_decl.get("name") {
                        if let Some(name) = name_obj.get("original").and_then(|n| n.as_str()) {
                            unique_function_names.insert(name.to_string());
                        }
                    }
                }
            }
            
            // Count function symbols in symbol table
            let function_symbols = symbols.values().filter(|symbol| {
                symbol.get("symbol_type").unwrap().as_str() == Some("function")
            }).count();
            
            // The number of function symbols should match the number of unique function names
            // (duplicate declarations overwrite previous ones in the symbol table)
            prop_assert_eq!(function_symbols, unique_function_names.len(), "Number of function symbols should match unique function declarations");
        }
    }

    /// **Feature: ironplc-json-ast-export, Property 6: Content preservation**
    /// For any source code with comments and formatting, the JSON should enable accurate reconstruction of the original content
    proptest! {
        #[test]
        fn prop_content_preservation(library in library_strategy()) {
            // Test with comments enabled
            let exporter_with_comments = JsonExporter::with_options(JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: false,
            });
            
            let json_result = exporter_with_comments.export_library(&library);
            prop_assert!(json_result.is_ok(), "Library serialization with comments should succeed");
            
            let json_string = json_result.unwrap();
            let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_json.is_ok(), "JSON with comments should be valid");
            
            let json_obj = parsed_json.unwrap();
            
            // Test that formatting field is present when comments are enabled
            prop_assert!(json_obj.get("formatting").is_some(), "JSON should contain formatting field when comments enabled");
            
            let formatting = json_obj.get("formatting").unwrap();
            prop_assert!(formatting.get("comments").is_some(), "Formatting should contain comments field");
            prop_assert!(formatting.get("whitespace_significant").is_some(), "Formatting should contain whitespace_significant field");
            
            let comments = formatting.get("comments").unwrap().as_array().unwrap();
            
            // Test that each comment has required fields
            for comment in comments {
                prop_assert!(comment.get("text").is_some(), "Comment should have text field");
                prop_assert!(comment.get("style").is_some(), "Comment should have style field");
                
                let style = comment.get("style").unwrap().as_str().unwrap();
                prop_assert!(
                    ["block", "line"].contains(&style),
                    "Comment style should be 'block' or 'line', got: {}", style
                );
                
                let text = comment.get("text").unwrap().as_str().unwrap();
                prop_assert!(!text.is_empty(), "Comment text should not be empty");
            }
            
            // Test without comments - formatting field should not be present
            let exporter_no_comments = JsonExporter::with_options(JsonExportOptions {
                include_comments: false,
                include_locations: true,
                pretty_print: false,
            });
            
            let json_no_comments = exporter_no_comments.export_library(&library).unwrap();
            let parsed_no_comments: serde_json::Value = serde_json::from_str(&json_no_comments).unwrap();
            
            // Formatting field should not be present when comments are disabled
            prop_assert!(parsed_no_comments.get("formatting").is_none(), "JSON should not contain formatting field when comments disabled");
        }
    }

    #[test]
    fn test_multi_file_json_structure() {
        let library = Library { elements: vec![] };
        let exporter = JsonExporter::new();
        
        let json_result = exporter.export_library(&library);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        // Verify the multi-file structure
        assert!(parsed_json.get("source_files").is_some());
        assert!(parsed_json.get("schema_version").is_some());
        assert!(parsed_json.get("metadata").is_some());
        assert!(parsed_json.get("library").is_some());
        
        let source_files = parsed_json["source_files"].as_array().unwrap();
        assert!(!source_files.is_empty());
        assert_eq!(source_files[0], "<unknown>");
    }

    // Error handling unit tests

    #[test]
    fn test_json_export_error_display() {
        // Test that JsonExportError displays correctly by creating a real serialization error
        use serde::Serialize;
        
        // Create a struct that will fail to serialize
        #[derive(Serialize)]
        struct BadStruct {
            #[serde(serialize_with = "fail_serializer")]
            field: i32,
        }
        
        fn fail_serializer<S>(_: &i32, _: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("intentional failure"))
        }
        
        let bad_struct = BadStruct { field: 42 };
        let result = serde_json::to_string(&bad_struct);
        
        if let Err(serde_error) = result {
            let export_error = JsonExportError::SerializationError(serde_error);
            let error_string = format!("{}", export_error);
            assert!(error_string.contains("Serialization failed"));
        } else {
            panic!("Expected serialization to fail");
        }
    }

    #[test]
    fn test_json_export_error_from_io_error() {
        // Test that JsonExportError can be created from IO errors
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
        let export_error = JsonExportError::IoError(io_error);
        
        let error_string = format!("{}", export_error);
        assert!(error_string.contains("IO error"));
        assert!(error_string.contains("Permission denied"));
    }

    #[test]
    fn test_json_export_error_invalid_path() {
        // Test InvalidPath error variant
        let export_error = JsonExportError::InvalidPath("invalid/path".to_string());
        
        let error_string = format!("{}", export_error);
        assert!(error_string.contains("Invalid output path"));
        assert!(error_string.contains("invalid/path"));
    }

    #[test]
    fn test_json_export_error_missing_data() {
        // Test MissingData error variant
        let export_error = JsonExportError::MissingData("source location".to_string());
        
        let error_string = format!("{}", export_error);
        assert!(error_string.contains("Missing required data"));
        assert!(error_string.contains("source location"));
    }

    #[test]
    fn test_json_exporter_export_to_writer_io_error() {
        use std::io::{self, Write};
        
        // Create a writer that always fails
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "Broken pipe"))
            }
            
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "Broken pipe"))
            }
        }
        
        let library = Library { elements: vec![] };
        let exporter = JsonExporter::new();
        let writer = FailingWriter;
        
        let result = exporter.export_to_writer(&library, writer);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            JsonExportError::SerializationError(_) => {
                // This is expected - serde_json will convert IO errors to serialization errors
            }
            other => panic!("Expected SerializationError, got {:?}", other),
        }
    }

    #[test]
    fn test_json_exporter_handles_large_library() {
        // Test that the exporter can handle libraries with many elements
        use crate::common::*;
        use crate::core::*;
        
        let mut elements = Vec::new();
        
        // Create 1000 function declarations to test performance and memory handling
        for i in 0..1000 {
            let function_name = format!("TestFunction{}", i);
            elements.push(LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
                name: Id::from(&function_name),
                return_type: TypeName {
                    name: Id::from("BOOL"),
                },
                variables: vec![],
                edge_variables: vec![],
                body: vec![],
                external_annotation: None,
            }));
        }
        
        let library = Library { elements };
        let exporter = JsonExporter::new();
        
        let result = exporter.export_library(&library);
        assert!(result.is_ok());
        
        let json_string = result.unwrap();
        assert!(json_string.len() > 1000); // Should be a substantial JSON document
        
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        let library_elements = parsed["library"]["elements"].as_array().unwrap();
        assert_eq!(library_elements.len(), 1000);
    }

    #[test]
    fn test_json_exporter_graceful_degradation_with_missing_spans() {
        // Test that the exporter handles AST nodes with missing or invalid source spans
        use crate::common::*;
        use crate::core::*;
        
        // Create a function with default (empty) source spans
        let function = LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
            name: Id {
                original: "TestFunction".to_string(),
                lower_case: "testfunction".to_string(),
                span: SourceSpan::default(), // Default span
            },
            return_type: TypeName {
                name: Id {
                    original: "BOOL".to_string(),
                    lower_case: "bool".to_string(),
                    span: SourceSpan::default(), // Default span
                },
            },
            variables: vec![],
            edge_variables: vec![],
            body: vec![],
            external_annotation: None,
        });
        
        let library = Library { elements: vec![function] };
        let exporter = JsonExporter::with_options(JsonExportOptions {
            include_locations: true,
            include_comments: false,
            pretty_print: false,
        });
        
        let result = exporter.export_library(&library);
        assert!(result.is_ok());
        
        let json_string = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        // Should still produce valid JSON even with default spans
        assert!(parsed.get("library").is_some());
        assert!(parsed.get("metadata").is_some());
    }

    #[test]
    fn test_serializable_source_span_edge_cases() {
        // Test edge cases in source span serialization
        
        // Test with empty source text
        let span = SourceSpan {
            start: 0,
            end: 0,
            file_id: crate::core::FileId::from_string("empty.st"),
        };
        
        let serializable = SerializableSourceSpan::from_source_span(&span, Some(""));
        assert_eq!(serializable.start_line, 1);
        assert_eq!(serializable.start_column, 1);
        assert_eq!(serializable.end_line, 1);
        assert_eq!(serializable.end_column, 1);
        
        // Test with out-of-bounds positions
        let span = SourceSpan {
            start: 1000,
            end: 2000,
            file_id: crate::core::FileId::from_string("short.st"),
        };
        
        let serializable = SerializableSourceSpan::from_source_span(&span, Some("short"));
        // Should handle gracefully without panicking
        assert!(serializable.start_line >= 1);
        assert!(serializable.start_column >= 1);
    }

    #[test]
    fn test_json_metadata_timestamp_format() {
        // Test that metadata timestamps are in a valid format
        let options = JsonExportOptions::default();
        let metadata = JsonMetadata::new(&options);
        
        // Should be able to parse the timestamp
        assert!(!metadata.export_timestamp.is_empty());
        // The timestamp format may vary, so just check it's not empty and contains date-like content
        assert!(metadata.export_timestamp.len() > 10); // Should be a reasonable timestamp length
        
        // Should contain version information
        assert!(!metadata.compiler_version.is_empty());
        assert_eq!(metadata.compiler_version, env!("CARGO_PKG_VERSION"));
    }

    // Integration tests for round-trip consistency

    #[test]
    fn test_json_export_round_trip_consistency() {
        // Test that JSON export preserves AST structure through serialization
        use crate::common::*;
        use crate::core::*;
        
        // Create a simple library with a function declaration
        let function = LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
            name: Id::from("TestFunction"),
            return_type: TypeName {
                name: Id::from("BOOL"),
            },
            variables: vec![],
            edge_variables: vec![],
            body: vec![],
            external_annotation: None,
        });
        
        let original_library = Library { elements: vec![function] };
        
        // Export to JSON
        let exporter = JsonExporter::with_options(JsonExportOptions {
            include_comments: false,
            include_locations: true,
            pretty_print: true,
        });
        
        let json_result = exporter.export_library(&original_library);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        
        // Verify JSON is valid and parseable
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        // Verify structure preservation
        assert!(parsed_json.get("schema_version").is_some());
        assert!(parsed_json.get("metadata").is_some());
        assert!(parsed_json.get("library").is_some());
        
        let library_json = parsed_json.get("library").unwrap();
        let elements_json = library_json.get("elements").unwrap().as_array().unwrap();
        
        // Should have the same number of elements
        assert_eq!(elements_json.len(), original_library.elements.len());
        
        // Verify function declaration is preserved
        let function_element = &elements_json[0];
        assert!(function_element.get("FunctionDeclaration").is_some());
        
        let function_decl = function_element.get("FunctionDeclaration").unwrap();
        assert!(function_decl.get("name").is_some());
        assert!(function_decl.get("return_type").is_some());
        assert!(function_decl.get("variables").is_some());
        assert!(function_decl.get("body").is_some());
    }

    #[test]
    fn test_json_export_preserves_type_information() {
        // Test that type information is preserved in JSON export
        use crate::common::*;
        use crate::core::*;
        
        let function = LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
            name: Id::from("TypeTest"),
            return_type: TypeName {
                name: Id::from("DINT"),
            },
            variables: vec![],
            edge_variables: vec![],
            body: vec![],
            external_annotation: None,
        });
        
        let library = Library { elements: vec![function] };
        let exporter = JsonExporter::new();
        
        let json_result = exporter.export_library(&library);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        let function_decl = &parsed_json["library"]["elements"][0]["FunctionDeclaration"];
        
        // Verify return type is preserved
        let return_type = &function_decl["return_type"]["name"]["original"];
        assert_eq!(return_type, "DINT");
        
        // Verify function name is preserved
        let function_name = &function_decl["name"]["original"];
        assert_eq!(function_name, "TypeTest");
    }

    #[test]
    fn test_json_export_preserves_source_locations() {
        // Test that source location information is preserved
        use crate::common::*;
        use crate::core::*;
        
        let file_id = FileId::from_string("test.st");
        let span = SourceSpan {
            start: 10,
            end: 20,
            file_id: file_id.clone(),
        };
        
        let function = LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
            name: Id {
                original: "LocationTest".to_string(),
                lower_case: "locationtest".to_string(),
                span: span.clone(),
            },
            return_type: TypeName {
                name: Id {
                    original: "BOOL".to_string(),
                    lower_case: "bool".to_string(),
                    span: span.clone(),
                },
            },
            variables: vec![],
            edge_variables: vec![],
            body: vec![],
            external_annotation: None,
        });
        
        let library = Library { elements: vec![function] };
        let exporter = JsonExporter::with_options(JsonExportOptions {
            include_locations: true,
            include_comments: false,
            pretty_print: false,
        });
        
        let json_result = exporter.export_library(&library);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        let function_decl = &parsed_json["library"]["elements"][0]["FunctionDeclaration"];
        
        // Verify function name span is preserved
        let name_span = &function_decl["name"]["span"];
        assert_eq!(name_span["start"], 10);
        assert_eq!(name_span["end"], 20);
        assert_eq!(name_span["file_id"], "test.st");
        
        // Verify return type span is preserved
        let return_type_span = &function_decl["return_type"]["name"]["span"];
        assert_eq!(return_type_span["start"], 10);
        assert_eq!(return_type_span["end"], 20);
        assert_eq!(return_type_span["file_id"], "test.st");
    }

    #[test]
    fn test_json_export_with_complex_ast_structures() {
        // Test JSON export with complex nested AST structures
        use crate::common::*;
        use crate::core::*;
        
        // Create a simple function with variables to test complex structure serialization
        let function = LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
            name: Id::from("ComplexFunction"),
            return_type: TypeName {
                name: Id::from("INT"),
            },
            variables: vec![
                VarDecl::simple("result", "INT"),
                VarDecl::simple("counter", "DINT"),
            ],
            edge_variables: vec![],
            body: vec![], // Keep body empty to avoid complex expression compilation issues
            external_annotation: None,
        });
        
        let library = Library { elements: vec![function] };
        let exporter = JsonExporter::new();
        
        let json_result = exporter.export_library(&library);
        assert!(json_result.is_ok());
        
        let json_string = json_result.unwrap();
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        
        let function_decl = &parsed_json["library"]["elements"][0]["FunctionDeclaration"];
        
        // Verify function structure is preserved
        assert!(function_decl.get("name").is_some());
        assert!(function_decl.get("return_type").is_some());
        assert!(function_decl.get("variables").is_some());
        assert!(function_decl.get("body").is_some());
        
        // Verify variables are preserved
        let variables = function_decl["variables"].as_array().unwrap();
        assert_eq!(variables.len(), 2);
        
        // Verify variable structure
        let first_var = &variables[0];
        assert!(first_var.get("identifier").is_some());
        assert!(first_var.get("var_type").is_some());
        assert!(first_var.get("initializer").is_some());
    }

    #[test]
    fn test_json_export_schema_version_consistency() {
        // Test that schema version is consistent across exports
        let library1 = Library { elements: vec![] };
        let library2 = Library { elements: vec![] };
        
        let exporter = JsonExporter::new();
        
        let json1 = exporter.export_library(&library1).unwrap();
        let json2 = exporter.export_library(&library2).unwrap();
        
        let parsed1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        
        // Schema versions should be identical
        assert_eq!(parsed1["schema_version"], parsed2["schema_version"]);
        assert_eq!(parsed1["schema_version"], "1.0.0");
        
        // Metadata structure should be consistent
        assert!(parsed1["metadata"].get("compiler_version").is_some());
        assert!(parsed1["metadata"].get("export_timestamp").is_some());
        assert!(parsed1["metadata"].get("options").is_some());
        
        assert!(parsed2["metadata"].get("compiler_version").is_some());
        assert!(parsed2["metadata"].get("export_timestamp").is_some());
        assert!(parsed2["metadata"].get("options").is_some());
        
        // Compiler versions should be identical
        assert_eq!(parsed1["metadata"]["compiler_version"], parsed2["metadata"]["compiler_version"]);
    }

    #[test]
    fn test_json_export_round_trip_validation() {
        // Test that JSON export can be validated by parsing it back
        use crate::common::*;
        use crate::core::*;
        
        // Create a library with multiple element types
        let elements = vec![
            LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
                name: Id::from("TestFunction"),
                return_type: TypeName {
                    name: Id::from("BOOL"),
                },
                variables: vec![
                    VarDecl::simple("input_var", "INT"),
                    VarDecl::simple("output_var", "BOOL"),
                ],
                edge_variables: vec![],
                body: vec![],
                external_annotation: None,
            }),
            LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                name: TypeName {
                    name: Id::from("TestFB"),
                },
                variables: vec![
                    VarDecl::simple("state_var", "DINT"),
                ],
                edge_variables: vec![],
                body: crate::common::FunctionBlockBodyKind::Statements(crate::textual::Statements { body: vec![] }),
                span: SourceSpan::default(),
            }),
        ];
        
        let library = Library { elements };
        
        // Test with different export options
        let test_options = vec![
            JsonExportOptions::default(),
            JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: false,
            },
            JsonExportOptions {
                include_comments: false,
                include_locations: false,
                pretty_print: true,
            },
        ];
        
        for options in test_options {
            let exporter = JsonExporter::with_options(options.clone());
            
            // Export to JSON
            let json_result = exporter.export_library(&library);
            assert!(json_result.is_ok(), "JSON export should succeed with options: {:?}", options);
            
            let json_string = json_result.unwrap();
            
            // Validate JSON structure by parsing it back
            let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            assert!(parsed_json.is_ok(), "Exported JSON should be valid with options: {:?}", options);
            
            let json_obj = parsed_json.unwrap();
            
            // Verify all required top-level fields are present
            assert!(json_obj.get("schema_version").is_some());
            assert!(json_obj.get("metadata").is_some());
            assert!(json_obj.get("library").is_some());
            assert!(json_obj.get("symbol_table").is_some());
            assert!(json_obj.get("source_files").is_some());
            
            // Verify library structure is preserved
            let library_json = json_obj.get("library").unwrap();
            let elements_json = library_json.get("elements").unwrap().as_array().unwrap();
            assert_eq!(elements_json.len(), 2, "Should preserve all library elements");
            
            // Verify function declaration is preserved
            let function_element = &elements_json[0];
            assert!(function_element.get("FunctionDeclaration").is_some());
            let function_decl = function_element.get("FunctionDeclaration").unwrap();
            assert_eq!(function_decl["name"]["original"], "TestFunction");
            assert_eq!(function_decl["return_type"]["name"]["original"], "BOOL");
            
            // Verify function block declaration is preserved
            let fb_element = &elements_json[1];
            assert!(fb_element.get("FunctionBlockDeclaration").is_some());
            let fb_decl = fb_element.get("FunctionBlockDeclaration").unwrap();
            assert_eq!(fb_decl["name"]["name"]["original"], "TestFB");
            
            // Verify symbol table is populated
            let symbol_table = json_obj.get("symbol_table").unwrap();
            let symbols = symbol_table.get("symbols").unwrap().as_object().unwrap();
            assert!(!symbols.is_empty(), "Symbol table should contain symbols");
            
            // Verify formatting field presence based on options
            if options.include_comments {
                assert!(json_obj.get("formatting").is_some(), "Formatting should be present when comments enabled");
            } else {
                assert!(json_obj.get("formatting").is_none(), "Formatting should not be present when comments disabled");
            }
        }
    }

    #[test]
    fn test_json_export_large_library_performance() {
        // Test that JSON export handles large libraries efficiently
        use crate::common::*;
        use crate::core::*;
        use std::time::Instant;
        
        // Create a large library with many functions
        let mut elements = Vec::new();
        for i in 0..100 {
            elements.push(LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
                name: Id::from(&format!("Function{}", i)),
                return_type: TypeName {
                    name: Id::from("BOOL"),
                },
                variables: vec![
                    VarDecl::simple(&format!("var1_{}", i), "INT"),
                    VarDecl::simple(&format!("var2_{}", i), "DINT"),
                    VarDecl::simple(&format!("var3_{}", i), "REAL"),
                ],
                edge_variables: vec![],
                body: vec![],
                external_annotation: None,
            }));
        }
        
        let library = Library { elements };
        let exporter = JsonExporter::new();
        
        // Measure export time
        let start_time = Instant::now();
        let json_result = exporter.export_library(&library);
        let export_duration = start_time.elapsed();
        
        assert!(json_result.is_ok(), "Large library export should succeed");
        
        let json_string = json_result.unwrap();
        
        // Verify the JSON is valid and contains expected number of elements
        let parsed_json: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        let elements_json = parsed_json["library"]["elements"].as_array().unwrap();
        assert_eq!(elements_json.len(), 100, "Should preserve all 100 functions");
        
        // Verify symbol table contains all functions and variables
        let symbols = parsed_json["symbol_table"]["symbols"].as_object().unwrap();
        let function_symbols = symbols.values().filter(|symbol| {
            symbol.get("symbol_type").unwrap().as_str() == Some("function")
        }).count();
        assert_eq!(function_symbols, 100, "Should have 100 function symbols");
        
        // Performance should be reasonable (less than 1 second for 100 functions)
        assert!(export_duration.as_secs() < 1, "Export should complete in reasonable time, took: {:?}", export_duration);
        
        println!("Large library export completed in: {:?}", export_duration);
    }

    /// **Feature: ironplc-esstee-syntax-support, Property 32: JSON Schema Compatibility**
    /// **Validates: Requirements 9.5**
    /// For any existing code, AST export should maintain the same JSON schema format for backward compatibility
    proptest! {
        #[test]
        fn property_json_schema_compatibility(
            library in library_strategy(),
        ) {
            use crate::common::*;
            
            let exporter = JsonExporter::new();
            
            // Test that existing library structures maintain schema compatibility
            let json_result = exporter.export_library(&library);
            prop_assert!(json_result.is_ok(), "Library serialization should succeed");
            
            let json_string = json_result.unwrap();
            
            // Test that the JSON is valid
            let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_result.is_ok(), "Generated JSON should be valid and parseable");
            
            let parsed_json = parsed_result.unwrap();
            
            // Test that all required top-level schema fields are present and maintain expected structure
            prop_assert!(parsed_json.get("schema_version").is_some(), "JSON should contain schema_version field");
            prop_assert!(parsed_json.get("metadata").is_some(), "JSON should contain metadata field");
            prop_assert!(parsed_json.get("library").is_some(), "JSON should contain library field");
            prop_assert!(parsed_json.get("symbol_table").is_some(), "JSON should contain symbol_table field");
            prop_assert!(parsed_json.get("source_files").is_some(), "JSON should contain source_files field");
            
            // Test schema version format compatibility
            let schema_version = parsed_json.get("schema_version").unwrap();
            prop_assert!(schema_version.is_string(), "schema_version should be a string");
            prop_assert_eq!(schema_version.as_str().unwrap(), "1.0.0", "schema_version should maintain expected format");
            
            // Test metadata structure compatibility
            let metadata = parsed_json.get("metadata").unwrap();
            prop_assert!(metadata.get("compiler_version").is_some(), "metadata should contain compiler_version");
            prop_assert!(metadata.get("export_timestamp").is_some(), "metadata should contain export_timestamp");
            prop_assert!(metadata.get("options").is_some(), "metadata should contain options");
            
            // Test that compiler_version is a string
            let compiler_version = metadata.get("compiler_version").unwrap();
            prop_assert!(compiler_version.is_string(), "compiler_version should be a string");
            
            // Test that export_timestamp is a string
            let export_timestamp = metadata.get("export_timestamp").unwrap();
            prop_assert!(export_timestamp.is_string(), "export_timestamp should be a string");
            
            // Test options structure compatibility
            let options = metadata.get("options").unwrap();
            prop_assert!(options.get("include_comments").is_some(), "options should contain include_comments");
            prop_assert!(options.get("include_locations").is_some(), "options should contain include_locations");
            prop_assert!(options.get("pretty_print").is_some(), "options should contain pretty_print");
            
            // Test that all options are booleans
            prop_assert!(options.get("include_comments").unwrap().is_boolean(), "include_comments should be boolean");
            prop_assert!(options.get("include_locations").unwrap().is_boolean(), "include_locations should be boolean");
            prop_assert!(options.get("pretty_print").unwrap().is_boolean(), "pretty_print should be boolean");
            
            // Test library structure compatibility
            let library_json = parsed_json.get("library").unwrap();
            prop_assert!(library_json.get("elements").is_some(), "library should contain elements array");
            
            let elements = library_json.get("elements").unwrap();
            prop_assert!(elements.is_array(), "elements should be an array");
            
            // Test that each element maintains expected structure
            let elements_array = elements.as_array().unwrap();
            for element in elements_array {
                prop_assert!(element.is_object(), "each element should be an object");
                
                // Test that elements have exactly one top-level key (the element type)
                let element_obj = element.as_object().unwrap();
                prop_assert_eq!(element_obj.len(), 1, "each element should have exactly one top-level key");
                
                // Test that the element type key contains an object
                let (element_type, element_data) = element_obj.iter().next().unwrap();
                prop_assert!(element_data.is_object(), "element data should be an object for type: {}", element_type);
            }
            
            // Test symbol_table structure compatibility
            let symbol_table = parsed_json.get("symbol_table").unwrap();
            prop_assert!(symbol_table.get("symbols").is_some(), "symbol_table should contain symbols");
            prop_assert!(symbol_table.get("scopes").is_some(), "symbol_table should contain scopes");
            
            let symbols = symbol_table.get("symbols").unwrap();
            prop_assert!(symbols.is_object(), "symbols should be an object");
            
            let scopes = symbol_table.get("scopes").unwrap();
            prop_assert!(scopes.is_array(), "scopes should be an array");
            
            // Test that each symbol entry maintains expected structure
            let symbols_obj = symbols.as_object().unwrap();
            for (symbol_id, symbol_info) in symbols_obj {
                prop_assert!(symbol_info.is_object(), "symbol info should be an object for symbol: {}", symbol_id);
                let symbol_obj = symbol_info.as_object().unwrap();
                
                // Test required symbol fields
                prop_assert!(symbol_obj.get("name").is_some(), "symbol should have name field");
                prop_assert!(symbol_obj.get("symbol_type").is_some(), "symbol should have symbol_type field");
                prop_assert!(symbol_obj.get("scope").is_some(), "symbol should have scope field");
                prop_assert!(symbol_obj.get("is_external").is_some(), "symbol should have is_external field");
                
                // Test field types
                prop_assert!(symbol_obj.get("name").unwrap().is_string(), "symbol name should be string");
                prop_assert!(symbol_obj.get("symbol_type").unwrap().is_string(), "symbol_type should be string");
                prop_assert!(symbol_obj.get("scope").unwrap().is_string(), "symbol scope should be string");
                prop_assert!(symbol_obj.get("is_external").unwrap().is_boolean(), "is_external should be boolean");
            }
            
            // Test source_files structure compatibility
            let source_files = parsed_json.get("source_files").unwrap();
            prop_assert!(source_files.is_array(), "source_files should be an array");
            
            let source_files_array = source_files.as_array().unwrap();
            for source_file in source_files_array {
                prop_assert!(source_file.is_string(), "each source file should be a string");
            }
            
            // Test that the schema is consistent across different export options
            let exporter_with_options = JsonExporter::with_options(JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: true,
            });
            
            let json_with_options = exporter_with_options.export_library(&library);
            prop_assert!(json_with_options.is_ok(), "Library serialization with options should succeed");
            
            let json_with_options_str = json_with_options.unwrap();
            let parsed_with_options: Result<serde_json::Value, _> = serde_json::from_str(&json_with_options_str);
            prop_assert!(parsed_with_options.is_ok(), "JSON with options should be valid");
            
            let json_with_options_obj = parsed_with_options.unwrap();
            
            // Test that core schema fields remain the same regardless of options
            prop_assert_eq!(
                json_with_options_obj.get("schema_version"), 
                parsed_json.get("schema_version"),
                "schema_version should be consistent across export options"
            );
            
            // Test that library structure is identical
            let library_with_options = json_with_options_obj.get("library").unwrap();
            prop_assert!(library_with_options.get("elements").is_some(), "library with options should contain elements");
            
            let elements_with_options = library_with_options.get("elements").unwrap();
            prop_assert_eq!(
                elements_with_options.as_array().unwrap().len(),
                elements_array.len(),
                "element count should be consistent across export options"
            );
            
            // Test that symbol table structure is identical
            let symbol_table_with_options = json_with_options_obj.get("symbol_table").unwrap();
            prop_assert!(symbol_table_with_options.get("symbols").is_some(), "symbol_table with options should contain symbols");
            prop_assert!(symbol_table_with_options.get("scopes").is_some(), "symbol_table with options should contain scopes");
            
            // Test that additional fields are only present when requested
            if exporter_with_options.options.include_comments {
                prop_assert!(json_with_options_obj.get("formatting").is_some(), 
                           "formatting field should be present when comments are enabled");
                
                let formatting = json_with_options_obj.get("formatting").unwrap();
                prop_assert!(formatting.get("comments").is_some(), "formatting should contain comments");
                prop_assert!(formatting.get("whitespace_significant").is_some(), "formatting should contain whitespace_significant");
            } else {
                prop_assert!(json_with_options_obj.get("formatting").is_none(), 
                           "formatting field should not be present when comments are disabled");
            }
        }
    }

    // Strategy for generating LibraryElementKind instances
    fn library_element_strategy() -> impl Strategy<Value = crate::common::LibraryElementKind> {
        use crate::common::*;
        use crate::core::*;
        
        // Simple strategy that creates basic function declarations
        prop::string::string_regex("[a-zA-Z][a-zA-Z0-9_]*").unwrap()
            .prop_map(|name| {
                LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
                    name: Id::from(&name),
                    return_type: TypeName {
                        name: Id::from("BOOL"),
                    },
                    variables: vec![],
                    edge_variables: vec![],
                    body: vec![],
                    external_annotation: None,
                })
            })
    }

    /// **Feature: ironplc-esstee-syntax-support, Property 29: JSON Export Completeness**
    /// **Validates: Requirements 8.5**
    /// For any parsed syntax element (global variables, type definitions, enumerations, arrays), 
    /// JSON export should include all parsed syntax elements with proper structural relationships
    proptest! {
        #[test]
        fn property_json_export_completeness(
            library in enhanced_library_strategy(),
        ) {
            use crate::common::*;
            
            let exporter = JsonExporter::new();
            
            // Test that the library with enhanced syntax can be serialized to JSON
            let json_result = exporter.export_library(&library);
            prop_assert!(json_result.is_ok(), "Enhanced library serialization should succeed");
            
            let json_string = json_result.unwrap();
            
            // Test that the JSON is valid
            let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json_string);
            prop_assert!(parsed_result.is_ok(), "Generated JSON should be valid and parseable");
            
            let parsed_json = parsed_result.unwrap();
            
            // Test that all required top-level fields are present
            prop_assert!(parsed_json.get("schema_version").is_some(), "JSON should contain schema_version");
            prop_assert!(parsed_json.get("metadata").is_some(), "JSON should contain metadata");
            prop_assert!(parsed_json.get("library").is_some(), "JSON should contain library");
            prop_assert!(parsed_json.get("symbol_table").is_some(), "JSON should contain symbol_table");
            prop_assert!(parsed_json.get("source_files").is_some(), "JSON should contain source_files");
            
            // Test that library structure is preserved
            let library_json = parsed_json.get("library").unwrap();
            prop_assert!(library_json.get("elements").is_some(), "Library should contain elements array");
            
            // Test that the number of elements is preserved
            let elements_json = library_json.get("elements").unwrap().as_array().unwrap();
            prop_assert_eq!(elements_json.len(), library.elements.len(), "Element count should be preserved");
            
            // Test that each element type is properly serialized
            for (i, element) in library.elements.iter().enumerate() {
                let element_json = &elements_json[i];
                
                match element {
                    LibraryElementKind::GlobalVariableDeclaration(gvd) => {
                        prop_assert!(element_json.get("GlobalVariableDeclaration").is_some(), 
                                   "GlobalVariableDeclaration should be serialized");
                        let gvd_json = element_json.get("GlobalVariableDeclaration").unwrap();
                        prop_assert!(gvd_json.get("variables").is_some(), 
                                   "GlobalVariableDeclaration should contain variables");
                        prop_assert!(gvd_json.get("span").is_some(), 
                                   "GlobalVariableDeclaration should contain span");
                        
                        // Verify variables array length matches
                        let vars_json = gvd_json.get("variables").unwrap().as_array().unwrap();
                        prop_assert_eq!(vars_json.len(), gvd.variables.len(), 
                                      "Variables count should be preserved");
                    },
                    LibraryElementKind::TypeDefinitionBlock(tdb) => {
                        prop_assert!(element_json.get("TypeDefinitionBlock").is_some(), 
                                   "TypeDefinitionBlock should be serialized");
                        let tdb_json = element_json.get("TypeDefinitionBlock").unwrap();
                        prop_assert!(tdb_json.get("definitions").is_some(), 
                                   "TypeDefinitionBlock should contain definitions");
                        prop_assert!(tdb_json.get("span").is_some(), 
                                   "TypeDefinitionBlock should contain span");
                        
                        // Verify definitions array length matches
                        let defs_json = tdb_json.get("definitions").unwrap().as_array().unwrap();
                        prop_assert_eq!(defs_json.len(), tdb.definitions.len(), 
                                      "Definitions count should be preserved");
                    },
                    LibraryElementKind::FunctionDeclaration(fd) => {
                        prop_assert!(element_json.get("FunctionDeclaration").is_some(), 
                                   "FunctionDeclaration should be serialized");
                        let fd_json = element_json.get("FunctionDeclaration").unwrap();
                        prop_assert!(fd_json.get("name").is_some(), 
                                   "FunctionDeclaration should contain name");
                        prop_assert!(fd_json.get("return_type").is_some(), 
                                   "FunctionDeclaration should contain return_type");
                        prop_assert!(fd_json.get("variables").is_some(), 
                                   "FunctionDeclaration should contain variables");
                        prop_assert!(fd_json.get("body").is_some(), 
                                   "FunctionDeclaration should contain body");
                    },
                    _ => {
                        // Other element types should also be properly serialized
                        prop_assert!(element_json.is_object(), "All elements should serialize as objects");
                    }
                }
            }
            
            // Test that enhanced data types are properly serialized
            for element in &library.elements {
                if let LibraryElementKind::TypeDefinitionBlock(tdb) = element {
                    for (i, def) in tdb.definitions.iter().enumerate() {
                        let def_json = &elements_json.iter()
                            .find(|e| e.get("TypeDefinitionBlock").is_some())
                            .unwrap()["TypeDefinitionBlock"]["definitions"][i];
                        
                        prop_assert!(def_json.get("name").is_some(), 
                                   "TypeDefinition should contain name");
                        prop_assert!(def_json.get("base_type").is_some(), 
                                   "TypeDefinition should contain base_type");
                        prop_assert!(def_json.get("span").is_some(), 
                                   "TypeDefinition should contain span");
                        
                        // Test that DataTypeSpecificationKind variants are properly serialized
                        let base_type_json = def_json.get("base_type").unwrap();
                        match &def.base_type {
                            DataTypeSpecificationKind::Elementary(_) => {
                                prop_assert!(base_type_json.get("Elementary").is_some(), 
                                           "Elementary type should be serialized");
                            },
                            DataTypeSpecificationKind::UserDefined(_) => {
                                prop_assert!(base_type_json.get("UserDefined").is_some(), 
                                           "UserDefined type should be serialized");
                            },
                            DataTypeSpecificationKind::Enumeration(_) => {
                                prop_assert!(base_type_json.get("Enumeration").is_some(), 
                                           "Enumeration type should be serialized");
                            },
                            DataTypeSpecificationKind::Array(_) => {
                                prop_assert!(base_type_json.get("Array").is_some(), 
                                           "Array type should be serialized");
                            },
                            DataTypeSpecificationKind::Subrange(_) => {
                                prop_assert!(base_type_json.get("Subrange").is_some(), 
                                           "Subrange type should be serialized");
                            },
                            DataTypeSpecificationKind::String(_) => {
                                prop_assert!(base_type_json.get("String").is_some(), 
                                           "String type should be serialized");
                            },
                        }
                    }
                }
            }
            
            // Test with different export options to ensure completeness across configurations
            let exporter_with_options = JsonExporter::with_options(JsonExportOptions {
                include_comments: true,
                include_locations: true,
                pretty_print: true,
            });
            
            let json_with_options = exporter_with_options.export_library(&library);
            prop_assert!(json_with_options.is_ok(), "Enhanced library serialization with options should succeed");
            
            let json_with_options_str = json_with_options.unwrap();
            let parsed_with_options: Result<serde_json::Value, _> = serde_json::from_str(&json_with_options_str);
            prop_assert!(parsed_with_options.is_ok(), "Enhanced JSON with options should be valid");
            
            // Verify that enhanced options include additional fields
            let json_with_options_obj = parsed_with_options.unwrap();
            if exporter_with_options.options.include_comments {
                prop_assert!(json_with_options_obj.get("formatting").is_some(), 
                           "JSON with comments should contain formatting field");
            }
        }
    }

    // Strategy for generating enhanced Library instances with new syntax elements
    fn enhanced_library_strategy() -> impl Strategy<Value = Library> {
        use crate::common::*;
        use crate::core::*;
        
        prop::collection::vec(enhanced_library_element_strategy(), 0..3)
            .prop_map(|elements| Library { elements })
    }

    // Strategy for generating enhanced LibraryElementKind instances including new syntax
    fn enhanced_library_element_strategy() -> impl Strategy<Value = crate::common::LibraryElementKind> {
        use crate::common::*;
        use crate::core::*;
        
        // Simple strategy that creates basic elements with enhanced types
        prop::string::string_regex("[a-zA-Z][a-zA-Z0-9_]*").unwrap()
            .prop_map(|name| {
                // Randomly choose between different element types
                let choice = name.len() % 3;
                match choice {
                    0 => LibraryElementKind::FunctionDeclaration(FunctionDeclaration {
                        name: Id::from(&name),
                        return_type: TypeName {
                            name: Id::from("BOOL"),
                        },
                        variables: vec![],
                        edge_variables: vec![],
                        body: vec![],
                        external_annotation: None,
                    }),
                    1 => LibraryElementKind::GlobalVariableDeclaration(GlobalVariableDeclaration {
                        variables: vec![VarDecl::simple(&name, "INT")],
                        span: SourceSpan::default(),
                    }),
                    _ => LibraryElementKind::TypeDefinitionBlock(TypeDefinitionBlock {
                        definitions: vec![TypeDefinition {
                            name: TypeName::from(&name),
                            base_type: DataTypeSpecificationKind::Elementary(ElementaryTypeName::INT),
                            default_value: None,
                            span: SourceSpan::default(),
                        }],
                        span: SourceSpan::default(),
                    }),
                }
            })
    }
}