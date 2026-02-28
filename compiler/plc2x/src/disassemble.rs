//! Disassembler for IPLC bytecode containers.
//!
//! Reads an IPLC bytecode container and produces structured JSON suitable
//! for display in a VS Code custom editor. The output includes the file
//! header, constant pool entries, and decoded bytecode instructions with
//! cross-referenced operands.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ironplc_container::{ConstType, Container};
use serde_json::{json, Value};

/// Opcode constants matching `compiler/vm/src/opcode.rs`.
const LOAD_CONST_I32: u8 = 0x01;
const LOAD_VAR_I32: u8 = 0x10;
const STORE_VAR_I32: u8 = 0x18;
const ADD_I32: u8 = 0x30;
const SUB_I32: u8 = 0x31;
const MUL_I32: u8 = 0x32;
const DIV_I32: u8 = 0x33;
const MOD_I32: u8 = 0x34;
const RET_VOID: u8 = 0xB5;

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
        "numFbInstances": h.num_fb_instances,
        "totalFbInstanceBytes": h.total_fb_instance_bytes,
        "totalStrVarBytes": h.total_str_var_bytes,
        "totalWstrVarBytes": h.total_wstr_var_bytes,
        "numTempStrBufs": h.num_temp_str_bufs,
        "numTempWstrBufs": h.num_temp_wstr_bufs,
        "maxStrLength": h.max_str_length,
        "maxWstrLength": h.max_wstr_length,
        "numFunctions": h.num_functions,
        "numFbTypes": h.num_fb_types,
        "numArrays": h.num_arrays,
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
                "taskId": t.task_id,
                "priority": t.priority,
                "taskType": t.task_type.as_str(),
                "enabled": (t.flags & 0x01) != 0,
                "intervalUs": t.interval_us,
                "singleVarIndex": t.single_var_index,
                "watchdogUs": t.watchdog_us,
            })
        })
        .collect();

    let programs: Vec<Value> = tt
        .programs
        .iter()
        .map(|p| {
            json!({
                "instanceId": p.instance_id,
                "taskId": p.task_id,
                "entryFunctionId": p.entry_function_id,
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
            let value_str = format_const_value(entry.const_type, &entry.value);
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
                "id": func.function_id,
                "bytecodeOffset": func.bytecode_offset,
                "bytecodeLength": func.bytecode_length,
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
            LOAD_CONST_I32 => {
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
            LOAD_VAR_I32 => {
                let var_index = read_u16(bytecode, pc + 1);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "LOAD_VAR_I32",
                    "operands": format!("var[{}]", var_index),
                    "comment": "",
                }));
                pc += 3;
            }
            STORE_VAR_I32 => {
                let var_index = read_u16(bytecode, pc + 1);
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "STORE_VAR_I32",
                    "operands": format!("var[{}]", var_index),
                    "comment": "",
                }));
                pc += 3;
            }
            ADD_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "ADD_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            SUB_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "SUB_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            MUL_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "MUL_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            DIV_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "DIV_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            MOD_I32 => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "MOD_I32",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
            }
            RET_VOID => {
                instructions.push(json!({
                    "offset": offset,
                    "opcode": "RET_VOID",
                    "operands": "",
                    "comment": "",
                }));
                pc += 1;
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

/// Looks up a constant pool entry by index and returns a display comment.
fn lookup_const_comment(container: &Container, pool_index: u16) -> String {
    let entry = container.constant_pool.iter().nth(pool_index as usize);
    match entry {
        Some(e) => format!("= {}", format_const_value(e.const_type, &e.value)),
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
    use ironplc_container::ContainerBuilder;
    use std::io::Cursor;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Builds the steel thread test container (x := 10; y := x + 32).
    fn steel_thread_container() -> Container {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x30,                   // ADD_I32
            0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]
            0xB5,                   // RET_VOID
        ];

        let container = ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(0, &bytecode, 2, 2)
            .build();

        // Round-trip through serialization to fill in offsets
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        Container::read_from(&mut Cursor::new(&buf)).unwrap()
    }

    // ---------------------------------------------------------------
    // Header tests
    // ---------------------------------------------------------------

    #[test]
    fn disassemble_when_steel_thread_then_header_has_format_version() {
        let container = steel_thread_container();
        let result = disassemble(&container);
        assert_eq!(result["header"]["formatVersion"], 1);
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
        assert_eq!(result["header"]["formatVersion"], 1);
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
}
