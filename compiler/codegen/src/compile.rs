//! Compiles an IEC 61131-3 AST into a bytecode container.
//!
//! This module walks the AST produced by the parser/analyzer and generates
//! bytecode that the IronPLC VM can execute.
//!
//! # Supported constructs
//!
//! - PROGRAM declarations with all 8 IEC 61131-3 integer types
//!   (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT),
//!   2 floating-point types (REAL, LREAL),
//!   5 bit string types (BOOL, BYTE, WORD, DWORD, LWORD)
//! - Assignment statements with truncation for narrow types
//! - Integer literal constants (i32 and i64)
//! - Real literal constants (f32 and f64)
//! - Boolean literal constants (TRUE, FALSE)
//! - Bit string literal constants (BYTE, WORD, DWORD, LWORD prefixed)
//! - Binary Add, Sub, Mul, Div, Mod, and Pow operators
//! - Unary Neg and Not operators
//! - Comparison operators (=, <>, <, <=, >, >=)
//! - Boolean operators (AND, OR, XOR, NOT) with logical semantics
//! - Bitwise operators (AND, OR, XOR, NOT) for bit string types (BYTE, WORD, DWORD, LWORD)
//! - Bit shift/rotate functions (SHL, SHR, ROL, ROR) for bit string types
//! - MUX (multiplexer) function with variable arity (2..16 inputs)
//! - Variable references (named symbolic variables)
//! - IF/ELSIF/ELSE statements
//! - CASE statements (integer and subrange selectors)
//! - WHILE, FOR, and REPEAT iteration statements
//! - EXIT (break from innermost loop) and RETURN (early program exit)
//!
//! # Not yet supported
//!
//! - TODO: STRING[N] in VAR_IN_OUT (parsed, but runtime pass-by-reference not implemented)
//! - TODO: STRING[N] in STRUCT members (parsed, but struct compilation not implemented)
//!
//! # Integer type strategy: promote-operate-truncate
//!
//! Two native operation widths: **i32** (for ≤32-bit types) and **i64**
//! (for 64-bit types). Variables are loaded/stored at native width.
//! After arithmetic at native width, narrow types (SINT, INT, USINT, UINT)
//! are truncated back to their declared range before storing.

use std::collections::HashMap;

use ironplc_container::debug_section::{
    function_id, iec_type_tag, var_section, FuncNameEntry, VarNameEntry,
};
use ironplc_container::{
    opcode, Container, ContainerBuilder, FbTypeId, FunctionId, UserFbDescriptor, VarIndex,
    STRING_HEADER_BYTES,
};
use ironplc_dsl::common::{
    ElementaryTypeName, FunctionBlockDeclaration, FunctionDeclaration, FunctionReturnType,
    GenericTypeName, InitialValueAssignmentKind, Library, LibraryElementKind, ProgramDeclaration,
    ReferenceInitialValue, SpecificationKind, VarDecl, VariableType,
};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::{FileId, Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Function;
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::IntermediateType;
use ironplc_analyzer::{FunctionEnvironment, SemanticContext, TypeEnvironment};

use crate::emit::Emitter;

use super::compile_call::collect_positional_args;
use super::compile_expr::{
    compile_constant, compile_expr, emit_load_var, emit_store_var, emit_truncation,
    resolve_variable,
};
use super::compile_stmt::{
    compile_body, compile_statements, resolve_string_max_length, resolve_string_spec_max_length,
};
use super::compile_string::resolve_string_arg;

/// The native operation width used for arithmetic and comparisons.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum OpWidth {
    /// 32-bit integer operations (for SINT, INT, DINT, USINT, UINT, UDINT).
    W32,
    /// 64-bit integer operations (for LINT, ULINT).
    W64,
    /// 32-bit float operations (for REAL).
    F32,
    /// 64-bit float operations (for LREAL).
    F64,
}

/// Whether a type uses signed or unsigned semantics for division and comparison.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Signedness {
    Signed,
    Unsigned,
}

/// Type information for a variable, used to select the correct opcodes.
#[derive(Clone, Copy)]
pub(crate) struct VarTypeInfo {
    /// The native operation width (i32 or i64).
    pub(crate) op_width: OpWidth,
    /// Whether signed or unsigned opcodes are used for division/comparison.
    pub(crate) signedness: Signedness,
    /// The declared storage width in bits (8, 16, 32, or 64).
    pub(crate) storage_bits: u8,
}

/// Shorthand for the operation type tuple used during expression compilation.
pub(crate) type OpType = (OpWidth, Signedness);

/// The default operation type: 32-bit signed (used for pure-constant expressions).
pub(crate) const DEFAULT_OP_TYPE: OpType = (OpWidth::W32, Signedness::Signed);

/// Maximum number of data-region slots (or array elements) that a single variable
/// may occupy. Keeps flat-index arithmetic within i32 range.
pub(crate) const MAX_DATA_REGION_SLOTS: u32 = 32768;

/// A constant in the pool: integer, float, or string.
enum PoolConstant {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(Vec<u8>),
}

/// The IEC 61131-3 default maximum length for STRING (254 characters).
pub(crate) const DEFAULT_STRING_MAX_LENGTH_U16: u16 = 254;

/// STRING_HEADER_BYTES re-exported as u32 for arithmetic in compile_array.
pub(crate) const STRING_HEADER_BYTES_U32: u32 = STRING_HEADER_BYTES as u32;

/// Metadata for a STRING variable allocated in the data region.
#[derive(Clone)]
pub(crate) struct StringVarInfo {
    /// Byte offset into the data region where this string starts.
    pub(crate) data_offset: u32,
    /// Maximum number of characters this string can hold.
    pub(crate) max_length: u16,
}

/// Compiles a library into a bytecode container.
///
/// Finds the first PROGRAM declaration in the library and compiles it
/// into a container suitable for execution by the VM. Only user-defined
/// functions reachable from the program root are included; unreachable
/// functions are automatically excluded.
///
/// Returns an error if no program is found or if the program contains
/// unsupported constructs.
/// Options that affect code generation.
#[derive(Default)]
pub struct CodegenOptions {
    /// When `true`, inject `__SYSTEM_UP_TIME` (TIME) and `__SYSTEM_UP_LTIME`
    /// (LTIME) as implicit globals at the start of the variable table.
    pub system_uptime_global: bool,
}

pub fn compile(
    library: &Library,
    context: &SemanticContext,
    options: &CodegenOptions,
) -> Result<Container, Diagnostic> {
    let program = find_program(library)?;
    let config = find_configuration(library);
    let user_globals: &[VarDecl] = config.map(|c| c.global_var.as_slice()).unwrap_or(&[]);

    // Prepend system uptime globals when the feature is enabled.
    let mut synthetic_globals: Vec<VarDecl> = Vec::new();
    if options.system_uptime_global {
        synthetic_globals
            .push(VarDecl::simple("__SYSTEM_UP_TIME", "TIME").with_type(VariableType::Global));
        synthetic_globals
            .push(VarDecl::simple("__SYSTEM_UP_LTIME", "LTIME").with_type(VariableType::Global));
    }

    // Collect top-level VAR_GLOBAL declarations (outside CONFIGURATION blocks).
    // These are common in the RuSTy dialect and OSCAT libraries.
    for element in &library.elements {
        if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
            synthetic_globals.extend_from_slice(decls);
        }
    }

    synthetic_globals.extend_from_slice(user_globals);
    let global_vars = &synthetic_globals;

    let reachable = context.reachable();

    // Collect user-defined function declarations from the library,
    // filtering to only reachable functions.
    let func_decls: Vec<&FunctionDeclaration> = library
        .elements
        .iter()
        .filter_map(|e| {
            if let LibraryElementKind::FunctionDeclaration(f) = e {
                reachable.contains(&f.name).then_some(f)
            } else {
                None
            }
        })
        .collect();

    // Collect user-defined function block declarations from the library,
    // filtering to only reachable function blocks.
    let fb_decls: Vec<&FunctionBlockDeclaration> = library
        .elements
        .iter()
        .filter_map(|e| {
            if let LibraryElementKind::FunctionBlockDeclaration(fb) = e {
                reachable.contains(&fb.name.name).then_some(fb)
            } else {
                None
            }
        })
        .collect();

    let mut container = compile_program_with_functions(
        program,
        &func_decls,
        &fb_decls,
        global_vars,
        context.functions(),
        context.types(),
    )?;

    if options.system_uptime_global {
        container.header.flags |= ironplc_container::FLAG_HAS_SYSTEM_UPTIME;
    }

    Ok(container)
}

/// Finds the first PROGRAM declaration in the library.
fn find_program(library: &Library) -> Result<&ProgramDeclaration, Diagnostic> {
    for element in &library.elements {
        if let LibraryElementKind::ProgramDeclaration(program) = element {
            return Ok(program);
        }
    }
    Err(Diagnostic::problem(
        Problem::NoProgramDeclaration,
        Label::file(
            FileId::default(),
            "Source does not contain a PROGRAM declaration",
        ),
    ))
}

/// Finds the first CONFIGURATION declaration in the library, if any.
fn find_configuration(library: &Library) -> Option<&ConfigurationDeclaration> {
    library.elements.iter().find_map(|e| {
        if let LibraryElementKind::ConfigurationDeclaration(config) = e {
            Some(config)
        } else {
            None
        }
    })
}

/// Holds the compiled bytecode and metadata for a user-defined function.
struct CompiledFunction {
    function_id: u16,
    bytecode: Vec<u8>,
    max_stack_depth: u16,
    num_locals: u16,
    num_params: u16,
    name: String,
}

/// Compiles a PROGRAM and its user-defined functions into a container.
///
/// Always emits at least two functions:
/// - Function 0: init (load constants + store variables, called once by VM)
/// - Function 1: scan (program body, called every scan cycle)
/// - Functions 2+: user-defined functions
///
/// When no initial values exist, the init function is a single RET_VOID.
fn compile_program_with_functions(
    program: &ProgramDeclaration,
    func_decls: &[&FunctionDeclaration],
    fb_decls: &[&FunctionBlockDeclaration],
    global_vars: &[VarDecl],
    functions: &FunctionEnvironment,
    types: &TypeEnvironment,
) -> Result<Container, Diagnostic> {
    let mut ctx = CompileContext::new();
    let mut builder = ContainerBuilder::new();

    // Assign global variable indices first (indices 0..G).
    assign_variables(&mut ctx, &mut builder, global_vars, types)?;
    let num_globals = ctx.variables.len() as u16;

    // Pre-scan user-defined FB declarations to register type metadata
    // (field indices, field op types, type IDs) before assign_variables runs.
    // This allows assign_variables to resolve user-defined FB instance variables.
    // The actual FB body compilation happens after program-local variables are
    // assigned, once var_offsets are known.
    let mut compiled_fb_bodies: Vec<CompiledFunction> = Vec::new();
    let mut next_function_id: u16 = 2;

    for fb_decl in fb_decls {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        let mut field_indices: HashMap<String, u8> = HashMap::new();
        let mut field_op_types: HashMap<String, OpType> = HashMap::new();
        let mut field_decls_tmp: Vec<&VarDecl> = Vec::new();

        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Input {
                field_decls_tmp.push(decl);
            }
        }
        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Output {
                field_decls_tmp.push(decl);
            }
        }
        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Var {
                field_decls_tmp.push(decl);
            }
        }
        for (i, decl) in field_decls_tmp.iter().enumerate() {
            if let Some(id) = decl.identifier.symbolic_id() {
                let name = id.to_string().to_lowercase();
                field_indices.insert(name.clone(), i as u8);
                if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
                    if let Some(vti) = resolve_type_name(&simple.type_name.name) {
                        field_op_types.insert(name, (vti.op_width, vti.signedness));
                    } else {
                        field_op_types.insert(name, DEFAULT_OP_TYPE);
                    }
                } else {
                    field_op_types.insert(name, DEFAULT_OP_TYPE);
                }
            }
        }

        let type_id = ctx.next_user_fb_type_id;
        ctx.next_user_fb_type_id += 1;
        ctx.user_fb_types.insert(
            fb_name,
            UserFbTypeInfo {
                type_id,
                num_fields: field_decls_tmp.len(),
                field_indices,
                function_id: next_function_id,
                var_offset: 0, // updated after program vars are assigned
                field_op_types,
            },
        );
        next_function_id += 1;
    }

    // Collect program-local variables, skipping VAR_EXTERNAL declarations
    // since they alias the corresponding global variables.
    let local_vars: Vec<VarDecl> = program
        .variables
        .iter()
        .filter(|v| v.var_type != VariableType::External)
        .cloned()
        .collect();

    // Assign program-local variable indices (indices G..N).
    // This can now resolve user-defined FB instances via ctx.user_fb_types.
    assign_variables(&mut ctx, &mut builder, &local_vars, types)?;
    let program_var_count = ctx.variables.len() as u16;

    // Now compile the FB bodies with correct var_offsets.
    let mut compiled_functions = Vec::new();
    let mut next_function_id: u16 = 2;
    let mut var_offset = VarIndex::new(program_var_count);

    for fb_decl in fb_decls {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        let fb_func_id = ctx.user_fb_types[&fb_name].function_id;

        // Update the var_offset in the registered type info.
        ctx.user_fb_types.get_mut(&fb_name).unwrap().var_offset = var_offset.raw();

        let compiled = compile_user_function_block(
            fb_decl,
            fb_func_id,
            var_offset.raw(),
            &mut ctx,
            &mut builder,
            types,
            num_globals,
        )?;
        var_offset = VarIndex::new(var_offset.raw() + compiled.num_locals);
        compiled_fb_bodies.push(compiled);
    }

    for func_decl in func_decls {
        let compiled = compile_user_function(
            func_decl,
            next_function_id,
            var_offset,
            &mut ctx,
            functions,
            &mut builder,
            types,
            num_globals,
        )?;
        var_offset = VarIndex::new(var_offset.raw() + compiled.num_locals);
        next_function_id += 1;
        compiled_functions.push(compiled);
    }

    let total_variables = var_offset;

    // Emit bytecode for variable initial values into the init emitter.
    let mut init_emitter = Emitter::new();
    emit_initial_values(&mut init_emitter, &mut ctx, global_vars, types)?;
    emit_initial_values(&mut init_emitter, &mut ctx, &local_vars, types)?;
    init_emitter.emit_ret_void();

    // Compile the program body into the scan emitter.
    let mut scan_emitter = Emitter::new();
    compile_body(&mut scan_emitter, &mut ctx, &program.body)?;
    scan_emitter.emit_ret_void();

    // Build the container.
    builder = builder.num_variables(total_variables.raw());

    // Configure data region for STRING variables.
    if ctx.data_region_offset > 0 {
        builder = builder.data_region_bytes(ctx.data_region_offset);
        if ctx.num_temp_bufs > 0 {
            builder = builder
                .num_temp_bufs(ctx.num_temp_bufs)
                .max_temp_buf_bytes((STRING_HEADER_BYTES as u16 + ctx.max_string_capacity) as u32);
        }
    }

    // Add constants to the pool.
    for constant in &ctx.constants {
        match constant {
            PoolConstant::I32(v) => builder = builder.add_i32_constant(*v),
            PoolConstant::I64(v) => builder = builder.add_i64_constant(*v),
            PoolConstant::F32(v) => builder = builder.add_f32_constant(*v),
            PoolConstant::F64(v) => builder = builder.add_f64_constant(*v),
            PoolConstant::Str(v) => builder = builder.add_str_constant(v),
        }
    }

    // Compute the max stack depth needed by any user-defined FB body.
    // The scan function's reported max_stack_depth must include the FB body's
    // depth because FB_CALL recursively enters execute() on the shared stack.
    let max_fb_body_stack: u16 = compiled_fb_bodies
        .iter()
        .map(|c| c.max_stack_depth)
        .max()
        .unwrap_or(0);

    // Function 0: init, Function 1: scan
    let init_stack = init_emitter.max_stack_depth();
    let init_bytecode = init_emitter.bytecode();
    builder = builder.add_function(
        FunctionId::INIT,
        init_bytecode,
        init_stack,
        program_var_count,
        0,
    );

    let scan_stack = scan_emitter.max_stack_depth() + max_fb_body_stack;
    let scan_bytecode = scan_emitter.bytecode();
    builder = builder.add_function(
        FunctionId::SCAN,
        scan_bytecode,
        scan_stack,
        program_var_count,
        0,
    );

    // Add user-defined function block bodies.
    for compiled in &compiled_fb_bodies {
        builder = builder.add_function(
            FunctionId::new(compiled.function_id),
            &compiled.bytecode,
            compiled.max_stack_depth,
            compiled.num_locals,
            compiled.num_params,
        );
    }

    // Add user FB type descriptors to the container.
    for fb_info in ctx.user_fb_types.values() {
        builder = builder.add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(fb_info.type_id),
            function_id: FunctionId::new(fb_info.function_id),
            var_offset: fb_info.var_offset,
            num_fields: fb_info.num_fields as u8,
        });
    }

    // Add user-defined functions.
    for compiled in &compiled_functions {
        builder = builder.add_function(
            FunctionId::new(compiled.function_id),
            &compiled.bytecode,
            compiled.max_stack_depth,
            compiled.num_locals,
            compiled.num_params,
        );
    }

    builder = builder
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .shared_globals_size(program_var_count);

    // Add debug info.
    let program_name = program.name.to_string();
    builder = builder
        .add_func_name(FuncNameEntry {
            function_id: FunctionId::INIT,
            name: format!("{program_name}_init"),
        })
        .add_func_name(FuncNameEntry {
            function_id: FunctionId::SCAN,
            name: program_name,
        });

    for compiled in &compiled_fb_bodies {
        builder = builder.add_func_name(FuncNameEntry {
            function_id: FunctionId::new(compiled.function_id),
            name: compiled.name.clone(),
        });
    }
    for compiled in &compiled_functions {
        builder = builder.add_func_name(FuncNameEntry {
            function_id: FunctionId::new(compiled.function_id),
            name: compiled.name.clone(),
        });
    }
    for entry in ctx.debug_var_names {
        builder = builder.add_var_name(entry);
    }

    Ok(builder.build())
}

/// Compiles a single user-defined function body.
///
/// Saves and restores the context's variable mappings so that function-local
/// variables don't interfere with the program's namespace.
///
/// Variable layout within the function's region (starting at `var_offset`):
/// - Input parameters (in declaration order)
/// - Local variables (VAR)
/// - Return value variable (named same as function)
#[allow(clippy::too_many_arguments)]
fn compile_user_function(
    func_decl: &FunctionDeclaration,
    function_id: u16,
    var_offset: VarIndex,
    ctx: &mut CompileContext,
    functions: &FunctionEnvironment,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    num_globals: u16,
) -> Result<CompiledFunction, Diagnostic> {
    // Save the program's variable mappings.
    let saved_variables = std::mem::take(&mut ctx.variables);
    let saved_var_types = std::mem::take(&mut ctx.var_types);
    let saved_string_vars = std::mem::take(&mut ctx.string_vars);
    let saved_array_vars = std::mem::take(&mut ctx.array_vars);
    let saved_struct_vars = std::mem::take(&mut ctx.struct_vars);

    // Re-insert global variable mappings so the function body can access them.
    for (id, index) in &saved_variables {
        if index.raw() < num_globals {
            ctx.variables.insert(id.clone(), *index);
        }
    }
    for (id, info) in &saved_var_types {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.var_types.insert(id.clone(), *info);
        }
    }
    for (id, info) in &saved_string_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.string_vars.insert(id.clone(), info.clone());
        }
    }
    for (id, info) in &saved_struct_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.struct_vars.insert(id.clone(), info.clone());
        }
    }

    // Assign variable slots for the function's parameters and locals,
    // starting at var_offset. Input parameters come first (declaration order),
    // then local variables.
    let mut current_index = var_offset;
    let mut num_params: u16 = 0;

    // First pass: input-compatible parameters (VAR_INPUT and VAR_IN_OUT)
    // must come first for CALL arg passing.
    for decl in &func_decl.variables {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), type_info);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP).
    for decl in &func_decl.variables {
        if !decl.var_type.is_local() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), type_info);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    // Assign the return variable (named same as the function).
    let return_var_index = current_index;
    let return_id = func_decl.name.clone();
    ctx.variables.insert(return_id.clone(), return_var_index);

    // Check if this function returns a STRING/WSTRING.
    let return_string_info = match &func_decl.return_type {
        FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
            let max_length = resolve_string_spec_max_length(spec)?;

            let data_offset = ctx.data_region_offset;
            let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
            ctx.data_region_offset = ctx
                .data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;

            if max_length > ctx.max_string_capacity {
                ctx.max_string_capacity = max_length;
            }

            ctx.string_vars.insert(
                return_id.clone(),
                StringVarInfo {
                    data_offset,
                    max_length,
                },
            );
            Some(StringReturnInfo {
                data_offset,
                max_length,
            })
        }
        FunctionReturnType::Named(_) => {
            let return_type_name = func_decl.return_type.to_type_name();
            if types.resolve_struct_type(&return_type_name).is_some() {
                // Return type is a struct — allocate data region space and
                // register the return variable so field assignments
                // (e.g. FUNC.X := val) work inside the function body.
                crate::compile_struct::allocate_struct_variable(
                    ctx,
                    builder,
                    types,
                    &return_type_name,
                    &return_id,
                    return_var_index,
                    &func_decl.name.span(),
                )?;
            } else if let Some(type_info) = resolve_type_name(&return_type_name.name) {
                ctx.var_types.insert(return_id.clone(), type_info);
            }
            None
        }
    };
    current_index = VarIndex::new(current_index.raw() + 1);

    let num_locals = current_index.raw() - var_offset.raw();

    // Determine return type's OpType.
    let return_type_name = func_decl.return_type.to_type_name();
    let return_op_type = resolve_type_name(&return_type_name.name)
        .map(|info| (info.op_width, info.signedness))
        .unwrap_or(DEFAULT_OP_TYPE);

    // Compile the function body.
    let mut func_emitter = Emitter::new();

    // Emit initialization prologue: IEC 61131-3 functions are stateless, so
    // local variables must be re-initialized on every call. The flat variable
    // table (ADR-0021) retains stale values between calls, so the prologue
    // resets non-parameter locals to their declared initial values (or zero).
    emit_function_local_prologue(
        &mut func_emitter,
        ctx,
        func_decl,
        return_var_index,
        return_op_type,
    )?;

    let body = ironplc_dsl::textual::Statements {
        body: func_decl.body.clone(),
    };
    compile_statements(&mut func_emitter, ctx, &body)?;

    // Load the return value and emit RET.
    if let Some(ref str_info) = return_string_info {
        // For STRING return: load the return string from the data region into
        // a temp buffer, leaving buf_idx on the stack for the caller.
        ctx.num_temp_bufs += 1;
        func_emitter.emit_str_load_var(str_info.data_offset);
    } else {
        emit_load_var(&mut func_emitter, return_var_index, return_op_type);
    }
    func_emitter.emit_ret();

    let max_stack_depth = func_emitter.max_stack_depth();
    let bytecode = func_emitter.bytecode().to_vec();

    // Record function metadata for use at call sites.
    let func_name = func_decl.name.lower_case();

    // Record parameter OpTypes and STRING info from the function's declarations.
    let mut param_op_types: Vec<OpType> = Vec::new();
    let mut param_string_info: Vec<Option<StringParamInfo>> = Vec::new();
    for decl in &func_decl.variables {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        match &decl.initializer {
            InitialValueAssignmentKind::String(_) => {
                param_op_types.push(DEFAULT_OP_TYPE);
                if let Some(id) = decl.identifier.symbolic_id() {
                    if let Some(info) = ctx.string_vars.get(id) {
                        param_string_info.push(Some(StringParamInfo {
                            data_offset: info.data_offset,
                            max_length: info.max_length,
                        }));
                    } else {
                        param_string_info.push(None);
                    }
                } else {
                    param_string_info.push(None);
                }
            }
            InitialValueAssignmentKind::Reference(_) => {
                param_op_types.push((OpWidth::W64, Signedness::Unsigned));
                param_string_info.push(None);
            }
            _ => {
                if let Some(sig) = functions.get(&func_decl.name) {
                    if let Some(param) = sig
                        .parameters
                        .iter()
                        .filter(|p| p.is_input)
                        .nth(param_op_types.len())
                    {
                        param_op_types.push(
                            resolve_type_name(&param.param_type.name)
                                .map(|info| (info.op_width, info.signedness))
                                .unwrap_or(DEFAULT_OP_TYPE),
                        );
                    } else {
                        param_op_types.push(DEFAULT_OP_TYPE);
                    }
                } else {
                    param_op_types.push(DEFAULT_OP_TYPE);
                }
                param_string_info.push(None);
            }
        }
    }

    ctx.user_functions.insert(
        func_name.to_string(),
        UserFunctionInfo {
            function_id,
            var_offset,
            num_params,
            param_op_types,
            param_string_info,
            return_string_info,
            max_stack_depth,
        },
    );

    // Restore the program's variable mappings.
    ctx.variables = saved_variables;
    ctx.var_types = saved_var_types;
    ctx.string_vars = saved_string_vars;
    ctx.array_vars = saved_array_vars;
    ctx.struct_vars = saved_struct_vars;

    Ok(CompiledFunction {
        function_id,
        bytecode,
        max_stack_depth,
        num_locals,
        num_params,
        name: func_name.to_string(),
    })
}

/// Compiles a single user-defined function block body.
///
/// Saves and restores the context's variable mappings so that FB-local
/// variables don't interfere with the program's namespace.
///
/// All FB fields (VAR_INPUT, VAR_OUTPUT, VAR) are mapped to contiguous
/// variable table slots starting at `var_offset`, in declaration order.
/// The VM's copy-in/copy-out logic mirrors this ordering.
fn compile_user_function_block(
    fb_decl: &FunctionBlockDeclaration,
    function_id: u16,
    var_offset: u16,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    _types: &TypeEnvironment,
    num_globals: u16,
) -> Result<CompiledFunction, Diagnostic> {
    let fb_name = fb_decl.name.name.to_string().to_uppercase();

    // Collect fields in a stable order: inputs first, then outputs, then locals.
    // This matches the data-region layout used by the VM's copy-in/copy-out.
    let mut field_decls: Vec<&VarDecl> = Vec::new();
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Input {
            field_decls.push(decl);
        }
    }
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Output {
            field_decls.push(decl);
        }
    }
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Var {
            field_decls.push(decl);
        }
    }

    // Save the program's variable mappings.
    let saved_variables = std::mem::take(&mut ctx.variables);
    let saved_var_types = std::mem::take(&mut ctx.var_types);
    let saved_string_vars = std::mem::take(&mut ctx.string_vars);
    let saved_array_vars = std::mem::take(&mut ctx.array_vars);
    let saved_struct_vars = std::mem::take(&mut ctx.struct_vars);
    let saved_fb_instances = std::mem::take(&mut ctx.fb_instances);

    // Re-insert global variable mappings so the FB body can access them.
    for (id, index) in &saved_variables {
        if index.raw() < num_globals {
            ctx.variables.insert(id.clone(), *index);
        }
    }
    for (id, info) in &saved_var_types {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.var_types.insert(id.clone(), *info);
        }
    }
    for (id, info) in &saved_string_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.string_vars.insert(id.clone(), info.clone());
        }
    }
    for (id, info) in &saved_struct_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.struct_vars.insert(id.clone(), info.clone());
        }
    }

    // Assign variable slots for all FB fields, in the same order as field_decls.
    let mut current_index = VarIndex::new(var_offset);
    for decl in &field_decls {
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(vti) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), vti);
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    let num_locals = current_index.raw() - var_offset;

    // Compile the FB body.
    let mut fb_emitter = Emitter::new();
    compile_body(&mut fb_emitter, ctx, &fb_decl.body)?;
    fb_emitter.emit_ret_void();

    let max_stack_depth = fb_emitter.max_stack_depth();
    let bytecode = fb_emitter.bytecode().to_vec();

    // Restore the program's variable mappings.
    ctx.variables = saved_variables;
    ctx.var_types = saved_var_types;
    ctx.string_vars = saved_string_vars;
    ctx.array_vars = saved_array_vars;
    ctx.struct_vars = saved_struct_vars;
    ctx.fb_instances = saved_fb_instances;

    Ok(CompiledFunction {
        function_id,
        bytecode,
        max_stack_depth,
        num_locals,
        num_params: 0,
        name: fb_name,
    })
}

/// Metadata for a STRING parameter in a user-defined function.
///
/// Used at call sites to copy the caller's string data into the function's
/// allocated data region space before executing the CALL instruction.
#[derive(Clone)]
pub(crate) struct StringParamInfo {
    /// Byte offset in the data region where this parameter's string is stored.
    pub(crate) data_offset: u32,
    /// Maximum number of characters this parameter can hold.
    pub(crate) max_length: u16,
}

/// Metadata for a STRING return value in a user-defined function.
///
/// When a function returns STRING/WSTRING, the return value lives in the
/// data region rather than on the operand stack.
#[derive(Clone)]
pub(crate) struct StringReturnInfo {
    /// Byte offset in the data region where the return string is stored.
    pub(crate) data_offset: u32,
    /// Maximum number of characters the return string can hold.
    pub(crate) max_length: u16,
}

/// Metadata for a compiled user-defined function.
#[derive(Clone)]
pub(crate) struct UserFunctionInfo {
    /// The function ID assigned in the container.
    pub(crate) function_id: u16,
    /// The absolute variable table offset where this function's parameters start.
    pub(crate) var_offset: VarIndex,
    /// Number of input parameters.
    pub(crate) num_params: u16,
    /// OpTypes for each input parameter, in declaration order.
    pub(crate) param_op_types: Vec<OpType>,
    /// For each input parameter (in order), `Some(info)` if it is a STRING
    /// parameter that needs copy-in at the call site, `None` for scalar params.
    pub(crate) param_string_info: Vec<Option<StringParamInfo>>,
    /// If the function returns STRING/WSTRING, info about the return string
    /// in the data region. Used at call sites to initialize the return string
    /// header before CALL.
    pub(crate) return_string_info: Option<StringReturnInfo>,
    /// Maximum stack depth used by this function's body.
    pub(crate) max_stack_depth: u16,
}

/// Tracks state during compilation of a single program.
/// Metadata for a function block instance variable.
pub(crate) struct FbInstanceInfo {
    /// Variable table index holding the data region offset.
    pub(crate) var_index: VarIndex,
    /// Type ID for FB_CALL dispatch.
    pub(crate) type_id: u16,
    /// Data region byte offset where this instance's fields start.
    pub(crate) data_offset: u32,
    /// Maps field name (lowercase) to field index.
    pub(crate) field_indices: HashMap<String, u8>,
}

/// Metadata for a compiled user-defined function block type.
pub(crate) struct UserFbTypeInfo {
    /// Unique type ID for FB_CALL dispatch (starts at 0x1000).
    pub(crate) type_id: u16,
    /// Number of data-region fields in each instance.
    pub(crate) num_fields: usize,
    /// Maps field name (lowercase) to field index (ordinal position).
    pub(crate) field_indices: HashMap<String, u8>,
    /// Function ID of the compiled FB body in the container.
    pub(crate) function_id: u16,
    /// Variable table offset where the FB body's slots start.
    pub(crate) var_offset: u16,
    /// Maps field name (lowercase) to its op type for codegen at call sites.
    pub(crate) field_op_types: HashMap<String, OpType>,
}

pub(crate) struct CompileContext {
    /// Maps variable identifiers to their variable table indices.
    pub(crate) variables: HashMap<Id, VarIndex>,
    /// Maps variable identifiers to their type information.
    pub(crate) var_types: HashMap<Id, VarTypeInfo>,
    /// Ordered list of constants added to the constant pool.
    constants: Vec<PoolConstant>,
    /// Stack of loop exit labels for EXIT statement compilation.
    /// Each enclosing loop pushes its end label; EXIT jumps to the top.
    pub(crate) loop_exit_labels: Vec<crate::emit::Label>,
    /// Maps STRING variable identifiers to their data region metadata.
    pub(crate) string_vars: HashMap<Id, StringVarInfo>,
    /// Maps FB instance variable identifiers to their metadata.
    pub(crate) fb_instances: HashMap<Id, FbInstanceInfo>,
    /// Maps array variable identifiers to their metadata.
    pub(crate) array_vars: HashMap<Id, crate::compile_array::ArrayVarInfo>,
    /// Maps structure variable identifiers to their metadata.
    pub(crate) struct_vars: HashMap<Id, crate::compile_struct::StructVarInfo>,
    /// Next available byte offset in the data region.
    pub(crate) data_region_offset: u32,
    /// Maximum string capacity across all STRING variables (for temp buffer sizing).
    pub(crate) max_string_capacity: u16,
    /// Number of temp buffers needed (one per string load in the init function).
    pub(crate) num_temp_bufs: u16,
    /// Debug info: variable name entries collected during assign_variables.
    debug_var_names: Vec<VarNameEntry>,
    /// Maps user-defined function name (lowercase) to compilation metadata.
    pub(crate) user_functions: HashMap<String, UserFunctionInfo>,
    /// Maps user-defined FB type name (uppercase) to compilation metadata.
    pub(crate) user_fb_types: HashMap<String, UserFbTypeInfo>,
    /// Next available type ID for user-defined function blocks.
    next_user_fb_type_id: u16,
}

impl CompileContext {
    fn new() -> Self {
        CompileContext {
            variables: HashMap::new(),
            var_types: HashMap::new(),
            constants: Vec::new(),
            loop_exit_labels: Vec::new(),
            string_vars: HashMap::new(),
            fb_instances: HashMap::new(),
            array_vars: HashMap::new(),
            struct_vars: HashMap::new(),
            data_region_offset: 0,
            max_string_capacity: 0,
            num_temp_bufs: 0,
            debug_var_names: Vec::new(),
            user_functions: HashMap::new(),
            user_fb_types: HashMap::new(),
            next_user_fb_type_id: 0x1000,
        }
    }

    /// Returns the exit label for the innermost enclosing loop, if any.
    pub(crate) fn current_loop_exit(&self) -> Option<crate::emit::Label> {
        self.loop_exit_labels.last().copied()
    }

    /// Looks up a variable index by identifier, using the provided span for error reporting.
    pub(crate) fn var_index(&self, name: &Id) -> Result<VarIndex, Diagnostic> {
        self.variables.get(name).copied().ok_or_else(|| {
            Diagnostic::problem(
                Problem::VariableUndefined,
                Label::span(name.span(), "Variable reference"),
            )
            .with_context("variable", &name.to_string())
        })
    }

    /// Looks up type information for a variable by identifier.
    pub(crate) fn var_type_info(&self, name: &Id) -> Option<VarTypeInfo> {
        self.var_types.get(name).copied()
    }

    /// Returns the op_type for a variable by identifier, falling back to defaults.
    pub(crate) fn var_op_type(&self, name: &Id) -> OpType {
        self.var_types
            .get(name)
            .map(|info| (info.op_width, info.signedness))
            .unwrap_or(DEFAULT_OP_TYPE)
    }

    /// Allocates a scratch variable with a synthetic name and returns its index.
    ///
    /// The name uses a `$` prefix which is illegal in IEC 61131-3 identifiers,
    /// guaranteeing no collision with user-defined variables.
    pub(crate) fn allocate_scratch_variable(&mut self, suffix: &str) -> VarIndex {
        let idx = VarIndex::new(self.variables.len() as u16);
        self.variables
            .insert(Id::from(&format!("$scratch_{}", suffix)), idx);
        idx
    }

    /// Adds an i32 constant to the pool and returns its index.
    pub(crate) fn add_i32_constant(&mut self, value: i32) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::I32(v) = existing {
                if *v == value {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::I32(value));
        index
    }

    /// Adds an f32 constant to the pool and returns its index.
    pub(crate) fn add_f32_constant(&mut self, value: f32) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::F32(v) = existing {
                if v.to_bits() == value.to_bits() {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::F32(value));
        index
    }

    /// Adds an f64 constant to the pool and returns its index.
    pub(crate) fn add_f64_constant(&mut self, value: f64) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::F64(v) = existing {
                if v.to_bits() == value.to_bits() {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::F64(value));
        index
    }

    /// Adds an i64 constant to the pool and returns its index.
    pub(crate) fn add_i64_constant(&mut self, value: i64) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::I64(v) = existing {
                if *v == value {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::I64(value));
        index
    }

    /// Adds a string constant (raw bytes) to the pool and returns its index.
    pub(crate) fn add_str_constant(&mut self, value: Vec<u8>) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::Str(v) = existing {
                if *v == value {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::Str(value));
        index
    }
}

/// Assigns variable table indices and type info for all variable declarations.
fn assign_variables(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    declarations: &[VarDecl],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            let index = VarIndex::new(ctx.variables.len() as u16);
            ctx.variables.insert(id.clone(), index);

            // Resolve type info and collect debug metadata.
            let (type_tag, type_name_str) = match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    // The global_var_decl parser produces Simple for all named
                    // types, including structs.  Detect struct types via the
                    // type environment and register them properly so that field
                    // access works in codegen.
                    if types.resolve_struct_type(&simple.type_name).is_some() {
                        crate::compile_struct::allocate_struct_variable(
                            ctx,
                            builder,
                            types,
                            &simple.type_name,
                            id,
                            index,
                            &decl.identifier.span(),
                        )?;
                        let type_name_str = simple.type_name.to_string().to_uppercase();
                        (iec_type_tag::OTHER, type_name_str)
                    } else {
                        if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                            ctx.var_types.insert(id.clone(), type_info);
                        }
                        let tag = resolve_iec_type_tag(&simple.type_name.name);
                        let name = simple.type_name.name.to_string().to_uppercase();
                        (tag, name)
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    // Allocate space in the data region: [max_length: u16][cur_length: u16][data]
                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                    (iec_type_tag::STRING, "STRING".into())
                }
                InitialValueAssignmentKind::FunctionBlock(fb_init) => {
                    let fb_name = fb_init.type_name.to_string().to_uppercase();
                    if let Some((type_id, num_fields, field_map)) = resolve_fb_type(&fb_name) {
                        // Standard library function block.
                        let instance_size = num_fields as u32 * 8;
                        let data_offset = ctx.data_region_offset;
                        ctx.data_region_offset = ctx
                            .data_region_offset
                            .checked_add(instance_size)
                            .ok_or_else(|| {
                                Diagnostic::problem(
                                    Problem::NotImplemented,
                                    Label::span(decl.identifier.span(), "Data region overflow"),
                                )
                            })?;

                        ctx.fb_instances.insert(
                            id.clone(),
                            FbInstanceInfo {
                                var_index: index,
                                type_id,
                                data_offset,
                                field_indices: field_map,
                            },
                        );
                    } else if let Some(user_fb) = ctx.user_fb_types.get(&fb_name) {
                        // User-defined function block.
                        let instance_size = user_fb.num_fields as u32 * 8;
                        let data_offset = ctx.data_region_offset;
                        ctx.data_region_offset = ctx
                            .data_region_offset
                            .checked_add(instance_size)
                            .ok_or_else(|| {
                                Diagnostic::problem(
                                    Problem::NotImplemented,
                                    Label::span(decl.identifier.span(), "Data region overflow"),
                                )
                            })?;

                        ctx.fb_instances.insert(
                            id.clone(),
                            FbInstanceInfo {
                                var_index: index,
                                type_id: user_fb.type_id,
                                data_offset,
                                field_indices: user_fb.field_indices.clone(),
                            },
                        );
                    }
                    (iec_type_tag::OTHER, fb_name)
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    let spec = match &array_init.spec {
                        SpecificationKind::Inline(array_subranges) => {
                            crate::compile_array::array_spec_from_inline(
                                array_subranges,
                                &decl.identifier.span(),
                            )?
                        }
                        SpecificationKind::Named(type_name) => {
                            let array_type =
                                types.resolve_array_type(type_name).ok_or_else(|| {
                                    Diagnostic::problem(
                                        Problem::NotImplemented,
                                        Label::span(type_name.span(), "Unknown array type"),
                                    )
                                })?;
                            let IntermediateType::Array {
                                element_type,
                                dimensions,
                            } = array_type
                            else {
                                unreachable!("resolve_array_type guarantees Array variant");
                            };
                            crate::compile_array::array_spec_from_named(element_type, dimensions)?
                        }
                    };
                    crate::compile_array::register_array_variable(
                        ctx,
                        builder,
                        id,
                        index,
                        &spec,
                        &decl.identifier.span(),
                    )?
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    // References are stored as 64-bit variable-table indices (unsigned).
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx, builder, id, index, ref_init,
                    )?;
                    (iec_type_tag::OTHER, "REF_TO".into())
                }
                InitialValueAssignmentKind::Structure(struct_init) => {
                    crate::compile_struct::allocate_struct_variable(
                        ctx,
                        builder,
                        types,
                        &struct_init.type_name,
                        id,
                        index,
                        &decl.identifier.span(),
                    )?;
                    let type_name_str = struct_init.type_name.to_string().to_uppercase();
                    (iec_type_tag::OTHER, type_name_str)
                }
                InitialValueAssignmentKind::LateResolvedType(_) => {
                    // LateResolvedType should have been resolved before codegen.
                    // If we reach here, it indicates a bug in the compiler.
                    return Err(Diagnostic::internal_error(file!(), line!()));
                }
                // Other initializer kinds (EnumeratedType, etc.)
                // do not yet have type info tracked in codegen.
                _ => (iec_type_tag::OTHER, String::new()),
            };

            ctx.debug_var_names.push(VarNameEntry {
                var_index: index,
                function_id: function_id::GLOBAL_SCOPE,
                var_section: map_var_section(&decl.var_type),
                iec_type_tag: type_tag,
                name: id.to_string(),
                type_name: type_name_str,
            });
        }
    }
    Ok(())
}

/// Maps a DSL VariableType to the debug section var_section encoding.
fn map_var_section(vt: &VariableType) -> u8 {
    match vt {
        VariableType::Var => var_section::VAR,
        VariableType::VarTemp => var_section::VAR_TEMP,
        VariableType::Input => var_section::VAR_INPUT,
        VariableType::Output => var_section::VAR_OUTPUT,
        VariableType::InOut => var_section::VAR_IN_OUT,
        VariableType::External => var_section::VAR_EXTERNAL,
        VariableType::Global => var_section::VAR_GLOBAL,
        VariableType::Access => var_section::VAR,
    }
}

/// Maps an IEC 61131-3 type name to its debug type tag.
fn resolve_iec_type_tag(name: &Id) -> u8 {
    match ElementaryTypeName::try_from(name) {
        Ok(elem) => match elem {
            ElementaryTypeName::BOOL => iec_type_tag::BOOL,
            ElementaryTypeName::SINT => iec_type_tag::SINT,
            ElementaryTypeName::INT => iec_type_tag::INT,
            ElementaryTypeName::DINT => iec_type_tag::DINT,
            ElementaryTypeName::LINT => iec_type_tag::LINT,
            ElementaryTypeName::USINT => iec_type_tag::USINT,
            ElementaryTypeName::UINT => iec_type_tag::UINT,
            ElementaryTypeName::UDINT => iec_type_tag::UDINT,
            ElementaryTypeName::ULINT => iec_type_tag::ULINT,
            ElementaryTypeName::REAL => iec_type_tag::REAL,
            ElementaryTypeName::LREAL => iec_type_tag::LREAL,
            ElementaryTypeName::BYTE => iec_type_tag::BYTE,
            ElementaryTypeName::WORD => iec_type_tag::WORD,
            ElementaryTypeName::DWORD => iec_type_tag::DWORD,
            ElementaryTypeName::LWORD => iec_type_tag::LWORD,
            ElementaryTypeName::STRING => iec_type_tag::STRING,
            ElementaryTypeName::WSTRING => iec_type_tag::WSTRING,
            ElementaryTypeName::TIME => iec_type_tag::TIME,
            ElementaryTypeName::LTIME => iec_type_tag::LTIME,
            ElementaryTypeName::DATE => iec_type_tag::DATE,
            ElementaryTypeName::LDATE => iec_type_tag::LDATE,
            ElementaryTypeName::TimeOfDay => iec_type_tag::TIME_OF_DAY,
            ElementaryTypeName::LTimeOfDay => iec_type_tag::LTOD,
            ElementaryTypeName::DateAndTime => iec_type_tag::DATE_AND_TIME,
            ElementaryTypeName::LDateAndTime => iec_type_tag::LDT,
        },
        Err(()) => iec_type_tag::OTHER,
    }
}

/// Emits bytecode to initialize variables that have declared initial values.
///
/// For scalar variables with a `SimpleInitializer`, emits load-constant +
/// truncate (if narrow) + store-variable instructions.
///
/// For STRING variables, emits STR_INIT to set up the data region header,
/// then optionally LOAD_CONST_STR + STR_STORE_VAR for the initial value.
fn emit_initial_values(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    declarations: &[VarDecl],
    _types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    // The global_var_decl parser produces Simple for all
                    // named types, including structs.  If the variable was
                    // registered as a struct during assign_variables,
                    // initialize it like a Structure initializer.
                    if let Some(struct_info) = ctx.struct_vars.get(id) {
                        let data_offset = struct_info.data_offset;
                        let var_index = struct_info.var_index;
                        let desc_index = struct_info.desc_index;
                        let fields: Vec<_> = struct_info
                            .fields
                            .iter()
                            .map(|f| crate::compile_struct::FieldInitInfo {
                                name: f.name.clone(),
                                slot_offset: f.slot_offset,
                                field_type: f.field_type.clone(),
                                op_type: f.op_type,
                                string_max_length: f.string_max_length,
                            })
                            .collect();

                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        crate::compile_struct::initialize_struct_fields(
                            emitter,
                            ctx,
                            var_index,
                            desc_index,
                            data_offset,
                            &fields,
                            &[],
                        )?;
                    } else if let Some(constant) = &simple.initial_value {
                        let var_index = ctx.var_index(id)?;
                        let type_info = ctx.var_type_info(id);
                        let op_type = type_info
                            .map(|ti| (ti.op_width, ti.signedness))
                            .unwrap_or(DEFAULT_OP_TYPE);

                        compile_constant(emitter, ctx, constant, op_type)?;

                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }

                        emit_store_var(emitter, var_index, op_type);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    if let Some(info) = ctx.string_vars.get(id) {
                        let data_offset = info.data_offset;
                        let max_length = info.max_length;

                        // Initialize the string header in the data region.
                        emitter.emit_str_init(data_offset, max_length);

                        // If there's an initial value, load and store it.
                        if let Some(chars) = &string_init.initial_value {
                            // Convert chars to Latin-1 bytes (STRING encoding per ADR-0016).
                            let bytes: Vec<u8> = chars.iter().map(|&ch| ch as u8).collect();
                            let pool_index = ctx.add_str_constant(bytes);
                            ctx.num_temp_bufs += 1;
                            emitter.emit_load_const_str(pool_index);
                            emitter.emit_str_store_var(data_offset);
                        }
                    }
                }
                InitialValueAssignmentKind::FunctionBlock(_) => {
                    if let Some(fb_info) = ctx.fb_instances.get(id) {
                        let data_offset = fb_info.data_offset;
                        let var_index = fb_info.var_index;
                        // Store the data region byte offset into the variable slot.
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);
                    }
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    if let Some(array_info) = ctx.array_vars.get(id) {
                        let data_offset = array_info.data_offset;
                        let var_index = array_info.var_index;
                        let desc_index = array_info.desc_index;
                        let element_vti = array_info.element_var_type_info;
                        let is_string = array_info.is_string_element;

                        // Store data_offset into the variable slot (like FB instances).
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        if is_string {
                            // Initialize all string headers in the array.
                            emitter.emit_str_init_array(var_index, desc_index);

                            // Emit STR_STORE_ARRAY_ELEM for each initial string value.
                            if !array_init.initial_values.is_empty() {
                                let values = crate::compile_array::flatten_array_initial_values(
                                    &array_init.initial_values,
                                )?;
                                for (i, value) in values.iter().enumerate() {
                                    compile_constant(emitter, ctx, value, DEFAULT_OP_TYPE)?;
                                    let idx_const = ctx.add_i32_constant(i as i32);
                                    emitter.emit_load_const_i32(idx_const);
                                    emitter.emit_str_store_array_elem(var_index, desc_index);
                                }
                            }
                        } else {
                            // Emit STORE_ARRAY for each initial value.
                            if !array_init.initial_values.is_empty() {
                                let values = crate::compile_array::flatten_array_initial_values(
                                    &array_init.initial_values,
                                )?;
                                let element_op_type =
                                    (element_vti.op_width, element_vti.signedness);
                                for (i, value) in values.iter().enumerate() {
                                    compile_constant(emitter, ctx, value, element_op_type)?;
                                    emit_truncation(emitter, element_vti);
                                    let idx_const = ctx.add_i32_constant(i as i32);
                                    emitter.emit_load_const_i32(idx_const);
                                    emitter.emit_store_array(var_index, desc_index);
                                }
                            }
                        }
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    let var_index = ctx.var_index(id)?;
                    match &ref_init.initial_value {
                        Some(ReferenceInitialValue::Ref(target_var)) => {
                            // REF(var) → load the target variable's index as a u64 constant.
                            let target_index = resolve_variable(ctx, target_var)?;
                            let pool_index = ctx.add_i64_constant(target_index.into());
                            emitter.emit_load_const_i64(pool_index);
                        }
                        _ => {
                            // NULL or no initializer → store null sentinel (u64::MAX).
                            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
                            emitter.emit_load_const_i64(pool_index);
                        }
                    }
                    emitter.emit_store_var_i64(var_index);
                }
                InitialValueAssignmentKind::Structure(struct_init) => {
                    if let Some(struct_info) = ctx.struct_vars.get(id) {
                        // Extract needed values before mutable borrow of ctx.
                        let data_offset = struct_info.data_offset;
                        let var_index = struct_info.var_index;
                        let desc_index = struct_info.desc_index;
                        let fields: Vec<_> = struct_info
                            .fields
                            .iter()
                            .map(|f| crate::compile_struct::FieldInitInfo {
                                name: f.name.clone(),
                                slot_offset: f.slot_offset,
                                field_type: f.field_type.clone(),
                                op_type: f.op_type,
                                string_max_length: f.string_max_length,
                            })
                            .collect();

                        // Store data_offset into the variable slot
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        // Initialize each field
                        crate::compile_struct::initialize_struct_fields(
                            emitter,
                            ctx,
                            var_index,
                            desc_index,
                            data_offset,
                            &fields,
                            &struct_init.elements_init,
                        )?;
                    }
                }
                // Other initializer kinds (EnumeratedType, etc.)
                // do not yet support initial values in codegen.
                _ => {}
            }
        }
    }
    Ok(())
}

/// Emits a bytecode prologue that re-initializes a function's non-parameter
/// local variables and return variable on every call. IEC 61131-3 requires
/// functions to be stateless (locals must not retain values between calls).
///
/// For locals with a declared initial value, emits the same LOAD_CONST +
/// TRUNC + STORE_VAR sequence that `emit_initial_values()` uses. For locals
/// without an initializer and for the return variable, emits a zero-store.
fn emit_function_local_prologue(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func_decl: &FunctionDeclaration,
    return_var_index: VarIndex,
    return_op_type: OpType,
) -> Result<(), Diagnostic> {
    // Re-initialize VAR locals (not Input parameters).
    for decl in &func_decl.variables {
        if decl.var_type != VariableType::Var {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            let var_index = ctx.var_index(id)?;
            let type_info = ctx.var_type_info(id);
            let op_type = type_info
                .map(|ti| (ti.op_width, ti.signedness))
                .unwrap_or(DEFAULT_OP_TYPE);

            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(constant) = &simple.initial_value {
                        // Has an explicit initial value: emit LOAD_CONST + TRUNC + STORE.
                        compile_constant(emitter, ctx, constant, op_type)?;
                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }
                    } else {
                        // No initializer: zero-fill.
                        emit_zero_const(emitter, ctx, op_type);
                    }
                    emit_store_var(emitter, var_index, op_type);
                }
                InitialValueAssignmentKind::String(string_init) => {
                    // Re-initialize STRING locals: emit STR_INIT to reset the
                    // header, then optionally load the initial value.
                    if let Some(info) = ctx.string_vars.get(id) {
                        let data_offset = info.data_offset;
                        let max_length = info.max_length;
                        emitter.emit_str_init(data_offset, max_length);

                        if let Some(chars) = &string_init.initial_value {
                            let bytes: Vec<u8> = chars.iter().map(|&ch| ch as u8).collect();
                            let pool_index = ctx.add_str_constant(bytes);
                            ctx.num_temp_bufs += 1;
                            emitter.emit_load_const_str(pool_index);
                            emitter.emit_str_store_var(data_offset);
                        }
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    match &ref_init.initial_value {
                        Some(ReferenceInitialValue::Ref(target_var)) => {
                            let target_index = resolve_variable(ctx, target_var)?;
                            let pool_index = ctx.add_i64_constant(target_index.into());
                            emitter.emit_load_const_i64(pool_index);
                        }
                        _ => {
                            // NULL or no initializer: store null sentinel (u64::MAX).
                            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
                            emitter.emit_load_const_i64(pool_index);
                        }
                    }
                    emitter.emit_store_var_i64(var_index);
                }
                _ => {
                    // Other initializer kinds (FunctionBlock, etc.)
                    // are not expected in function locals; zero-fill as default.
                    emit_zero_const(emitter, ctx, op_type);
                    emit_store_var(emitter, var_index, op_type);
                }
            }
        }
    }

    // Zero-initialize the return variable.
    if let Some(struct_info) = ctx.struct_vars.get(&func_decl.name).cloned() {
        // Struct return: store data_offset into the return var slot and
        // zero all struct fields. Functions are stateless, so the struct
        // must be re-initialized on every call.
        let offset_const = ctx.add_i32_constant(struct_info.data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(return_var_index);

        let fields: Vec<_> = struct_info
            .fields
            .iter()
            .map(|f| crate::compile_struct::FieldInitInfo {
                name: f.name.clone(),
                slot_offset: f.slot_offset,
                field_type: f.field_type.clone(),
                op_type: f.op_type,
                string_max_length: f.string_max_length,
            })
            .collect();

        crate::compile_struct::initialize_struct_fields(
            emitter,
            ctx,
            return_var_index,
            struct_info.desc_index,
            struct_info.data_offset,
            &fields,
            &[],
        )?;
    } else if let Some(info) = ctx.string_vars.get(&func_decl.name) {
        // STRING return: initialize the string header in the data region.
        emitter.emit_str_init(info.data_offset, info.max_length);
    } else {
        emit_zero_const(emitter, ctx, return_op_type);
        emit_store_var(emitter, return_var_index, return_op_type);
    }

    Ok(())
}

/// Emits a LOAD_CONST instruction that pushes a zero value of the given type.
pub(crate) fn emit_zero_const(emitter: &mut Emitter, ctx: &mut CompileContext, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => {
            let pool_index = ctx.add_i32_constant(0);
            emitter.emit_load_const_i32(pool_index);
        }
        OpWidth::W64 => {
            let pool_index = ctx.add_i64_constant(0);
            emitter.emit_load_const_i64(pool_index);
        }
        OpWidth::F32 => {
            let pool_index = ctx.add_f32_constant(0.0);
            emitter.emit_load_const_f32(pool_index);
        }
        OpWidth::F64 => {
            let pool_index = ctx.add_f64_constant(0.0);
            emitter.emit_load_const_f64(pool_index);
        }
    }
}

/// Maps an IEC 61131-3 type name to its `VarTypeInfo`.
///
/// Returns `None` for unrecognized type names (e.g., user-defined types)
/// and for STRING/WSTRING which are handled separately.
pub(crate) fn resolve_type_name(name: &Id) -> Option<VarTypeInfo> {
    // Try as elementary type first (the common case), then fall back to
    // generic types mapped to their default concrete representation.
    // Generic types may reach codegen for expressions like `5 + 5` where
    // no concrete type context was available during type resolution.
    let elem = ElementaryTypeName::try_from(name)
        .or_else(|_| match GenericTypeName::try_from(name)? {
            GenericTypeName::AnyInt | GenericTypeName::AnyNum | GenericTypeName::AnyMagnitude => {
                Ok(ElementaryTypeName::DINT)
            }
            GenericTypeName::AnyReal => Ok(ElementaryTypeName::REAL),
            _ => Err(()),
        })
        .ok()?;
    match elem {
        ElementaryTypeName::SINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 8,
        }),
        ElementaryTypeName::INT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 16,
        }),
        ElementaryTypeName::DINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LINT => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::USINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        ElementaryTypeName::UINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        ElementaryTypeName::UDINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::ULINT => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::REAL => Some(VarTypeInfo {
            op_width: OpWidth::F32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LREAL => Some(VarTypeInfo {
            op_width: OpWidth::F64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::BOOL => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 1,
        }),
        ElementaryTypeName::BYTE => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        ElementaryTypeName::WORD => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        ElementaryTypeName::DWORD => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::LWORD => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::TIME => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LTIME => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::DATE => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::TimeOfDay => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::DateAndTime => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::LDATE => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::LTimeOfDay => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::LDateAndTime => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        // STRING and WSTRING are handled separately in codegen
        ElementaryTypeName::STRING | ElementaryTypeName::WSTRING => None,
    }
}

/// Resolves a standard FB type name to its (type_id, total_num_fields, field_name->index map).
/// Returns None for unknown FB types.
fn resolve_fb_type(name: &str) -> Option<(u16, usize, HashMap<String, u8>)> {
    match name {
        "TON" => Some((opcode::fb_type::TON, 6, timer_fb_fields())),
        "TOF" => Some((opcode::fb_type::TOF, 6, timer_fb_fields())),
        "TP" => Some((opcode::fb_type::TP, 6, timer_fb_fields())),
        "CTU" | "CTU_INT" | "CTU_DINT" | "CTU_LINT" | "CTU_UDINT" | "CTU_ULINT" => {
            Some((opcode::fb_type::CTU, 6, ctu_fb_fields()))
        }
        "CTD" | "CTD_INT" | "CTD_DINT" | "CTD_LINT" | "CTD_UDINT" | "CTD_ULINT" => {
            Some((opcode::fb_type::CTD, 6, ctd_fb_fields()))
        }
        "CTUD" | "CTUD_INT" | "CTUD_DINT" | "CTUD_LINT" | "CTUD_UDINT" | "CTUD_ULINT" => {
            Some((opcode::fb_type::CTUD, 10, ctud_fb_fields()))
        }
        "SR" => Some((opcode::fb_type::SR, 3, sr_fb_fields())),
        "RS" => Some((opcode::fb_type::RS, 3, rs_fb_fields())),
        "R_TRIG" => Some((opcode::fb_type::R_TRIG, 3, edge_trig_fb_fields())),
        "F_TRIG" => Some((opcode::fb_type::F_TRIG, 3, edge_trig_fb_fields())),
        _ => None,
    }
}

/// Returns the shared field map for timer FBs (TON, TOF, TP).
/// Fields 4-5 are hidden (start_time, running) and not included.
fn timer_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("in".to_string(), 0);
    fields.insert("pt".to_string(), 1);
    fields.insert("q".to_string(), 2);
    fields.insert("et".to_string(), 3);
    fields
}

/// Returns the field map for CTU (count up) FBs.
/// Field 5 is hidden (prev_cu) and not included.
fn ctu_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTD (count down) FBs.
/// Field 5 is hidden (prev_cd) and not included.
fn ctd_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cd".to_string(), 0);
    fields.insert("ld".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTUD (count up/down) FBs.
/// Fields 8-9 are hidden (prev_cu, prev_cd) and not included.
fn ctud_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("cd".to_string(), 1);
    fields.insert("r".to_string(), 2);
    fields.insert("ld".to_string(), 3);
    fields.insert("pv".to_string(), 4);
    fields.insert("qu".to_string(), 5);
    fields.insert("qd".to_string(), 6);
    fields.insert("cv".to_string(), 7);
    fields
}

/// Returns the field map for SR (set-reset) FBs.
fn sr_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s1".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for RS (reset-set) FBs.
fn rs_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s".to_string(), 0);
    fields.insert("r1".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for edge trigger FBs (R_TRIG, F_TRIG).
/// Field 2 is hidden (M / previous CLK) and not included.
fn edge_trig_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("clk".to_string(), 0);
    fields.insert("q".to_string(), 1);
    fields
}

/// Checks if a function name is a type conversion (e.g., "int_to_real").
/// Returns `Some((source_type_info, target_type_info))` if both parts are
/// recognized type names, `None` otherwise.
pub(crate) fn parse_type_conversion(name: &str) -> Option<(VarTypeInfo, VarTypeInfo)> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    let source = resolve_type_name(&Id::from(parts[0]))?;
    let target = resolve_type_name(&Id::from(parts[1]))?;
    Some((source, target))
}

/// Describes a string ↔ numeric conversion direction.
pub(crate) enum StringConversion {
    /// Numeric → STRING (e.g., INT_TO_STRING, DWORD_TO_STRING).
    NumToString { source: VarTypeInfo },
    /// STRING → Numeric (e.g., STRING_TO_INT, STRING_TO_REAL).
    StringToNum { target: VarTypeInfo },
}

/// Checks if a function name is a string conversion (e.g., "int_to_string").
///
/// Returns `Some(StringConversion)` if the name matches `*_TO_STRING` or
/// `STRING_TO_*` and the non-string part is a recognized type name.
pub(crate) fn parse_string_conversion(name: &str) -> Option<StringConversion> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    if parts[1] == "STRING" {
        let source = resolve_type_name(&Id::from(parts[0]))?;
        Some(StringConversion::NumToString { source })
    } else if parts[0] == "STRING" {
        let target = resolve_type_name(&Id::from(parts[1]))?;
        Some(StringConversion::StringToNum { target })
    } else {
        None
    }
}

/// Compiles a string ↔ numeric conversion function call.
pub(crate) fn compile_string_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    conv: StringConversion,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    match conv {
        StringConversion::NumToString { source } => {
            let source_op_type: OpType = (source.op_width, source.signedness);
            compile_expr(emitter, ctx, args[0], source_op_type)?;

            let func_id = match (source.op_width, source.signedness) {
                (OpWidth::W32, Signedness::Signed) => opcode::builtin::CONV_I32_TO_STR,
                (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::CONV_U32_TO_STR,
                (OpWidth::F32, _) => opcode::builtin::CONV_F32_TO_STR,
                _ => {
                    return Err(Diagnostic::todo_with_span(
                        func.name.span(),
                        file!(),
                        line!(),
                    ));
                }
            };
            emitter.emit_builtin(func_id);
            ctx.num_temp_bufs += 1;
            Ok(())
        }
        StringConversion::StringToNum { target } => {
            let data_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
            let pool_index = ctx.add_i32_constant(data_offset as i32);
            emitter.emit_load_const_i32(pool_index);

            let func_id = match target.op_width {
                OpWidth::W32 => opcode::builtin::CONV_STR_TO_I32,
                OpWidth::F32 => opcode::builtin::CONV_STR_TO_F32,
                _ => {
                    return Err(Diagnostic::todo_with_span(
                        func.name.span(),
                        file!(),
                        line!(),
                    ));
                }
            };
            emitter.emit_builtin(func_id);
            emit_truncation(emitter, target);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::CompilerOptions;
    use ironplc_parser::parse_program;

    use ironplc_analyzer::SemanticContext;

    /// Helper to parse and analyze an IEC 61131-3 program string into a Library.
    ///
    /// Runs the analyzer's type resolution pass so that `Expr.resolved_type` is
    /// populated, which codegen requires for control flow and bitwise operations.
    fn parse(source: &str) -> (Library, SemanticContext) {
        let library =
            parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (analyzed, ctx) =
            ironplc_analyzer::stages::resolve_types(&[&library], &CompilerOptions::default())
                .unwrap();
        (analyzed, ctx)
    }

    #[test]
    fn compile_when_simple_assignment_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(container.header.num_functions, 2);
        assert_eq!(
            container
                .constant_pool
                .get_i32(ironplc_container::ConstantIndex::new(0))
                .unwrap(),
            10
        );

        // Function 0: init (RET_VOID only, no initial values)
        let init_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(0))
            .unwrap();
        assert_eq!(init_bytecode, &[0xB5]);

        // Function 1: scan — LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0, RET_VOID
        let scan_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(scan_bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn compile_when_no_program_then_p4020_error() {
        let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK
";
        let (library, context) = parse(source);
        let result = compile(&library, &context, &CodegenOptions::default());

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::NoProgramDeclaration.code());
    }

    #[test]
    fn compile_when_empty_program_then_produces_ret_void() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        // Should have two functions (init + scan), both just RET_VOID
        assert_eq!(container.header.num_functions, 2);
        let init_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(0))
            .unwrap();
        assert_eq!(init_bytecode, &[0xB5]); // RET_VOID only
        let scan_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(scan_bytecode, &[0xB5]); // RET_VOID only
    }

    #[test]
    fn compile_when_duplicate_constants_then_deduplicates() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := 10;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        // Should only have one constant (10 is deduplicated)
        assert_eq!(container.constant_pool.len(), 1);
    }

    #[test]
    fn compile_when_variable_to_variable_assignment_then_load_store() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.num_functions, 2);

        // Function 1 (scan):
        // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
        // y := x:  LOAD_VAR_I32 var:0, STORE_VAR_I32 var:1
        // RET_VOID
        let bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(
            bytecode,
            &[
                0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
                0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
                0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
                0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
                0xB5, // RET_VOID
            ]
        );
    }

    #[test]
    fn compile_when_negative_constant_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := -5;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        assert_eq!(
            container
                .constant_pool
                .get_i32(ironplc_container::ConstantIndex::new(0))
                .unwrap(),
            -5
        );

        // Function 1 (scan): LOAD_CONST_I32 pool:0 (-5), STORE_VAR_I32 var:0, RET_VOID
        let bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn compile_when_simple_if_then_succeeds() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  IF x > 0 THEN
    x := 1;
  END_IF;
END_PROGRAM
";
        let (library, context) = parse(source);
        let result = compile(&library, &context, &CodegenOptions::default());

        assert!(result.is_ok());
    }

    #[test]
    fn compile_when_exit_outside_loop_then_p4021_error() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
  EXIT;
END_PROGRAM
";
        let (library, context) = parse(source);
        let result = compile(&library, &context, &CodegenOptions::default());

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::ExitOutsideLoop.code());
    }

    #[test]
    fn compile_when_for_non_constant_step_then_p9999_error() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    s : DINT;
  END_VAR
  FOR x := 1 TO 10 BY s DO
    x := x;
  END_FOR;
END_PROGRAM
";
        let (library, context) = parse(source);
        let result = compile(&library, &context, &CodegenOptions::default());

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::NotImplemented.code());
    }

    #[test]
    fn compile_when_byte_variable_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 42;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(
            container
                .constant_pool
                .get_i32(ironplc_container::ConstantIndex::new(0))
                .unwrap(),
            42
        );
    }

    #[test]
    fn compile_when_dword_bit_string_literal_then_loads_constant() {
        let source = "
PROGRAM main
  VAR
    x : DWORD;
  END_VAR
  x := DWORD#16#FF;
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(
            container
                .constant_pool
                .get_i32(ironplc_container::ConstantIndex::new(0))
                .unwrap(),
            255
        );
    }

    #[test]
    fn compile_when_user_function_with_real_comparison_then_produces_container() {
        let source = "
FUNCTION SIGN_R : BOOL
VAR_INPUT
    in : REAL;
END_VAR
    SIGN_R := in < 0.0;
END_FUNCTION
PROGRAM main
VAR
    result : BOOL;
END_VAR
    result := SIGN_R(in := 2.5);
END_PROGRAM
";
        let (library, context) = parse(source);
        let container = compile(&library, &context, &CodegenOptions::default()).unwrap();

        // Should have 3 functions: init, scan, SIGN_R
        assert_eq!(container.header.num_functions, 3);
    }
}
