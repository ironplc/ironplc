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

use std::collections::{HashMap, HashSet};

use ironplc_container::debug_section::{
    EnumDefEntry, FuncNameEntry, StringLayoutEntry, VarNameEntry,
};
use ironplc_container::{
    CharWidth, Container, ContainerBuilder, FbTypeId, FunctionId, UserFbDescriptor, VarIndex,
    STRING_HEADER_BYTES,
};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, InitialValueAssignmentKind, Library,
    LibraryElementKind, ProgramDeclaration, StringType, VarDecl, VariableType,
};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::{FileId, Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use ironplc_analyzer::{FunctionEnvironment, SemanticContext, TypeEnvironment};

use crate::emit::Emitter;

use super::compile_fn::{compile_user_function, compile_user_function_block};
use super::compile_setup::{assign_variables, emit_initial_values, resolve_type_name};
use super::compile_stmt::compile_body;

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
///
/// `Str` holds Latin-1 bytes (narrow STRING); `WStr` holds UTF-16LE bytes
/// (wide WSTRING). The encoding is tagged per pool entry so the VM can verify
/// it against the destination (ADR-0034).
pub(crate) enum PoolConstant {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(Vec<u8>),
    WStr(Vec<u8>),
}

/// The IEC 61131-3 default maximum length for STRING (254 characters).
pub(crate) const DEFAULT_STRING_MAX_LENGTH_U16: u16 = 254;

/// Metadata for a STRING/WSTRING variable allocated in the data region.
#[derive(Clone)]
pub(crate) struct StringVarInfo {
    /// Byte offset into the data region where this string starts.
    pub(crate) data_offset: u32,
    /// Maximum number of code units this string can hold.
    pub(crate) max_length: u16,
    /// Per-code-unit byte width: `Narrow` for STRING, `Wide` for WSTRING.
    pub(crate) char_width: CharWidth,
}

/// Per-code-unit byte width for STRING (Latin-1 per ADR-0016).
pub(crate) const NARROW_CHAR_WIDTH: CharWidth = CharWidth::Narrow;
/// Per-code-unit byte width for WSTRING (UTF-16LE per ADR-0016).
pub(crate) const WIDE_CHAR_WIDTH: CharWidth = CharWidth::Wide;

/// Returns the per-code-unit byte width for an IEC string type:
/// `Narrow` for STRING, `Wide` for WSTRING (ADR-0016).
pub(crate) fn char_width_for_string_type(width: &StringType) -> CharWidth {
    match width {
        StringType::String => NARROW_CHAR_WIDTH,
        StringType::WString => WIDE_CHAR_WIDTH,
    }
}

/// Total bytes needed in the data region for a STRING/WSTRING value with the
/// given maximum length (in code units) and `char_width`: header plus
/// `max_length * char_width` payload bytes.
pub(crate) fn string_region_size(max_length: u16, char_width: CharWidth) -> u32 {
    STRING_HEADER_BYTES as u32 + (max_length as u32) * (char_width.byte_width() as u32)
}

/// Encode a character-string literal into bytes for the constant pool.
///
/// `char_width` selects the encoding per ADR-0016: `Narrow` for STRING
/// (Latin-1, one byte per character), `Wide` for WSTRING (UTF-16LE, two
/// bytes per code unit). Characters above U+FFFF are out of scope (BMP
/// only); higher code points are truncated to their low 16 bits.
pub(crate) fn encode_string_literal(chars: &[char], char_width: CharWidth) -> Vec<u8> {
    match char_width {
        CharWidth::Narrow => chars.iter().map(|&ch| ch as u8).collect(),
        CharWidth::Wide => chars
            .iter()
            .flat_map(|&ch| (ch as u16).to_le_bytes())
            .collect(),
    }
}

/// Loads a string literal into a temp buffer at the given encoding width.
///
/// Encodes the literal (Latin-1 for narrow, UTF-16LE for wide), registers it as
/// a width-tagged constant-pool entry, accounts for the temp buffer it lands
/// in, and emits LOAD_CONST_STR (leaving `buf_idx` on the stack). The caller
/// stores that temp buffer into a destination of the same width; the VM
/// verifies the encoding match (ADR-0034).
pub(crate) fn emit_string_literal_load(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    chars: &[char],
    char_width: CharWidth,
) {
    let bytes = encode_string_literal(chars, char_width);
    let pool_index = if char_width.is_wide() {
        ctx.add_wstr_constant(bytes)
    } else {
        ctx.add_str_constant(bytes)
    };
    ctx.num_temp_bufs += 1;
    emitter.emit_load_const_str(pool_index);
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
    sources: &dyn crate::source_lookup::SourceLookup,
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

    let enum_map = crate::compile_enum::build_enum_ordinal_map(library);

    let mut container = compile_program_with_functions(
        ProgramInputs {
            program,
            func_decls: &func_decls,
            fb_decls: &fb_decls,
            global_vars,
        },
        context.functions(),
        context.types(),
        enum_map,
        sources,
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

/// Idempotently registers a POU's source file with the debug section's
/// SOURCE_FILE_TABLE. Called from `compile_program_with_functions` for
/// each top-level program / function / function-block declaration.
fn register_pou_source_file(
    ctx: &mut CompileContext,
    file_id: &ironplc_dsl::core::FileId,
    sources: &dyn crate::source_lookup::SourceLookup,
) {
    if ctx.debug_source_files.get(file_id).is_some() {
        return;
    }
    let bytes = sources.source_bytes(file_id);
    ctx.debug_source_files.register(file_id, bytes);
}

/// Pushes a function's line-map entries (already remapped through the
/// optimizer) into the container builder, paired with the function's
/// `FunctionId`.
fn add_line_map_entries(
    mut builder: ContainerBuilder,
    function_id: FunctionId,
    entries: &[crate::emit::EmittedLineMapEntry],
) -> ContainerBuilder {
    for entry in entries {
        builder = builder.add_line_map_entry(ironplc_container::LineMapEntry {
            function_id,
            bytecode_offset: entry.bytecode_offset,
            file_id: entry.file_id,
            source_line: entry.source_line,
            source_column: entry.source_column,
        });
    }
    builder
}

/// Holds the compiled bytecode and metadata for a user-defined function.
pub(crate) struct CompiledFunction {
    pub(crate) function_id: FunctionId,
    pub(crate) bytecode: Vec<u8>,
    pub(crate) max_stack_depth: u16,
    pub(crate) num_locals: u16,
    pub(crate) num_params: u16,
    pub(crate) name: String,
    /// Per-statement line-map entries with `bytecode_offset` already
    /// remapped through the optimizer. The codegen driver pairs each
    /// entry with this function's `FunctionId` before pushing to the
    /// container builder.
    pub(crate) line_map: Vec<crate::emit::EmittedLineMapEntry>,
}

/// Holds the finalized bytecode and stack depth for a single emitted function.
///
/// Returned by [`finalize_function`] to centralize the
/// `emitter.bytecode()` → optimize → `max_stack_depth()` sequence shared by
/// every code path that emits a function (init, scan, user functions, FB
/// bodies). Centralizing this makes future cross-cutting additions (e.g.
/// source-map plumbing) a one-site change.
pub(crate) struct FinalizedFunction {
    pub(crate) bytecode: Vec<u8>,
    pub(crate) max_stack_depth: u16,
    /// Per-statement line-map entries with `bytecode_offset` already
    /// remapped through the optimizer's old→new offset table. Entries
    /// whose pre-optimization offset fell on an instruction that was
    /// elided snap forward to the next surviving instruction; entries
    /// past the new end-of-function are dropped.
    pub(crate) line_map: Vec<crate::emit::EmittedLineMapEntry>,
}

/// Finalizes an emitter into ready-to-store bytecode plus stack depth.
///
/// `emitter.bytecode()` must be called before `max_stack_depth()` because the
/// peephole optimizer (run inside `bytecode()`) may increase max_stack_depth.
pub(crate) fn finalize_function(emitter: &mut Emitter, ctx: &CompileContext) -> FinalizedFunction {
    let raw_line_map = emitter.take_line_map();
    let (bytecode, offset_map) = crate::optimize::optimize(emitter.bytecode(), &ctx.constants);
    let max_stack_depth = emitter.max_stack_depth();
    let line_map =
        crate::optimize::remap_line_map(raw_line_map, &offset_map, bytecode.len() as u16);
    FinalizedFunction {
        bytecode,
        max_stack_depth,
        line_map,
    }
}

/// The AST inputs that [`compile_program_with_functions`] operates on,
/// all extracted from the same `Library` at the start of `compile()`.
/// Grouped to keep the helper's argument count manageable.
struct ProgramInputs<'a> {
    program: &'a ProgramDeclaration,
    func_decls: &'a [&'a FunctionDeclaration],
    fb_decls: &'a [&'a FunctionBlockDeclaration],
    global_vars: &'a [VarDecl],
}

/// Compiles a PROGRAM and its user-defined functions into a container.
///
/// Always emits at least two functions:
/// - Function 0: init (load constants + store variables, called once by VM)
/// - Function 1: scan (program body, called every scan cycle)
/// - Functions 2+: user-defined functions
///
/// When no initial values exist, the init function is a single RET_VOID.
// Internal codegen helper split out from `compile()` purely for
// readability of the public function; called from exactly one site.
fn compile_program_with_functions(
    inputs: ProgramInputs<'_>,
    functions: &FunctionEnvironment,
    types: &TypeEnvironment,
    enum_map: crate::compile_enum::EnumOrdinalMap,
    sources: &dyn crate::source_lookup::SourceLookup,
) -> Result<Container, Diagnostic> {
    let ProgramInputs {
        program,
        func_decls,
        fb_decls,
        global_vars,
    } = inputs;
    let mut ctx = CompileContext::new();
    ctx.enum_map = enum_map;
    let mut builder = ContainerBuilder::new();

    // Register every top-level POU's source file with the debug
    // section's SOURCE_FILE_TABLE. Each statement's `compile_statement`
    // call later looks up the resulting `SourceFileId` via
    // `ctx.debug_source_files.get(&span.file_id)` and pairs it with the
    // statement's (line, column) for the LineMapEntry. The
    // registration order (program first, then functions, then FB
    // bodies) determines the `file_id` numbering, which appears in
    // the on-disk container.
    register_pou_source_file(&mut ctx, &program.name.span.file_id, sources);
    for f in func_decls {
        register_pou_source_file(&mut ctx, &f.name.span.file_id, sources);
    }
    for fb in fb_decls {
        register_pou_source_file(&mut ctx, &fb.name.name.span.file_id, sources);
    }

    // Assign global variable indices first (indices 0..G).
    assign_variables(&mut ctx, &mut builder, global_vars, types)?;
    let num_globals = ctx.variables.len() as u16;

    // Pre-scan user-defined FB declarations to register type metadata
    // (field indices, field op types, type IDs) before assign_variables runs.
    // This allows assign_variables to resolve user-defined FB instance variables.
    // The actual FB body compilation happens after program-local variables are
    // assigned, once var_offsets are known.
    let mut compiled_fb_bodies: Vec<CompiledFunction> = Vec::new();

    for (next_function_id, fb_decl) in (2_u16..).zip(fb_decls.iter()) {
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
                function_id: FunctionId::new(next_function_id),
                var_offset: 0, // updated after program vars are assigned
                field_op_types,
            },
        );
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
    let mut var_offset = VarIndex::new(program_var_count);

    // Compile user FUNCTIONs before FUNCTION_BLOCK bodies so that FB bodies
    // can resolve calls to user functions via `ctx.user_functions` (functions
    // register themselves at the end of `compile_user_function`). Per
    // IEC 61131-3 functions cannot instantiate FBs, so they don't require FB
    // bodies to have been compiled first.
    //
    // Function IDs are POSITIONAL in the container (`get_function`
    // indexes the `functions` Vec). FB bodies are inserted at positions
    // 2..(2 + N_fb), so user functions must start at the next free
    // position so each callable POU has a unique runtime id.
    let user_fn_id_base = 2u16 + fb_decls.len() as u16;
    for (next_function_id, func_decl) in (user_fn_id_base..).zip(func_decls.iter()) {
        let compiled = compile_user_function(
            func_decl,
            FunctionId::new(next_function_id),
            var_offset,
            &mut ctx,
            functions,
            &mut builder,
            types,
            num_globals,
        )?;
        var_offset = VarIndex::new(var_offset.raw() + compiled.num_locals);
        compiled_functions.push(compiled);
    }

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

    let total_variables = var_offset;

    // Emit bytecode for variable initial values into the init emitter.
    let mut init_emitter = Emitter::new();
    emit_initial_values(&mut init_emitter, &mut ctx, global_vars, types)?;
    emit_initial_values(&mut init_emitter, &mut ctx, &local_vars, types)?;
    init_emitter.emit_ret_void();

    // Compile the program body into the scan emitter. Mark
    // FunctionId::SCAN as the current caller so any CALL / user FB_CALL
    // emitted from inside the body records a call-graph edge.
    let mut scan_emitter = Emitter::new();
    ctx.current_function_id = Some(FunctionId::SCAN);
    compile_body(&mut scan_emitter, &mut ctx, &program.body)?;
    ctx.current_function_id = None;
    scan_emitter.emit_ret_void();

    // Build the container.
    builder = builder.num_variables(total_variables.raw());

    // Configure data region for STRING variables.
    if ctx.data_region_offset > 0 {
        builder = builder.data_region_bytes(ctx.data_region_offset);
        if ctx.num_temp_bufs > 0 {
            // Temp-buffer slots are uniform. When any wide string exists, size
            // each slot in wide bytes so an intermediate WSTRING value of up to
            // max_string_capacity code units fits (the VM allocator divides the
            // slot capacity by the value's char_width). Narrow-only programs
            // keep the original byte-identical sizing.
            let temp_char_width = if ctx.has_wide_string {
                WIDE_CHAR_WIDTH
            } else {
                NARROW_CHAR_WIDTH
            };
            builder = builder
                .num_temp_bufs(ctx.num_temp_bufs)
                .max_temp_buf_bytes(string_region_size(ctx.max_string_capacity, temp_char_width));
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
            PoolConstant::WStr(v) => builder = builder.add_wstr_constant(v),
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
    let init = finalize_function(&mut init_emitter, &ctx);
    builder = builder.add_function(
        FunctionId::INIT,
        &init.bytecode,
        init.max_stack_depth,
        program_var_count,
        0,
    );
    builder = add_line_map_entries(builder, FunctionId::INIT, &init.line_map);

    let scan = finalize_function(&mut scan_emitter, &ctx);
    builder = builder.add_function(
        FunctionId::SCAN,
        &scan.bytecode,
        // FB_CALL recursively enters execute() on the shared stack, so the
        // scan function's reported stack depth must include the deepest FB
        // body's depth.
        scan.max_stack_depth + max_fb_body_stack,
        program_var_count,
        0,
    );
    builder = add_line_map_entries(builder, FunctionId::SCAN, &scan.line_map);

    // Add user-defined function block bodies.
    for compiled in &compiled_fb_bodies {
        builder = builder.add_function(
            compiled.function_id,
            &compiled.bytecode,
            compiled.max_stack_depth,
            compiled.num_locals,
            compiled.num_params,
        );
        builder = add_line_map_entries(builder, compiled.function_id, &compiled.line_map);
    }

    // Add user FB type descriptors to the container.
    for fb_info in ctx.user_fb_types.values() {
        builder = builder.add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(fb_info.type_id),
            function_id: fb_info.function_id,
            var_offset: fb_info.var_offset,
            num_fields: fb_info.num_fields as u8,
        });
    }

    // Add user-defined functions.
    for compiled in &compiled_functions {
        builder = builder.add_function(
            compiled.function_id,
            &compiled.bytecode,
            compiled.max_stack_depth,
            compiled.num_locals,
            compiled.num_params,
        );
        builder = add_line_map_entries(builder, compiled.function_id, &compiled.line_map);
    }

    // Add the SOURCE_FILE_TABLE (tag 6). `ctx.debug_source_files`
    // accumulated one entry per POU source file; insertion order is
    // the file's SourceFileId, which the line-map entries above
    // already reference.
    let source_file_entries = std::mem::take(&mut ctx.debug_source_files).into_entries();
    if !source_file_entries.is_empty() {
        builder = builder.add_source_files(source_file_entries);
    }

    // Compute the worst-case PLC call depth from the static call graph
    // and stamp it on the header so `VmReady::start` can reject
    // containers that wouldn't fit in the embedder's frame buffer
    // (Trap::ProgramExceedsCallDepth). Cycles should already be
    // rejected by semantic analysis (Problem::RecursiveCycle); the DFS
    // is a defensive backstop that surfaces an internal error.
    let max_call_depth =
        crate::call_graph::compute_max_call_depth(&ctx.call_graph, FunctionId::SCAN)?;

    builder = builder
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .shared_globals_size(program_var_count)
        .max_call_depth(max_call_depth);

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
            function_id: compiled.function_id,
            name: compiled.name.clone(),
        });
    }
    for compiled in &compiled_functions {
        builder = builder.add_func_name(FuncNameEntry {
            function_id: compiled.function_id,
            name: compiled.name.clone(),
        });
    }
    for entry in ctx.debug_var_names {
        builder = builder.add_var_name(entry);
    }
    for entry in ctx.debug_string_layouts {
        builder = builder.add_string_layout(entry);
    }
    for (type_name, values) in &ctx.enum_map.definitions {
        builder = builder.add_enum_def(EnumDefEntry {
            type_name: type_name.clone(),
            values: values.clone(),
        });
    }

    Ok(builder.build())
}

#[derive(Clone)]
pub(crate) struct StringParamInfo {
    /// Byte offset in the data region where this parameter's string is stored.
    pub(crate) data_offset: u32,
    /// Maximum number of code units this parameter can hold.
    pub(crate) max_length: u16,
    /// Per-code-unit byte width: `Narrow` for STRING, `Wide` for WSTRING.
    pub(crate) char_width: CharWidth,
}

/// Metadata for a STRING/WSTRING return value in a user-defined function.
///
/// When a function returns STRING/WSTRING, the return value lives in the
/// data region rather than on the operand stack.
#[derive(Clone)]
pub(crate) struct StringReturnInfo {
    /// Byte offset in the data region where the return string is stored.
    pub(crate) data_offset: u32,
    /// Maximum number of code units the return string can hold.
    pub(crate) max_length: u16,
    /// Per-code-unit byte width: `Narrow` for STRING, `Wide` for WSTRING.
    pub(crate) char_width: CharWidth,
}

/// Metadata for a compiled user-defined function.
#[derive(Clone)]
pub(crate) struct UserFunctionInfo {
    /// The function ID assigned in the container.
    pub(crate) function_id: FunctionId,
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
    pub(crate) function_id: FunctionId,
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
    pub(crate) constants: Vec<PoolConstant>,
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
    /// Pre-computed ordinal mappings for named enumeration types.
    pub(crate) enum_map: crate::compile_enum::EnumOrdinalMap,
    /// Next available byte offset in the data region.
    pub(crate) data_region_offset: u32,
    /// Maximum string capacity across all STRING variables (for temp buffer sizing).
    pub(crate) max_string_capacity: u16,
    /// True when any WSTRING (wide) string is declared. Temp buffers are then
    /// sized in wide bytes so an intermediate wide value fits (ADR-0035).
    pub(crate) has_wide_string: bool,
    /// Number of temp buffers needed (one per string load in the init function).
    pub(crate) num_temp_bufs: u16,
    /// Debug info: variable name entries collected during assign_variables.
    pub(crate) debug_var_names: Vec<VarNameEntry>,
    /// Debug info: STRING variable data-region layouts collected during assign_variables.
    pub(crate) debug_string_layouts: Vec<StringLayoutEntry>,
    /// Debug info: registry mapping each referenced source `FileId` to its
    /// `SourceFileId` (debug section SOURCE_FILE_TABLE index) plus cached
    /// source bytes for (line, column) conversion in `compile_statement`.
    pub(crate) debug_source_files: crate::source_lookup::SourceFileRegistry,
    /// Maps user-defined function name (lowercase) to compilation metadata.
    pub(crate) user_functions: HashMap<String, UserFunctionInfo>,
    /// Maps user-defined FB type name (uppercase) to compilation metadata.
    pub(crate) user_fb_types: HashMap<String, UserFbTypeInfo>,
    /// Next available type ID for user-defined function blocks.
    next_user_fb_type_id: u16,
    /// When compiling a function body that returns a value, describes how an
    /// early `RETURN` statement should produce the return value before the
    /// `RET` opcode. `None` for programs and FBs (RETURN emits `RET_VOID`).
    pub(crate) current_function_return: Option<CurrentFunctionReturn>,
    /// Function whose body is currently being emitted; `None` outside any
    /// body. Set by the body-compilation entry points (SCAN, user
    /// functions, user-FB bodies) so [`record_call_edge`] knows who the
    /// caller is. Edges are aggregated into [`call_graph`] for the
    /// post-emission `max_call_depth` computation.
    ///
    /// [`record_call_edge`]: CompileContext::record_call_edge
    /// [`call_graph`]: CompileContext::call_graph
    pub(crate) current_function_id: Option<FunctionId>,
    /// Adjacency map: caller `FunctionId` → set of callees. Populated by
    /// [`record_call_edge`] from inside `compile_call.rs` (user CALL)
    /// and `compile_stmt.rs` (user `FB_CALL`). Empty for intrinsic
    /// FBs, which have no PLC body.
    ///
    /// [`record_call_edge`]: CompileContext::record_call_edge
    pub(crate) call_graph: HashMap<FunctionId, HashSet<FunctionId>>,
}

/// Describes how a `RETURN` statement should yield the function's value.
#[derive(Clone, Copy)]
pub(crate) enum CurrentFunctionReturn {
    /// Scalar return: load the variable, then emit `RET`.
    Scalar {
        var_index: VarIndex,
        op_type: OpType,
    },
    /// STRING/WSTRING return: load the string from the data region into a
    /// temp buffer, then emit `RET`.
    String { data_offset: u32 },
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
            has_wide_string: false,
            num_temp_bufs: 0,
            debug_var_names: Vec::new(),
            debug_string_layouts: Vec::new(),
            debug_source_files: crate::source_lookup::SourceFileRegistry::new(),
            user_functions: HashMap::new(),
            user_fb_types: HashMap::new(),
            next_user_fb_type_id: 0x1000,
            enum_map: crate::compile_enum::EnumOrdinalMap::default(),
            current_function_return: None,
            current_function_id: None,
            call_graph: HashMap::new(),
        }
    }

    /// Returns the exit label for the innermost enclosing loop, if any.
    pub(crate) fn current_loop_exit(&self) -> Option<crate::emit::Label> {
        self.loop_exit_labels.last().copied()
    }

    /// Records a static call-graph edge from the function whose body is
    /// currently being emitted to `callee`.
    ///
    /// No-op when no body is being emitted (e.g. during INIT, which
    /// only stores constants and never CALLs) — `current_function_id`
    /// is `None` outside body-compilation entry points.
    pub(crate) fn record_call_edge(&mut self, callee: FunctionId) {
        if let Some(caller) = self.current_function_id {
            self.call_graph.entry(caller).or_default().insert(callee);
        }
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

    /// Returns the index of an existing constant matching `matches`, or
    /// pushes `new_value` and returns its index.
    fn intern_constant(
        &mut self,
        matches: impl FnMut(&PoolConstant) -> bool,
        new_value: PoolConstant,
    ) -> u16 {
        if let Some(i) = self.constants.iter().position(matches) {
            return i as u16;
        }
        let index = self.constants.len() as u16;
        self.constants.push(new_value);
        index
    }

    /// Adds an i32 constant to the pool and returns its index.
    pub(crate) fn add_i32_constant(&mut self, value: i32) -> u16 {
        self.intern_constant(
            |c| matches!(c, PoolConstant::I32(v) if *v == value),
            PoolConstant::I32(value),
        )
    }

    /// Adds an f32 constant to the pool and returns its index.
    ///
    /// Equality uses `to_bits()` so that distinct NaN bit patterns are not
    /// collapsed to the same pool entry.
    pub(crate) fn add_f32_constant(&mut self, value: f32) -> u16 {
        self.intern_constant(
            |c| matches!(c, PoolConstant::F32(v) if v.to_bits() == value.to_bits()),
            PoolConstant::F32(value),
        )
    }

    /// Adds an f64 constant to the pool and returns its index.
    ///
    /// Equality uses `to_bits()` so that distinct NaN bit patterns are not
    /// collapsed to the same pool entry.
    pub(crate) fn add_f64_constant(&mut self, value: f64) -> u16 {
        self.intern_constant(
            |c| matches!(c, PoolConstant::F64(v) if v.to_bits() == value.to_bits()),
            PoolConstant::F64(value),
        )
    }

    /// Adds an i64 constant to the pool and returns its index.
    pub(crate) fn add_i64_constant(&mut self, value: i64) -> u16 {
        self.intern_constant(
            |c| matches!(c, PoolConstant::I64(v) if *v == value),
            PoolConstant::I64(value),
        )
    }

    /// Adds a string constant (raw bytes) to the pool and returns its index.
    pub(crate) fn add_str_constant(&mut self, value: Vec<u8>) -> u16 {
        if let Some(i) = self
            .constants
            .iter()
            .position(|c| matches!(c, PoolConstant::Str(v) if v == &value))
        {
            return i as u16;
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::Str(value));
        index
    }

    /// Adds a wide (UTF-16LE / WSTRING) string constant, deduplicating against
    /// existing wide entries. Kept distinct from [`Self::add_str_constant`] so
    /// the pool entry carries the wide encoding tag the VM verifies.
    pub(crate) fn add_wstr_constant(&mut self, value: Vec<u8>) -> u16 {
        if let Some(i) = self
            .constants
            .iter()
            .position(|c| matches!(c, PoolConstant::WStr(v) if v == &value))
        {
            return i as u16;
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::WStr(value));
        index
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
    fn encode_string_literal_when_narrow_then_one_byte_per_char() {
        let bytes = encode_string_literal(&['h', 'i'], NARROW_CHAR_WIDTH);
        assert_eq!(bytes, vec![b'h', b'i']);
    }

    #[test]
    fn encode_string_literal_when_wide_then_utf16le_two_bytes_per_char() {
        let bytes = encode_string_literal(&['h', 'i'], WIDE_CHAR_WIDTH);
        assert_eq!(bytes, vec![0x68, 0x00, 0x69, 0x00]);
    }

    #[test]
    fn encode_string_literal_when_wide_and_bmp_above_ascii_then_utf16le() {
        // U+00E9 LATIN SMALL LETTER E WITH ACUTE
        let bytes = encode_string_literal(&['é'], WIDE_CHAR_WIDTH);
        assert_eq!(bytes, vec![0xE9, 0x00]);
    }

    #[test]
    fn encode_string_literal_when_empty_then_returns_empty() {
        assert!(encode_string_literal(&[], NARROW_CHAR_WIDTH).is_empty());
        assert!(encode_string_literal(&[], WIDE_CHAR_WIDTH).is_empty());
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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

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
        assert_eq!(init_bytecode, &[0x8C]);

        // Function 1: scan — LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0, RET_VOID
        let scan_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(scan_bytecode, &[0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x8C]);
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
        let result = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        );

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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

        // Should have two functions (init + scan), both just RET_VOID
        assert_eq!(container.header.num_functions, 2);
        let init_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(0))
            .unwrap();
        assert_eq!(init_bytecode, &[0x8C]); // RET_VOID only
        let scan_bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(scan_bytecode, &[0x8C]); // RET_VOID only
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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.num_functions, 2);

        // Function 1 (scan):
        // x := 10: LOAD_CONST_I32 pool:0
        // (store-load peephole): DUP, STORE_VAR_I32 var:0, NOP, NOP
        // y := x:  STORE_VAR_I32 var:1
        // RET_VOID
        let bytecode = container
            .code
            .get_function_bytecode(ironplc_container::FunctionId::new(1))
            .unwrap();
        assert_eq!(
            bytecode,
            &[
                0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0
                0x91, // DUP (store-load optimization)
                0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
                0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
                0x8C, // RET_VOID
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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

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
        assert_eq!(bytecode, &[0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x8C]);
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
        let result = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        );

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
        let result = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        );

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
        let result = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        );

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P9999");
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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

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
        let container = compile(
            &library,
            &context,
            &CodegenOptions::default(),
            &crate::EmptyLookup,
        )
        .unwrap();

        // Should have 3 functions: init, scan, SIGN_R
        assert_eq!(container.header.num_functions, 3);
    }
}
