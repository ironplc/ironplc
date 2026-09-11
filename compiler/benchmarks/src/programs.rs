//! IEC 61131-3 source programs compiled by the benchmark suite.
//!
//! The Criterion benchmarks in `benches/st_benchmark.rs` compile every one
//! of these when the benchmark groups are registered, so a program that the
//! analyzer rejects takes down the whole bench binary before any group
//! runs -- including groups selected by a name filter. Keeping the sources
//! here lets `tests/compile_programs.rs` compile each one as an ordinary
//! test, which fails on its own instead of at `cargo bench` time.
//!
//! Programs with a size parameter are exposed as functions; [`all`] lists
//! every program with a representative parameter.

/// WHILE loop decrementing a counter -- dispatch overhead baseline.
pub const COUNTER_LOOP: &str = "PROGRAM main
  VAR counter : DINT; END_VAR
  WHILE counter > 0 DO
    counter := counter - 1;
  END_WHILE;
END_PROGRAM";

/// Straight-line DINT arithmetic with `reps` statements.
pub fn arithmetic_i32(reps: usize) -> String {
    let mut source = String::from("PROGRAM main\n  VAR x : DINT; END_VAR\n");
    for _ in 0..reps {
        source.push_str("  x := (x + 7 - 3) * 2;\n");
    }
    source.push_str("END_PROGRAM\n");
    source
}

/// Straight-line LREAL arithmetic with `reps` statements.
pub fn arithmetic_f64(reps: usize) -> String {
    let mut source = String::from("PROGRAM main\n  VAR x : LREAL; END_VAR\n");
    for _ in 0..reps {
        source.push_str("  x := (x + 7.0 - 3.0) * 2.0;\n");
    }
    source.push_str("END_PROGRAM\n");
    source
}

/// IF-ELSIF chain with `branches` arms -- worst-case sequential comparison.
pub fn branching(branches: i32) -> String {
    let mut source = String::from("PROGRAM main\n  VAR sel : DINT; result : DINT; END_VAR\n");
    for i in 0..branches {
        if i == 0 {
            source.push_str(&format!("  IF sel = {} THEN\n    result := {};\n", i, i));
        } else {
            source.push_str(&format!("  ELSIF sel = {} THEN\n    result := {};\n", i, i));
        }
    }
    source.push_str("  END_IF;\nEND_PROGRAM\n");
    source
}

/// FOR loop summing 1..limit -- structured loop overhead.
pub const FOR_LOOP: &str = "PROGRAM main
  VAR i : DINT; sum : DINT; limit : DINT; END_VAR
  sum := 0;
  FOR i := 1 TO limit DO
    sum := sum + i;
  END_FOR;
END_PROGRAM";

/// Nested FOR loops -- loop overhead at scale.
pub const NESTED_LOOPS: &str = "PROGRAM main
  VAR i : DINT; j : DINT; acc : DINT;
      outer : DINT; inner : DINT; END_VAR
  acc := 0;
  FOR i := 1 TO outer DO
    FOR j := 1 TO inner DO
      acc := acc + i * j;
    END_FOR;
  END_FOR;
END_PROGRAM";

/// Narrow opcode diversity -- the loop body uses only ~5 distinct opcodes
/// (LOAD_VAR, LOAD_CONST, ADD, STORE_VAR, GT, JMP_IF_NOT, JMP).
pub const NARROW_OPCODES: &str = "PROGRAM main
  VAR i : DINT; x : DINT; limit : DINT; END_VAR
  FOR i := 1 TO limit DO
    x := x + 1;
    x := x + 2;
    x := x + 3;
    x := x + 4;
  END_FOR;
END_PROGRAM";

/// Diverse opcode mix -- the loop body touches many distinct opcode
/// handlers: i32 arithmetic (ADD, SUB, MUL, DIV, MOD, NEG), f64 arithmetic
/// (ADD, SUB, MUL, DIV), comparisons (GT, LT, EQ, NE, GE, LE), boolean
/// logic (AND, OR, XOR, NOT), bitwise ops on DWORD (AND, OR, XOR, NOT),
/// builtins (ABS, MIN, MAX, LIMIT, SQRT, SHL), and type conversions
/// (DINT_TO_LREAL, LREAL_TO_DINT, DINT_TO_DWORD, DWORD_TO_DINT).
///
/// The bit-string operators and `SHL` are typed `ANY_BIT`, so the bitwise
/// work happens on the DWORD variables with `DWORD#` literals, and the
/// DINT results cross into the bit-string domain through explicit
/// conversions rather than implicit widening.
pub const DIVERSE_OPCODES: &str = "PROGRAM main
  VAR
    i : DINT;
    limit : DINT;
    d1 : DINT; d2 : DINT; d3 : DINT;
    f1 : LREAL; f2 : LREAL;
    b1 : BOOL; b2 : BOOL; b3 : BOOL;
    w1 : DWORD; w2 : DWORD;
  END_VAR
  d1 := 100;
  d2 := 7;
  f1 := 3.14;
  b1 := TRUE;
  w1 := DWORD#16#FF00FF00;
  FOR i := 1 TO limit DO
    (* i32 arithmetic: ADD, SUB, MUL, DIV, MOD, NEG *)
    d3 := (d1 + d2) - (d1 * 2);
    d3 := d1 / d2;
    d3 := d1 MOD d2;
    d3 := -d3;

    (* f64 arithmetic: ADD, SUB, MUL, DIV *)
    f2 := (f1 + 1.0) * 2.0;
    f2 := f2 - 0.5;
    f2 := f2 / 3.0;

    (* comparisons: GT, LT, EQ, NE, GE, LE *)
    b1 := d1 > d2;
    b2 := d1 < d2;
    b3 := d1 = d2;
    b1 := d1 <> d2;
    b2 := d1 >= d2;
    b3 := d1 <= d2;

    (* boolean logic: AND, OR, XOR, NOT *)
    b1 := b1 AND b2;
    b1 := b1 OR b3;
    b1 := b1 XOR b2;
    b1 := NOT b1;

    (* bitwise on DWORD: AND, OR, XOR, NOT *)
    w2 := w1 AND DWORD#16#0F0F0F0F;
    w2 := w2 OR DWORD#16#F0F0F0F0;
    w2 := w2 XOR DWORD#16#AAAAAAAA;
    w2 := NOT w2;

    (* builtins: ABS, MIN, MAX, LIMIT, SQRT, SHL *)
    d3 := ABS(d3);
    d3 := MIN(d1, d2);
    d3 := MAX(d1, d2);
    d3 := LIMIT(0, d3, 1000);
    f2 := SQRT(f2 * f2 + 1.0);
    w2 := SHL(w2, 2);

    (* type conversions: DINT_TO_LREAL, LREAL_TO_DINT, DINT_TO_DWORD, DWORD_TO_DINT *)
    f2 := DINT_TO_LREAL(d3);
    d3 := LREAL_TO_DINT(f2);
    w2 := DINT_TO_DWORD(d3);
    d3 := DWORD_TO_DINT(w2);
  END_FOR;
END_PROGRAM";

/// Five distinct INT arithmetic operations on the same operands -- covers
/// ADD, SUB, MUL, DIV, MOD in one scan with no looping.
pub const ARITHMETIC: &str = "PROGRAM arithmetic
  VAR
      a : INT := 20;
      b : INT := 10;
      result_add : INT;
      result_sub : INT;
      result_mul : INT;
      result_div : INT;
      result_mod : INT;
  END_VAR
      result_add := a + b;
      result_sub := a - b;
      result_mul := a * b;
      result_div := a / b;
      result_mod := a MOD b;
  END_PROGRAM";

/// CASE statement state machine with four states that advance per scan.
pub const CASE_STATE: &str = "PROGRAM case_state
  VAR
      state : INT := 0;
      output_a : BOOL := FALSE;
      output_b : BOOL := FALSE;
      output_c : BOOL := FALSE;
      output_d : BOOL := FALSE;
  END_VAR
      CASE state OF
          0:
              output_a := TRUE;
              output_b := FALSE;
              output_c := FALSE;
              output_d := FALSE;
              state := 1;
          1:
              output_a := FALSE;
              output_b := TRUE;
              output_c := FALSE;
              output_d := FALSE;
              state := 2;
          2:
              output_a := FALSE;
              output_b := FALSE;
              output_c := TRUE;
              output_d := FALSE;
              state := 3;
          3:
              output_a := FALSE;
              output_b := FALSE;
              output_c := FALSE;
              output_d := TRUE;
              state := 0;
      END_CASE;
  END_PROGRAM";

/// IF/ELSE counter with a saturation reset -- minimal state-machine shape
/// representative of typical PLC scan code.
pub const COUNTER_UP: &str = "PROGRAM counter_up
  VAR
      count : INT := 0;
      threshold : INT := 1000;
      reset_flag : BOOL := FALSE;
  END_VAR
      IF count >= threshold THEN
          count := 0;
          reset_flag := TRUE;
      ELSE
          count := count + 1;
          reset_flag := FALSE;
      END_IF;
  END_PROGRAM";

/// Every benchmark program paired with a name, using a small representative
/// parameter for the generated ones. The smoke test compiles each entry.
pub fn all() -> Vec<(&'static str, String)> {
    vec![
        ("counter_loop", COUNTER_LOOP.to_string()),
        ("arithmetic_i32", arithmetic_i32(10)),
        ("arithmetic_f64", arithmetic_f64(10)),
        ("branching", branching(5)),
        ("for_loop", FOR_LOOP.to_string()),
        ("nested_loops", NESTED_LOOPS.to_string()),
        ("narrow_opcodes", NARROW_OPCODES.to_string()),
        ("diverse_opcodes", DIVERSE_OPCODES.to_string()),
        ("arithmetic", ARITHMETIC.to_string()),
        ("case_state", CASE_STATE.to_string()),
        ("counter_up", COUNTER_UP.to_string()),
    ]
}
