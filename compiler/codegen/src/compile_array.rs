//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_dsl::common::{ArrayInitialElementKind, ConstantKind};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, SymbolicVariableKind, UnaryOp, Variable};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::{ArrayDimension, ByteSized, IntermediateType};
use ironplc_container::{CharWidth, ContainerBuilder, SlotIndex, VarIndex};

use super::compile::{CompileContext, OpType, OpWidth, Signedness, VarTypeInfo};
use super::compile_expr::compile_expr;
use crate::emit::Emitter;

/// Normalized array specification, independent of AST representation.
/// Both inline (`ARRAY[1..3, 1..4] OF INT`) and named type paths
/// convert to this form before registration.
pub(crate) struct ArraySpec {
    /// Per-dimension bounds as (lower, upper) inclusive pairs.
    pub dimensions: Vec<(i32, i32)>,
    /// Element type name (e.g., "INT", "DINT", "STRING").
    pub element_type_name: Id,
    /// Whether each element is a REF_TO the named type.
    pub ref_to: bool,
    /// For STRING/WSTRING arrays, the maximum string length per element
    /// (in code units). `None` for non-string element types.
    pub string_max_len: Option<u16>,
    /// For STRING/WSTRING arrays, the per-code-unit byte width:
    /// `Narrow` for STRING, `Wide` for WSTRING. `None` for non-string
    /// element types.
    pub string_char_width: Option<CharWidth>,
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
    pub var_index: VarIndex,
    pub desc_index: u16,
    pub data_offset: u32,
    pub element_var_type_info: VarTypeInfo,
    pub total_elements: u32,
    pub dimensions: Vec<DimensionInfo>,
    /// True when the array element type is STRING.
    pub is_string_element: bool,
    /// For STRING arrays, the max string length per element.
    pub string_max_len: u16,
    /// For STRING/WSTRING arrays, the per-code-unit byte width of each element.
    pub string_char_width: CharWidth,
    /// True when this entry describes a `REF_TO ARRAY` parameter rather than a
    /// real array. Such a slot holds the target's variable index, not a
    /// data-region offset, and no region is allocated for it -- so it must
    /// never be used as the source or destination of a whole-value copy.
    pub is_ref: bool,
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
pub(crate) enum ResolvedAccess<'ctx, 'ast> {
    /// Simple named variable — use LOAD_VAR/STORE_VAR.
    Scalar { var_index: VarIndex },
    /// `VAR_IN_OUT` parameter of the function being compiled. Its slot holds
    /// a reference to the caller's variable: load the reference with
    /// LOAD_VAR_I64, then use LOAD_INDIRECT/STORE_INDIRECT.
    InOut { ref_slot: VarIndex },
    /// Array element — compute flat index, use LOAD_ARRAY/STORE_ARRAY.
    ArrayElement {
        info: &'ctx ArrayVarInfo,
        subscripts: Vec<&'ast Expr>,
    },
    /// Array element through a dereferenced reference — use LOAD_ARRAY_DEREF/STORE_ARRAY_DEREF.
    DerefArrayElement {
        info: &'ctx ArrayVarInfo,
        subscripts: Vec<&'ast Expr>,
    },
    /// Array element within a struct field — compute flat index + struct field offset,
    /// then use the struct's LOAD_ARRAY/STORE_ARRAY descriptor.
    StructFieldArrayElement {
        /// Struct variable table index.
        var_index: VarIndex,
        /// Struct array descriptor index (treats struct as flat slot array).
        desc_index: u16,
        /// Compile-time slot offset of the array field within the struct.
        field_slot_offset: SlotIndex,
        /// Dimension info for computing the flat index from subscripts.
        dimensions: Vec<DimensionInfo>,
        /// Subscript expressions.
        subscripts: Vec<&'ast Expr>,
        /// Element op type for compile_expr width.
        element_op_type: OpType,
        /// Element intermediate type for truncation on store.
        element_type: IntermediateType,
    },
    /// STRING array element within a struct field — see [`StructStringElement`].
    StructFieldStringArrayElement(StructStringElement<'ast>),
}

/// A STRING element of an array that lives inside a structure's data region.
///
/// `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM` address an element as
/// `base + flat_index * stride`, reading `base` from a variable. The
/// structure's variable holds the start of the whole structure rather than of
/// the array, so the array's start is first computed into a scratch variable.
pub(crate) struct StructStringElement<'ast> {
    /// Struct variable table index (holds struct data_offset).
    pub var_index: VarIndex,
    /// Scratch variable for the adjusted base offset.
    pub scratch_var_index: VarIndex,
    /// STRING array descriptor index (element_extra = max_str_len).
    pub string_desc_index: u16,
    /// Byte offset of the array field within the struct (slot_offset * 8).
    pub field_byte_offset: u32,
    /// Dimension info for computing the flat index from subscripts.
    pub dimensions: Vec<DimensionInfo>,
    /// Subscript expressions.
    pub subscripts: Vec<&'ast Expr>,
}

impl StructStringElement<'_> {
    /// Emits what `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM` need
    /// before they run: stores `struct_data_offset + field_byte_offset` into
    /// the scratch variable, then pushes the flat element index. The caller
    /// follows with either opcode, passing `scratch_var_index` and
    /// `string_desc_index`.
    pub(crate) fn emit_base_and_index(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        emitter.emit_load_var_i32(self.var_index);
        let offset_const = ctx.add_i32_constant(self.field_byte_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_add_i32();
        emitter.emit_store_var_i32(self.scratch_var_index);
        emit_flat_index(emitter, ctx, &self.subscripts, &self.dimensions, span)
    }
}

/// Resolves a variable reference into its access kind.
///
/// For named variables, returns Scalar with the variable table index.
/// For array variables, walks the ArrayVariable chain to collect
/// all subscripts and resolve the base variable's ArrayVarInfo.
///
/// Two lifetimes separate the context borrow (`'ctx` for `info`) from the
/// AST borrow (`'ast` for `subscripts`). This allows callers to drop `info`
/// and then use `ctx` mutably while still holding `subscripts`.
pub(crate) fn resolve_access<'ctx, 'ast>(
    ctx: &'ctx CompileContext,
    variable: &'ast Variable,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Array(array_var)) => {
            // Walk the chain collecting subscript groups innermost-first,
            // then reverse. For nested arrays arr[i][j], the AST is:
            //   ArrayVariable {
            //       subscripted_variable: Array(ArrayVariable {
            //           subscripted_variable: Named(Id("arr")),
            //           subscripts: [i],
            //       }),
            //       subscripts: [j],
            //   }
            // We collect: [[j], [i]], reverse to [[i], [j]], flatten to [i, j].
            let mut levels: Vec<&[Expr]> = Vec::new();
            let mut current = array_var;
            loop {
                levels.push(&current.subscripts);
                match current.subscripted_variable.as_ref() {
                    SymbolicVariableKind::Array(inner) => {
                        current = inner;
                    }
                    SymbolicVariableKind::Named(named) => {
                        levels.reverse();
                        let all_subscripts: Vec<&Expr> = levels.into_iter().flatten().collect();
                        let info = ctx.array_vars.get(&named.name).ok_or_else(|| {
                            // An element of an array of structures spans
                            // several slots, so it has no single-value load or
                            // store; only its fields are addressable.
                            if ctx.struct_array_vars.contains_key(&named.name) {
                                Diagnostic::not_implemented(Label::span(
                                    named.name.span(),
                                    format!(
                                        "Whole-element access of '{}' -- select a field of the element instead",
                                        named.name
                                    ),
                                ))
                            } else {
                                Diagnostic::todo_with_span(named.name.span())
                            }
                        })?;
                        return Ok(ResolvedAccess::ArrayElement {
                            info,
                            subscripts: all_subscripts,
                        });
                    }
                    SymbolicVariableKind::Deref(deref) => {
                        // Dereference in array chain (e.g., PT^[0] where PT is REF_TO ARRAY).
                        // Walk through the deref to find the base variable name.
                        let mut inner = deref.variable.as_ref();
                        while let SymbolicVariableKind::Deref(d) = inner {
                            inner = d.variable.as_ref();
                        }
                        match inner {
                            SymbolicVariableKind::Named(named) => {
                                levels.reverse();
                                let all_subscripts: Vec<&Expr> =
                                    levels.into_iter().flatten().collect();
                                let info = ctx
                                    .array_vars
                                    .get(&named.name)
                                    .ok_or_else(|| Diagnostic::todo_with_span(named.name.span()))?;
                                return Ok(ResolvedAccess::DerefArrayElement {
                                    info,
                                    subscripts: all_subscripts,
                                });
                            }
                            other => {
                                return Err(Diagnostic::todo_with_span(other.span()));
                            }
                        }
                    }
                    SymbolicVariableKind::Structured(structured) => {
                        levels.reverse();
                        let all_subscripts: Vec<&Expr> = levels.into_iter().flatten().collect();
                        // `a[i].values[j]` -- the record is itself an array
                        // element, so the subscripts collected here index the
                        // *field*, and the element index is still inside the
                        // record. The array-of-struct path combines the two.
                        if matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) {
                            return crate::compile_array_struct::resolve_struct_array_element_field(
                                ctx,
                                structured,
                                all_subscripts,
                            );
                        }
                        return resolve_struct_field_array(ctx, structured, all_subscripts);
                    }
                    other => {
                        return Err(Diagnostic::todo_with_span(other.span()));
                    }
                }
            }
        }
        // `s.arr[i].field` -- a field selected from an element of an
        // array-of-struct. The record is an array element rather than a
        // fixed-offset struct field, so it resolves through the array path.
        Variable::Symbolic(SymbolicVariableKind::Structured(structured))
            if matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) =>
        {
            crate::compile_array_struct::resolve_struct_array_element_field(
                ctx,
                structured,
                Vec::new(),
            )
        }
        _ => {
            if let Some(ref_slot) = super::compile_expr::in_out_ref_slot(ctx, variable) {
                return Ok(ResolvedAccess::InOut { ref_slot });
            }
            // Fall through to existing resolve_variable() for scalars.
            let var_index = super::compile_expr::resolve_variable(ctx, variable)?;
            Ok(ResolvedAccess::Scalar { var_index })
        }
    }
}

/// Resolves an array subscript whose base is a struct field.
///
/// For `math.FACTS[x]`, the struct field `FACTS` is an array. We resolve the
/// struct chain to get the field's slot offset and type, extract array dimension
/// info, and return a `StructFieldArrayElement` that the caller uses to emit
/// `flat_index + slot_offset` followed by the struct's LOAD_ARRAY/STORE_ARRAY.
pub(crate) fn resolve_struct_field_array<'ctx, 'ast>(
    ctx: &'ctx CompileContext,
    structured: &ironplc_dsl::textual::StructuredVariable,
    subscripts: Vec<&'ast Expr>,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
    let (root_name, slot_offset, field_type) =
        crate::compile_struct::walk_struct_chain(ctx, &structured.record, &structured.field, 0)?;

    let IntermediateType::Array {
        element_type,
        dimensions: array_dims,
    } = &field_type
    else {
        return Err(Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            format!("Field '{}' is not an array type", structured.field),
        )));
    };

    let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            structured.span(),
            format!("Variable '{}' is not a structure", root_name),
        ))
    })?;

    // STRING array fields use dedicated STR_LOAD/STORE_ARRAY_ELEM opcodes
    // with a scratch variable and a STRING-specific array descriptor.
    if let IntermediateType::String { .. } = element_type.as_ref() {
        let field_name = structured.field.to_string().to_lowercase();
        let &(str_desc_index, _, _) =
            struct_info
                .string_array_descs
                .get(&field_name)
                .ok_or_else(|| {
                    Diagnostic::not_implemented(Label::span(
                        structured.field.span(),
                        "STRING array descriptor not registered for field",
                    ))
                })?;
        let scratch = struct_info.scratch_var_index.ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                structured.field.span(),
                "Scratch variable not allocated for struct",
            ))
        })?;
        let dimensions = dimensions_from_intermediate(array_dims);
        let field_byte_offset = slot_offset.raw() * 8;
        return Ok(ResolvedAccess::StructFieldStringArrayElement(
            StructStringElement {
                var_index: struct_info.var_index,
                scratch_var_index: scratch,
                string_desc_index: str_desc_index,
                field_byte_offset,
                dimensions,
                subscripts,
            },
        ));
    }

    let element_op_type =
        crate::compile_struct::resolve_field_op_type(element_type).ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                    structured.field.span(),
                    "Array element type is not a primitive (nested struct/array elements not supported)",
                ),
            )
        })?;

    let dimensions = dimensions_from_intermediate(array_dims);

    Ok(ResolvedAccess::StructFieldArrayElement {
        var_index: struct_info.var_index,
        desc_index: struct_info.desc_index,
        field_slot_offset: slot_offset,
        dimensions,
        subscripts,
        element_op_type,
        element_type: element_type.as_ref().clone(),
    })
}

/// Converts `ArrayDimension` bounds into `DimensionInfo` with computed strides.
///
/// Strides follow row-major order: the last dimension has stride 1, each
/// preceding dimension's stride is the product of all subsequent dimension sizes.
pub(crate) fn dimensions_from_intermediate(dims: &[ArrayDimension]) -> Vec<DimensionInfo> {
    let sizes: Vec<u32> = dims
        .iter()
        .map(|d| (d.upper as i64 - d.lower as i64 + 1).max(0) as u32)
        .collect();

    let mut strides = vec![1u32; sizes.len()];
    for i in (0..sizes.len().saturating_sub(1)).rev() {
        strides[i] = strides[i + 1].saturating_mul(sizes[i + 1]);
    }

    dims.iter()
        .zip(sizes.iter().zip(strides.iter()))
        .map(|(d, (&size, &stride))| DimensionInfo {
            lower_bound: d.lower,
            size,
            stride,
        })
        .collect()
}

/// Converts an inline array specification (from the AST) to a normalized ArraySpec.
pub(crate) fn array_spec_from_inline(
    subranges: &ironplc_dsl::common::ArraySubranges,
    _span: &ironplc_dsl::core::SourceSpan,
) -> Result<ArraySpec, Diagnostic> {
    let dimensions: Vec<(i32, i32)> = subranges
        .ranges
        .iter()
        .map(|range| {
            let lower = super::compile_stmt::signed_integer_to_i32(
                range.start.as_signed_integer().unwrap(),
            )?;
            let upper =
                super::compile_stmt::signed_integer_to_i32(range.end.as_signed_integer().unwrap())?;
            Ok((lower, upper))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let (string_max_len, string_char_width) = match &subranges.type_name {
        ironplc_dsl::common::ArrayElementType::String(spec) => {
            let len = spec
                .length
                .as_ref()
                .and_then(|l| l.as_integer().map(|i| i.value as u16))
                .unwrap_or(super::compile::DEFAULT_STRING_MAX_LENGTH);
            (Some(len), Some(CharWidth::Narrow))
        }
        ironplc_dsl::common::ArrayElementType::WString(spec) => {
            let len = spec
                .length
                .as_ref()
                .and_then(|l| l.as_integer().map(|i| i.value as u16))
                .unwrap_or(super::compile::DEFAULT_STRING_MAX_LENGTH);
            (Some(len), Some(CharWidth::Wide))
        }
        _ => (None, None),
    };
    Ok(ArraySpec {
        dimensions,
        element_type_name: Id::from(&subranges.type_name.to_type_name().to_string()),
        ref_to: subranges.ref_to.is_some(),
        string_max_len,
        string_char_width,
    })
}

/// Normalizes an array variable declaration -- inline or by named type -- into
/// an [`ArraySpec`].
///
/// Element types that are structures are laid out differently and register
/// through `compile_array_struct` instead; callers route those away first.
pub(crate) fn array_spec_for_declaration(
    types: &ironplc_analyzer::TypeEnvironment,
    spec: &ironplc_dsl::common::SpecificationKind<ironplc_dsl::common::ArraySubranges>,
    span: &SourceSpan,
) -> Result<ArraySpec, Diagnostic> {
    match spec {
        ironplc_dsl::common::SpecificationKind::Inline(subranges) => {
            array_spec_from_inline(subranges, span)
        }
        ironplc_dsl::common::SpecificationKind::Named(type_name) => {
            // The caller reaches this arm only for a declaration the type
            // environment already resolved to an array, so a miss here is a
            // compiler invariant rather than anything the program did.
            let array_type = types.resolve_array_type(type_name).ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    type_name.span(),
                    "Array type is absent from the type environment",
                ))
            })?;
            let IntermediateType::Array {
                element_type,
                dimensions,
            } = array_type
            else {
                // `resolve_array_type` returns only the Array variant, so this
                // is a compiler invariant rather than anything the program did.
                return Err(Diagnostic::internal_error_at(Label::span(
                    type_name.span(),
                    "Array type resolved to a non-array representation",
                )));
            };
            array_spec_from_named(element_type, dimensions, span)
        }
    }
}

/// Converts a named array type (from the TypeEnvironment) to a normalized ArraySpec.
///
/// `span` locates the declaration being compiled; the intermediate type has
/// no span of its own.
pub(crate) fn array_spec_from_named(
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
    span: &SourceSpan,
) -> Result<ArraySpec, Diagnostic> {
    let dims: Vec<(i32, i32)> = dimensions.iter().map(|d| (d.lower, d.upper)).collect();
    let ref_to = matches!(element_type, IntermediateType::Reference { .. });
    let inner_type = if let IntermediateType::Reference { target_type } = element_type {
        target_type.as_ref()
    } else {
        element_type
    };
    let element_type_name = intermediate_type_to_name(inner_type, span)?;
    let (string_max_len, string_char_width) = match inner_type {
        IntermediateType::String {
            max_len,
            char_width,
        } => {
            let len = max_len
                .map(|v| v as u16)
                .unwrap_or(super::compile::DEFAULT_STRING_MAX_LENGTH);
            (Some(len), Some(*char_width))
        }
        _ => (None, None),
    };
    Ok(ArraySpec {
        dimensions: dims,
        element_type_name,
        ref_to,
        string_max_len,
        string_char_width,
    })
}

/// Maps an IntermediateType to the IEC 61131-3 type name (as an Id) that
/// `type_info::resolve_type_name()` can look up. Only primitive types are
/// supported (arrays of complex types are out of scope).
fn intermediate_type_to_name(ty: &IntermediateType, span: &SourceSpan) -> Result<Id, Diagnostic> {
    let name = match ty {
        IntermediateType::Bool => "BOOL",
        IntermediateType::Int {
            size: ByteSized::B8,
        } => "SINT",
        IntermediateType::Int {
            size: ByteSized::B16,
        } => "INT",
        IntermediateType::Int {
            size: ByteSized::B32,
        } => "DINT",
        IntermediateType::Int {
            size: ByteSized::B64,
        } => "LINT",
        IntermediateType::UInt {
            size: ByteSized::B8,
        } => "USINT",
        IntermediateType::UInt {
            size: ByteSized::B16,
        } => "UINT",
        IntermediateType::UInt {
            size: ByteSized::B32,
        } => "UDINT",
        IntermediateType::UInt {
            size: ByteSized::B64,
        } => "ULINT",
        IntermediateType::Bytes {
            size: ByteSized::B8,
        } => "BYTE",
        IntermediateType::Bytes {
            size: ByteSized::B16,
        } => "WORD",
        IntermediateType::Bytes {
            size: ByteSized::B32,
        } => "DWORD",
        IntermediateType::Bytes {
            size: ByteSized::B64,
        } => "LWORD",
        IntermediateType::Real {
            size: ByteSized::B32,
        } => "REAL",
        IntermediateType::Real {
            size: ByteSized::B64,
        } => "LREAL",
        IntermediateType::Time {
            size: ByteSized::B32,
        } => "TIME",
        IntermediateType::Time {
            size: ByteSized::B64,
        } => "LTIME",
        IntermediateType::String { .. } => "STRING",
        _ => {
            return Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                "Unsupported array element type",
            )))
        }
    };
    Ok(Id::from(name))
}

/// Maps VarTypeInfo to the element type byte used in array descriptors.
///
/// | VarTypeInfo | Type byte |
/// |-------------|-----------|
/// | W32 + Signed | 0 (I32) |
/// | W32 + Unsigned | 1 (U32) |
/// | W64 + Signed | 2 (I64) |
/// | W64 + Unsigned | 3 (U64) |
/// | F32 | 4 (F32) |
/// | F64 | 5 (F64) |
pub(crate) fn var_type_info_to_type_byte(vti: &VarTypeInfo) -> u8 {
    use super::compile::{OpWidth, Signedness};
    match (vti.op_width, vti.signedness) {
        (OpWidth::W32, Signedness::Signed) => 0,
        (OpWidth::W32, Signedness::Unsigned) => 1,
        (OpWidth::W64, Signedness::Signed) => 2,
        (OpWidth::W64, Signedness::Unsigned) => 3,
        (OpWidth::F32, _) => 4,
        (OpWidth::F64, _) => 5,
    }
}

/// Builds the per-dimension metadata and the total element count for the
/// given inclusive `(lower, upper)` bounds. Strides are computed in row-major
/// order (the last dimension is contiguous). Shared by plain arrays and
/// `REF_TO ARRAY` variables so both report the same diagnostics for arrays
/// that are too large.
///
/// An array over the element limit reports P9997 (`NotSupported`) rather than
/// P9999 (`NotImplemented`): the cap exists so that flat-index arithmetic
/// stays within i32, which is a fixed property of the bytecode format and not
/// a feature awaiting work. A program that reaches it has to hold less data,
/// so promising "not yet" would be a promise the compiler cannot keep.
pub(crate) fn compute_dimensions(
    bounds: &[(i32, i32)],
    span: &SourceSpan,
) -> Result<(Vec<DimensionInfo>, u32), Diagnostic> {
    // 1. Build DimensionInfo from normalized bounds
    let mut dimensions: Vec<DimensionInfo> = Vec::new();
    let mut total_elements: u32 = 1;
    for &(lower, upper) in bounds {
        let size = (upper as i64 - lower as i64 + 1) as u32;
        dimensions.push(DimensionInfo {
            lower_bound: lower,
            size,
            stride: 0,
        });
        total_elements = total_elements.checked_mul(size).ok_or_else(|| {
            Diagnostic::not_supported(Label::span(span.clone(), "Array too large"))
        })?;
    }

    // 2. Validate element limit (i32 safety for flat-index arithmetic)
    if total_elements > super::compile::MAX_DATA_REGION_SLOTS {
        return Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Array exceeds maximum 32768 elements",
        )));
    }

    // 3. Compute strides (reverse pass)
    let n = dimensions.len();
    if n > 0 {
        dimensions[n - 1].stride = 1;
        for k in (0..n - 1).rev() {
            dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
        }
    }

    Ok((dimensions, total_elements))
}

/// Registers an array variable from a normalized ArraySpec.
/// Single code path for both inline and named array types.
pub(crate) fn register_array_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    spec: &ArraySpec,
    span: &ironplc_dsl::core::SourceSpan,
) -> Result<(u8, String), Diagnostic> {
    let is_string = spec.string_max_len.is_some();
    let string_max_len = spec.string_max_len.unwrap_or(0);
    let string_char_width = spec.string_char_width.unwrap_or(CharWidth::Narrow);

    // 1. Resolve element type
    let element_vti = if spec.ref_to {
        // References are stored as 64-bit unsigned variable-table indices
        VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }
    } else if is_string {
        // STRING arrays use string-specific opcodes; VarTypeInfo is a placeholder.
        VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 0,
        }
    } else {
        super::type_info::resolve_type_name(&spec.element_type_name).ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(span.clone(), "Unsupported array element type"))
        })?
    };

    // 2. Build DimensionInfo (with strides) and the element count
    let (dimensions, total_elements) = compute_dimensions(&spec.dimensions, span)?;

    // 3. Allocate data region space
    let total_bytes = if is_string {
        // STRING/WSTRING elements: each element is [max_len:u16][cur_len:u16][data]
        let element_stride = super::compile::string_region_size(string_max_len, string_char_width);
        total_elements.checked_mul(element_stride).ok_or_else(|| {
            Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow"))
        })?
    } else {
        total_elements * 8
    };
    let data_offset = crate::data_region::reserve(ctx, total_bytes, span)?;

    // 4. Register descriptor in the container and get its index
    let (element_type_byte, element_extra) = if is_string {
        let element_field_type = if string_char_width.is_wide() {
            ironplc_container::FieldType::WString
        } else {
            ironplc_container::FieldType::String
        };
        (element_field_type as u8, string_max_len)
    } else {
        (var_type_info_to_type_byte(&element_vti), 0)
    };
    let desc_index = builder.add_array_descriptor(element_type_byte, total_elements, element_extra);

    // 5. Track max string capacity for temp buffer sizing.
    if is_string && string_max_len > ctx.max_string_capacity {
        ctx.max_string_capacity = string_max_len;
    }
    if is_string && string_char_width.is_wide() {
        ctx.has_wide_string = true;
    }

    // 6. Store in context
    ctx.array_vars.insert(
        id.clone(),
        ArrayVarInfo {
            var_index,
            desc_index,
            data_offset,
            element_var_type_info: element_vti,
            total_elements,
            dimensions,
            is_string_element: is_string,
            string_max_len,
            string_char_width,
            is_ref: false,
        },
    );

    let type_tag = ironplc_container::debug_section::iec_type_tag::ARRAY;
    let type_name_str = if spec.ref_to {
        format!(
            "ARRAY OF REF_TO {}",
            spec.element_type_name.to_string().to_uppercase()
        )
    } else {
        format!(
            "ARRAY OF {}",
            spec.element_type_name.to_string().to_uppercase()
        )
    };
    Ok((type_tag, type_name_str))
}

/// Recursively walks the `ArrayInitialElementKind` tree and produces
/// a flat `Vec<ConstantKind>` of initial values in element order.
pub(crate) fn flatten_array_initial_values(
    elements: &[ArrayInitialElementKind],
) -> Result<Vec<ConstantKind>, Diagnostic> {
    let mut result = Vec::new();
    for elem in elements {
        match elem {
            ArrayInitialElementKind::Constant(value) => {
                result.push(value.clone());
            }
            ArrayInitialElementKind::EnumValue(value) => {
                return Err(Diagnostic::not_implemented(Label::span(
                    value.span(),
                    "Enumerated value in an array initializer",
                )));
            }
            ArrayInitialElementKind::Repeated(repeated) => {
                let count = repeated.size.value as usize;
                match repeated.init.as_ref().as_ref() {
                    Some(inner) => {
                        let inner_values =
                            flatten_array_initial_values(std::slice::from_ref(inner))?;
                        for _ in 0..count {
                            result.extend_from_slice(&inner_values);
                        }
                    }
                    None => {
                        let zero = ConstantKind::integer_literal("0")
                            .expect("literal '0' is always valid");
                        for _ in 0..count {
                            result.push(zero.clone());
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}

/// Emits bytecode to compute the 0-based flat index from subscript expressions.
///
/// For constant subscripts, computes the flat index at compile time.
/// For variable subscripts, emits i64 arithmetic:
///   `(s_0 - l_0) * stride_0 + (s_1 - l_1) * stride_1 + ...`
pub(crate) fn emit_flat_index(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    subscripts: &[&Expr],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if subscripts.len() != dimensions.len() {
        return Err(Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Wrong number of array subscripts",
        )));
    }

    // Try compile-time constant folding for all-literal subscripts.
    if let Some(flat_index) = try_constant_flat_index(subscripts, dimensions, span)? {
        let const_index = ctx.add_i32_constant(flat_index);
        emitter.emit_load_const_i32(const_index);
        return Ok(());
    }

    // Variable case: emit runtime computation using i64 arithmetic.
    let subscript_op_type = (OpWidth::W32, Signedness::Signed);
    for (k, (subscript, dim)) in subscripts.iter().zip(dimensions.iter()).enumerate() {
        compile_expr(emitter, ctx, subscript, subscript_op_type)?;
        if dim.lower_bound != 0 {
            let lb_const = ctx.add_i64_constant(dim.lower_bound as i64);
            emitter.emit_load_const_i64(lb_const);
            emitter.emit_sub_i64();
        }
        if dim.stride != 1 {
            let stride_const = ctx.add_i64_constant(dim.stride as i64);
            emitter.emit_load_const_i64(stride_const);
            emitter.emit_mul_i64();
        }
        if k > 0 {
            emitter.emit_add_i64();
        }
    }
    Ok(())
}

/// Tries to compute the flat index at compile time when all subscripts are literals.
/// Returns `None` if any subscript is not a literal (fall through to runtime).
/// Returns `Err` if a literal subscript is out of bounds.
fn try_constant_flat_index(
    subscripts: &[&Expr],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<Option<i32>, Diagnostic> {
    let mut flat_index: i32 = 0;
    for (subscript, dim) in subscripts.iter().zip(dimensions.iter()) {
        let value = match try_extract_integer_literal(subscript) {
            Some(v) => v,
            None => return Ok(None),
        };
        let upper = dim.lower_bound + dim.size as i32 - 1;
        if value < dim.lower_bound || value > upper {
            return Err(Diagnostic::problem(
                Problem::ArrayIndexOutOfBounds,
                Label::span(span.clone(), "Array index out of bounds"),
            ));
        }
        flat_index += (value - dim.lower_bound) * dim.stride as i32;
    }
    Ok(Some(flat_index))
}

/// Extracts an i32 value from an expression if it is a literal integer.
/// Returns `None` for any non-literal expression.
fn try_extract_integer_literal(expr: &Expr) -> Option<i32> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            let unsigned = i32::try_from(lit.value.value.value).ok()?;
            if lit.value.is_neg {
                unsigned.checked_neg()
            } else {
                Some(unsigned)
            }
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => match &unary.term.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
                let val = i32::try_from(lit.value.value.value).ok()?;
                if lit.value.is_neg {
                    Some(val)
                } else {
                    val.checked_neg()
                }
            }
            _ => None,
        },
        _ => None,
    }
}
