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
        "sourceHash": hex_string(&h.source_hash),
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
fn decode_instructions(bytecode: &[u8], container: &Container) -> Vec<Value> {
    let mut instructions = Vec::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let opcode_byte = bytecode[pc];
        let offset = pc;

        match opcode_byte {
            opcode::LOAD_CONST_I32 => {
                let pool_index = read_u16(bytecode, pc + 1);
                let comment = lookup_const_comment(container, pool_index);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LOAD_CONST_I32",
                    "operands": format!("pool[{}]", pool_index),
                    "comment": comment,
                }));
                pc += 3;
            }
            opcode::LOAD_TRUE => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LOAD_TRUE",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::LOAD_FALSE => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LOAD_FALSE",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::LOAD_VAR_I32 => {
                let var_index = read_u16(bytecode, pc + 1);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LOAD_VAR_I32",
                    "operands": format!("var[{}]", var_index),
                    "comment": "",
                }));
                pc += 3;
            }
            opcode::STORE_VAR_I32 => {
                let var_index = read_u16(bytecode, pc + 1);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "STORE_VAR_I32",
                    "operands": format!("var[{}]", var_index),
                    "comment": "",
                }));
                pc += 3;
            }
            opcode::ADD_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "ADD_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::SUB_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "SUB_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::MUL_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "MUL_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::DIV_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "DIV_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::MOD_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "MOD_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::NEG_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "NEG_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::EQ_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "EQ_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::NE_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "NE_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::LT_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LT_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::LE_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LE_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::GT_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "GT_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::GE_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "GE_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::BOOL_AND => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "BOOL_AND",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::BOOL_OR => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "BOOL_OR",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::BOOL_XOR => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "BOOL_XOR",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::BOOL_NOT => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "BOOL_NOT",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::JMP => {
                let jump_offset = read_i16(bytecode, pc + 1);
                let target = (pc as isize + 3 + jump_offset as isize) as usize;
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "JMP",
                    "operands": format!("offset: {:+}", jump_offset),
                    "comment": format!("-> {}", target),
                }));
                pc += 3;
            }
            opcode::JMP_IF_NOT => {
                let jump_offset = read_i16(bytecode, pc + 1);
                let target = (pc as isize + 3 + jump_offset as isize) as usize;
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "JMP_IF_NOT",
                    "operands": format!("offset: {:+}", jump_offset),
                    "comment": format!("-> {}", target),
                }));
                pc += 3;
            }
            opcode::BUILTIN => {
                let func_id = read_u16(bytecode, pc + 1);
                let operand = match func_id {
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
                };
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "BUILTIN",
                    "operands": operand,
                    "comment": "",
                }));
                pc += 3;
            }
            opcode::DUP => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "DUP",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::SWAP => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "SWAP",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::RET_VOID => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "RET_VOID",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            opcode::FIND_STR => {
                let in1 = read_u32(bytecode, pc + 1);
                let in2 = read_u32(bytecode, pc + 5);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "FIND_STR",
                    "operands": format!("data[{}], data[{}]", in1, in2),
                    "comment": "",
                }));
                pc += 9;
            }
            opcode::REPLACE_STR => {
                let in1 = read_u32(bytecode, pc + 1);
                let in2 = read_u32(bytecode, pc + 5);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "REPLACE_STR",
                    "operands": format!("data[{}], data[{}]", in1, in2),
                    "comment": "",
                }));
                pc += 9;
            }
            opcode::INSERT_STR => {
                let in1 = read_u32(bytecode, pc + 1);
                let in2 = read_u32(bytecode, pc + 5);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "INSERT_STR",
                    "operands": format!("data[{}], data[{}]", in1, in2),
                    "comment": "",
                }));
                pc += 9;
            }
            opcode::CONCAT_STR => {
                let in1 = read_u32(bytecode, pc + 1);
                let in2 = read_u32(bytecode, pc + 5);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "CONCAT_STR",
                    "operands": format!("data[{}], data[{}]", in1, in2),
                    "comment": "",
                }));
                pc += 9;
            }
            opcode::CMP_BR_I32 | opcode::CMP_BR_I64 => {
                let cmp_op_byte = bytecode[pc + 1];
                let var_idx = read_u16(bytecode, pc + 2);
                let const_idx = read_u16(bytecode, pc + 4);
                let jump_offset = read_i16(bytecode, pc + 6);
                let target = (pc as isize + 8 + jump_offset as isize) as usize;
                let mnemonic = if opcode_byte == opcode::CMP_BR_I32 {
                    "CMP_BR_I32"
                } else {
                    "CMP_BR_I64"
                };
                let cmp_str = match cmp_op_byte {
                    opcode::cmp_op::EQ => "EQ",
                    opcode::cmp_op::NE => "NE",
                    opcode::cmp_op::LT_S => "LT_S",
                    opcode::cmp_op::LE_S => "LE_S",
                    opcode::cmp_op::GT_S => "GT_S",
                    opcode::cmp_op::GE_S => "GE_S",
                    _ => "INVALID",
                };
                instructions.push(json!({
                    "offset": offset,
                    "opcode": mnemonic,
                    "operands": format!("{}, var[{}], const[{}], offset: {:+}", cmp_str, var_idx, const_idx, jump_offset),
                    "comment": format!("-> {}", target),
                }));
                pc += 8;
            }
            unknown => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": format!("UNKNOWN(0x{:02X})", unknown),
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
        }
    }

    instructions
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
    use std::io::Cursor;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
    /// The container has no constants and no variables. Round-trips through
    /// serialization so all section offsets are populated correctly.
    fn container_with_bytecode(bytecode: Vec<u8>) -> Container {
        let container = ContainerBuilder::new()
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

    #[test]
    fn decode_when_load_true_then_opcode_name_and_empty_operands() {
        let instr = first_instruction(vec![opcode::LOAD_TRUE, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "LOAD_TRUE");
        assert_eq!(instr["operands"], "");
    }

    #[test]
    fn decode_when_load_false_then_opcode_name_and_empty_operands() {
        let instr = first_instruction(vec![opcode::LOAD_FALSE, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "LOAD_FALSE");
        assert_eq!(instr["operands"], "");
    }

    #[test]
    fn decode_when_sub_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::SUB_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "SUB_I32");
    }

    #[test]
    fn decode_when_mul_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::MUL_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "MUL_I32");
    }

    #[test]
    fn decode_when_div_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::DIV_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "DIV_I32");
    }

    #[test]
    fn decode_when_mod_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::MOD_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "MOD_I32");
    }

    #[test]
    fn decode_when_neg_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::NEG_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "NEG_I32");
    }

    #[test]
    fn decode_when_eq_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::EQ_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "EQ_I32");
    }

    #[test]
    fn decode_when_ne_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::NE_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "NE_I32");
    }

    #[test]
    fn decode_when_lt_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::LT_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "LT_I32");
    }

    #[test]
    fn decode_when_le_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::LE_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "LE_I32");
    }

    #[test]
    fn decode_when_gt_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::GT_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "GT_I32");
    }

    #[test]
    fn decode_when_ge_i32_then_opcode_name() {
        let instr = first_instruction(vec![opcode::GE_I32, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "GE_I32");
    }

    #[test]
    fn decode_when_bool_and_then_opcode_name() {
        let instr = first_instruction(vec![opcode::BOOL_AND, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "BOOL_AND");
    }

    #[test]
    fn decode_when_bool_or_then_opcode_name() {
        let instr = first_instruction(vec![opcode::BOOL_OR, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "BOOL_OR");
    }

    #[test]
    fn decode_when_bool_xor_then_opcode_name() {
        let instr = first_instruction(vec![opcode::BOOL_XOR, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "BOOL_XOR");
    }

    #[test]
    fn decode_when_bool_not_then_opcode_name() {
        let instr = first_instruction(vec![opcode::BOOL_NOT, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "BOOL_NOT");
    }

    #[test]
    fn decode_when_dup_then_opcode_name() {
        let instr = first_instruction(vec![opcode::DUP, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "DUP");
    }

    #[test]
    fn decode_when_swap_then_opcode_name() {
        let instr = first_instruction(vec![opcode::SWAP, opcode::RET_VOID]);
        assert_eq!(instr["opcode"], "SWAP");
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
        assert_eq!(instr["operands"], "offset: +2");
        assert_eq!(instr["comment"], "-> 5");
    }

    #[test]
    fn decode_when_jmp_if_not_then_comment_shows_target_address() {
        // JMP_IF_NOT offset=-3: target = 0 + 3 + (-3) = 0
        let bytecode = vec![opcode::JMP_IF_NOT, 0xFD, 0xFF, opcode::RET_VOID];
        let instr = first_instruction(bytecode);
        assert_eq!(instr["opcode"], "JMP_IF_NOT");
        assert_eq!(instr["comment"], "-> 0");
    }

    // ---------------------------------------------------------------
    // decode_instructions: BUILTIN named sub-IDs
    // ---------------------------------------------------------------

    fn builtin_instruction(func_id: u16) -> serde_json::Value {
        let id = func_id.to_le_bytes();
        first_instruction(vec![opcode::BUILTIN, id[0], id[1], opcode::RET_VOID])
    }

    #[test]
    fn decode_when_builtin_expt_i32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::EXPT_I32);
        assert_eq!(instr["operands"], "EXPT_I32 (0x0340)");
    }

    #[test]
    fn decode_when_builtin_abs_i32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::ABS_I32);
        assert_eq!(instr["operands"], "ABS_I32 (0x0343)");
    }

    #[test]
    fn decode_when_builtin_min_i32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::MIN_I32);
        assert_eq!(instr["operands"], "MIN_I32 (0x0344)");
    }

    #[test]
    fn decode_when_builtin_max_f64_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::MAX_F64);
        assert_eq!(instr["operands"], "MAX_F64 (0x0359)");
    }

    #[test]
    fn decode_when_builtin_limit_f32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::LIMIT_F32);
        assert_eq!(instr["operands"], "LIMIT_F32 (0x035A)");
    }

    #[test]
    fn decode_when_builtin_sel_i32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::SEL_I32);
        assert_eq!(instr["operands"], "SEL_I32 (0x0347)");
    }

    #[test]
    fn decode_when_builtin_shl_i32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::SHL_I32);
        assert_eq!(instr["operands"], "SHL_I32 (0x0348)");
    }

    #[test]
    fn decode_when_builtin_rol_u8_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::ROL_U8);
        assert_eq!(instr["operands"], "ROL_U8 (0x0350)");
    }

    #[test]
    fn decode_when_builtin_bcd_to_int_32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::BCD_TO_INT_32);
        assert_eq!(instr["operands"], "BCD_TO_INT_32 (0x0393)");
    }

    #[test]
    fn decode_when_builtin_int_to_bcd_64_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::INT_TO_BCD_64);
        assert_eq!(instr["operands"], "INT_TO_BCD_64 (0x0398)");
    }

    #[test]
    fn decode_when_builtin_sqrt_f32_then_operand_shows_name() {
        let instr = builtin_instruction(opcode::builtin::SQRT_F32);
        assert_eq!(instr["operands"], "SQRT_F32 (0x035E)");
    }

    #[test]
    fn decode_when_builtin_mux_i32_then_operand_shows_width_and_n() {
        // MUX_I32_BASE + 3 = MUX with 3 inputs
        let instr = builtin_instruction(opcode::builtin::MUX_I32_BASE + 3);
        let operands = instr["operands"].as_str().unwrap();
        assert!(operands.starts_with("MUX_I32(3)"), "got: {operands}");
    }

    #[test]
    fn decode_when_builtin_mux_i64_then_operand_shows_width_and_n() {
        let instr = builtin_instruction(opcode::builtin::MUX_I64_BASE + 2);
        let operands = instr["operands"].as_str().unwrap();
        assert!(operands.starts_with("MUX_I64(2)"), "got: {operands}");
    }

    #[test]
    fn decode_when_builtin_mux_f32_then_operand_shows_width_and_n() {
        let instr = builtin_instruction(opcode::builtin::MUX_F32_BASE + 4);
        let operands = instr["operands"].as_str().unwrap();
        assert!(operands.starts_with("MUX_F32(4)"), "got: {operands}");
    }

    #[test]
    fn decode_when_builtin_mux_f64_then_operand_shows_width_and_n() {
        let instr = builtin_instruction(opcode::builtin::MUX_F64_BASE + 5);
        let operands = instr["operands"].as_str().unwrap();
        assert!(operands.starts_with("MUX_F64(5)"), "got: {operands}");
    }

    #[test]
    fn decode_when_builtin_unknown_id_then_operand_shows_hex() {
        let instr = builtin_instruction(0x00FF);
        assert_eq!(instr["operands"], "0x00FF");
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
