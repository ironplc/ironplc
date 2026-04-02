//! End-to-end tests for time/date type conversions.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_time_to_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : TIME := T#5s;
    result : DWORD;
  END_VAR
  result := TIME_TO_DWORD(t);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // T#5s = 5000 milliseconds
    assert_eq!(bufs.vars[1].as_i32() as u32, 5000);
}

#[test]
fn end_to_end_when_dword_to_time_then_correct() {
    let source = "
PROGRAM main
  VAR
    dw : DWORD := 3000;
    result : TIME;
  END_VAR
  result := DWORD_TO_TIME(dw);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 3000 milliseconds = T#3s
    assert_eq!(bufs.vars[1].as_i32(), 3000);
}

#[test]
fn end_to_end_when_time_to_dint_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : TIME := T#5s;
    result : DINT;
  END_VAR
  result := TIME_TO_DINT(t);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 5000);
}

#[test]
fn end_to_end_when_time_to_int_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : TIME := T#2s;
    result : INT;
  END_VAR
  result := TIME_TO_INT(t);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 2000);
}

#[test]
fn end_to_end_when_time_to_real_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : TIME := T#5s;
    result : REAL;
  END_VAR
  result := TIME_TO_REAL(t);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f32() - 5000.0).abs() < 1e-1);
}

#[test]
fn end_to_end_when_date_to_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DATE := D#2024-06-15;
    result : DWORD;
  END_VAR
  result := DATE_TO_DWORD(d);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // DATE is stored as unsigned seconds since 1970-01-01
    assert!(bufs.vars[1].as_i32() as u32 > 0);
}

#[test]
fn end_to_end_when_date_to_udint_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DATE := D#2024-06-15;
    result : UDINT;
  END_VAR
  result := DATE_TO_UDINT(d);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!(bufs.vars[1].as_i32() as u32 > 0);
}

#[test]
fn end_to_end_when_tod_to_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    t : TOD := TOD#12:30:00;
    result : DWORD;
  END_VAR
  result := TOD_TO_DWORD(t);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // TOD is stored as unsigned milliseconds since midnight
    // 12:30:00 = 12*3600*1000 + 30*60*1000 = 45_000_000
    assert_eq!(bufs.vars[1].as_i32() as u32, 45_000_000);
}

#[test]
fn end_to_end_when_dt_to_dword_then_correct() {
    let source = "
PROGRAM main
  VAR
    d : DT := DT#2024-06-15-12:30:00;
    result : DWORD;
  END_VAR
  result := DT_TO_DWORD(d);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // DT is stored as unsigned seconds since 1970-01-01
    assert!(bufs.vars[1].as_i32() as u32 > 0);
}
