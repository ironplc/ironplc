//! Runs one program on both targets and compares them cycle by cycle.
//!
//! The bytecode container runs on the in-tree VM, the WebAssembly module on
//! wasmtime and on wasmi, all with the same virtual clock. After each cycle
//! the top-level variables the container names in its debug section are
//! read on every engine and compared.

use std::fmt;

use ironplc_codegen::{CodegenOptions, EmptyLookup};
use ironplc_container::debug_section::{function_id, iec_type_tag};
use ironplc_container::FunctionId;
use ironplc_container::{Container, VarIndex};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_project::MemoryBackedProject;
use ironplc_vm::{DebugHook, HookAction, PauseReason, RoundOutcome, Vm, VmBuffers};
use ironplc_wasm::{WasmOptions, WasmOutput};
use ironplc_wasm_runner::{Engine, Plc, Value};

/// A value as both targets are compared: integers widened, reals as
/// `f64` bits so that `NaN` equals itself.
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Bool(bool),
    Int(i128),
    Real(u64),
    Text(String),
}

impl fmt::Display for V {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            V::Bool(b) => write!(f, "{b}"),
            V::Int(i) => write!(f, "{i}"),
            V::Real(r) => write!(f, "{}", f64::from_bits(*r)),
            V::Text(t) => write!(f, "{t:?}"),
        }
    }
}

/// What a run gives: the value of each observed variable after each cycle,
/// or the trap that stopped it.
#[derive(Clone, Debug, PartialEq)]
pub enum Cycle {
    Values(Vec<V>),
    Trap,
    /// The cycle did not end within the step or fuel budget.
    Hang,
}

/// The outcome of a program.
#[derive(Debug)]
pub enum Outcome {
    /// IronPLC does not compile the program with any option set tried.
    NotAccepted,
    /// The bytecode code generator refuses it.
    NoBytecode(String),
    /// The WebAssembly target refuses it.
    NoWasm(String),
    /// Both ran, and every engine agrees on every cycle.
    Same { cycles: usize, variables: usize },
    /// An engine disagrees with the VM.
    Differs(String),
}

/// A program compiled for both targets.
pub struct Compiled {
    pub container: Container,
    pub wasm: WasmOutput,
}

/// Option sets tried in turn: the default dialect, edition 3, then every
/// extension flag.
pub fn option_sets() -> Vec<CompilerOptions> {
    let mut all = CompilerOptions::default();
    for f in CompilerOptions::FEATURE_DESCRIPTORS {
        all.set_flag_by_key(f.option_key, true);
    }
    vec![
        CompilerOptions::default(),
        CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Iec61131_3Ed3),
        all,
    ]
}

/// Compiles for both targets; `Err` carries the outcome when one refuses.
pub fn compile(source: &str) -> Result<Compiled, Outcome> {
    let file = FileId::from_string("main.st");
    for options in option_sets() {
        let mut project = MemoryBackedProject::new(options);
        project.add_source(file.clone(), source.to_string());
        let Ok((library, context)) = ironplc_project::analyze(&mut project, vec![]) else {
            continue;
        };
        let container = ironplc_codegen::compile(
            library,
            context,
            &CodegenOptions::from(&options),
            &EmptyLookup,
        )
        .map_err(|d| Outcome::NoBytecode(format!("{} {}", d.code, d.primary.message)))?;
        let wasm = ironplc_wasm::compile(
            library,
            context,
            &WasmOptions {
                fuel: true,
                ..WasmOptions::default()
            },
            &[(file.clone(), source.to_string())],
        )
        .map_err(|d| Outcome::NoWasm(format!("{} {}", d.code, d.primary.message)))?;
        return Ok(Compiled { container, wasm });
    }
    Err(Outcome::NotAccepted)
}

/// Compiles for the WebAssembly target alone, for what the VM cannot run.
pub fn compile_wasm(source: &str) -> WasmOutput {
    let file = FileId::from_string("main.st");
    let mut project = MemoryBackedProject::new(CompilerOptions::default());
    project.add_source(file.clone(), source.to_string());
    let (library, context) =
        ironplc_project::analyze(&mut project, vec![]).unwrap_or_else(|d| panic!("{d:?}"));
    let options = WasmOptions::default();
    ironplc_wasm::compile(library, context, &options, &[(file, source.to_string())])
        .unwrap_or_else(|d| panic!("{d:?}"))
}

/// A variable observed on both targets.
struct Observed {
    name: String,
    index: VarIndex,
    tag: u8,
    path: String,
    string: Option<(u32, bool)>,
}

fn observed(c: &Compiled) -> Vec<Observed> {
    let Some(debug) = &c.container.debug_section else {
        return vec![];
    };
    let symbols = &c.wasm.symbols;
    let instance = symbols
        .tasks
        .iter()
        .flat_map(|t| t.programs.iter())
        .next()
        .cloned()
        .unwrap_or_default();
    let mut out = vec![];
    for v in &debug.var_names {
        if v.function_id != function_id::GLOBAL_SCOPE {
            continue;
        }
        if matches!(
            v.iec_type_tag,
            iec_type_tag::STRUCT | iec_type_tag::ARRAY | iec_type_tag::FB_INSTANCE
        ) {
            continue;
        }
        // A reference holds a variable index on the VM and an address in
        // WebAssembly: the values it designates are compared, not it.
        let upper = v.type_name.to_uppercase();
        if upper.starts_with("REF_TO") || upper.starts_with("POINTER") {
            continue;
        }
        let name = v.name.to_uppercase();
        let path = if symbols.leaf(&name).is_some() {
            name.clone()
        } else {
            format!("{instance}.{name}")
        };
        let string = debug
            .string_layouts
            .iter()
            .find(|s| s.var_index == v.var_index)
            .map(|s| (s.data_offset, v.iec_type_tag == iec_type_tag::WSTRING));
        out.push(Observed {
            name: v.name.clone(),
            index: v.var_index,
            tag: v.iec_type_tag,
            path,
            string,
        });
    }
    out
}

/// Decodes the bytes of a WebAssembly variable as the VM tag says.
fn from_bytes(tag: u8, bytes: &[u8]) -> V {
    let mut b = [0u8; 8];
    let n = bytes.len().min(8);
    b[..n].copy_from_slice(&bytes[..n]);
    let raw = u64::from_le_bytes(b);
    from_raw(tag, raw, n as u32)
}

/// Decodes a value of `size` bytes from its bits.
fn from_raw(tag: u8, raw: u64, size: u32) -> V {
    use iec_type_tag::*;
    let bits = size * 8;
    let masked = if bits >= 64 {
        raw
    } else {
        raw & ((1u64 << bits) - 1)
    };
    let signed = || {
        let shift = 64 - bits;
        V::Int((((masked << shift) as i64) >> shift) as i128)
    };
    match tag {
        BOOL => V::Bool(masked != 0),
        REAL => V::Real((f32::from_bits(masked as u32) as f64).to_bits()),
        LREAL => V::Real(raw),
        SINT | INT | DINT | LINT | TIME | LTIME | OTHER => signed(),
        _ => V::Int(masked as i128),
    }
}

fn vm_string(data: &[u8], offset: u32, wide: bool) -> V {
    let o = offset as usize;
    let len = u16::from_le_bytes([data[o + 2], data[o + 3]]) as usize;
    let body = &data[o + 6..];
    if wide {
        let units: Vec<u16> = (0..len)
            .map(|i| u16::from_le_bytes([body[2 * i], body[2 * i + 1]]))
            .collect();
        V::Text(String::from_utf16_lossy(&units))
    } else {
        V::Text(body[..len].iter().map(|b| *b as char).collect())
    }
}

/// Microseconds between two cycles: the interval of the task, 100 ms for a
/// freewheeling one.
fn period_us(c: &Compiled) -> u64 {
    c.wasm
        .symbols
        .tasks
        .iter()
        .find(|t| !t.programs.is_empty())
        .map(|t| t.interval_ns / 1000)
        .filter(|us| *us > 0)
        .unwrap_or(100_000)
}

fn task(c: &Compiled) -> u32 {
    c.wasm
        .symbols
        .tasks
        .iter()
        .find(|t| !t.programs.is_empty())
        .map(|t| t.id)
        .unwrap_or(0)
}

/// Instructions a VM cycle may execute before it counts as not ending.
const STEPS: u64 = 2_000_000;
/// Fuel of a WebAssembly call, the same bound in WebAssembly instructions.
const FUEL: i64 = 20_000_000;

/// Pauses a scan that runs longer than [`STEPS`] instructions.
struct StepLimit(u64);

impl DebugHook for StepLimit {
    fn before_instruction(&mut self, _: FunctionId, _: usize, _: u8) -> HookAction {
        self.0 += 1;
        if self.0 > STEPS {
            HookAction::Pause(PauseReason::Step)
        } else {
            HookAction::Continue
        }
    }
}

/// Runs the container on the VM.
pub fn run_vm(c: &Compiled, cycles: usize) -> Vec<Cycle> {
    let vars = observed(c);
    let mut bufs = VmBuffers::from_container(&c.container);
    let Ok(ready) = Vm::new().load(&c.container, &mut bufs) else {
        return vec![Cycle::Trap];
    };
    let Ok(mut vm) = ready.start() else {
        return vec![Cycle::Trap];
    };
    let period = period_us(c);
    let mut out = vec![];
    for k in 0..cycles {
        match vm.run_round_debug(k as u64 * period, &mut StepLimit(0)) {
            Ok(RoundOutcome::Completed) => {}
            Ok(_) => {
                out.push(Cycle::Hang);
                break;
            }
            Err(_) => {
                out.push(Cycle::Trap);
                break;
            }
        }
        let values = vars
            .iter()
            .map(|v| match v.string {
                Some((off, wide)) => vm_string(vm.data_region(), off, wide),
                None => {
                    let raw = vm.read_variable_raw(v.index).unwrap_or(0);
                    let size = c.wasm.symbols.leaf(&v.path).map(|l| l.size).unwrap_or(8);
                    from_raw(v.tag, raw, size)
                }
            })
            .collect();
        out.push(Cycle::Values(values));
    }
    out
}

/// Runs the module on one engine.
pub fn run_wasm(c: &Compiled, engine: Engine, cycles: usize) -> Result<Vec<Cycle>, String> {
    let vars = observed(c);
    let mut plc = Plc::with_engine(&c.wasm.wasm, engine).map_err(|e| e.to_string())?;
    plc.set_fuel_per_call(Some(FUEL));
    if plc.init().is_err() {
        return Ok(vec![Cycle::Trap]);
    }
    let period = period_us(c) as i64 * 1000;
    let task = task(c);
    let mut out = vec![];
    for k in 0..cycles {
        if let Err(t) = plc.run_at(task, k as i64 * period) {
            out.push(if t.code == 1 {
                Cycle::Hang
            } else {
                Cycle::Trap
            });
            break;
        }
        let mut values = vec![];
        for v in &vars {
            let value = match plc.read(&v.path).map_err(|e| e.to_string())? {
                Value::Text(t) if v.string.is_some() => V::Text(t),
                _ => from_bytes(v.tag, &plc.read_bytes(&v.path).map_err(|e| e.to_string())?),
            };
            values.push(value);
        }
        out.push(Cycle::Values(values));
    }
    Ok(out)
}

/// Compiles and runs a program on every engine for `cycles` cycles.
pub fn differential(source: &str, cycles: usize) -> Outcome {
    differential_on(source, cycles, &Engine::ALL)
}

/// Compiles and runs a program on the VM and on `engines`.
pub fn differential_on(source: &str, cycles: usize, engines: &[Engine]) -> Outcome {
    let c = match compile(source) {
        Ok(c) => c,
        Err(o) => return o,
    };
    let names: Vec<String> = observed(&c).into_iter().map(|v| v.name).collect();
    let reference = run_vm(&c, cycles);
    for &engine in engines {
        let got = match run_wasm(&c, engine, cycles) {
            Ok(r) => r,
            Err(e) => return Outcome::Differs(format!("{}: {e}", engine.name())),
        };
        if let Some(d) = first_difference(&reference, &got, &names) {
            return Outcome::Differs(format!("{}: {d}", engine.name()));
        }
    }
    Outcome::Same {
        cycles: reference.len(),
        variables: names.len(),
    }
}

fn first_difference(vm: &[Cycle], wasm: &[Cycle], names: &[String]) -> Option<String> {
    for (k, (a, b)) in vm.iter().zip(wasm).enumerate() {
        match (a, b) {
            (Cycle::Values(x), Cycle::Values(y)) => {
                for (i, (p, q)) in x.iter().zip(y).enumerate() {
                    if p != q {
                        return Some(format!(
                            "cycle {k}: {} is {p} on the VM, {q} in WebAssembly",
                            names[i]
                        ));
                    }
                }
            }
            (Cycle::Trap, Cycle::Trap) | (Cycle::Hang, Cycle::Hang) => {}
            (x, y) => return Some(format!("cycle {k}: {x:?} on the VM, {y:?} in WebAssembly")),
        }
    }
    if vm.len() != wasm.len() {
        return Some(format!(
            "{} cycles on the VM, {} in WebAssembly",
            vm.len(),
            wasm.len()
        ));
    }
    None
}
