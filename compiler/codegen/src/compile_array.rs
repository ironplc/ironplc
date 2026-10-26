//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use crate::access::{Access, Field};
use ironplc_ir::{Diagnostic, Label};
use ironplc_ir::{Located, SourceSpan};
use ironplc_problems::Problem;

use ironplc_container::{CharWidth, SlotIndex, VarIndex};

use super::compile::{CompileContext, VarTypeInfo};
use super::compile_subscript::Subscript;
use crate::body::slot_type;
use crate::emit::Emitter;
use ironplc_ir::lowered::{TypeKind, TypeTable};

/// Metadata for a single dimension of an array, used for index computation.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DimensionInfo {
    pub lower_bound: i32,
    pub size: u32,
    pub stride: u32,
}

/// Metadata for an array variable, stored in CompileContext.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
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
pub(crate) enum ResolvedAccess<'ctx, S> {
    /// Simple named variable — use LOAD_VAR/STORE_VAR.
    Scalar { var_index: VarIndex },
    /// `VAR_IN_OUT` parameter of the function being compiled. Its slot holds
    /// a reference to the caller's variable: load the reference with
    /// LOAD_VAR_I64, then use LOAD_INDIRECT/STORE_INDIRECT.
    InOut { ref_slot: VarIndex },
    /// Array element — compute flat index, use LOAD_ARRAY/STORE_ARRAY.
    ArrayElement {
        info: &'ctx ArrayVarInfo,
        subscripts: Vec<S>,
    },
    /// Array element through a dereferenced reference — use LOAD_ARRAY_DEREF/STORE_ARRAY_DEREF.
    DerefArrayElement {
        info: &'ctx ArrayVarInfo,
        subscripts: Vec<S>,
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
        subscripts: Vec<S>,
        /// The type the element is loaded and stored at.
        type_info: VarTypeInfo,
    },
    /// STRING array element within a struct field — see [`StructStringElement`].
    StructFieldStringArrayElement(StructStringElement<S>),
}

/// A STRING element of an array that lives inside a structure's data region.
///
/// `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM` address an element as
/// `base + flat_index * stride`, reading `base` from a variable. The
/// structure's variable holds the start of the whole structure rather than of
/// the array, so the array's start is first computed into a scratch variable
/// ([`StringPlace`](super::compile_string_place::StringPlace)).
pub(crate) struct StructStringElement<S> {
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
    pub subscripts: Vec<S>,
}

/// Resolves a place into its access kind.
///
/// For a named variable, returns Scalar with the variable table index. For
/// an element of an array, walks the chain of subscripts to collect them
/// all and resolve the base variable's ArrayVarInfo.
pub(crate) fn resolve_access<'ctx, S: Clone>(
    types: &TypeTable,
    ctx: &'ctx CompileContext,
    access: &Access<S>,
) -> Result<ResolvedAccess<'ctx, S>, Diagnostic> {
    match access {
        Access::Index(array_var) => {
            // Walk the chain collecting subscript groups innermost-first,
            // then reverse. For nested arrays arr[i][j] the chain is
            // Index(Index(Named(arr), [i]), [j]): we collect [[j], [i]],
            // reverse to [[i], [j]], flatten to [i, j].
            let mut levels: Vec<&[S]> = Vec::new();
            let mut current = array_var;
            loop {
                levels.push(&current.subscripts);
                match current.array.as_ref() {
                    Access::Index(inner) => {
                        current = inner;
                    }
                    Access::Named(name) => {
                        levels.reverse();
                        let all_subscripts: Vec<S> =
                            levels.into_iter().flatten().cloned().collect();
                        let info = ctx.array_var(name).ok_or_else(|| {
                            // An element of an array of structures spans
                            // several slots, so it has no single-value load or
                            // store; only its fields are addressable.
                            if ctx.struct_array_var(name).is_some() {
                                Diagnostic::not_implemented(Label::span(
                                    name.span(),
                                    format!(
                                        "Whole-element access of '{}' -- select a field of the element instead",
                                        name
                                    ),
                                ))
                            } else {
                                Diagnostic::todo_with_span(name.span())
                            }
                        })?;
                        return Ok(ResolvedAccess::ArrayElement {
                            info,
                            subscripts: all_subscripts,
                        });
                    }
                    Access::Deref(deref) => {
                        // Dereference in array chain (e.g., PT^[0] where PT is REF_TO ARRAY).
                        // Walk through the deref to find the base variable name.
                        let mut inner = deref.as_ref();
                        while let Access::Deref(d) = inner {
                            inner = d.as_ref();
                        }
                        match inner {
                            Access::Named(name) => {
                                levels.reverse();
                                let all_subscripts: Vec<S> =
                                    levels.into_iter().flatten().cloned().collect();
                                let info = ctx
                                    .array_var(name)
                                    .ok_or_else(|| Diagnostic::todo_with_span(name.span()))?;
                                return Ok(ResolvedAccess::DerefArrayElement {
                                    info,
                                    subscripts: all_subscripts,
                                });
                            }
                            other @ (Access::Field(_) | Access::Index(_) | Access::Deref(_)) => {
                                return Err(Diagnostic::todo_with_span(other.span()));
                            }
                        }
                    }
                    Access::Field(structured) => {
                        levels.reverse();
                        let all_subscripts: Vec<S> =
                            levels.into_iter().flatten().cloned().collect();
                        // `a[i].values[j]` -- the record is itself an array
                        // element, so the subscripts collected here index the
                        // *field*, and the element index is still inside the
                        // record. The array-of-struct path combines the two.
                        if crate::compile_array_struct::element_field_chain(structured).is_some() {
                            return crate::compile_array_struct::resolve_struct_array_element_field(
                                types,
                                ctx,
                                structured,
                                all_subscripts,
                            );
                        }
                        return resolve_struct_field_array(types, ctx, structured, all_subscripts);
                    }
                }
            }
        }
        // `s.arr[i].field` and `arr[i].inner.field` -- a field selected
        // from an element of an array-of-struct. The element is not at a
        // fixed offset, so it resolves through the array path.
        Access::Field(structured)
            if crate::compile_array_struct::element_field_chain(structured).is_some() =>
        {
            crate::compile_array_struct::resolve_struct_array_element_field(
                types,
                ctx,
                structured,
                Vec::new(),
            )
        }
        Access::Named(name) => {
            if let Some(ref_slot) = ctx.in_out_ref_slot(name) {
                return Ok(ResolvedAccess::InOut { ref_slot });
            }
            let var_index = ctx.var_index(name)?;
            Ok(ResolvedAccess::Scalar { var_index })
        }
        // A field of a structure is at a fixed offset, which `Place::resolve`
        // addresses, and a whole referenced value has no access kind.
        Access::Field(_) | Access::Deref(_) => Err(Diagnostic::todo_with_span(access.span())),
    }
}

/// Resolves an array subscript whose base is a struct field.
///
/// For `math.FACTS[x]`, the struct field `FACTS` is an array. We resolve the
/// struct chain to get the field's slot offset and type, extract array dimension
/// info, and return a `StructFieldArrayElement` that the caller uses to emit
/// `flat_index + slot_offset` followed by the struct's LOAD_ARRAY/STORE_ARRAY.
pub(crate) fn resolve_struct_field_array<'ctx, S>(
    types: &TypeTable,
    ctx: &'ctx CompileContext,
    structured: &Field<S>,
    subscripts: Vec<S>,
) -> Result<ResolvedAccess<'ctx, S>, Diagnostic> {
    let (root_name, slot_offset, field_type) =
        crate::compile_struct::walk_struct_chain(types, ctx, structured, 0)?;

    let Some(TypeKind::Array {
        element,
        dimensions: array_dims,
    }) = types.kind(field_type)
    else {
        return Err(Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            format!("Field '{}' is not an array type", structured.field),
        )));
    };

    let struct_info = ctx.struct_var(&root_name).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            format!("Variable '{}' is not a structure", root_name),
        ))
    })?;

    // STRING array fields use dedicated STR_LOAD/STORE_ARRAY_ELEM opcodes
    // with a scratch variable and a STRING-specific array descriptor.
    if let Some(TypeKind::String(_)) = types.kind(*element) {
        // The structure registers a descriptor for each of its own fields
        // that is an array of strings, not for those of a structure it holds.
        let own_field = matches!(structured.record.as_ref(), Access::Named(_));
        let &(str_desc_index, _, _) = own_field
            .then(|| struct_info.string_array_descs.get(&structured.idx))
            .flatten()
            .ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    structured.field.span(),
                    "STRING array descriptor not registered for field",
                ))
            })?;
        // Allocated for every structure that has a STRING-array descriptor.
        let scratch = struct_info.scratch_var_index.ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                structured.field.span(),
                "Scratch variable not allocated for struct",
            ))
        })?;
        let dimensions = dimensions_of_bounds(array_dims);
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

    let type_info = slot_type(types, *element).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            "Array element type is not a primitive (nested struct/array elements not supported)",
        ))
    })?;

    let dimensions = dimensions_of_bounds(array_dims);

    Ok(ResolvedAccess::StructFieldArrayElement {
        var_index: struct_info.var_index,
        desc_index: struct_info.desc_index,
        field_slot_offset: slot_offset,
        dimensions,
        subscripts,
        type_info,
    })
}

/// Converts the bounds of each dimension, `(lower, upper)`, into
/// `DimensionInfo` with computed strides.
///
/// Strides follow row-major order: the last dimension has stride 1, each
/// preceding dimension's stride is the product of all subsequent dimension sizes.
pub(crate) fn dimensions_of_bounds(dims: &[(i32, i32)]) -> Vec<DimensionInfo> {
    let sizes: Vec<u32> = dims
        .iter()
        .map(|&(lower, upper)| (upper as i64 - lower as i64 + 1).max(0) as u32)
        .collect();

    let mut strides = vec![1u32; sizes.len()];
    for i in (0..sizes.len().saturating_sub(1)).rev() {
        strides[i] = strides[i + 1].saturating_mul(sizes[i + 1]);
    }

    dims.iter()
        .zip(sizes.iter().zip(strides.iter()))
        .map(|(&(lower, _), (&size, &stride))| DimensionInfo {
            lower_bound: lower,
            size,
            stride,
        })
        .collect()
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

/// Emits bytecode to compute the 0-based flat index from subscript expressions.
///
/// For constant subscripts, computes the flat index at compile time.
/// For variable subscripts, emits i64 arithmetic:
///   `(s_0 - l_0) * stride_0 + (s_1 - l_1) * stride_1 + ...`
pub(crate) fn emit_flat_index<S: Subscript>(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    subscripts: &[S],
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
    for (k, (subscript, dim)) in subscripts.iter().zip(dimensions.iter()).enumerate() {
        subscript.emit(emitter, ctx)?;
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
/// Returns `Err` if a literal subscript is out of bounds. Analysis reports such
/// a subscript first (`rule_array_index_range`, P2027), so this is the fallback
/// for a caller that compiles without analysis.
fn try_constant_flat_index<S: Subscript>(
    subscripts: &[S],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<Option<i32>, Diagnostic> {
    let mut flat_index: i32 = 0;
    for (subscript, dim) in subscripts.iter().zip(dimensions.iter()) {
        let value = match subscript.literal() {
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
