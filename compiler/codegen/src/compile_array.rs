//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_dsl::common::{
    ArrayInitialElementKind, ConstantKind, ReferenceInitializer, ReferenceTarget,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, SymbolicVariableKind, UnaryOp, Variable};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::{ArrayDimension, ByteSized, IntermediateType};
use ironplc_container::{ContainerBuilder, SlotIndex, VarIndex};

use super::compile::{compile_expr, CompileContext, OpType, OpWidth, Signedness, VarTypeInfo};
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
    /// For STRING arrays, the maximum string length per element.
    /// `None` for non-string element types.
    pub string_max_len: Option<u16>,
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
                            Diagnostic::todo_with_span(named.name.span(), file!(), line!())
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
                                let info = ctx.array_vars.get(&named.name).ok_or_else(|| {
                                    Diagnostic::todo_with_span(named.name.span(), file!(), line!())
                                })?;
                                return Ok(ResolvedAccess::DerefArrayElement {
                                    info,
                                    subscripts: all_subscripts,
                                });
                            }
                            other => {
                                return Err(Diagnostic::todo_with_span(
                                    other.span(),
                                    file!(),
                                    line!(),
                                ));
                            }
                        }
                    }
                    SymbolicVariableKind::Structured(structured) => {
                        levels.reverse();
                        let all_subscripts: Vec<&Expr> = levels.into_iter().flatten().collect();
                        return resolve_struct_field_array(ctx, structured, all_subscripts);
                    }
                    other => {
                        return Err(Diagnostic::todo_with_span(other.span(), file!(), line!()));
                    }
                }
            }
        }
        _ => {
            // Fall through to existing resolve_variable() for scalars.
            let var_index = super::compile::resolve_variable(ctx, variable)?;
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
fn resolve_struct_field_array<'ctx, 'ast>(
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
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                structured.field.span(),
                format!("Field '{}' is not an array type", structured.field),
            ),
        ));
    };

    let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                structured.span(),
                format!("Variable '{}' is not a structure", root_name),
            ),
        )
    })?;

    let element_op_type =
        crate::compile_struct::resolve_field_op_type(element_type).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(
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
fn dimensions_from_intermediate(dims: &[ArrayDimension]) -> Vec<DimensionInfo> {
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
            let lower =
                super::compile::signed_integer_to_i32(range.start.as_signed_integer().unwrap())?;
            let upper =
                super::compile::signed_integer_to_i32(range.end.as_signed_integer().unwrap())?;
            Ok((lower, upper))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let string_max_len = match &subranges.type_name {
        ironplc_dsl::common::ArrayElementType::String(spec)
        | ironplc_dsl::common::ArrayElementType::WString(spec) => {
            let len = spec
                .length
                .as_ref()
                .and_then(|l| l.as_integer().map(|i| i.value as u16))
                .unwrap_or(super::compile::DEFAULT_STRING_MAX_LENGTH_U16);
            Some(len)
        }
        _ => None,
    };
    Ok(ArraySpec {
        dimensions,
        element_type_name: Id::from(&subranges.type_name.to_type_name().to_string()),
        ref_to: subranges.ref_to,
        string_max_len,
    })
}

/// Converts a named array type (from the TypeEnvironment) to a normalized ArraySpec.
pub(crate) fn array_spec_from_named(
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
) -> Result<ArraySpec, Diagnostic> {
    let dims: Vec<(i32, i32)> = dimensions.iter().map(|d| (d.lower, d.upper)).collect();
    let ref_to = matches!(element_type, IntermediateType::Reference { .. });
    let inner_type = if let IntermediateType::Reference { target_type } = element_type {
        target_type.as_ref()
    } else {
        element_type
    };
    let element_type_name = intermediate_type_to_name(inner_type)?;
    let string_max_len = match inner_type {
        IntermediateType::String { max_len } => {
            let len = max_len
                .map(|v| v as u16)
                .unwrap_or(super::compile::DEFAULT_STRING_MAX_LENGTH_U16);
            Some(len)
        }
        _ => None,
    };
    Ok(ArraySpec {
        dimensions: dims,
        element_type_name,
        ref_to,
        string_max_len,
    })
}

/// Maps an IntermediateType to the IEC 61131-3 type name (as an Id) that
/// `resolve_type_name()` in compile.rs can look up. Only primitive types
/// are supported (arrays of complex types are out of scope).
fn intermediate_type_to_name(ty: &IntermediateType) -> Result<Id, Diagnostic> {
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
        _ => return Err(Diagnostic::todo(file!(), line!())),
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
        super::compile::resolve_type_name(&spec.element_type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Unsupported array element type"),
            )
        })?
    };

    // 2. Build DimensionInfo from normalized bounds
    let mut dimensions: Vec<DimensionInfo> = Vec::new();
    let mut total_elements: u32 = 1;
    for &(lower, upper) in &spec.dimensions {
        let size = (upper as i64 - lower as i64 + 1) as u32;
        dimensions.push(DimensionInfo {
            lower_bound: lower,
            size,
            stride: 0,
        });
        total_elements = total_elements.checked_mul(size).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Array too large"),
            )
        })?;
    }

    // 3. Validate element limit (i32 safety for flat-index arithmetic)
    if total_elements > super::compile::MAX_DATA_REGION_SLOTS {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Array exceeds maximum 32768 elements"),
        ));
    }

    // 4. Compute strides (reverse pass)
    let n = dimensions.len();
    if n > 0 {
        dimensions[n - 1].stride = 1;
        for k in (0..n - 1).rev() {
            dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
        }
    }

    // 5. Allocate data region space
    let data_offset = ctx.data_region_offset;
    let total_bytes = if is_string {
        // STRING elements: each element is [max_len:u16][cur_len:u16][data:max_len bytes]
        let element_stride = super::compile::STRING_HEADER_BYTES_U32 + string_max_len as u32;
        total_elements.checked_mul(element_stride).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Data region overflow"),
            )
        })?
    } else {
        total_elements * 8
    };
    ctx.data_region_offset = ctx
        .data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Data region overflow"),
            )
        })?;

    // 6. Assert data_offset fits in i32 (stored in slot via LOAD_CONST_I32)
    if data_offset > i32::MAX as u32 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region exceeds 2 GiB limit"),
        ));
    }

    // 7. Register descriptor in the container and get its index
    let (element_type_byte, element_extra) = if is_string {
        (ironplc_container::FieldType::String as u8, string_max_len)
    } else {
        (var_type_info_to_type_byte(&element_vti), 0)
    };
    let desc_index = builder.add_array_descriptor(element_type_byte, total_elements, element_extra);

    // 8. Track max string capacity for temp buffer sizing.
    if is_string && string_max_len > ctx.max_string_capacity {
        ctx.max_string_capacity = string_max_len;
    }

    // 9. Store in context
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
        },
    );

    let type_tag = ironplc_container::debug_section::iec_type_tag::OTHER;
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

/// Registers array metadata for a `REF_TO ARRAY` variable so that
/// `PT^[idx]` can be compiled with deref array opcodes.
///
/// No data region space is allocated — the reference parameter
/// points to an array in the caller's scope.
pub(crate) fn register_ref_to_array_metadata(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    ref_init: &ReferenceInitializer,
) -> Result<(), Diagnostic> {
    if let ReferenceTarget::Array(subranges) = &ref_init.target {
        let span = id.span();
        let spec = array_spec_from_inline(subranges, &span)?;
        let element_vti = if spec.ref_to {
            VarTypeInfo {
                op_width: OpWidth::W64,
                signedness: Signedness::Unsigned,
                storage_bits: 64,
            }
        } else {
            super::compile::resolve_type_name(&spec.element_type_name).unwrap_or(VarTypeInfo {
                op_width: OpWidth::W32,
                signedness: Signedness::Unsigned,
                storage_bits: 32,
            })
        };
        let element_type_byte = var_type_info_to_type_byte(&element_vti);
        let mut dimensions = Vec::new();
        let mut total_elements: u32 = 1;
        for &(lower, upper) in &spec.dimensions {
            let size = (upper as i64 - lower as i64 + 1) as u32;
            dimensions.push(DimensionInfo {
                lower_bound: lower,
                size,
                stride: 0,
            });
            total_elements *= size;
        }
        let n = dimensions.len();
        if n > 0 {
            dimensions[n - 1].stride = 1;
            for k in (0..n - 1).rev() {
                dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
            }
        }
        let desc_index = builder.add_array_descriptor(element_type_byte, total_elements, 0);
        ctx.array_vars.insert(
            id.clone(),
            ArrayVarInfo {
                var_index,
                desc_index,
                data_offset: 0,
                element_var_type_info: element_vti,
                total_elements,
                dimensions,
                is_string_element: false,
                string_max_len: 0,
            },
        );
    }
    Ok(())
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
            ArrayInitialElementKind::EnumValue(_) => {
                return Err(Diagnostic::todo(file!(), line!()));
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
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Wrong number of array subscripts"),
        ));
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
