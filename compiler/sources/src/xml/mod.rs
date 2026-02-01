//! PLCopen XML parsing module
//!
//! This module provides parsing support for PLCopen TC6 XML format (IEC 61131-3).
//! It uses roxmltree for XML parsing with accurate position tracking,
//! then transforms the parsed structures to IronPLC's DSL.

pub mod position;
pub mod schema;
pub mod transform;

pub use position::parse_plcopen_xml;
pub use schema::*;
pub use transform::transform_project;
