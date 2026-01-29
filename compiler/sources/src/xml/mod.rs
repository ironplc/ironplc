//! PLCopen XML parsing module
//!
//! This module provides parsing support for PLCopen TC6 XML format (IEC 61131-3).
//! It defines Rust structs that map to the PLCopen XML schema and provides
//! transformation to IronPLC's DSL.

pub mod position;
pub mod schema;
pub mod transform;

pub use position::*;
pub use schema::*;
pub use transform::*;
