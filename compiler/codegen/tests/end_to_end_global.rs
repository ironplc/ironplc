//! End-to-end integration tests for global variables (VAR_GLOBAL + VAR_EXTERNAL).
//!
//! These tests verify that global variables declared in a CONFIGURATION block
//! are accessible from a PROGRAM via VAR_EXTERNAL declarations.

#[macro_use]
mod common;

use ironplc_parser::options::{CompilerOptions, Dialect};

// --- CONFIGURATION-based globals accessed via VAR_EXTERNAL ---

e2e_i32!(
    end_to_end_when_global_var_with_initial_value_then_external_reads_value,
    "CONFIGURATION config VAR_GLOBAL shared : INT := 42; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL shared : INT; END_VAR VAR result : INT; END_VAR result := shared; END_PROGRAM",
    &[(0, 42), (1, 42)],
);

e2e_i32!(
    end_to_end_when_global_var_written_via_external_then_value_updated,
    "CONFIGURATION config VAR_GLOBAL counter : DINT := 0; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL counter : DINT; END_VAR counter := counter + 10; END_PROGRAM",
    &[(0, 10)],
);

e2e_i32!(
    end_to_end_when_global_var_no_initial_value_then_defaults_to_zero,
    "CONFIGURATION config VAR_GLOBAL value : INT; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL value : INT; END_VAR VAR result : INT; END_VAR result := value; END_PROGRAM",
    &[(0, 0), (1, 0)],
);

// a=10, b=20, c=TRUE(1), sum=30
e2e_i32!(
    end_to_end_when_multiple_globals_then_all_accessible,
    "CONFIGURATION config VAR_GLOBAL a : INT := 10; b : DINT := 20; c : BOOL := TRUE; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL a : INT; b : DINT; c : BOOL; END_VAR VAR sum : DINT; END_VAR sum := a + b; END_PROGRAM",
    &[(0, 10), (1, 20), (2, 1), (3, 30)],
);

e2e_i32!(
    end_to_end_when_global_constant_then_readable,
    "CONFIGURATION config VAR_GLOBAL CONSTANT max_value : INT := 100; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL CONSTANT max_value : INT; END_VAR VAR result : INT; END_VAR result := max_value; END_PROGRAM",
    &[(0, 100), (1, 100)],
);

e2e_i32!(
    end_to_end_when_global_array_then_external_can_read_elements,
    "CONFIGURATION config VAR_GLOBAL readings : ARRAY[1..3] OF INT := [10, 20, 30]; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL readings : ARRAY[1..3] OF INT; END_VAR VAR first : INT; second : INT; third : INT; END_VAR first := readings[1]; second := readings[2]; third := readings[3]; END_PROGRAM",
    &[(1, 10), (2, 20), (3, 30)],
);

// result = 100 + 200 + 300 = 600
e2e_i32!(
    end_to_end_when_global_array_then_external_can_write_elements,
    "CONFIGURATION config VAR_GLOBAL data : ARRAY[1..3] OF DINT := [0, 0, 0]; END_VAR RESOURCE resource1 ON PLC TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1); PROGRAM plc_task_instance WITH plc_task : main; END_RESOURCE END_CONFIGURATION PROGRAM main VAR_EXTERNAL data : ARRAY[1..3] OF DINT; END_VAR VAR result : DINT; END_VAR data[1] := 100; data[2] := 200; data[3] := 300; result := data[1] + data[2] + data[3]; END_PROGRAM",
    &[(1, 600)],
);

// Minimal smoke test: no CONFIGURATION present.
e2e_i32!(
    end_to_end_when_no_configuration_then_program_still_works,
    "PROGRAM main VAR x : DINT; END_VAR x := 99; END_PROGRAM",
    &[(0, 99)],
);

// --- Top-level VAR_GLOBAL (Rusty dialect). Rusty prepends 2 system-uptime
// globals, so the first user-declared global is at index 2. ---

// phys (idx 2) struct with T0+T1 summed into result (idx 3).
e2e_i32_with!(
    end_to_end_when_top_level_global_struct_then_field_readable_from_program,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "TYPE MY_CONSTANTS : STRUCT T0 : DINT; T1 : DINT; END_STRUCT; END_TYPE VAR_GLOBAL phys : MY_CONSTANTS; END_VAR PROGRAM main VAR result : DINT; END_VAR phys.T0 := 100; phys.T1 := 200; result := phys.T0 + phys.T1; END_PROGRAM",
    &[(3, 300)],
);

e2e_i32_with!(
    end_to_end_when_top_level_global_struct_then_field_readable_from_function,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "TYPE MY_CONSTANTS : STRUCT T0 : DINT; END_STRUCT; END_TYPE VAR_GLOBAL phys : MY_CONSTANTS; END_VAR FUNCTION GET_T0 : DINT VAR_INPUT dummy : DINT; END_VAR GET_T0 := phys.T0; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR phys.T0 := 273; result := GET_T0(dummy := 0); END_PROGRAM",
    &[(3, 273)],
);

e2e_i32_with!(
    end_to_end_when_top_level_global_scalar_then_readable_from_program,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "VAR_GLOBAL counter : DINT := 42; END_VAR PROGRAM main VAR result : DINT; END_VAR result := counter; END_PROGRAM",
    &[(2, 42), (3, 42)],
);

e2e_i32_with!(
    end_to_end_when_top_level_global_scalar_then_readable_from_function,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "VAR_GLOBAL counter : DINT := 42; END_VAR FUNCTION read_counter : DINT read_counter := counter; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := read_counter(); END_PROGRAM",
    &[(2, 42), (3, 42)],
);

e2e_i32_with!(
    end_to_end_when_top_level_global_constant_then_readable_from_function,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "VAR_GLOBAL CONSTANT MY_CONST : DINT := 100; END_VAR FUNCTION use_const : DINT use_const := MY_CONST; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := use_const(); END_PROGRAM",
    &[(2, 100), (3, 100)],
);

// scale_factor (idx 2) = 5; fb_result (idx 3) = 10 * 5 = 50 after FB call.
e2e_i32_with!(
    end_to_end_when_top_level_global_then_writable_from_function_block,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "VAR_GLOBAL scale_factor : DINT := 5; fb_result : DINT := 0; END_VAR FUNCTION_BLOCK Scaler VAR_INPUT value : DINT; END_VAR fb_result := value * scale_factor; END_FUNCTION_BLOCK PROGRAM main VAR s : Scaler; END_VAR s(value := 10); END_PROGRAM",
    &[(2, 5), (3, 50)],
);

// setup.FLAG set TRUE, so IF branch returns x=42 into result (idx 3).
e2e_i32_with!(
    end_to_end_when_global_struct_field_used_as_condition_then_branch_works,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "TYPE SETUP_DATA : STRUCT FLAG : BOOL; END_STRUCT END_TYPE VAR_GLOBAL setup : SETUP_DATA; END_VAR FUNCTION USE_GLOBAL_STRUCT : DINT VAR_INPUT x : DINT; END_VAR IF setup.FLAG THEN USE_GLOBAL_STRUCT := x; ELSE USE_GLOBAL_STRUCT := 0; END_IF; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR setup.FLAG := TRUE; result := USE_GLOBAL_STRUCT(x := 42); END_PROGRAM",
    &[(3, 42)],
);
