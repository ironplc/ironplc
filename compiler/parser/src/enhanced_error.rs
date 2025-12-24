//! Enhanced error reporting system for IronPLC compiler
//! 
//! This module provides comprehensive error reporting with specific error types,
//! exact position reporting, multiple error collection, and suggestion engine
//! for unsupported syntax alternatives.

use dsl::core::{FileId, SourceSpan};
use dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use std::collections::HashMap;

/// Enhanced compiler error types with specific categorization
#[derive(Debug, Clone, PartialEq)]
pub enum CompilerError {
    /// Syntax errors - malformed code that doesn't follow grammar rules
    SyntaxError {
        expected: Vec<String>,
        found: String,
        location: SourceSpan,
        suggestion: Option<String>,
    },
    
    /// Unsupported feature errors - valid syntax but not yet implemented
    UnsupportedFeature {
        feature: String,
        location: SourceSpan,
        suggestion: Option<String>,
        workaround: Option<String>,
    },
    
    /// Type-related errors
    TypeMismatch {
        expected: String,
        found: String,
        location: SourceSpan,
        context: String,
    },
    
    /// Undefined reference errors
    UndefinedReference {
        name: String,
        location: SourceSpan,
        available_names: Vec<String>,
    },
    
    /// Invalid member access errors
    InvalidMemberAccess {
        type_name: String,
        member: String,
        location: SourceSpan,
        available_members: Vec<String>,
    },
    
    /// Array bounds errors
    ArrayBoundsError {
        index: String,
        bounds: String,
        location: SourceSpan,
    },
    
    /// String length errors
    StringLengthError {
        declared_length: u32,
        actual_length: usize,
        location: SourceSpan,
    },
    
    /// Comment parsing errors
    CommentError {
        error_type: CommentErrorType,
        location: SourceSpan,
    },
    
    /// Timer-related errors
    TimerError {
        error_type: TimerErrorType,
        location: SourceSpan,
    },
    
    /// CASE statement errors
    CaseError {
        error_type: CaseErrorType,
        location: SourceSpan,
    },
    
    /// VAR_GLOBAL declaration errors
    GlobalVarError {
        error_type: GlobalVarErrorType,
        location: SourceSpan,
    },
    
    /// TYPE...END_TYPE block errors
    TypeDefinitionError {
        error_type: TypeDefinitionErrorType,
        location: SourceSpan,
    },
    
    /// Enumeration type errors
    EnumerationError {
        error_type: EnumerationErrorType,
        location: SourceSpan,
    },
    
    /// Array type errors
    ArrayTypeError {
        error_type: ArrayTypeErrorType,
        location: SourceSpan,
    },
    
    /// Subrange type errors
    SubrangeError {
        error_type: SubrangeErrorType,
        location: SourceSpan,
    },
}

/// Specific comment error types
#[derive(Debug, Clone, PartialEq)]
pub enum CommentErrorType {
    UnterminatedComment,
    InvalidNesting,
    MalformedDelimiter,
}

/// Specific timer error types
#[derive(Debug, Clone, PartialEq)]
pub enum TimerErrorType {
    InvalidTimeLiteral,
    InvalidTimerType,
    MissingParameter,
    InvalidParameterType,
}

/// Specific CASE statement error types
#[derive(Debug, Clone, PartialEq)]
pub enum CaseErrorType {
    InvalidSelector,
    DuplicateLabel,
    MissingEndCase,
    InvalidLabel,
}

/// Specific VAR_GLOBAL error types
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalVarErrorType {
    MissingEndVar,
    InvalidVariableDeclaration,
    DuplicateVariableName,
    InvalidInitializer,
    MalformedVarGlobal,
}

/// Specific TYPE...END_TYPE error types
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDefinitionErrorType {
    MissingEndType,
    InvalidTypeDefinition,
    DuplicateTypeName,
    InvalidBaseType,
    CircularReference,
    MalformedTypeBlock,
}

/// Specific enumeration error types
#[derive(Debug, Clone, PartialEq)]
pub enum EnumerationErrorType {
    MissingClosingParen,
    EmptyEnumeration,
    DuplicateEnumValue,
    InvalidEnumValue,
    MalformedEnumeration,
}

/// Specific array type error types
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayTypeErrorType {
    InvalidBounds,
    MissingElementType,
    InvalidDimensions,
    BoundsOrderError,
    MalformedArrayDeclaration,
}

/// Specific subrange error types
#[derive(Debug, Clone, PartialEq)]
pub enum SubrangeErrorType {
    InvalidRange,
    MinGreaterThanMax,
    InvalidBaseType,
    OutOfBounds,
    MalformedSubrange,
}

/// Enhanced error collector that can gather multiple errors in a single pass
#[derive(Debug, Default)]
pub struct ErrorCollector {
    errors: Vec<CompilerError>,
    warnings: Vec<CompilerError>,
    file_id: Option<FileId>,
}

impl ErrorCollector {
    /// Create a new error collector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a new error collector with a specific file ID
    pub fn with_file_id(file_id: FileId) -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            file_id: Some(file_id),
        }
    }
    
    /// Add an error to the collection
    pub fn add_error(&mut self, error: CompilerError) {
        self.errors.push(error);
    }
    
    /// Add a warning to the collection
    pub fn add_warning(&mut self, warning: CompilerError) {
        self.warnings.push(warning);
    }
    
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
    
    /// Get all errors
    pub fn errors(&self) -> &[CompilerError] {
        &self.errors
    }
    
    /// Get all warnings
    pub fn warnings(&self) -> &[CompilerError] {
        &self.warnings
    }
    
    /// Convert collected errors to diagnostics
    pub fn to_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        for error in &self.errors {
            diagnostics.push(error.to_diagnostic());
        }
        
        for warning in &self.warnings {
            diagnostics.push(warning.to_diagnostic());
        }
        
        diagnostics
    }
    
    /// Clear all collected errors and warnings
    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }
}

/// Suggestion engine for providing helpful alternatives to unsupported syntax
pub struct SuggestionEngine {
    feature_suggestions: HashMap<String, String>,
    workarounds: HashMap<String, String>,
}

impl SuggestionEngine {
    /// Create a new suggestion engine with default suggestions
    pub fn new() -> Self {
        let mut engine = Self {
            feature_suggestions: HashMap::new(),
            workarounds: HashMap::new(),
        };
        
        engine.initialize_default_suggestions();
        engine
    }
    
    /// Initialize default suggestions for common unsupported features
    fn initialize_default_suggestions(&mut self) {
        // VAR_GLOBAL suggestions
        self.feature_suggestions.insert(
            "VAR_GLOBAL".to_string(),
            "Use VAR_GLOBAL...END_VAR syntax for global variable declarations".to_string(),
        );
        
        // TYPE...END_TYPE suggestions
        self.feature_suggestions.insert(
            "TYPE".to_string(),
            "Use TYPE...END_TYPE syntax for custom type definitions".to_string(),
        );
        
        // Enumeration suggestions
        self.feature_suggestions.insert(
            "ENUMERATION".to_string(),
            "Use (value1, value2, value3) syntax for enumeration types".to_string(),
        );
        
        // STRUCT suggestions
        self.feature_suggestions.insert(
            "STRUCT".to_string(),
            "Use TYPE...END_TYPE with STRUCT...END_STRUCT syntax".to_string(),
        );
        
        // ARRAY suggestions
        self.feature_suggestions.insert(
            "ARRAY".to_string(),
            "Use ARRAY[bounds] OF type syntax".to_string(),
        );
        
        // Subrange suggestions
        self.feature_suggestions.insert(
            "SUBRANGE".to_string(),
            "Use INT(min..max) syntax for subrange types".to_string(),
        );
        
        // STRING(n) suggestions
        self.feature_suggestions.insert(
            "STRING(n)".to_string(),
            "Use STRING(length) syntax for fixed-length strings".to_string(),
        );
        
        // Timer suggestions
        self.feature_suggestions.insert(
            "TON".to_string(),
            "Use TON timer with time literals like T#5S".to_string(),
        );
        
        // CASE suggestions
        self.feature_suggestions.insert(
            "CASE".to_string(),
            "Use CASE...OF...END_CASE syntax with labeled cases".to_string(),
        );
        
        // Workarounds for legacy compatibility
        self.workarounds.insert(
            "VAR_GLOBAL".to_string(),
            "Consider using external variables or function block parameters".to_string(),
        );
        
        self.workarounds.insert(
            "TYPE".to_string(),
            "Consider using elementary types directly or function block approach".to_string(),
        );
        
        self.workarounds.insert(
            "STRUCT".to_string(),
            "Consider using separate variables or function block approach".to_string(),
        );
        
        self.workarounds.insert(
            "ARRAY".to_string(),
            "Consider using individual variables with numeric suffixes".to_string(),
        );
        
        self.workarounds.insert(
            "SUBRANGE".to_string(),
            "Consider using elementary integer types with validation logic".to_string(),
        );
    }
    
    /// Get suggestion for a specific feature
    pub fn get_suggestion(&self, feature: &str) -> Option<&String> {
        self.feature_suggestions.get(feature)
    }
    
    /// Get workaround for a specific feature
    pub fn get_workaround(&self, feature: &str) -> Option<&String> {
        self.workarounds.get(feature)
    }
    
    /// Add a custom suggestion
    pub fn add_suggestion(&mut self, feature: String, suggestion: String) {
        self.feature_suggestions.insert(feature, suggestion);
    }
    
    /// Add a custom workaround
    pub fn add_workaround(&mut self, feature: String, workaround: String) {
        self.workarounds.insert(feature, workaround);
    }
}

impl Default for SuggestionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerError {
    /// Convert a CompilerError to a Diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            CompilerError::SyntaxError { expected, found, location, suggestion } => {
                let mut diagnostic = Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(
                        location.clone(),
                        format!(
                            "Expected {}. Found '{}'",
                            expected.join(" | "),
                            found
                        ),
                    ),
                );
                
                if let Some(suggestion) = suggestion {
                    diagnostic = diagnostic.with_context("Suggestion", suggestion);
                }
                
                diagnostic
            },
            
            CompilerError::UnsupportedFeature { feature, location, suggestion, workaround } => {
                let mut diagnostic = Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(
                        location.clone(),
                        format!("Unsupported feature: {}", feature),
                    ),
                );
                
                if let Some(suggestion) = suggestion {
                    diagnostic = diagnostic.with_context("Suggestion", suggestion);
                }
                
                if let Some(workaround) = workaround {
                    diagnostic = diagnostic.with_context("Workaround", workaround);
                }
                
                diagnostic
            },
            
            CompilerError::TypeMismatch { expected, found, location, context } => {
                Diagnostic::problem(
                    Problem::TypeMismatchError,
                    Label::span(
                        location.clone(),
                        format!(
                            "Type mismatch in {}: expected '{}', found '{}'",
                            context, expected, found
                        ),
                    ),
                )
            },
            
            CompilerError::UndefinedReference { name, location, available_names } => {
                let mut diagnostic = Diagnostic::problem(
                    Problem::VariableUndefined,
                    Label::span(
                        location.clone(),
                        format!("Undefined reference: '{}'", name),
                    ),
                );
                
                if !available_names.is_empty() {
                    let suggestions = available_names.join(", ");
                    diagnostic = diagnostic.with_context("Available names", &suggestions);
                }
                
                diagnostic
            },
            
            CompilerError::InvalidMemberAccess { type_name, member, location, available_members } => {
                let mut diagnostic = Diagnostic::problem(
                    Problem::ClassMemberNotFound,
                    Label::span(
                        location.clone(),
                        format!("Invalid member access: '{}' has no member '{}'", type_name, member),
                    ),
                );
                
                if !available_members.is_empty() {
                    let suggestions = available_members.join(", ");
                    diagnostic = diagnostic.with_context("Available members", &suggestions);
                }
                
                diagnostic
            },
            
            CompilerError::ArrayBoundsError { index, bounds, location } => {
                Diagnostic::problem(
                    Problem::RuntimeArrayBoundsCheck,
                    Label::span(
                        location.clone(),
                        format!("Array index '{}' is out of bounds '{}'", index, bounds),
                    ),
                )
            },
            
            CompilerError::StringLengthError { declared_length, actual_length, location } => {
                Diagnostic::problem(
                    Problem::TypeMismatchError,
                    Label::span(
                        location.clone(),
                        format!(
                            "String length mismatch: declared length {}, actual length {}",
                            declared_length, actual_length
                        ),
                    ),
                )
            },
            
            CompilerError::CommentError { error_type, location } => {
                let message = match error_type {
                    CommentErrorType::UnterminatedComment => "Unterminated comment block",
                    CommentErrorType::InvalidNesting => "Invalid comment nesting",
                    CommentErrorType::MalformedDelimiter => "Malformed comment delimiter",
                };
                
                Diagnostic::problem(
                    Problem::OpenComment,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::TimerError { error_type, location } => {
                let message = match error_type {
                    TimerErrorType::InvalidTimeLiteral => "Invalid time literal format",
                    TimerErrorType::InvalidTimerType => "Invalid timer type",
                    TimerErrorType::MissingParameter => "Missing required timer parameter",
                    TimerErrorType::InvalidParameterType => "Invalid timer parameter type",
                };
                
                Diagnostic::problem(
                    Problem::TypeMismatchError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::CaseError { error_type, location } => {
                let message = match error_type {
                    CaseErrorType::InvalidSelector => "Invalid CASE selector expression",
                    CaseErrorType::DuplicateLabel => "Duplicate CASE label",
                    CaseErrorType::MissingEndCase => "Missing END_CASE",
                    CaseErrorType::InvalidLabel => "Invalid CASE label",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::GlobalVarError { error_type, location } => {
                let message = match error_type {
                    GlobalVarErrorType::MissingEndVar => "Missing END_VAR for VAR_GLOBAL block",
                    GlobalVarErrorType::InvalidVariableDeclaration => "Invalid variable declaration in VAR_GLOBAL block",
                    GlobalVarErrorType::DuplicateVariableName => "Duplicate variable name in VAR_GLOBAL block",
                    GlobalVarErrorType::InvalidInitializer => "Invalid initializer in global variable declaration",
                    GlobalVarErrorType::MalformedVarGlobal => "Malformed VAR_GLOBAL block syntax",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::TypeDefinitionError { error_type, location } => {
                let message = match error_type {
                    TypeDefinitionErrorType::MissingEndType => "Missing END_TYPE for TYPE block",
                    TypeDefinitionErrorType::InvalidTypeDefinition => "Invalid type definition syntax",
                    TypeDefinitionErrorType::DuplicateTypeName => "Duplicate type name in TYPE block",
                    TypeDefinitionErrorType::InvalidBaseType => "Invalid base type in type definition",
                    TypeDefinitionErrorType::CircularReference => "Circular reference in type definition",
                    TypeDefinitionErrorType::MalformedTypeBlock => "Malformed TYPE block syntax",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::EnumerationError { error_type, location } => {
                let message = match error_type {
                    EnumerationErrorType::MissingClosingParen => "Missing closing parenthesis in enumeration",
                    EnumerationErrorType::EmptyEnumeration => "Empty enumeration not allowed",
                    EnumerationErrorType::DuplicateEnumValue => "Duplicate value in enumeration",
                    EnumerationErrorType::InvalidEnumValue => "Invalid enumeration value",
                    EnumerationErrorType::MalformedEnumeration => "Malformed enumeration syntax",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::ArrayTypeError { error_type, location } => {
                let message = match error_type {
                    ArrayTypeErrorType::InvalidBounds => "Invalid array bounds specification",
                    ArrayTypeErrorType::MissingElementType => "Missing element type in array declaration",
                    ArrayTypeErrorType::InvalidDimensions => "Invalid array dimensions",
                    ArrayTypeErrorType::BoundsOrderError => "Array lower bound must be less than or equal to upper bound",
                    ArrayTypeErrorType::MalformedArrayDeclaration => "Malformed array declaration syntax",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
            
            CompilerError::SubrangeError { error_type, location } => {
                let message = match error_type {
                    SubrangeErrorType::InvalidRange => "Invalid subrange specification",
                    SubrangeErrorType::MinGreaterThanMax => "Subrange minimum value must be less than or equal to maximum",
                    SubrangeErrorType::InvalidBaseType => "Invalid base type for subrange",
                    SubrangeErrorType::OutOfBounds => "Subrange bounds exceed base type limits",
                    SubrangeErrorType::MalformedSubrange => "Malformed subrange syntax",
                };
                
                Diagnostic::problem(
                    Problem::SyntaxError,
                    Label::span(location.clone(), message),
                )
            },
        }
    }
    
    /// Get the location of this error
    pub fn location(&self) -> &SourceSpan {
        match self {
            CompilerError::SyntaxError { location, .. } => location,
            CompilerError::UnsupportedFeature { location, .. } => location,
            CompilerError::TypeMismatch { location, .. } => location,
            CompilerError::UndefinedReference { location, .. } => location,
            CompilerError::InvalidMemberAccess { location, .. } => location,
            CompilerError::ArrayBoundsError { location, .. } => location,
            CompilerError::StringLengthError { location, .. } => location,
            CompilerError::CommentError { location, .. } => location,
            CompilerError::TimerError { location, .. } => location,
            CompilerError::CaseError { location, .. } => location,
            CompilerError::GlobalVarError { location, .. } => location,
            CompilerError::TypeDefinitionError { location, .. } => location,
            CompilerError::EnumerationError { location, .. } => location,
            CompilerError::ArrayTypeError { location, .. } => location,
            CompilerError::SubrangeError { location, .. } => location,
        }
    }
    
    /// Check if this is a syntax error
    pub fn is_syntax_error(&self) -> bool {
        matches!(self, CompilerError::SyntaxError { .. })
    }
    
    /// Check if this is an unsupported feature error
    pub fn is_unsupported_feature(&self) -> bool {
        matches!(self, CompilerError::UnsupportedFeature { .. })
    }
}

/// Helper functions for creating common error types
impl CompilerError {
    /// Create a syntax error with suggestions
    pub fn syntax_error(
        expected: Vec<String>,
        found: String,
        location: SourceSpan,
        suggestion: Option<String>,
    ) -> Self {
        CompilerError::SyntaxError {
            expected,
            found,
            location,
            suggestion,
        }
    }
    
    /// Create an unsupported feature error with suggestions and workarounds
    pub fn unsupported_feature(
        feature: String,
        location: SourceSpan,
        suggestion: Option<String>,
        workaround: Option<String>,
    ) -> Self {
        CompilerError::UnsupportedFeature {
            feature,
            location,
            suggestion,
            workaround,
        }
    }
    
    /// Create a type mismatch error
    pub fn type_mismatch(
        expected: String,
        found: String,
        location: SourceSpan,
        context: String,
    ) -> Self {
        CompilerError::TypeMismatch {
            expected,
            found,
            location,
            context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl::core::FileId;

    #[test]
    fn test_error_collector_basic_functionality() {
        let mut collector = ErrorCollector::new();
        
        assert!(!collector.has_errors());
        assert!(!collector.has_warnings());
        
        let error = CompilerError::syntax_error(
            vec!["STRUCT".to_string()],
            "INVALID".to_string(),
            SourceSpan::range(0, 5).with_file_id(&FileId::default()),
            None,
        );
        
        collector.add_error(error);
        
        assert!(collector.has_errors());
        assert_eq!(collector.errors().len(), 1);
    }
    
    #[test]
    fn test_suggestion_engine() {
        let engine = SuggestionEngine::new();
        
        assert!(engine.get_suggestion("STRUCT").is_some());
        assert!(engine.get_suggestion("ARRAY").is_some());
        assert!(engine.get_suggestion("NONEXISTENT").is_none());
    }
    
    #[test]
    fn test_compiler_error_to_diagnostic() {
        let error = CompilerError::syntax_error(
            vec!["STRUCT".to_string()],
            "INVALID".to_string(),
            SourceSpan::range(0, 5).with_file_id(&FileId::default()),
            Some("Use TYPE...END_TYPE syntax".to_string()),
        );
        
        let diagnostic = error.to_diagnostic();
        assert_eq!(diagnostic.code, "P0002"); // SyntaxError code
    }
    
    #[test]
    fn test_error_categorization() {
        let syntax_error = CompilerError::syntax_error(
            vec!["END".to_string()],
            "INVALID".to_string(),
            SourceSpan::range(0, 5).with_file_id(&FileId::default()),
            None,
        );
        
        let unsupported_error = CompilerError::unsupported_feature(
            "STRUCT".to_string(),
            SourceSpan::range(0, 5).with_file_id(&FileId::default()),
            None,
            None,
        );
        
        assert!(syntax_error.is_syntax_error());
        assert!(!syntax_error.is_unsupported_feature());
        
        assert!(!unsupported_error.is_syntax_error());
        assert!(unsupported_error.is_unsupported_feature());
    }
}