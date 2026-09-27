//! Conformance to `specs/design/wasm-target.md`.

use ironplc_wasm_runner::{Engine, Plc, Value};
use spec_test_macro::spec_test;

use crate::harness::{compile, differential, Outcome};

/// Runs a program in wasmtime for `cycles` cycles of 100 ms.
fn run(source: &str, cycles: usize) -> Plc {
    let c = compile(source).unwrap_or_else(|o| panic!("{o:?}"));
    let mut plc = Plc::with_engine(&c.wasm.wasm, Engine::Wasmtime).unwrap();
    plc.set_fuel_per_call(Some(1 << 30));
    plc.init().unwrap();
    for _ in 0..cycles {
        plc.cycle(0, 100_000_000).unwrap();
    }
    plc
}

fn same(source: &str) {
    match differential(source, 10) {
        Outcome::Same { .. } => {}
        other => panic!("{other:?}"),
    }
}

#[spec_test(REQ_WT_wasm_010)]
fn integers_when_stored_then_width_and_signedness_of_type() {
    let p = run(
        "PROGRAM main VAR s : SINT; u : USINT; l : LINT; END_VAR s := -1; u := 255; l := -5; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.S").unwrap(), Value::Int(-1));
    assert_eq!(p.read("MAIN.U").unwrap(), Value::UInt(255));
    assert_eq!(p.symbols().leaf("MAIN.L").unwrap().size, 8);
}

#[spec_test(REQ_WT_wasm_011)]
fn bit_strings_when_stored_then_unsigned_of_their_width() {
    let p = run(
        "PROGRAM main VAR w : WORD; END_VAR w := 16#FFFF; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.W").unwrap(), Value::UInt(0xFFFF));
    assert_eq!(p.symbols().leaf("MAIN.W").unwrap().size, 2);
}

#[spec_test(REQ_WT_wasm_012)]
fn time_when_stored_then_32_bit_milliseconds() {
    let p = run(
        "PROGRAM main VAR t : TIME; END_VAR t := T#1500ms; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.T").unwrap(), Value::Int(1500));
    assert_eq!(p.symbols().leaf("MAIN.T").unwrap().size, 4);
}

#[spec_test(REQ_WT_wasm_013)]
fn date_when_stored_then_unsigned_seconds() {
    let p = run(
        "PROGRAM main VAR d : DATE; END_VAR d := D#1970-01-02; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.D").unwrap(), Value::UInt(86_400));
}

#[spec_test(REQ_WT_wasm_020)]
fn narrow_arithmetic_when_intermediate_overflows_then_computed_at_32_bits() {
    let p = run(
        "PROGRAM main VAR s : SINT := 100; r : SINT; END_VAR r := (s + 50) / 2; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.R").unwrap(), Value::Int(75));
}

#[spec_test(REQ_WT_wasm_021)]
fn operation_width_when_destination_is_wider_then_computed_at_result_type() {
    // 100000 * 100000 wraps at 32 bits (10^10 mod 2^32), then widens.
    let p = run(
        "PROGRAM main VAR d : DINT := 100000; l : LINT; END_VAR l := d * d; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.L").unwrap(), Value::Int(1_410_065_408));
}

#[spec_test(REQ_WT_wasm_022)]
fn division_when_divisor_is_zero_then_trap_code_2() {
    let c = compile("PROGRAM main VAR a : DINT := 1; b : DINT; END_VAR a := a / b; END_PROGRAM")
        .unwrap_or_else(|o| panic!("{o:?}"));
    let mut plc = Plc::new(&c.wasm.wasm).unwrap();
    plc.set_fuel_per_call(Some(1 << 30));
    plc.init().unwrap();
    assert_eq!(plc.cycle(0, 1).unwrap_err().code, 2);
}

#[spec_test(REQ_WT_wasm_023)]
fn real_to_integer_when_fraction_then_truncated() {
    let p = run(
        "PROGRAM main VAR r : REAL := 3.7; i : INT; END_VAR i := REAL_TO_INT(r); END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.I").unwrap(), Value::Int(3));
}

#[spec_test(REQ_WT_wasm_024)]
fn for_loop_when_finished_then_control_past_the_end() {
    let p = run(
        "PROGRAM main VAR i : INT; n : INT; END_VAR n := 0; FOR i := 1 TO 10 DO n := n + i; END_FOR; END_PROGRAM",
        1,
    );
    assert_eq!(p.read("MAIN.I").unwrap(), Value::Int(11));
    assert_eq!(p.read("MAIN.N").unwrap(), Value::Int(55));
}

#[spec_test(REQ_WT_wasm_025)]
fn ton_when_input_held_then_same_as_the_vm() {
    same(
        "PROGRAM main VAR t : TON; q : BOOL; et : TIME; c : CTU; n : INT; END_VAR
         t(IN := TRUE, PT := T#350ms, Q => q, ET => et);
         c(CU := q, PV := 3, CV => n);
         END_PROGRAM",
    );
}

#[spec_test(REQ_WT_wasm_030)]
fn leaves_when_instances_then_paths_of_sym_010() {
    let p = run(
        "FUNCTION_BLOCK counter VAR_OUTPUT n : INT; END_VAR n := n + 1; END_FUNCTION_BLOCK
         PROGRAM main VAR c : counter; t : TON; END_VAR c(); END_PROGRAM",
        2,
    );
    assert_eq!(p.read("MAIN.C.N").unwrap(), Value::Int(2));
    assert!(p.symbols().leaf("MAIN.T.Q").is_some());
    assert!(p.symbols().leaf("MAIN.T.START").is_none());
}

#[spec_test(REQ_WT_wasm_031)]
fn leaves_when_array_of_elementary_type_then_one_leaf_with_dimensions() {
    let p = run(
        "PROGRAM main VAR a : ARRAY[1..3] OF INT; END_VAR a[2] := 7; END_PROGRAM",
        1,
    );
    let leaf = p.symbols().leaf("MAIN.A").unwrap();
    assert_eq!(leaf.array.as_ref().unwrap().dims, vec![[1, 3]]);
    assert_eq!(leaf.size, 6);
}

#[spec_test(REQ_WT_wasm_032)]
fn leaves_when_located_then_in_the_region_of_the_prefix() {
    let p = run(
        "PROGRAM main VAR o AT %QW2 : WORD; END_VAR o := 5; END_PROGRAM",
        1,
    );
    let leaf = p.symbols().leaf("MAIN.O").unwrap();
    let out = p.symbols().regions.output;
    assert!(leaf.offset >= out.base && leaf.offset < out.base + out.size);
    assert!(leaf.location.is_some());
}

#[spec_test(REQ_WT_wasm_042)]
fn compile_when_construct_not_lowered_then_p9999() {
    let r = compile("PROGRAM main VAR x : REAL; END_VAR x := SIN(1.0); END_PROGRAM");
    match r {
        Err(Outcome::NoWasm(m)) => assert!(m.starts_with("P9999"), "{m}"),
        Err(o) => panic!("{o:?}"),
        Ok(_) => panic!("SIN is not lowered"),
    }
}

#[spec_test(REQ_WT_wasm_033)]
fn debug_hooks_when_enabled_then_one_site_per_statement() {
    let file = ironplc_dsl::core::FileId::from_string("main.st");
    let source = "PROGRAM main VAR x : INT; END_VAR x := 1; x := x + 1; END_PROGRAM";
    let mut project = ironplc_project::MemoryBackedProject::new(Default::default());
    project.add_source(file.clone(), source.into());
    let (l, c) = ironplc_project::analyze(&mut project, vec![]).unwrap();
    let options = ironplc_wasm::WasmOptions {
        debug_hooks: true,
        ..Default::default()
    };
    let out = ironplc_wasm::compile(l, c, &options, &[(file, source.into())]).unwrap();
    let starts: Vec<u32> = out.symbols.sites.iter().map(|s| s.start).collect();
    assert!(
        starts.contains(&(source.find("x := 1").unwrap() as u32)),
        "{:?}",
        out.symbols.sites
    );
    assert!(starts.contains(&(source.find("x := x").unwrap() as u32)));
    let needle = b"debug_hook";
    assert!(out.wasm.windows(needle.len()).any(|w| w == needle));
}

/// Several tasks and program instances: beyond the VM, so checked on
/// WebAssembly alone. (IronPLC's parser accepts one `RESOURCE` per
/// configuration.)
#[test]
fn configuration_when_two_tasks_then_each_runs_its_instance() {
    let wasm = crate::harness::compile_wasm(
        "PROGRAM counter VAR n : DINT; END_VAR n := n + 1; END_PROGRAM
         CONFIGURATION plant
           RESOURCE cpu ON PLC
             TASK fast (INTERVAL := T#10ms, PRIORITY := 1);
             TASK slow (INTERVAL := T#100ms, PRIORITY := 2);
             PROGRAM a WITH fast : counter;
             PROGRAM b WITH slow : counter;
           END_RESOURCE
         END_CONFIGURATION",
    );
    let mut plc = Plc::with_engine(&wasm.wasm, Engine::Wasmi).unwrap();
    let tasks = &plc.symbols().tasks;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].programs, vec!["A".to_string()]);
    assert_eq!(tasks[1].interval_ns, 100_000_000);
    plc.init().unwrap();
    for _ in 0..3 {
        plc.run_at(0, 0).unwrap();
    }
    plc.run_at(1, 0).unwrap();
    assert_eq!(plc.read("A.N").unwrap(), Value::Int(3));
    assert_eq!(plc.read("B.N").unwrap(), Value::Int(1));
}
