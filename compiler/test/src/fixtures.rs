//! IEC 61131-3 source snippets shared across crates' tests.
//!
//! Each constant is the smallest source that exhibits one shape a test needs
//! (a valid program, a syntax error, a declared variable, a user type, a
//! program that runs, ...). Tests share the *data* only: two tests that use
//! the same constant still assert their own contracts.
//!
//! These are plain `const` strings so an integration test, which cannot see a
//! crate's `#[cfg(test)]` helpers, and a `wasm` crate, which cannot read the
//! filesystem, can both use them.

/// A minimal valid program.
pub const VALID_PROGRAM: &str = "PROGRAM p\nEND_PROGRAM";

/// A program with a syntax error (unterminated declaration).
pub const SYNTAX_ERROR_PROGRAM: &str = "PROGRAM";

/// A program with a semantic error (undeclared variable `y`).
pub const SEMANTIC_ERROR_PROGRAM: &str = "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM";

/// A program declaring one local variable `x`.
pub const PROGRAM_WITH_VAR: &str = "PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM";

/// A program declaring one input `start`.
pub const PROGRAM_WITH_INPUT: &str = "PROGRAM p\nVAR_INPUT start : BOOL; END_VAR\nEND_PROGRAM";

/// A program declaring an initialized input `start` and an uninitialized
/// local `count`.
pub const PROGRAM_WITH_INPUT_AND_LOCAL: &str =
    "PROGRAM p\nVAR_INPUT start : BOOL := FALSE; END_VAR\nVAR count : DINT; END_VAR\nEND_PROGRAM";

/// An enumeration type `MyEnum` declared alongside [`VALID_PROGRAM`].
pub const ENUM_TYPE_PROGRAM: &str = "TYPE MyEnum : (A, B, C); END_TYPE\nPROGRAM p\nEND_PROGRAM";

/// One user-defined type of each kind (enumeration `MotorState`, structure
/// `PidParams`, array `Buf`, subrange `Percent`) declared alongside
/// [`VALID_PROGRAM`].
pub const USER_TYPES_PROGRAM: &str = "TYPE MotorState : (Stopped, Running, Fault); END_TYPE\nTYPE PidParams : STRUCT Kp : REAL; END_STRUCT; END_TYPE\nTYPE Buf : ARRAY[1..10] OF INT; END_TYPE\nTYPE Percent : INT (0..100); END_TYPE\nPROGRAM p\nEND_PROGRAM";

/// A function block `Counter` and a program `Main` that instantiates it, so
/// `Counter` is upstream of `Main`.
pub const PROGRAM_USING_FB: &str = "FUNCTION_BLOCK Counter\nVAR_INPUT Inc : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR c : Counter; END_VAR\nEND_PROGRAM";

/// A program `MotorStartStop` whose only dependency is the standard library
/// timer `TON`.
pub const PROGRAM_USING_STDLIB_FB: &str = "PROGRAM MotorStartStop\nVAR Star_Timer : TON; Run : BOOL; END_VAR\nStar_Timer(IN := Run, PT := T#5s);\nEND_PROGRAM";

/// A program `Main` that increments `Counter` every cycle. It declares no
/// CONFIGURATION, so it compiles to a single freewheeling task.
pub const COUNTER_PROGRAM: &str = "PROGRAM Main
VAR
  Counter : INT;
END_VAR
  Counter := Counter + 1;
END_PROGRAM";

/// [`COUNTER_PROGRAM`] plus a CONFIGURATION that binds `Main` as `program1`
/// to the 100 ms cyclic task `plc_task`. The program text is repeated rather
/// than spliced because `concat!` cannot take a `const`.
pub const COUNTER_PROGRAM_WITH_TASK: &str = "PROGRAM Main
VAR
  Counter : INT;
END_VAR
  Counter := Counter + 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION";
