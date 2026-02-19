//! IronPLC Sources - Source file handling and parsing
//!
//! This module provides a unified interface for handling different types of
//! source files in the IronPLC compiler, including Structured Text (.st, .iec)
//! and XML (.xml) files.
//!
//! # Architecture
//!
//! The module is organized around several key concepts:
//!
//! - **FileType**: Enum representing different supported file types
//! - **Source**: Abstraction for a single source file with parsing capabilities
//! - **SourceProject**: Collection of source files that can be analyzed together
//! - **Parsers**: Pluggable parsers for different file formats
//!
//! # Example Usage
//!
//! ```rust
//! use ironplc_sources::{SourceProject, Source};
//! use ironplc_dsl::core::FileId;
//!
//! // Create a new project
//! let mut project = SourceProject::new();
//!
//! // Add a Structured Text file
//! let st_content = "PROGRAM Main\nEND_PROGRAM";
//! project.add_source(
//!     FileId::from_string("main.st"),
//!     st_content.to_string()
//! );
//!
//! // Add an XML file
//! let xml_content = "<?xml version=\"1.0\"?><project></project>";
//! project.add_source(
//!     FileId::from_string("config.xml"),
//!     xml_content.to_string()
//! );
//!
//! // Parse all sources
//! for source in project.sources_mut() {
//!     match source.library() {
//!         Ok(library) => println!("Parsed successfully: {} elements", library.elements.len()),
//!         Err(diagnostics) => println!("Parse errors: {:?}", diagnostics),
//!     }
//! }
//! ```

// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

pub mod discovery;
pub mod file_type;
pub mod parsers;
pub mod project;
pub mod source;
pub mod xml;

// Re-export main types for convenience
pub use file_type::FileType;
pub use parsers::parse_source;
pub use project::SourceProject;
pub use source::Source;
