//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

/// Normalized array specification, independent of AST representation.
/// Both inline (`ARRAY[1..3, 1..4] OF INT`) and named type paths
/// convert to this form before registration.
#[allow(dead_code)]
pub(crate) struct ArraySpec {
    /// Per-dimension bounds as (lower, upper) inclusive pairs.
    pub dimensions: Vec<(i32, i32)>,
    /// Element type name (e.g., "INT", "DINT").
    pub element_type_name: ironplc_dsl::core::Id,
}

/// Metadata for a single dimension of an array, used for index computation.
#[allow(dead_code)]
pub(crate) struct DimensionInfo {
    pub lower_bound: i32,
    pub size: u32,
    pub stride: u32,
}

/// Metadata for an array variable, stored in CompileContext.
#[allow(dead_code)]
pub(crate) struct ArrayVarInfo {
    pub var_index: u16,
    pub desc_index: u16,
    pub data_offset: u32,
    pub element_var_type_info: super::compile::VarTypeInfo,
    pub total_elements: u32,
    pub dimensions: Vec<DimensionInfo>,
}

/// The resolved target of a variable access.
///
/// This enum decouples variable resolution from code emission.
/// Each dispatch site (compile_expr, compile_statement) matches
/// on the variant and calls the appropriate emission logic.
///
/// Designed for extensibility: when struct access is implemented,
/// add a `StructField` variant and extend `resolve_access()`.
/// The dispatch sites gain a new match arm without changing shape.
#[allow(dead_code)]
pub(crate) enum ResolvedAccess<'a> {
    /// Simple named variable — use LOAD_VAR/STORE_VAR.
    Scalar { var_index: u16 },
    /// Array element — compute flat index, use LOAD_ARRAY/STORE_ARRAY.
    ArrayElement {
        info: &'a ArrayVarInfo,
        subscripts: Vec<&'a ironplc_dsl::textual::Expr>,
    },
}
