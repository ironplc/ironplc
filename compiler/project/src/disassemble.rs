//! Disassembler for IPLC bytecode containers.
//!
//! Reads an IPLC bytecode container and produces structured JSON suitable
//! for display in a VS Code custom editor. The output includes the file
//! header, constant pool entries, and decoded bytecode instructions with
//! cross-referenced operands.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ironplc_container::opcode;
use ironplc_container::opcode::{Instruction, Operand};
use ironplc_container::{ConstType, Container};
use serde_json::{json, Value};

/// Disassembles a bytecode container into a structured JSON value.
///
/// The returned JSON has three top-level keys:
/// - `header`: file header fields
/// - `constants`: array of constant pool entries
/// - `functions`: array of function disassemblies with decoded instructions
pub fn disassemble(container: &Container) -> Value {
    let header = disassemble_header(container);
    let task_table = disassemble_task_table(container);
    let constants = disassemble_constants(container);
    let functions = disassemble_functions(container);

    json!({
        "header": header,
        "taskTable": task_table,
        "constants": constants,
        "functions": functions,
    })
}

/// Disassembles an IPLC file at the given path, returning structured JSON.
///
/// On error, returns `{"error": "message"}`.
pub fn disassemble_file(path: &Path) -> Value {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => return json!({"error": format!("Failed to open file: {}", e)}),
    };

    let mut reader = BufReader::new(file);
    let container = match Container::read_from(&mut reader) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Failed to parse container: {}", e)}),
    };

    disassemble(&container)
}

/// Converts the file header into a JSON object.
fn disassemble_header(container: &Container) -> Value {
    let h = &container.header;
    let flags = h.flags;

    json!({
        "formatVersion": h.format_version,
        "profile": h.profile,
        "flags": {
            "raw": flags,
            "hasContentSignature": (flags & 0x01) != 0,
            "hasDebugSection": (flags & 0x02) != 0,
            "hasTypeSection": (flags & 0x04) != 0,
        },
        "contentHash": hex_string(&h.content_hash),
        "debugHash": hex_string(&h.debug_hash),
        "layoutHash": hex_string(&h.layout_hash),
        "maxStackDepth": h.max_stack_depth,
        "maxCallDepth": h.max_call_depth,
        "numVariables": h.num_variables,
        "dataRegionBytes": h.data_region_bytes,
        "numTempBufs": h.num_temp_bufs,
        "maxTempBufBytes": h.max_temp_buf_bytes,
        "numFunctions": h.num_functions,
        "numFbTypes": h.num_fb_types,
        "inputImageBytes": h.input_image_bytes,
        "outputImageBytes": h.output_image_bytes,
        "memoryImageBytes": h.memory_image_bytes,
        "sigSection": {
            "offset": h.sig_section_offset,
            "size": h.sig_section_size,
        },
        "debugSigSection": {
            "offset": h.debug_sig_offset,
            "size": h.debug_sig_size,
        },
        "typeSection": {
            "offset": h.type_section_offset,
            "size": h.type_section_size,
        },
        "constSection": {
            "offset": h.const_section_offset,
            "size": h.const_section_size,
        },
        "codeSection": {
            "offset": h.code_section_offset,
            "size": h.code_section_size,
        },
        "debugSection": {
            "offset": h.debug_section_offset,
            "size": h.debug_section_size,
        },
        "taskSection": {
            "offset": h.task_section_offset,
            "size": h.task_section_size,
        },
    })
}

/// Converts the task table into a JSON object with tasks and programs.
fn disassemble_task_table(container: &Container) -> Value {
    let tt = &container.task_table;

    let tasks: Vec<Value> = tt
        .tasks
        .iter()
        .map(|t| {
            json!({
                "taskId": t.task_id.raw(),
                "priority": t.priority,
                "taskType": t.task_type.as_str(),
                "enabled": (t.flags & 0x01) != 0,
                "intervalUs": t.interval_us,
                "singleVarIndex": t.single_var_index.raw(),
                "watchdogUs": t.watchdog_us,
            })
        })
        .collect();

    let programs: Vec<Value> = tt
        .programs
        .iter()
        .map(|p| {
            json!({
                "instanceId": p.instance_id.raw(),
                "taskId": p.task_id.raw(),
                "entryFunctionId": p.entry_function_id.raw(),
                "varTableOffset": p.var_table_offset,
                "varTableCount": p.var_table_count,
                "fbInstanceOffset": p.fb_instance_offset,
                "fbInstanceCount": p.fb_instance_count,
            })
        })
        .collect();

    json!({
        "sharedGlobalsSize": tt.shared_globals_size,
        "tasks": tasks,
        "programs": programs,
    })
}

/// Converts the constant pool into a JSON array of entries.
fn disassemble_constants(container: &Container) -> Value {
    let entries: Vec<Value> = container
        .constant_pool
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let value_str = format_const_value(entry.const_type, entry.bytes());
            json!({
                "index": index,
                "type": entry.const_type.as_str(),
                "value": value_str,
            })
        })
        .collect();

    Value::Array(entries)
}

/// Formats a constant value as a human-readable string based on its type.
fn format_const_value(const_type: ConstType, bytes: &[u8]) -> String {
    match const_type {
        ConstType::I32 if bytes.len() >= 4 => {
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        ConstType::U32 if bytes.len() >= 4 => {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        ConstType::I64 if bytes.len() >= 8 => i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
        .to_string(),
        ConstType::U64 if bytes.len() >= 8 => u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
        .to_string(),
        ConstType::F32 if bytes.len() >= 4 => {
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string()
        }
        ConstType::F64 if bytes.len() >= 8 => f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
        .to_string(),
        // Narrow strings are Latin-1: each byte maps directly to a code point.
        ConstType::Str => format!(
            "\"{}\"",
            bytes.iter().map(|&b| b as char).collect::<String>()
        ),
        // Wide strings are UTF-16LE: pair up bytes into code units.
        ConstType::WStr if bytes.len().is_multiple_of(2) => {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            format!("\"{}\"", String::from_utf16_lossy(&units))
        }
        _ => format!("<invalid: {} bytes>", bytes.len()),
    }
}

/// Disassembles all functions into a JSON array.
fn disassemble_functions(container: &Container) -> Value {
    let functions: Vec<Value> = container
        .code
        .functions
        .iter()
        .map(|func| {
            let bytecode = container
                .code
                .get_function_bytecode(func.function_id)
                .unwrap_or(&[]);
            let instructions = decode_instructions(bytecode, container);
            json!({
                "id": func.function_id.raw(),
                "bytecodeOffset": func.code_offset,
                "bytecodeLength": func.code_length,
                "maxStackDepth": func.max_stack_depth,
                "numLocals": func.num_locals,
                "instructions": instructions,
            })
        })
        .collect();

    Value::Array(functions)
}

/// Decodes a bytecode slice into an array of instruction JSON objects.
///
/// Every row is rendered from the instruction's own declaration -- the
/// mnemonic and operand layout the instruction set gives it -- so there is no
/// per-opcode code here that can fall out of step with the instruction set.
/// An `UNKNOWN(0x..)` row therefore means what it says: a byte that is not an
/// assigned opcode, from a corrupt container or one written by a newer
/// compiler.
fn decode_instructions(bytecode: &[u8], container: &Container) -> Vec<Value> {
    let mut instructions = Vec::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let opcode_byte = bytecode[pc];
        let size = opcode::instruction_size(opcode_byte);

        instructions.push(match Instruction::decode(opcode_byte) {
            Some(instruction) => decode_instruction(instruction, bytecode, pc, size, container),
            None => instruction_json(
                pc,
                &format!("UNKNOWN(0x{opcode_byte:02X})"),
                String::new(),
                String::new(),
            ),
        });

        pc += size;
    }

    instructions
}

/// Decodes one instruction: its mnemonic, its operands in order, and whatever
/// comments those operands contribute (a constant's value, a branch target).
fn decode_instruction(
    instruction: Instruction,
    bytecode: &[u8],
    pc: usize,
    size: usize,
    container: &Container,
) -> Value {
    // A truncated container can end part-way through an instruction. Say so
    // rather than reading past the end of the function's bytecode.
    if pc + size > bytecode.len() {
        return instruction_json(
            pc,
            instruction.mnemonic,
            "<truncated>".to_string(),
            String::new(),
        );
    }

    let mut operands = Vec::new();
    let mut comments = Vec::new();
    let mut at = pc + 1;

    if !instruction.note.is_empty() {
        comments.push(instruction.note.to_string());
    }

    for &operand in instruction.operands {
        let (text, comment) = format_operand(operand, bytecode, at, pc + size, container);
        operands.push(text);
        comments.extend(comment);
        at += operand.width();
    }

    instruction_json(
        pc,
        instruction.mnemonic,
        operands.join(", "),
        comments.join(", "),
    )
}

/// Renders the operand at byte offset `at`, and the comment it contributes.
///
/// The match is exhaustive over [`Operand`] with no catch-all, so an operand
/// shape added to the instruction set does not compile until it is rendered
/// here. `next_pc` is the offset of the following instruction, which is what
/// a branch offset is measured from.
fn format_operand(
    operand: Operand,
    bytecode: &[u8],
    at: usize,
    next_pc: usize,
    container: &Container,
) -> (String, Option<String>) {
    match operand {
        Operand::ConstIndex => {
            let pool_index = read_u16(bytecode, at);
            (
                format!("pool[{pool_index}]"),
                Some(lookup_const_comment(container, pool_index)),
            )
        }
        Operand::VarIndex => (format!("var[{}]", read_u16(bytecode, at)), None),
        Operand::RefIndex => (format!("ref[{}]", read_u16(bytecode, at)), None),
        Operand::ArrayDescIndex => (format!("desc[{}]", read_u16(bytecode, at)), None),
        Operand::DataOffset => (format!("data[{}]", read_u32(bytecode, at)), None),
        Operand::FieldIndex => (format!("field[{}]", bytecode[at]), None),
        Operand::FbTypeId => (format!("type[{}]", read_u16(bytecode, at)), None),
        Operand::FunctionId => {
            let function_id = read_u16(bytecode, at);
            (
                format!("func[{function_id}]"),
                lookup_function_comment(container, function_id),
            )
        }
        Operand::BuiltinId => (format_builtin(read_u16(bytecode, at)), None),
        Operand::JumpOffset => {
            let jump_offset = read_i16(bytecode, at);
            let target = (next_pc as isize + jump_offset as isize) as usize;
            (
                format!("offset: {}", format_jump_offset(jump_offset)),
                Some(format!("-> 0x{target:04X}")),
            )
        }
        Operand::CmpOp => (format_cmp_op(bytecode[at]).to_string(), None),
        Operand::MaxLength => (format!("max_len: {}", read_u16(bytecode, at)), None),
        Operand::CharWidth => (format!("char_width: {}", bytecode[at]), None),
        Operand::NumFields => (format!("num_fields: {}", bytecode[at]), None),
        Operand::FieldVarOffset => (format!("fields[{}]", read_u16(bytecode, at)), None),
        Operand::ParamVarOffset => (format!("params[{}]", read_u16(bytecode, at)), None),
    }
}

/// Builds the JSON object for one decoded instruction.
fn instruction_json(offset: usize, opcode: &str, operands: String, comment: String) -> Value {
    json!({
        "offset": offset,
        "opcode": opcode,
        "operands": operands,
        "comment": comment,
    })
}

/// Names the comparison operator a `CMP_BR_*` instruction branches on.
fn format_cmp_op(cmp_op: u8) -> &'static str {
    match cmp_op {
        opcode::cmp_op::EQ => "EQ",
        opcode::cmp_op::NE => "NE",
        opcode::cmp_op::LT_S => "LT_S",
        opcode::cmp_op::LE_S => "LE_S",
        opcode::cmp_op::GT_S => "GT_S",
        opcode::cmp_op::GE_S => "GE_S",
        _ => "INVALID",
    }
}

/// Names the built-in a `BUILTIN` instruction calls, falling back to its raw
/// ID.
fn format_builtin(func_id: u16) -> String {
    match func_id {
        opcode::builtin::EXPT_I32 => format!("EXPT_I32 (0x{:04X})", func_id),
        opcode::builtin::EXPT_F32 => format!("EXPT_F32 (0x{:04X})", func_id),
        opcode::builtin::EXPT_F64 => format!("EXPT_F64 (0x{:04X})", func_id),
        opcode::builtin::ABS_I32 => format!("ABS_I32 (0x{:04X})", func_id),
        opcode::builtin::ABS_F32 => format!("ABS_F32 (0x{:04X})", func_id),
        opcode::builtin::ABS_F64 => format!("ABS_F64 (0x{:04X})", func_id),
        opcode::builtin::MIN_I32 => format!("MIN_I32 (0x{:04X})", func_id),
        opcode::builtin::MIN_F32 => format!("MIN_F32 (0x{:04X})", func_id),
        opcode::builtin::MIN_F64 => format!("MIN_F64 (0x{:04X})", func_id),
        opcode::builtin::MAX_I32 => format!("MAX_I32 (0x{:04X})", func_id),
        opcode::builtin::MAX_F32 => format!("MAX_F32 (0x{:04X})", func_id),
        opcode::builtin::MAX_F64 => format!("MAX_F64 (0x{:04X})", func_id),
        opcode::builtin::LIMIT_I32 => format!("LIMIT_I32 (0x{:04X})", func_id),
        opcode::builtin::LIMIT_F32 => format!("LIMIT_F32 (0x{:04X})", func_id),
        opcode::builtin::LIMIT_F64 => format!("LIMIT_F64 (0x{:04X})", func_id),
        opcode::builtin::SEL_I32 => format!("SEL_I32 (0x{:04X})", func_id),
        opcode::builtin::SHL_I32 => format!("SHL_I32 (0x{:04X})", func_id),
        opcode::builtin::SHL_I64 => format!("SHL_I64 (0x{:04X})", func_id),
        opcode::builtin::SHR_I32 => format!("SHR_I32 (0x{:04X})", func_id),
        opcode::builtin::SHR_I64 => format!("SHR_I64 (0x{:04X})", func_id),
        opcode::builtin::ROL_I32 => format!("ROL_I32 (0x{:04X})", func_id),
        opcode::builtin::ROL_I64 => format!("ROL_I64 (0x{:04X})", func_id),
        opcode::builtin::ROR_I32 => format!("ROR_I32 (0x{:04X})", func_id),
        opcode::builtin::ROR_I64 => format!("ROR_I64 (0x{:04X})", func_id),
        opcode::builtin::ROL_U8 => format!("ROL_U8 (0x{:04X})", func_id),
        opcode::builtin::ROL_U16 => format!("ROL_U16 (0x{:04X})", func_id),
        opcode::builtin::ROR_U8 => format!("ROR_U8 (0x{:04X})", func_id),
        opcode::builtin::ROR_U16 => format!("ROR_U16 (0x{:04X})", func_id),
        opcode::builtin::SEL_F32 => format!("SEL_F32 (0x{:04X})", func_id),
        opcode::builtin::SEL_F64 => format!("SEL_F64 (0x{:04X})", func_id),
        opcode::builtin::SQRT_F32 => format!("SQRT_F32 (0x{:04X})", func_id),
        opcode::builtin::SQRT_F64 => format!("SQRT_F64 (0x{:04X})", func_id),
        opcode::builtin::BCD_TO_INT_8 => {
            format!("BCD_TO_INT_8 (0x{:04X})", func_id)
        }
        opcode::builtin::BCD_TO_INT_16 => {
            format!("BCD_TO_INT_16 (0x{:04X})", func_id)
        }
        opcode::builtin::BCD_TO_INT_32 => {
            format!("BCD_TO_INT_32 (0x{:04X})", func_id)
        }
        opcode::builtin::BCD_TO_INT_64 => {
            format!("BCD_TO_INT_64 (0x{:04X})", func_id)
        }
        opcode::builtin::INT_TO_BCD_8 => {
            format!("INT_TO_BCD_8 (0x{:04X})", func_id)
        }
        opcode::builtin::INT_TO_BCD_16 => {
            format!("INT_TO_BCD_16 (0x{:04X})", func_id)
        }
        opcode::builtin::INT_TO_BCD_32 => {
            format!("INT_TO_BCD_32 (0x{:04X})", func_id)
        }
        opcode::builtin::INT_TO_BCD_64 => {
            format!("INT_TO_BCD_64 (0x{:04X})", func_id)
        }
        opcode::builtin::TRUNC_F64 => format!("TRUNC_F64 (0x{:04X})", func_id),
        opcode::builtin::MOD_F64 => format!("MOD_F64 (0x{:04X})", func_id),
        opcode::builtin::TRUNC_F32 => format!("TRUNC_F32 (0x{:04X})", func_id),
        opcode::builtin::MOD_F32 => format!("MOD_F32 (0x{:04X})", func_id),
        id if opcode::builtin::is_mux(id) => {
            let n = opcode::builtin::mux_info(id).unwrap();
            let width = if id >= opcode::builtin::MUX_F64_BASE {
                "F64"
            } else if id >= opcode::builtin::MUX_F32_BASE {
                "F32"
            } else if id >= opcode::builtin::MUX_I64_BASE {
                "I64"
            } else {
                "I32"
            };
            format!("MUX_{width}({n}) (0x{id:04X})")
        }
        _ => format!("0x{:04X}", func_id),
    }
}

/// Reads a little-endian u16 from the bytecode at the given position.
fn read_u16(bytecode: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([bytecode[pos], bytecode[pos + 1]])
}

/// Reads a little-endian i16 from the bytecode at the given position.
fn read_i16(bytecode: &[u8], pos: usize) -> i16 {
    i16::from_le_bytes([bytecode[pos], bytecode[pos + 1]])
}

/// Reads a little-endian u32 from the bytecode at the given position.
fn read_u32(bytecode: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([
        bytecode[pos],
        bytecode[pos + 1],
        bytecode[pos + 2],
        bytecode[pos + 3],
    ])
}

/// Formats a jump offset as a sign and hexadecimal magnitude, for example
/// `+0x0002` or `-0x0003`.
///
/// The magnitude comes from [`i16::unsigned_abs`] rather than from formatting
/// the `i16` directly: `UpperHex` for a signed integer emits the two's
/// complement bit pattern, which would print `-3` as `-0xFFFD`. `unsigned_abs`
/// is also total, so `i16::MIN` in a corrupt container cannot panic.
fn format_jump_offset(value: i16) -> String {
    let sign = if value < 0 { "-" } else { "+" };
    format!("{}0x{:04X}", sign, value.unsigned_abs())
}

/// Looks up a called function's name in the debug section, if the container
/// carries one, and returns it as a display comment.
fn lookup_function_comment(container: &Container, function_id: u16) -> Option<String> {
    let name = &container
        .debug_section
        .as_ref()?
        .func_names
        .iter()
        .find(|entry| entry.function_id.raw() == function_id)?
        .name;
    Some(format!("= {name}"))
}

/// Looks up a constant pool entry by index and returns a display comment.
fn lookup_const_comment(container: &Container, pool_index: u16) -> String {
    let entry = container.constant_pool.iter().nth(pool_index as usize);
    match entry {
        Some(e) => format!("= {}", format_const_value(e.const_type, e.bytes())),
        None => format!("= <invalid pool index {}>", pool_index),
    }
}

/// Converts a byte slice to a lowercase hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::{ContainerBuilder, FunctionId};
    use rstest::rstest;
    use std::io::Cursor;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// A COPY_REGION between two 2-slot regions, preceded and followed by
    /// one-byte instructions so a wrong operand length would visibly
    /// misalign what comes after it.
    fn copy_region_container() -> Container {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x0C, 0x01, 0x00,                          // LOAD_VAR_I32 var[1]
            0xAD, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,  // COPY_REGION var[0], desc[0], desc[1]
            0x8C,                                      // RET_VOID
        ];

        let mut builder = ContainerBuilder::new()
            .num_variables(2)
            .data_region_bytes(32);
        builder.add_array_descriptor(0, 2, 0);
        builder.add_array_descriptor(2, 2, 0);
        let container = builder
            .add_function(FunctionId::new(0), &bytecode, 2, 2, 0)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        Container::read_from(&mut Cursor::new(buf)).unwrap()
    }

    /// Pins both the decoded name and the alignment: an operand layout that
    /// disagreed with the instruction's real length would render the row but
    /// leave everything after it at the wrong offset.
    #[test]
    fn decode_instructions_when_copy_region_then_decodes_and_stays_aligned() {
        let container = copy_region_container();
        let bytecode = container
            .code
            .get_function_bytecode(FunctionId::new(0))
            .unwrap();
        let instructions = decode_instructions(bytecode, &container);

        let opcodes: Vec<&str> = instructions
            .iter()
            .map(|i| i["opcode"].as_str().unwrap())
            .collect();
        assert_eq!(opcodes, vec!["LOAD_VAR_I32", "COPY_REGION", "RET_VOID"]);

        assert_eq!(
            instructions[1]["operands"].as_str().unwrap(),
            "var[0], desc[0], desc[1]"
        );
        // RET_VOID sits at 3 + 7 = 10; anything else means the operand
        // length is wrong.
        assert_eq!(instructions[2]["offset"].as_u64().unwrap(), 10);
    }

    /// Builds the steel thread test container (x := 10; y := x + 32).
    fn steel_thread_container() -> Container {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x0C, 0x00, 0x00,       // LOAD_VAR_I32   var[0]
            0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x20,                   // ADD_I32
            0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]
            0x8C,                   // RET_VOID
        ];

        let container = ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(FunctionId::new(0), &bytecode, 2, 2, 0)
            .build();

        // Round-trip through serialization to fill in offsets
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        Container::read_from(&mut Cursor::new(&buf)).unwrap()
    }

    /// Builds a minimal container whose single function contains the given bytecode.
    ///
    /// The container has three constants -- enough for a pool operand to
    /// resolve to a value -- and no variables. Round-trips through
    /// serialization so all section offsets are populated correctly.
    fn container_with_bytecode(bytecode: Vec<u8>) -> Container {
        let container = ContainerBuilder::new()
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_i32_constant(99)
            .add_function(FunctionId::new(0), &bytecode, 4, 0, 0)
            .build();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        Container::read_from(&mut Cursor::new(&buf)).unwrap()
    }

    /// Returns the first decoded instruction from a container built with the
    /// given bytecode. Convenience wrapper used by single-opcode tests.
    fn first_instruction(bytecode: Vec<u8>) -> serde_json::Value {
        let container = container_with_bytecode(bytecode);
        let result = disassemble(&container);
        result["functions"][0]["instructions"][0].clone()
    }

    // ---------------------------------------------------------------
    // Header tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_when_steel_thread_then_header_has_format_version() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["header"]["formatVersion"], 3);
    }

    #[test]
    fn disassemble_when_steel_thread_then_header_has_num_variables() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["header"]["numVariables"], 2);
    }

    #[test]
    fn disassemble_when_steel_thread_then_header_has_num_functions() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["header"]["numFunctions"], 1);
    }

    #[test]
    fn disassemble_when_steel_thread_then_header_has_task_section() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["header"]["taskSection"]["offset"], 256);
        assert!(result["header"]["taskSection"]["size"].as_u64().unwrap() > 0);
    }

    // ---------------------------------------------------------------
    // Task table tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_when_steel_thread_then_has_task_table() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert!(result["taskTable"].is_object());
    }

    #[test]
    fn disassemble_when_steel_thread_then_task_table_has_one_task() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let tasks = result["taskTable"]["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["taskType"], "Freewheeling");
        assert_eq!(tasks[0]["enabled"], true);
    }

    #[test]
    fn disassemble_when_steel_thread_then_task_table_has_one_program() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let programs = result["taskTable"]["programs"].as_array().unwrap();
        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0]["entryFunctionId"], 0);
        assert_eq!(programs[0]["varTableCount"], 2);
    }

    // ---------------------------------------------------------------
    // Constants tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_when_steel_thread_then_has_two_constants() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["constants"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn disassemble_when_steel_thread_then_first_constant_is_i32_10() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let first = &result["constants"][0];
        assert_eq!(first["index"], 0);
        assert_eq!(first["type"], "I32");
        assert_eq!(first["value"], "10");
    }

    #[test]
    fn disassemble_when_steel_thread_then_second_constant_is_i32_32() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let second = &result["constants"][1];
        assert_eq!(second["index"], 1);
        assert_eq!(second["type"], "I32");
        assert_eq!(second["value"], "32");
    }

    // ---------------------------------------------------------------
    // Functions tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_when_steel_thread_then_has_one_function() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["functions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn disassemble_when_steel_thread_then_function_has_metadata() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let func = &result["functions"][0];
        assert_eq!(func["id"], 0);
        assert_eq!(func["maxStackDepth"], 2);
        assert_eq!(func["numLocals"], 2);
    }

    #[test]
    fn disassemble_when_steel_thread_then_function_has_seven_instructions() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let instructions = result["functions"][0]["instructions"].as_array().unwrap();
        assert_eq!(instructions.len(), 7);
    }

    #[test]
    fn disassemble_when_steel_thread_then_first_instruction_is_load_const() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let instr = &result["functions"][0]["instructions"][0];
        assert_eq!(instr["offset"], 0);
        assert_eq!(instr["opcode"], "LOAD_CONST_I32");
        assert_eq!(instr["operands"], "pool[0]");
        assert_eq!(instr["comment"], "= 10");
    }

    #[test]
    fn disassemble_when_steel_thread_then_add_instruction_has_no_operands() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let instr = &result["functions"][0]["instructions"][4];
        assert_eq!(instr["opcode"], "ADD_I32");
        assert_eq!(instr["operands"], "");
    }

    #[test]
    fn disassemble_when_steel_thread_then_last_instruction_is_ret_void() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        let instructions = result["functions"][0]["instructions"].as_array().unwrap();
        let last = instructions.last().unwrap();
        assert_eq!(last["opcode"], "RET_VOID");
    }

    // ---------------------------------------------------------------
    // File-level tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_file_when_valid_iplc_then_returns_header() {
        let container = steel_thread_container();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&buf).unwrap();
        tmp.flush().unwrap();

        let result = disassemble_file(tmp.path());
        assert_eq!(result["header"]["formatVersion"], 3);
        assert_eq!(result["header"]["numVariables"], 2);
    }

    #[test]
    fn disassemble_file_when_invalid_file_then_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"this is not a valid iplc file").unwrap();
        tmp.flush().unwrap();

        let result = disassemble_file(tmp.path());
        assert!(result["error"].is_string());
    }

    // ---------------------------------------------------------------
    // disassemble_file: missing-file error path
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_file_when_path_does_not_exist_then_returns_error_with_message() {
        let result = disassemble_file(std::path::Path::new("/nonexistent/path/file.iplc"));
        let msg = result["error"].as_str().unwrap();
        assert!(
            msg.contains("Failed to open file"),
            "unexpected message: {msg}"
        );
    }

    // ---------------------------------------------------------------
    // format_const_value: non-I32 types
    // ---------------------------------------------------------------

    #[test]
    fn format_const_value_when_u32_then_formats_correctly() {
        let bytes = 4294967295u32.to_le_bytes(); // u32::MAX
        assert_eq!(format_const_value(ConstType::U32, &bytes), "4294967295");
    }

    #[test]
    fn format_const_value_when_i64_then_formats_correctly() {
        let bytes = (-1i64).to_le_bytes();
        assert_eq!(format_const_value(ConstType::I64, &bytes), "-1");
    }

    #[test]
    fn format_const_value_when_u64_then_formats_correctly() {
        let bytes = 100u64.to_le_bytes();
        assert_eq!(format_const_value(ConstType::U64, &bytes), "100");
    }

    #[test]
    fn format_const_value_when_f32_then_formats_correctly() {
        let bytes = 1.5f32.to_le_bytes();
        assert_eq!(format_const_value(ConstType::F32, &bytes), "1.5");
    }

    #[test]
    fn format_const_value_when_f64_then_formats_correctly() {
        let bytes = 2.5f64.to_le_bytes();
        assert_eq!(format_const_value(ConstType::F64, &bytes), "2.5");
    }

    #[test]
    fn format_const_value_when_too_few_bytes_then_returns_invalid() {
        assert_eq!(
            format_const_value(ConstType::I32, &[0u8; 2]),
            "<invalid: 2 bytes>"
        );
    }

    // ---------------------------------------------------------------
    // hex_string
    // ---------------------------------------------------------------

    #[test]
    fn hex_string_when_bytes_then_returns_lowercase_hex() {
        assert_eq!(hex_string(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    }

    #[test]
    fn hex_string_when_empty_then_returns_empty_string() {
        assert_eq!(hex_string(&[]), "");
    }

    // ---------------------------------------------------------------
    // decode_instructions: no-operand opcodes
    // ---------------------------------------------------------------

    #[rstest]
    #[case::load_true(opcode::LOAD_TRUE, "LOAD_TRUE")]
    #[case::load_false(opcode::LOAD_FALSE, "LOAD_FALSE")]
    #[case::sub_i32(opcode::SUB_I32, "SUB_I32")]
    #[case::mul_i32(opcode::MUL_I32, "MUL_I32")]
    #[case::div_i32(opcode::DIV_I32, "DIV_I32")]
    #[case::mod_i32(opcode::MOD_I32, "MOD_I32")]
    #[case::neg_i32(opcode::NEG_I32, "NEG_I32")]
    #[case::eq_i32(opcode::EQ_I32, "EQ_I32")]
    #[case::ne_i32(opcode::NE_I32, "NE_I32")]
    #[case::lt_i32(opcode::LT_I32, "LT_I32")]
    #[case::le_i32(opcode::LE_I32, "LE_I32")]
    #[case::gt_i32(opcode::GT_I32, "GT_I32")]
    #[case::ge_i32(opcode::GE_I32, "GE_I32")]
    #[case::bool_and(opcode::BOOL_AND, "BOOL_AND")]
    #[case::bool_or(opcode::BOOL_OR, "BOOL_OR")]
    #[case::bool_xor(opcode::BOOL_XOR, "BOOL_XOR")]
    #[case::bool_not(opcode::BOOL_NOT, "BOOL_NOT")]
    #[case::dup(opcode::DUP, "DUP")]
    #[case::swap(opcode::SWAP, "SWAP")]
    fn decode_when_no_operand_opcode_then_opcode_name_and_empty_operands(
        #[case] opcode_byte: u8,
        #[case] expected_name: &str,
    ) {
        let instr = first_instruction(vec![opcode_byte, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], expected_name);
        assert_eq!(instr["operands"], "");
    }

    // ---------------------------------------------------------------
    // decode_instructions: jump opcodes (computed target comment)
    // ---------------------------------------------------------------

    #[test]
    fn decode_when_jmp_forward_then_comment_shows_target_address() {
        // JMP offset=+2: target = 0 + 3 + 2 = 5
        // Note: 0xFE is an unknown opcode (op-class 0x3F is reserved/free), used as
        // padding so the disassembler doesn't try to decode the byte as a valid op.
        let bytecode = vec![
            opcode::JMP,
            0x02,
            0x00,
            opcode::RET_VOID,
            0xFE,
            opcode::RET_VOID,
        ];
        let instr = first_instruction(bytecode);
        assert_eq!(instr["opcode"], "JMP");
        assert_eq!(instr["operands"], "offset: +0x0002");
        assert_eq!(instr["comment"], "-> 0x0005");
    }

    #[test]
    fn decode_when_jmp_if_not_then_comment_shows_target_address() {
        // JMP_IF_NOT offset=-3: target = 0 + 3 + (-3) = 0
        let bytecode = vec![opcode::JMP_IF_NOT, 0xFD, 0xFF, opcode::RET_VOID];
        let instr = first_instruction(bytecode);
        assert_eq!(instr["opcode"], "JMP_IF_NOT");
        assert_eq!(instr["operands"], "offset: -0x0003");
        assert_eq!(instr["comment"], "-> 0x0000");
    }

    #[rstest]
    #[case::jmp_forward(opcode::JMP, "JMP", 2, "offset: +0x0002")]
    // A backward offset prints its magnitude, not the two's complement encoding
    // (which would read as -0xFFFD).
    #[case::jmp_backward(opcode::JMP, "JMP", -3, "offset: -0x0003")]
    #[case::jmp_zero(opcode::JMP, "JMP", 0, "offset: +0x0000")]
    // i16::MIN has no positive counterpart; unsigned_abs keeps the format total.
    #[case::jmp_min(opcode::JMP, "JMP", i16::MIN, "offset: -0x8000")]
    #[case::jmp_if_not_forward(opcode::JMP_IF_NOT, "JMP_IF_NOT", 2, "offset: +0x0002")]
    #[case::jmp_if_not_backward(opcode::JMP_IF_NOT, "JMP_IF_NOT", -3, "offset: -0x0003")]
    fn decode_when_jump_then_operand_is_sign_and_magnitude(
        #[case] opcode_byte: u8,
        #[case] expected_opcode: &str,
        #[case] jump_offset: i16,
        #[case] expected_operands: &str,
    ) {
        let offset_bytes = jump_offset.to_le_bytes();
        let bytecode = vec![
            opcode_byte,
            offset_bytes[0],
            offset_bytes[1],
            opcode::RET_VOID,
        ];
        let instr = first_instruction(bytecode);
        assert_eq!(instr["opcode"], expected_opcode);
        assert_eq!(instr["operands"], expected_operands);
    }

    // CMP_BR_* is 8 bytes: opcode, cmp op, var index, const index, offset. The
    // cases below use var[1] and const[2], so target = 0 + 8 + offset.
    #[rstest]
    #[case::i32_backward(
        opcode::CMP_BR_I32,
        "CMP_BR_I32",
        opcode::cmp_op::LT_S,
        -3,
        "LT_S, var[1], pool[2], offset: -0x0003",
        "= 99, -> 0x0005"
    )]
    #[case::i64_forward(
        opcode::CMP_BR_I64,
        "CMP_BR_I64",
        opcode::cmp_op::EQ,
        2,
        "EQ, var[1], pool[2], offset: +0x0002",
        "= 99, -> 0x000A"
    )]
    #[case::i32_zero(
        opcode::CMP_BR_I32,
        "CMP_BR_I32",
        opcode::cmp_op::GE_S,
        0,
        "GE_S, var[1], pool[2], offset: +0x0000",
        "= 99, -> 0x0008"
    )]
    fn decode_when_cmp_br_then_operand_is_sign_and_magnitude(
        #[case] opcode_byte: u8,
        #[case] expected_opcode: &str,
        #[case] cmp_op_byte: u8,
        #[case] jump_offset: i16,
        #[case] expected_operands: &str,
        #[case] expected_comment: &str,
    ) {
        let offset_bytes = jump_offset.to_le_bytes();
        let bytecode = vec![
            opcode_byte,
            cmp_op_byte,
            0x01,
            0x00,
            0x02,
            0x00,
            offset_bytes[0],
            offset_bytes[1],
            opcode::RET_VOID,
        ];
        let instr = first_instruction(bytecode);
        assert_eq!(instr["opcode"], expected_opcode);
        assert_eq!(instr["operands"], expected_operands);
        assert_eq!(instr["comment"], expected_comment);
    }

    // ---------------------------------------------------------------
    // decode_instructions: BUILTIN named sub-IDs
    // ---------------------------------------------------------------

    fn builtin_instruction(func_id: u16) -> serde_json::Value {
        let id = func_id.to_le_bytes();
        first_instruction(vec![opcode::BUILTIN, id[0], id[1], opcode::RET_VOID])
    }

    #[rstest]
    #[case::expt_i32(opcode::builtin::EXPT_I32, "EXPT_I32 (0x0340)")]
    #[case::abs_i32(opcode::builtin::ABS_I32, "ABS_I32 (0x0343)")]
    #[case::min_i32(opcode::builtin::MIN_I32, "MIN_I32 (0x0344)")]
    #[case::max_f64(opcode::builtin::MAX_F64, "MAX_F64 (0x0359)")]
    #[case::limit_f32(opcode::builtin::LIMIT_F32, "LIMIT_F32 (0x035A)")]
    #[case::sel_i32(opcode::builtin::SEL_I32, "SEL_I32 (0x0347)")]
    #[case::shl_i32(opcode::builtin::SHL_I32, "SHL_I32 (0x0348)")]
    #[case::rol_u8(opcode::builtin::ROL_U8, "ROL_U8 (0x0350)")]
    #[case::bcd_to_int_32(opcode::builtin::BCD_TO_INT_32, "BCD_TO_INT_32 (0x0393)")]
    #[case::int_to_bcd_64(opcode::builtin::INT_TO_BCD_64, "INT_TO_BCD_64 (0x0398)")]
    #[case::sqrt_f32(opcode::builtin::SQRT_F32, "SQRT_F32 (0x035E)")]
    #[case::unknown_id(0x00FF, "0x00FF")]
    fn decode_when_builtin_then_operand_shows_name(
        #[case] func_id: u16,
        #[case] expected_operands: &str,
    ) {
        let instr = builtin_instruction(func_id);
        assert_eq!(instr["operands"], expected_operands);
    }

    #[rstest]
    #[case::mux_i32(opcode::builtin::MUX_I32_BASE + 3, "MUX_I32(3)")]
    #[case::mux_i64(opcode::builtin::MUX_I64_BASE + 2, "MUX_I64(2)")]
    #[case::mux_f32(opcode::builtin::MUX_F32_BASE + 4, "MUX_F32(4)")]
    #[case::mux_f64(opcode::builtin::MUX_F64_BASE + 5, "MUX_F64(5)")]
    fn decode_when_builtin_mux_then_operand_shows_width_and_n(
        #[case] func_id: u16,
        #[case] expected_prefix: &str,
    ) {
        let instr = builtin_instruction(func_id);
        let operands = instr["operands"].as_str().unwrap();
        assert!(operands.starts_with(expected_prefix), "got: {operands}");
    }

    // ---------------------------------------------------------------
    // decode_instructions: METHOD_CALL (8-byte OOP call)
    // ---------------------------------------------------------------

    #[test]
    fn decode_when_method_call_then_shows_func_fields_and_params() {
        // METHOD_CALL func_id=3, field_var_off=7, num_fields=2, param_var_off=9
        let bytecode = vec![
            opcode::METHOD_CALL,
            0x03,
            0x00,
            0x07,
            0x00,
            0x02,
            0x09,
            0x00,
            opcode::RET_VOID,
        ];
        let container = container_with_bytecode(bytecode);
        let result = disassemble(&container);
        let instructions = result["functions"][0]["instructions"].as_array().unwrap();
        // The 8-byte instruction must be consumed whole, leaving only RET_VOID.
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0]["opcode"], "METHOD_CALL");
        assert_eq!(
            instructions[0]["operands"],
            "func[3], fields[7], num_fields: 2, params[9]"
        );
        assert_eq!(instructions[1]["opcode"], "RET_VOID");
    }

    // ---------------------------------------------------------------
    // decode_instructions: completeness over the assigned opcode space
    // ---------------------------------------------------------------

    /// Builds a one-instruction buffer for `op` with all-zero operands.
    fn single_instruction(op: u8) -> Vec<u8> {
        let mut bytecode = vec![op];
        bytecode.resize(opcode::instruction_size(op), 0);
        bytecode
    }

    #[test]
    fn decode_when_opcode_assigned_then_renders_its_mnemonic() {
        // Every opcode the instruction set assigns must render as itself.
        // `UNKNOWN(0x..)` is reserved for bytes that are not opcodes at all.
        for op in 0..=u8::MAX {
            if !opcode::is_assigned(op) {
                continue;
            }
            let instr = first_instruction(single_instruction(op));
            let rendered = instr["opcode"].as_str().unwrap();
            assert_eq!(
                rendered,
                Instruction::decode(op).unwrap().mnemonic,
                "opcode 0x{op:02X} rendered as {rendered}"
            );
        }
    }

    #[test]
    fn decode_when_opcode_assigned_then_consumes_the_whole_instruction() {
        // A row that renders but mis-reads its operands would leave the
        // decoder mid-instruction and garble every row after it.
        for op in 0..=u8::MAX {
            if !opcode::is_assigned(op) {
                continue;
            }
            let mut bytecode = single_instruction(op);
            bytecode.push(opcode::RET_VOID);
            let container = container_with_bytecode(bytecode);
            let result = disassemble(&container);
            let instructions = result["functions"][0]["instructions"].as_array().unwrap();
            assert_eq!(instructions.len(), 2, "opcode 0x{op:02X}");
            assert_eq!(
                instructions[1]["offset"],
                opcode::instruction_size(op),
                "opcode 0x{op:02X}"
            );
        }
    }

    // Opcodes that rendered as UNKNOWN before the viewer became table-driven:
    // a function call, 64-bit and float arithmetic, unsigned comparison, and
    // the typed stores whose matching loads were already handled.
    #[rstest]
    #[case::call(opcode::CALL, "CALL", "func[0], params[0]")]
    #[case::add_i64(opcode::ADD_I64, "ADD_I64", "")]
    #[case::div_u32(opcode::DIV_U32, "DIV_U32", "")]
    #[case::ge_u64(opcode::GE_U64, "GE_U64", "")]
    #[case::add_f64(opcode::ADD_F64, "ADD_F64", "")]
    #[case::neg_f32(opcode::NEG_F32, "NEG_F32", "")]
    #[case::lt_f64(opcode::LT_F64, "LT_F64", "")]
    #[case::store_var_i64(opcode::STORE_VAR_I64, "STORE_VAR_I64", "var[0]")]
    #[case::store_var_f32(opcode::STORE_VAR_F32, "STORE_VAR_F32", "var[0]")]
    #[case::store_var_f64(opcode::STORE_VAR_F64, "STORE_VAR_F64", "var[0]")]
    fn decode_when_previously_unhandled_opcode_then_named_with_operands(
        #[case] opcode_byte: u8,
        #[case] expected_opcode: &str,
        #[case] expected_operands: &str,
    ) {
        let instr = first_instruction(single_instruction(opcode_byte));
        assert_eq!(instr["opcode"], expected_opcode);
        assert_eq!(instr["operands"], expected_operands);
    }

    #[test]
    fn decode_when_call_and_debug_section_names_callee_then_comment_shows_name() {
        // CALL func_id=1, param base 4.
        let bytecode = vec![opcode::CALL, 0x01, 0x00, 0x04, 0x00, opcode::RET_VOID];
        let container = ContainerBuilder::new()
            .add_function(FunctionId::new(0), &bytecode, 2, 0, 0)
            .add_func_name(ironplc_container::FuncNameEntry {
                function_id: FunctionId::new(1),
                name: "COMPUTE".to_string(),
            })
            .build();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        let container = Container::read_from(&mut Cursor::new(&buf)).unwrap();
        let instr = &disassemble(&container)["functions"][0]["instructions"][0];
        assert_eq!(instr["opcode"], "CALL");
        assert_eq!(instr["operands"], "func[1], params[4]");
        assert_eq!(instr["comment"], "= COMPUTE");
    }

    #[test]
    fn decode_when_instruction_runs_past_end_of_bytecode_then_marks_truncated() {
        // A 3-byte LOAD_VAR_I32 with only one operand byte present.
        let instr = first_instruction(vec![opcode::LOAD_VAR_I32, 0x00]);
        assert_eq!(instr["opcode"], "LOAD_VAR_I32");
        assert_eq!(instr["operands"], "<truncated>");
    }

    // ---------------------------------------------------------------
    // decode_instructions: unknown opcode fallback
    // ---------------------------------------------------------------

    #[test]
    fn decode_when_unknown_opcode_then_shows_hex_and_advances_one_byte() {
        let bytecode = vec![0xFE, opcode::RET_VOID];
        let container = container_with_bytecode(bytecode);
        let result = disassemble(&container);
        let instructions = result["functions"][0]["instructions"].as_array().unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0]["opcode"], "UNKNOWN(0xFE)");
        assert_eq!(instructions[0]["operands"], "");
    }

    // ---------------------------------------------------------------
    // decode_instructions: 9-byte string opcodes (u32 + u32 operands)
    // ---------------------------------------------------------------

    /// Builds a 9-byte string-op instruction with two u32 operands.
    fn string_op_bytecode(op: u8, in1: u32, in2: u32) -> Vec<u8> {
        let in1_b = in1.to_le_bytes();
        let in2_b = in2.to_le_bytes();
        vec![
            op,
            in1_b[0],
            in1_b[1],
            in1_b[2],
            in1_b[3],
            in2_b[0],
            in2_b[1],
            in2_b[2],
            in2_b[3],
            opcode::RET_VOID,
        ]
    }

    #[test]
    fn decode_when_find_str_then_opcode_and_two_data_offsets() {
        let instr = first_instruction(string_op_bytecode(opcode::FIND_STR, 0, 0x1C));
        assert_eq!(instr["opcode"], "FIND_STR");
        assert_eq!(instr["operands"], "data[0], data[28]");
    }

    #[test]
    fn decode_when_replace_str_then_opcode_and_two_data_offsets() {
        let instr = first_instruction(string_op_bytecode(opcode::REPLACE_STR, 4, 16));
        assert_eq!(instr["opcode"], "REPLACE_STR");
        assert_eq!(instr["operands"], "data[4], data[16]");
    }

    #[test]
    fn decode_when_insert_str_then_opcode_and_two_data_offsets() {
        let instr = first_instruction(string_op_bytecode(opcode::INSERT_STR, 8, 32));
        assert_eq!(instr["opcode"], "INSERT_STR");
        assert_eq!(instr["operands"], "data[8], data[32]");
    }

    #[test]
    fn decode_when_concat_str_then_opcode_and_two_data_offsets() {
        let instr = first_instruction(string_op_bytecode(opcode::CONCAT_STR, 0, 0x1C));
        assert_eq!(instr["opcode"], "CONCAT_STR");
        assert_eq!(instr["operands"], "data[0], data[28]");
    }

    #[test]
    fn decode_when_find_str_then_advances_nine_bytes() {
        let mut bytecode = string_op_bytecode(opcode::FIND_STR, 1, 2);
        // Append a second instruction so we can confirm the next decode starts at 9.
        bytecode.pop(); // remove RET_VOID added by helper
        bytecode.extend_from_slice(&[opcode::ADD_I32, opcode::RET_VOID]);
        let container = container_with_bytecode(bytecode);
        let result = disassemble(&container);
        let instructions = result["functions"][0]["instructions"].as_array().unwrap();
        assert_eq!(instructions.len(), 3);
        assert_eq!(instructions[0]["opcode"], "FIND_STR");
        assert_eq!(instructions[0]["offset"], 0);
        assert_eq!(instructions[1]["opcode"], "ADD_I32");
        assert_eq!(instructions[1]["offset"], 9);
    }

    // ---------------------------------------------------------------
    // lookup_const_comment: out-of-range pool index
    // ---------------------------------------------------------------

    #[test]
    fn decode_when_const_pool_index_out_of_range_then_comment_shows_invalid() {
        // pool[99] but the pool only has one entry
        let bytecode = vec![opcode::LOAD_CONST_I32, 0x63, 0x00, opcode::RET_VOID];
        let container = ContainerBuilder::new()
            .add_i32_constant(42)
            .add_function(ironplc_container::FunctionId::new(0), &bytecode, 2, 0, 0)
            .build();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        let container = Container::read_from(&mut Cursor::new(&buf)).unwrap();
        let result = disassemble(&container);
        let instr = &result["functions"][0]["instructions"][0];
        assert_eq!(instr["comment"], "= <invalid pool index 99>");
    }
}
