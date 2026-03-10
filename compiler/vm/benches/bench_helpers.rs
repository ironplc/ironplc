//! Shared helpers for VM benchmarks.
//!
//! Builds containers with hand-crafted bytecode for benchmarking specific
//! VM execution patterns. Mirrors the approach used in `tests/common/mod.rs`.

#![allow(dead_code)]

use ironplc_container::{opcode, Container, ContainerBuilder};

/// Builds a container whose scan function is a tight WHILE loop that
/// decrements var[0] until it reaches 0.
///
/// Bytecode equivalent:
/// ```text
/// WHILE var[0] > 0 DO
///     var[0] := var[0] - 1;
/// END_WHILE
/// ```
///
/// This is dispatch-dominated: every iteration executes LOAD_VAR, LOAD_CONST,
/// GT, JMP_IF_NOT, LOAD_VAR, LOAD_CONST, SUB, STORE_VAR, JMP (9 dispatches).
pub fn counter_loop_container() -> Container {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // LOOP (offset 0):
        opcode::LOAD_VAR_I32, 0x00, 0x00,   // LOAD_VAR_I32 var[0]
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)
        opcode::GT_I32,                       // GT_I32
        opcode::JMP_IF_NOT, 0x0D, 0x00,      // JMP_IF_NOT +13 -> END (offset 23)
        // body:
        opcode::LOAD_VAR_I32, 0x00, 0x00,   // LOAD_VAR_I32 var[0]
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        opcode::SUB_I32,                      // SUB_I32
        opcode::STORE_VAR_I32, 0x00, 0x00,   // STORE_VAR_I32 var[0]
        opcode::JMP, 0xE9, 0xFF,              // JMP -23 -> LOOP (offset 0)
        // END (offset 23):
        opcode::RET_VOID,
    ];
    ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(0)
        .add_i32_constant(1)
        .add_function(0, &[opcode::RET_VOID], 0, 1) // init
        .add_function(1, &bytecode, 16, 1) // scan
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container whose scan function performs a chain of i32 arithmetic
/// with no branches. Exercises LOAD_VAR, LOAD_CONST, ADD, SUB, MUL, STORE_VAR.
///
/// Bytecode equivalent:
/// ```text
/// var[0] := ((var[0] + const[0]) - const[1]) * const[2];
/// ```
/// repeated `repetitions` times in straight-line code.
pub fn arithmetic_i32_container(repetitions: usize) -> Container {
    let mut bytecode: Vec<u8> = Vec::new();
    for _ in 0..repetitions {
        #[rustfmt::skip]
        bytecode.extend_from_slice(&[
            opcode::LOAD_VAR_I32, 0x00, 0x00,   // load var[0]
            opcode::LOAD_CONST_I32, 0x00, 0x00,  // load pool[0] (7)
            opcode::ADD_I32,                      // add
            opcode::LOAD_CONST_I32, 0x01, 0x00,  // load pool[1] (3)
            opcode::SUB_I32,                      // sub
            opcode::LOAD_CONST_I32, 0x02, 0x00,  // load pool[2] (2)
            opcode::MUL_I32,                      // mul
            opcode::STORE_VAR_I32, 0x00, 0x00,   // store var[0]
        ]);
    }
    bytecode.push(opcode::RET_VOID);
    ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(7)
        .add_i32_constant(3)
        .add_i32_constant(2)
        .add_function(0, &[opcode::RET_VOID], 0, 1)
        .add_function(1, &bytecode, 16, 1)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container whose scan function performs a chain of f64 arithmetic
/// with no branches. Same pattern as `arithmetic_i32_container` but with
/// floating-point operations.
pub fn arithmetic_f64_container(repetitions: usize) -> Container {
    let mut bytecode: Vec<u8> = Vec::new();
    for _ in 0..repetitions {
        #[rustfmt::skip]
        bytecode.extend_from_slice(&[
            opcode::LOAD_VAR_F64, 0x00, 0x00,   // load var[0]
            opcode::LOAD_CONST_F64, 0x00, 0x00,  // load pool[0] (7.0)
            opcode::ADD_F64,                      // add
            opcode::LOAD_CONST_F64, 0x01, 0x00,  // load pool[1] (3.0)
            opcode::SUB_F64,                      // sub
            opcode::LOAD_CONST_F64, 0x02, 0x00,  // load pool[2] (2.0)
            opcode::MUL_F64,                      // mul
            opcode::STORE_VAR_F64, 0x00, 0x00,   // store var[0]
        ]);
    }
    bytecode.push(opcode::RET_VOID);
    ContainerBuilder::new()
        .num_variables(1)
        .add_f64_constant(7.0)
        .add_f64_constant(3.0)
        .add_f64_constant(2.0)
        .add_function(0, &[opcode::RET_VOID], 0, 1)
        .add_function(1, &bytecode, 16, 1)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an IF-ELSIF chain of `branches` comparisons.
///
/// Bytecode equivalent:
/// ```text
/// IF var[0] = 0 THEN var[1] := 0;
/// ELSIF var[0] = 1 THEN var[1] := 1;
/// ...
/// ELSIF var[0] = N THEN var[1] := N;
/// END_IF
/// ```
///
/// Each branch: LOAD_VAR + LOAD_CONST + EQ + JMP_IF_NOT + LOAD_CONST + STORE_VAR + JMP
/// This exercises branch prediction and dispatch overhead for control flow.
pub fn branching_container(branches: usize) -> Container {
    // Pre-calculate: each branch except the last has a JMP to END.
    // branch body: LOAD_VAR(3) + LOAD_CONST(3) + EQ(1) + JMP_IF_NOT(3) + LOAD_CONST(3) + STORE_VAR(3) + JMP(3) = 19 bytes
    // last branch: LOAD_VAR(3) + LOAD_CONST(3) + EQ(1) + JMP_IF_NOT(3) + LOAD_CONST(3) + STORE_VAR(3) = 16 bytes
    // + RET_VOID(1)
    let branch_size: usize = 19;
    let last_branch_size: usize = 16;
    let total_size = if branches > 1 {
        (branches - 1) * branch_size + last_branch_size + 1
    } else {
        last_branch_size + 1
    };

    let mut bytecode: Vec<u8> = Vec::with_capacity(total_size);

    for i in 0..branches {
        let is_last = i == branches - 1;
        // LOAD_VAR_I32 var[0]
        bytecode.extend_from_slice(&[opcode::LOAD_VAR_I32, 0x00, 0x00]);
        // LOAD_CONST_I32 pool[i] (value i)
        bytecode.extend_from_slice(&[opcode::LOAD_CONST_I32, i as u8, (i >> 8) as u8]);
        // EQ_I32
        bytecode.push(opcode::EQ_I32);

        if is_last {
            // JMP_IF_NOT to RET_VOID (skip LOAD_CONST + STORE = 6 bytes)
            bytecode.extend_from_slice(&[opcode::JMP_IF_NOT, 0x06, 0x00]);
        } else {
            // JMP_IF_NOT to next branch (skip LOAD_CONST + STORE + JMP = 9 bytes)
            bytecode.extend_from_slice(&[opcode::JMP_IF_NOT, 0x09, 0x00]);
        }

        // body: var[1] := i
        bytecode.extend_from_slice(&[opcode::LOAD_CONST_I32, i as u8, (i >> 8) as u8]);
        bytecode.extend_from_slice(&[opcode::STORE_VAR_I32, 0x01, 0x00]);

        if !is_last {
            // JMP to END (skip remaining branches)
            let remaining_bytes = if i + 2 < branches {
                // more branches after next
                (branches - i - 2) * branch_size + last_branch_size
            } else {
                // next is the last branch
                last_branch_size
            };
            let offset = remaining_bytes as i16;
            let offset_bytes = offset.to_le_bytes();
            bytecode.extend_from_slice(&[opcode::JMP, offset_bytes[0], offset_bytes[1]]);
        }
    }

    bytecode.push(opcode::RET_VOID);

    let mut builder = ContainerBuilder::new().num_variables(2);
    for i in 0..branches {
        builder = builder.add_i32_constant(i as i32);
    }
    builder
        .add_function(0, &[opcode::RET_VOID], 0, 2)
        .add_function(1, &bytecode, 16, 2)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with a string assignment: load a string constant into
/// a temp buffer, then store it to a string variable in the data region.
/// Repeated `repetitions` times.
///
/// This exercises string copy overhead (byte-by-byte loops in the current VM).
pub fn string_assign_container(str_len: usize, repetitions: usize) -> Container {
    // String constant: `str_len` bytes of 'A'.
    let str_value: Vec<u8> = vec![b'A'; str_len];

    // Data region layout: one string variable at offset 0.
    // Header: max_length (u16) + cur_length (u16) = 4 bytes, then `str_len` bytes of data.
    let data_region_size = 4 + str_len;

    // Init bytecode: STR_INIT to set up the string header.
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, str_len as u8, (str_len >> 8) as u8,
        opcode::RET_VOID,
    ];

    // Scan bytecode: LOAD_CONST_STR + STR_STORE_VAR repeated.
    let mut scan_bytecode: Vec<u8> = Vec::new();
    for _ in 0..repetitions {
        #[rustfmt::skip]
        scan_bytecode.extend_from_slice(&[
            opcode::LOAD_CONST_STR, 0x00, 0x00,  // load string pool[0] into temp buf
            opcode::STR_STORE_VAR, 0x00, 0x00,    // store temp buf to data_region[0]
        ]);
    }
    scan_bytecode.push(opcode::RET_VOID);

    // Each LOAD_CONST_STR allocates one temp buffer from a bump allocator
    // that does not reset within a single function call, so we need one
    // temp buffer per repetition.
    let temp_buf_size = 4 + str_len;

    ContainerBuilder::new()
        .num_variables(0)
        .data_region_bytes(data_region_size as u32)
        .num_temp_bufs(repetitions as u16)
        .max_temp_buf_bytes(temp_buf_size as u32)
        .add_str_constant(&str_value)
        .add_function(0, &init_bytecode, 4, 0) // init: STR_INIT
        .add_function(1, &scan_bytecode, 16, 0) // scan: assignments
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}
