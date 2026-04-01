//! End-to-end integration tests for deeply nested combinations of arrays,
//! structures, and strings.
//!
//! These tests verify that the full pipeline (parse → analyze → compile → VM)
//! handles complex, multi-level nesting of data types correctly.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::{parse_and_run, parse_and_run_rounds};
use ironplc_container::STRING_HEADER_BYTES;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

#[test]
fn end_to_end_when_three_level_nested_struct_then_leaf_field_correct() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_nested_struct_deep_field_read_then_default_zero() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_struct_field_read_and_array_store_then_roundtrips() {
    // Combines struct field reads with array store/load to verify both
    // data region types coexist without interference.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // sensor=0, readings=1, result_id=2, result_reading=3
    assert_eq!(bufs.vars[2].as_i32(), 7);
    assert_eq!(bufs.vars[3].as_i32(), 7);
}

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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // label=0, data=1, result=2
    assert_eq!(bufs.vars[2].as_i32(), 99);
    // String at start of data region (declared first)
    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "sensor-1");
}

#[test]
fn end_to_end_when_four_level_nested_struct_then_deepest_field_accessible() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_nested_struct_field_used_in_array_loop_then_correct_sum() {
    // Uses struct field values to drive array computation.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // data = [3, 6, 9, 12, 15], sum = 45
    // cfg=0, data=1, sum=2, i=3, mult=4
    assert_eq!(bufs.vars[2].as_i32(), 45);
}

#[test]
fn end_to_end_when_two_structs_and_array_then_no_interference() {
    // Two independent struct instances alongside an array, verifying
    // no data region interference between allocations.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // p1=0, p2=1, distances=2, r1=3, r2=4
    assert_eq!(bufs.vars[3].as_i32(), 30); // 10+20
    assert_eq!(bufs.vars[4].as_i32(), 70); // 30+40
}

#[test]
fn end_to_end_when_2d_array_and_struct_then_both_correct() {
    // 2D array alongside a struct to verify data region coexistence.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // cal=0, matrix=1, result_high=2, result_cell=3
    assert_eq!(bufs.vars[2].as_i32(), 100);
    assert_eq!(bufs.vars[3].as_i32(), 105);
}

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
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        // ctr=0, history=1, scan=2, lim=3
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1);

        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 2);

        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 3);
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // label=0, meta=1, values=2, ver=3, ch=4, total=5
    assert_eq!(bufs.vars[3].as_i32(), 2); // ver
    assert_eq!(bufs.vars[4].as_i32(), 5); // ch
    assert_eq!(bufs.vars[5].as_i32(), 24); // 2+5+7+10
    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "data-log");
}

#[test]
fn end_to_end_when_nested_struct_init_then_inner_fields_initialized() {
    // Verifies nested struct initialization: inner fields receive
    // explicit values rather than being silently default-initialized.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // line=0, r1=1, r2=2, r3=3, r4=4
    assert_eq!(bufs.vars[1].as_i32(), 10);
    assert_eq!(bufs.vars[2].as_i32(), 20);
    assert_eq!(bufs.vars[3].as_i32(), 30);
    assert_eq!(bufs.vars[4].as_i32(), 40);
}

#[test]
fn end_to_end_when_nested_struct_partial_init_then_unspecified_fields_zero() {
    // Only some inner fields are explicitly initialized; the rest
    // should be default-initialized to zero.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // o=0, ra=1, rb=2, rc=3, rtag=4
    assert_eq!(bufs.vars[1].as_i32(), 0); // a: default
    assert_eq!(bufs.vars[2].as_i32(), 42); // b: explicit
    assert_eq!(bufs.vars[3].as_i32(), 0); // c: default
    assert_eq!(bufs.vars[4].as_i32(), 7); // tag: explicit
}

#[test]
fn end_to_end_when_struct_field_store_then_value_updated() {
    // Tests struct field assignment (store), which was added in PR #799.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // c=0, result_total=1, result_count=2
    assert_eq!(bufs.vars[1].as_i32(), 10);
    assert_eq!(bufs.vars[2].as_i32(), 1);
}

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
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        // acc=0, history=1, scan=2
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // scan=1

        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 2); // scan=2

        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 3); // scan=3
    });
}

#[test]
fn end_to_end_when_deeply_nested_init_and_array_loop_then_correct_result() {
    // Combines 3-level nested struct init with array loop computation.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // data = [3, 6, 9, 12, 15], sum of first 5 = 45
    // dev=0, data=1, sum=2, i=3, n=4, mult=5
    assert_eq!(bufs.vars[2].as_i32(), 45);
    assert_eq!(bufs.vars[4].as_i32(), 5);
    assert_eq!(bufs.vars[5].as_i32(), 3);
}

#[test]
fn end_to_end_when_2d_array_and_nested_struct_init_then_both_correct() {
    // 2D array alongside nested struct with explicit init values.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // cal=0, matrix=1, result_high=2, result_cell=3
    assert_eq!(bufs.vars[2].as_i32(), 100);
    assert_eq!(bufs.vars[3].as_i32(), 105);
}
