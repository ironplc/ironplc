//! Shared container fixtures for tests across the workspace.
//!
//! This module is compiled only for `container`'s own tests or when the
//! `test-support` feature is enabled; it never reaches a normal build. It
//! lives here rather than in `ironplc-vm` because every fixture is built out
//! of [`ContainerBuilder`], which this crate owns, and none of them needs the
//! VM — homing them in `vm` would force `container` into a dev-dependency
//! cycle. `ironplc-vm` re-exports everything here through its own
//! `test_support` module, so VM-side callers keep a single import surface.
//!
//! What belongs here is *scaffolding* — the container shapes that several
//! crates would otherwise hand-assemble. Scenario-specific bytecode (a
//! divide-by-zero program, a cyclic task) stays with the test that owns the
//! scenario; where such a fixture is steel-thread-shaped it composes one of
//! the partially-applied builders below.

use std::io::Cursor;
use std::vec;
use std::vec::Vec;

use crate::builder::ContainerBuilder;
use crate::container::Container;
use crate::debug_section::{iec_type_tag, var_section, VarNameEntry};
use crate::id_types::{FunctionId, VarIndex};
use crate::opcode;

/// The "steel thread" constant pool: `10` at index 0, `32` at index 1.
pub const STEEL_THREAD_CONSTANTS: [i32; 2] = [10, 32];

/// The canonical "steel thread" program: `x := 10; y := x + 32`.
///
/// Bytecode offsets, referenced by breakpoint and line-map tests:
/// `LOAD_CONST@0 STORE_VAR x@3 LOAD_VAR x@6 LOAD_CONST@9 ADD@12
/// STORE_VAR y@13 RET_VOID@16`.
///
/// Expects [`STEEL_THREAD_CONSTANTS`] in the constant pool and two variables.
#[rustfmt::skip]
pub fn steel_thread_bytecode() -> Vec<u8> {
    vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00, // pool[0] (10)
        opcode::STORE_VAR_I32,  0x00, 0x00, // var[0]  (x := 10)
        opcode::LOAD_VAR_I32,   0x00, 0x00, // var[0]  (push x)
        opcode::LOAD_CONST_I32, 0x01, 0x00, // pool[1] (32)
        opcode::ADD_I32,                    //         (10 + 32)
        opcode::STORE_VAR_I32,  0x01, 0x00, // var[1]  (y := 42)
        opcode::RET_VOID,
    ]
}

/// Completes a builder with the standard init (`RET_VOID`) + scan pair.
///
/// The scan function carries `bytecode`; both functions see `num_vars`
/// variables. `max_stack_depth` is a generous 16, which suits every
/// single-function test.
fn scan_scaffold(builder: ContainerBuilder, bytecode: &[u8], num_vars: u16) -> Container {
    builder
        .add_function(FunctionId::INIT, &[opcode::RET_VOID], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .max_call_depth(1)
        .build()
}

/// Builds an init + scan container from `bytecode` with `num_vars` variables
/// and the given i32 constants.
pub fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i32_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with an f32 constant pool.
pub fn single_function_container_f32(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[f32],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_f32_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with an f64 constant pool.
pub fn single_function_container_f64(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[f64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_f64_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with an i64 constant pool.
pub fn single_function_container_i64(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[i64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i64_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with i32 constants followed by i64 constants.
pub fn single_function_container_i32_i64(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    i64_constants: &[i64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in i64_constants {
        builder = builder.add_i64_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with i32 constants followed by f32 constants.
pub fn single_function_container_i32_f32(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    f32_constants: &[f32],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in f32_constants {
        builder = builder.add_f32_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// [`single_function_container`] with i32 constants followed by f64 constants.
pub fn single_function_container_i32_f64(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    f64_constants: &[f64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in f64_constants {
        builder = builder.add_f64_constant(c);
    }
    scan_scaffold(builder, bytecode, num_vars)
}

/// The steel thread as an init + scan container — the VM-side shape.
pub fn steel_thread_container() -> Container {
    single_function_container(&steel_thread_bytecode(), 2, &STEEL_THREAD_CONSTANTS)
}

/// A builder holding the steel thread as a single function (id 0).
///
/// This is the container-format shape: no separate init function, so the
/// synthesized default task table points its one program instance at the
/// program body. Callers chain their own extras (debug entries, tasks,
/// `max_call_depth`) before `build()`.
pub fn steel_thread_single_function_builder() -> ContainerBuilder {
    ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(STEEL_THREAD_CONSTANTS[0])
        .add_i32_constant(STEEL_THREAD_CONSTANTS[1])
        .add_function(FunctionId::INIT, &steel_thread_bytecode(), 2, 2, 0)
}

/// [`steel_thread_single_function_builder`] built as-is.
pub fn steel_thread_single_function_container() -> Container {
    steel_thread_single_function_builder().build()
}

/// [`steel_thread_single_function_builder`] plus debug names for `x` and `y`
/// and a runnable call depth — the shape `ironplcvm` fixtures need in order to
/// dump named variables.
pub fn steel_thread_debug_builder() -> ContainerBuilder {
    steel_thread_single_function_builder()
        .add_var_name(steel_thread_var_name(0, "x"))
        .add_var_name(steel_thread_var_name(1, "y"))
        .max_call_depth(1)
}

/// A global-scope `DINT` debug entry for one of the steel thread's variables.
fn steel_thread_var_name(index: u16, name: &str) -> VarNameEntry {
    VarNameEntry {
        var_index: VarIndex::new(index),
        function_id: FunctionId::GLOBAL_SCOPE,
        var_section: var_section::VAR,
        iec_type_tag: iec_type_tag::DINT,
        name: name.into(),
        type_name: "DINT".into(),
    }
}

/// Builds a container for timer FB tests (TON, TOF, etc.).
///
/// The container runs: load fb_ref, store IN, store PT, call FB, load Q, load ET.
///
/// Variable layout:
///   var[0] = fb_ref (offset 0 into data region)
///   var[1] = IN value (set by test via write_variable)
///   var[2] = Q output (read by test)
///   var[3] = ET output (read by test)
/// Constant layout:
///   constant[0] = PT value (i32 milliseconds)
pub fn timer_test_container(pt_ms: i32, fb_type_id: u16) -> Container {
    let type_id_bytes = fb_type_id.to_le_bytes();
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,              // push fb_ref from var[0]
        opcode::LOAD_VAR_I32,     0x01, 0x00,              // push IN from var[1]
        opcode::FB_STORE_PARAM,   0x00,                     // store to FB.IN (field 0)
        opcode::LOAD_CONST_I32,   0x00, 0x00,              // push PT constant (i32 ms)
        opcode::FB_STORE_PARAM,   0x01,                     // store to FB.PT (field 1)
        opcode::FB_CALL,          type_id_bytes[0], type_id_bytes[1], // call FB
        opcode::FB_LOAD_PARAM,    0x02,                     // load FB.Q (field 2)
        opcode::STORE_VAR_I32,    0x02, 0x00,               // store Q to var[2]
        opcode::FB_LOAD_PARAM,    0x03,                     // load FB.ET (field 3)
        opcode::STORE_VAR_I32,    0x03, 0x00,               // store ET to var[3]
        opcode::POP,                                        // discard fb_ref
        opcode::RET_VOID,
    ];

    let builder = ContainerBuilder::new()
        .num_variables(4)
        .data_region_bytes(48) // 6 fields * 8 bytes
        .add_i32_constant(pt_ms);
    scan_scaffold(builder, &bytecode, 4)
}

/// Serializes a container to its wire bytes.
pub fn container_bytes(container: &Container) -> Vec<u8> {
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    buf
}

/// Serializes a container and parses it back, so section offsets that only
/// the writer computes are populated.
pub fn round_trip(container: &Container) -> Container {
    Container::read_from(&mut Cursor::new(&container_bytes(container))).unwrap()
}
