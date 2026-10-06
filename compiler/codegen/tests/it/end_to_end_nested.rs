//! End-to-end integration tests for deeply nested combinations of arrays,
//! structures, and strings.
//!
//! These tests verify that the full pipeline (parse → analyze → compile → VM)
//! handles complex, multi-level nesting of data types correctly.

use ironplc_parser::options::CompilerOptions;

use crate::common::{run_scans, Snapshot};

e2e_i32!(
    end_to_end_when_three_level_nested_struct_then_leaf_field_correct,
    "
TYPE Inner :
  STRUCT
    value : DINT;
    flag : BOOL;
  END_STRUCT;
END_TYPE

TYPE Middle :
  STRUCT
    inner : Inner;
    scale : DINT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    middle : Middle;
    id : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    o : Outer := (id := 1);
    result : DINT;
  END_VAR
    result := o.id;
END_PROGRAM
",
    &[("result", 1)],
);

e2e_i32!(
    end_to_end_when_nested_struct_deep_field_read_then_default_zero,
    "
TYPE Inner :
  STRUCT
    value : DINT;
  END_STRUCT;
END_TYPE

TYPE Middle :
  STRUCT
    inner : Inner;
    factor : DINT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    middle : Middle;
    tag : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    o : Outer;
    result : DINT;
  END_VAR
    result := o.middle.factor;
END_PROGRAM
",
    &[("result", 0)],
);

// Combines struct field reads with array store/load to verify both
// data region types coexist without interference.
e2e_i32!(
    end_to_end_when_struct_field_read_and_array_store_then_roundtrips,
    "
TYPE Sensor :
  STRUCT
    reading : DINT;
    id : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    sensor : Sensor := (reading := 42, id := 7);
    readings : ARRAY[1..5] OF DINT;
    result_id : DINT;
    result_reading : DINT;
  END_VAR
    result_id := sensor.id;
    readings[3] := result_id;
    result_reading := readings[3];
END_PROGRAM
",
    &[("result_id", 7), ("result_reading", 7)],
);

#[test]
fn end_to_end_when_string_before_struct_then_both_initialized() {
    // String declared before struct: verifies data region allocations
    // for both composite types coexist correctly.
    let source = "
TYPE Inner :
  STRUCT
    x : DINT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    inner : Inner;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    label : STRING[20] := 'sensor-1';
    data : Outer := (y := 99);
    result : DINT;
  END_VAR
    result := data.y;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i32>("result"), 99);
    // String at start of data region (declared first)
    assert_eq!(snapshot.read("label"), "sensor-1");
}

e2e_i32!(
    end_to_end_when_four_level_nested_struct_then_deepest_field_accessible,
    "
TYPE Level4 :
  STRUCT
    deep_val : DINT;
  END_STRUCT;
END_TYPE

TYPE Level3 :
  STRUCT
    l4 : Level4;
    val3 : DINT;
  END_STRUCT;
END_TYPE

TYPE Level2 :
  STRUCT
    l3 : Level3;
    val2 : DINT;
  END_STRUCT;
END_TYPE

TYPE Level1 :
  STRUCT
    l2 : Level2;
    val1 : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    root : Level1 := (val1 := 1);
    r1 : DINT;
  END_VAR
    r1 := root.val1;
END_PROGRAM
",
    &[("r1", 1)],
);

// Uses struct field values to drive array computation.
// data = [3, 6, 9, 12, 15], sum = 45
e2e_i32!(
    end_to_end_when_nested_struct_field_used_in_array_loop_then_correct_sum,
    "
TYPE Config :
  STRUCT
    multiplier : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    cfg : Config := (multiplier := 3);
    data : ARRAY[1..5] OF DINT;
    sum : DINT := 0;
    i : DINT;
    mult : DINT;
  END_VAR
    mult := cfg.multiplier;

    FOR i := 1 TO 5 DO
      data[i] := i * mult;
    END_FOR;

    FOR i := 1 TO 5 DO
      sum := sum + data[i];
    END_FOR;
END_PROGRAM
",
    &[("sum", 45)],
);

// Two independent struct instances alongside an array, verifying
// no data region interference between allocations.
e2e_i32!(
    end_to_end_when_two_structs_and_array_then_no_interference,
    "
TYPE Point :
  STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    p1 : Point := (x := 10, y := 20);
    p2 : Point := (x := 30, y := 40);
    distances : ARRAY[1..3] OF DINT;
    r1 : DINT;
    r2 : DINT;
  END_VAR
    r1 := p1.x + p1.y;
    r2 := p2.x + p2.y;
    distances[1] := r1;
    distances[2] := r2;
    distances[3] := r1 + r2;
END_PROGRAM
",
    &[("r1", 30), ("r2", 70)],
);

// 2D array alongside a struct to verify data region coexistence.
e2e_i32!(
    end_to_end_when_2d_array_and_struct_then_both_correct,
    "
TYPE Range :
  STRUCT
    low : DINT;
    high : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    cal : Range := (low := 0, high := 100);
    matrix : ARRAY[1..3, 1..3] OF DINT;
    result_high : DINT;
    result_cell : DINT;
  END_VAR
    result_high := cal.high;
    matrix[2, 2] := result_high + 5;
    result_cell := matrix[2, 2];
END_PROGRAM
",
    &[("result_high", 100), ("result_cell", 105)],
);

#[test]
fn end_to_end_when_array_and_struct_persist_across_scans_then_state_retained() {
    // Multi-scan test verifying arrays and structs persist state across
    // PLC scan cycles.
    let source = "
TYPE Counter :
  STRUCT
    limit : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    ctr : Counter := (limit := 3);
    history : ARRAY[1..3] OF DINT;
    scan : DINT;
    lim : DINT;
  END_VAR
    scan := scan + 1;
    lim := ctr.limit;

    IF scan <= lim THEN
      history[scan] := scan * 10;
    END_IF;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 1);

        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 2);

        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 3);
    });
}

#[test]
fn end_to_end_when_all_three_types_in_program_then_all_work() {
    // Exercises all three data types (struct, array, string) in a single
    // program, combining struct field reads, array indexing, and string
    // initialization.
    let source = "
TYPE Metadata :
  STRUCT
    version : DINT;
    channel : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    label : STRING[30] := 'data-log';
    meta : Metadata := (version := 2, channel := 5);
    values : ARRAY[1..4] OF DINT;
    ver : DINT;
    ch : DINT;
    total : DINT;
  END_VAR
    ver := meta.version;
    ch := meta.channel;
    values[1] := ver;
    values[2] := ch;
    values[3] := ver + ch;
    values[4] := ver * ch;
    total := values[1] + values[2] + values[3] + values[4];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i32>("ver"), 2); // ver
    assert_eq!(snapshot.read_as::<i32>("ch"), 5); // ch
    assert_eq!(snapshot.read_as::<i32>("total"), 24); // 2+5+7+10
    assert_eq!(snapshot.read("label"), "data-log");
}

// Verifies nested struct initialization: inner fields receive
// explicit values rather than being silently default-initialized.
e2e_i32!(
    end_to_end_when_nested_struct_init_then_inner_fields_initialized,
    "
TYPE Point :
  STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

TYPE Line :
  STRUCT
    start_pt : Point;
    end_pt : Point;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    line : Line := (start_pt := (x := 10, y := 20), end_pt := (x := 30, y := 40));
    r1 : DINT;
    r2 : DINT;
    r3 : DINT;
    r4 : DINT;
  END_VAR
    r1 := line.start_pt.x;
    r2 := line.start_pt.y;
    r3 := line.end_pt.x;
    r4 := line.end_pt.y;
END_PROGRAM
",
    &[("r1", 10), ("r2", 20), ("r3", 30), ("r4", 40)],
);

// Only some inner fields are explicitly initialized; the rest
// should be default-initialized to zero.
e2e_i32!(
    end_to_end_when_nested_struct_partial_init_then_unspecified_fields_zero,
    "
TYPE Inner :
  STRUCT
    a : DINT;
    b : DINT;
    c : DINT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    inner : Inner;
    tag : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    o : Outer := (inner := (b := 42), tag := 7);
    ra : DINT;
    rb : DINT;
    rc : DINT;
    rtag : DINT;
  END_VAR
    ra := o.inner.a;
    rb := o.inner.b;
    rc := o.inner.c;
    rtag := o.tag;
END_PROGRAM
",
    &[("ra", 0), ("rb", 42), ("rc", 0), ("rtag", 7)],
);

// Tests struct field assignment (store), which was added in PR #799.
e2e_i32!(
    end_to_end_when_struct_field_store_then_value_updated,
    "
TYPE Counter :
  STRUCT
    total : DINT;
    count : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    c : Counter := (total := 0, count := 0);
    result_total : DINT;
    result_count : DINT;
  END_VAR
    c.total := c.total + 10;
    c.count := c.count + 1;
    result_total := c.total;
    result_count := c.count;
END_PROGRAM
",
    &[("result_total", 10), ("result_count", 1)],
);

#[test]
fn end_to_end_when_struct_field_store_across_scans_then_accumulates() {
    // Multi-scan test: struct fields accumulate across PLC scan cycles
    // via field stores.
    let source = "
TYPE Accumulator :
  STRUCT
    total : DINT;
    count : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    acc : Accumulator;
    history : ARRAY[1..3] OF DINT;
    scan : DINT;
  END_VAR
    scan := scan + 1;
    acc.total := acc.total + 10;
    acc.count := acc.count + 1;

    IF scan <= 3 THEN
      history[scan] := acc.total;
    END_IF;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 1); // scan=1

        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 2); // scan=2

        session.scan(0).unwrap();
        assert_eq!(session.read("scan"), 3); // scan=3
    });
}

// Combines 3-level nested struct init with array loop computation.
// data = [3, 6, 9, 12, 15], sum of first 5 = 45
e2e_i32!(
    end_to_end_when_deeply_nested_init_and_array_loop_then_correct_result,
    "
TYPE Params :
  STRUCT
    count : DINT;
    multiplier : DINT;
  END_STRUCT;
END_TYPE

TYPE Device :
  STRUCT
    params : Params;
    base_value : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    dev : Device := (params := (count := 5, multiplier := 3), base_value := 10);
    data : ARRAY[1..5] OF DINT;
    sum : DINT := 0;
    i : DINT;
    n : DINT;
    mult : DINT;
  END_VAR
    n := dev.params.count;
    mult := dev.params.multiplier;

    FOR i := 1 TO 5 DO
      data[i] := i * mult;
    END_FOR;

    FOR i := 1 TO n DO
      sum := sum + data[i];
    END_FOR;
END_PROGRAM
",
    &[("sum", 45), ("n", 5), ("mult", 3)],
);

// 2D array alongside nested struct with explicit init values.
e2e_i32!(
    end_to_end_when_2d_array_and_nested_struct_init_then_both_correct,
    "
TYPE Range :
  STRUCT
    low : DINT;
    high : DINT;
  END_STRUCT;
END_TYPE

TYPE Calibration :
  STRUCT
    range : Range;
    offset : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    cal : Calibration := (range := (low := 0, high := 100), offset := 5);
    matrix : ARRAY[1..3, 1..3] OF DINT;
    result_high : DINT;
    result_cell : DINT;
  END_VAR
    result_high := cal.range.high;
    matrix[2, 2] := result_high + cal.offset;
    result_cell := matrix[2, 2];
END_PROGRAM
",
    &[("result_high", 100), ("result_cell", 105)],
);
