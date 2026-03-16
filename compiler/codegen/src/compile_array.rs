//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, SymbolicVariableKind, Variable};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::{ArrayDimension, ByteSized, IntermediateType};
use ironplc_container::ContainerBuilder;

use super::compile::{CompileContext, VarTypeInfo};

/// Normalized array specification, independent of AST representation.
/// Both inline (`ARRAY[1..3, 1..4] OF INT`) and named type paths
/// convert to this form before registration.
pub(crate) struct ArraySpec {
    /// Per-dimension bounds as (lower, upper) inclusive pairs.
    pub dimensions: Vec<(i32, i32)>,
    /// Element type name (e.g., "INT", "DINT").
    pub element_type_name: Id,
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
    pub element_var_type_info: VarTypeInfo,
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
        subscripts: Vec<&'a Expr>,
    },
}

/// Resolves a variable reference into its access kind.
///
/// For named variables, returns Scalar with the variable table index.
/// For array variables, walks the ArrayVariable chain to collect
/// all subscripts and resolve the base variable's ArrayVarInfo.
///
/// This function is the single dispatch point for variable access
/// resolution. When struct access is added, extend the match in
/// this function and add a new ResolvedAccess variant — the call
/// sites in compile_expr and compile_statement stay unchanged.
#[allow(dead_code)]
pub(crate) fn resolve_access<'a>(
    ctx: &'a CompileContext,
    variable: &'a Variable,
) -> Result<ResolvedAccess<'a>, Diagnostic> {
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

/// Converts an inline array specification (from the AST) to a normalized ArraySpec.
pub(crate) fn array_spec_from_inline(
    subranges: &ironplc_dsl::common::ArraySubranges,
    _span: &ironplc_dsl::core::SourceSpan,
) -> Result<ArraySpec, Diagnostic> {
    let dimensions: Vec<(i32, i32)> = subranges
        .ranges
        .iter()
        .map(|range| {
            let lower = super::compile::signed_integer_to_i32(&range.start)?;
            let upper = super::compile::signed_integer_to_i32(&range.end)?;
            Ok((lower, upper))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(ArraySpec {
        dimensions,
        element_type_name: Id::from(&subranges.type_name.to_string()),
    })
}

/// Converts a named array type (from the TypeEnvironment) to a normalized ArraySpec.
pub(crate) fn array_spec_from_named(
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
) -> Result<ArraySpec, Diagnostic> {
    let dims: Vec<(i32, i32)> = dimensions.iter().map(|d| (d.lower, d.upper)).collect();
    let element_type_name = intermediate_type_to_name(element_type)?;
    Ok(ArraySpec {
        dimensions: dims,
        element_type_name,
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
    var_index: u16,
    spec: &ArraySpec,
    span: &ironplc_dsl::core::SourceSpan,
) -> Result<(u8, String), Diagnostic> {
    // 1. Resolve element type
    let element_vti =
        super::compile::resolve_type_name(&spec.element_type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Unsupported array element type"),
            )
        })?;

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
    if total_elements > 32768 {
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
    let total_bytes = total_elements * 8;
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
    let element_type_byte = var_type_info_to_type_byte(&element_vti);
    let desc_index = builder.add_array_descriptor(element_type_byte, total_elements);

    // 8. Store in context
    ctx.array_vars.insert(
        id.clone(),
        ArrayVarInfo {
            var_index,
            desc_index,
            data_offset,
            element_var_type_info: element_vti,
            total_elements,
            dimensions,
        },
    );

    let type_tag = ironplc_container::debug_section::iec_type_tag::OTHER;
    let type_name_str = format!(
        "ARRAY OF {}",
        spec.element_type_name.to_string().to_uppercase()
    );
    Ok((type_tag, type_name_str))
}
