//! Runs WebAssembly logic modules (logic module ABI 1.1) for tests.
//!
//! A [`Plc`] instantiates a module in one of two engines, wasmtime (compiled
//! with Cranelift) or wasmi (an interpreter, the engine of microcontrollers),
//! drives it with a virtual clock, and reads variables through the symbol
//! map that the module carries in its `plc.meta` section.
//!
//! The runner exists for the tests of the WebAssembly target: it is not a
//! runtime and does not schedule tasks.

mod engines;

use ironplc_wasm_symbols::{Leaf, SymbolMap};

use crate::engines::{Instance, WasmiInstance, WasmtimeInstance};

/// The engine that runs a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// wasmtime with Cranelift, ahead-of-time compiled.
    Wasmtime,
    /// wasmi, an interpreter.
    Wasmi,
}

impl Engine {
    /// Both engines, in a fixed order.
    pub const ALL: [Engine; 2] = [Engine::Wasmtime, Engine::Wasmi];

    /// Lower-case name of the engine.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Wasmtime => "wasmtime",
            Engine::Wasmi => "wasmi",
        }
    }
}

/// Error of the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub(crate) fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}

/// A trap raised during a call of the module (ABI section 10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trap {
    /// `plc_trap_code`: 0 when the engine raised the trap itself.
    pub code: i32,
    /// `plc_trap_site` when the code is not 0.
    pub site: Option<u32>,
    /// Message of the engine.
    pub message: String,
}

/// The value of a variable, decoded from its type name and size.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `BOOL`.
    Bool(bool),
    /// Signed integers and durations.
    Int(i64),
    /// Unsigned integers, bit strings, dates and times of day.
    UInt(u64),
    /// `REAL` and `LREAL`.
    Real(f64),
    /// `STRING` and `WSTRING`.
    Text(String),
    /// Anything else.
    Bytes(Vec<u8>),
}

/// A logic module instantiated in one engine.
pub struct Plc {
    instance: Box<dyn Instance>,
    symbols: SymbolMap,
    cycles: u64,
    fuel: Option<i64>,
}

impl Plc {
    /// Loads a module in wasmtime.
    pub fn new(wasm: &[u8]) -> Result<Self, Error> {
        Self::with_engine(wasm, Engine::Wasmtime)
    }

    /// Loads a module in the given engine. Imports other than those of
    /// `plc_rt` are refused (ABI-003), and the ABI major version must be 1.
    pub fn with_engine(wasm: &[u8], engine: Engine) -> Result<Self, Error> {
        let symbols = SymbolMap::from_module(wasm).map_err(err)?;
        let mut instance: Box<dyn Instance> = match engine {
            Engine::Wasmtime => Box::new(WasmtimeInstance::new(wasm)?),
            Engine::Wasmi => Box::new(WasmiInstance::new(wasm)?),
        };
        let major = instance.abi_version()?;
        if major != 1 {
            return Err(Error(format!("ABI major version {major}, expected 1")));
        }
        Ok(Plc {
            instance,
            symbols,
            cycles: 0,
            fuel: None,
        })
    }

    /// The symbol map of the module.
    pub fn symbols(&self) -> &SymbolMap {
        &self.symbols
    }

    /// Number of cycles run since the last [`Plc::init`].
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// Calls `plc_init` at time 0 and resets the cycle counter.
    pub fn init(&mut self) -> Result<i32, Trap> {
        self.cycles = 0;
        self.instance.set_now_ns(0);
        if let Some(f) = self.fuel {
            self.instance.set_fuel(f);
        }
        self.instance.init()
    }

    /// Runs one cycle of `task` at the virtual time
    /// `cycles * interval_ns`: the first cycle runs at time 0.
    pub fn cycle(&mut self, task: u32, interval_ns: i64) -> Result<i32, Trap> {
        let now = self.cycles as i64 * interval_ns;
        self.run_at(task, now)
    }

    /// Gives each later call of `plc_init` and `plc_task_run` this much fuel,
    /// for a module generated with fuel instrumentation (ABI-071).
    pub fn set_fuel_per_call(&mut self, fuel: Option<i64>) {
        self.fuel = fuel;
    }

    /// Runs one cycle of `task` with `now_ns` returning `now_ns`.
    pub fn run_at(&mut self, task: u32, now_ns: i64) -> Result<i32, Trap> {
        self.instance.set_now_ns(now_ns);
        if let Some(f) = self.fuel {
            self.instance.set_fuel(f);
        }
        let result = self.instance.run(task as i32);
        self.cycles += 1;
        result
    }

    /// Reads a variable by path (case-insensitive).
    pub fn read(&self, path: &str) -> Result<Value, Error> {
        let leaf = self
            .symbols
            .leaf(path)
            .ok_or_else(|| Error(format!("no variable {path}")))?;
        Ok(decode_leaf(leaf, &self.bytes(leaf)?))
    }

    /// The raw bytes of a variable by path (case-insensitive).
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, Error> {
        let leaf = self
            .symbols
            .leaf(path)
            .ok_or_else(|| Error(format!("no variable {path}")))?;
        self.bytes(leaf)
    }

    /// Writes the raw bytes of a variable by path.
    pub fn write_bytes(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
        let leaf = self
            .symbols
            .leaf(path)
            .ok_or_else(|| Error(format!("no variable {path}")))?;
        if bytes.len() != leaf.size as usize {
            return Err(Error(format!(
                "{path} has {} bytes, not {}",
                leaf.size,
                bytes.len()
            )));
        }
        self.instance.write(leaf.offset, bytes)
    }

    /// Every leaf of the symbol map with its current value, in the order of
    /// the symbol map.
    pub fn snapshot(&self) -> Result<Vec<(String, Value)>, Error> {
        self.symbols
            .leaves
            .iter()
            .map(|leaf| Ok((leaf.path.clone(), decode_leaf(leaf, &self.bytes(leaf)?))))
            .collect()
    }

    fn bytes(&self, leaf: &Leaf) -> Result<Vec<u8>, Error> {
        self.instance.read(leaf.offset, leaf.size)
    }
}

/// Decodes the bytes of a leaf.
pub fn decode_leaf(leaf: &Leaf, bytes: &[u8]) -> Value {
    if leaf.array.is_some() {
        return Value::Bytes(bytes.to_vec());
    }
    decode(&leaf.type_name, bytes)
}

/// Decodes a value of an elementary type from its little-endian bytes. The
/// width comes from the number of bytes, so that the same name decodes
/// whatever representation the compiler chose for it.
pub fn decode(type_name: &str, bytes: &[u8]) -> Value {
    let name = type_name.to_ascii_uppercase();
    let base = name.split('[').next().unwrap_or_default();
    match base {
        "BOOL" if bytes.len() == 1 => Value::Bool(bytes[0] != 0),
        "SINT" | "INT" | "DINT" | "LINT" | "TIME" | "LTIME" => signed(bytes),
        "USINT" | "UINT" | "UDINT" | "ULINT" | "BYTE" | "WORD" | "DWORD" | "LWORD" | "DATE"
        | "TIME_OF_DAY" | "TOD" | "DATE_AND_TIME" | "DT" | "LDATE" | "LTOD" | "LDT"
        | "LTIME_OF_DAY" | "LDATE_AND_TIME" | "CHAR" | "WCHAR" => unsigned(bytes),
        "REAL" if bytes.len() == 4 => {
            Value::Real(f32::from_le_bytes(bytes.try_into().unwrap_or_default()) as f64)
        }
        "LREAL" if bytes.len() == 8 => {
            Value::Real(f64::from_le_bytes(bytes.try_into().unwrap_or_default()))
        }
        "STRING" => {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            Value::Text(bytes[..end].iter().map(|b| *b as char).collect())
        }
        "WSTRING" => {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|u| *u != 0)
                .collect();
            Value::Text(String::from_utf16_lossy(&units))
        }
        _ => Value::Bytes(bytes.to_vec()),
    }
}

fn signed(bytes: &[u8]) -> Value {
    match bytes.len() {
        1 => Value::Int(bytes[0] as i8 as i64),
        2 => Value::Int(i16::from_le_bytes([bytes[0], bytes[1]]) as i64),
        4 => Value::Int(i32::from_le_bytes(bytes.try_into().unwrap_or_default()) as i64),
        8 => Value::Int(i64::from_le_bytes(bytes.try_into().unwrap_or_default())),
        _ => Value::Bytes(bytes.to_vec()),
    }
}

fn unsigned(bytes: &[u8]) -> Value {
    match bytes.len() {
        1 | 2 | 4 | 8 => {
            let mut b = [0u8; 8];
            b[..bytes.len()].copy_from_slice(bytes);
            Value::UInt(u64::from_le_bytes(b))
        }
        _ => Value::Bytes(bytes.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, Value};

    #[test]
    fn decode_when_time_has_four_bytes_then_signed_milliseconds() {
        assert_eq!(decode("TIME", &(-5i32).to_le_bytes()), Value::Int(-5));
    }

    #[test]
    fn decode_when_ltime_has_eight_bytes_then_signed() {
        assert_eq!(decode("LTIME", &(7i64).to_le_bytes()), Value::Int(7));
    }

    #[test]
    fn decode_when_word_then_unsigned() {
        assert_eq!(decode("WORD", &[0xff, 0xff]), Value::UInt(0xffff));
    }

    #[test]
    fn decode_when_sized_string_then_text_up_to_zero() {
        assert_eq!(decode("STRING[4]", b"ab\0\0\0"), Value::Text("ab".into()));
    }

    #[test]
    fn decode_when_wstring_then_text() {
        assert_eq!(
            decode("WSTRING", &[0x41, 0, 0x42, 0, 0, 0]),
            Value::Text("AB".into())
        );
    }

    #[test]
    fn decode_when_real_then_value() {
        assert_eq!(decode("REAL", &1.5f32.to_le_bytes()), Value::Real(1.5));
        assert_eq!(decode("LREAL", &2.5f64.to_le_bytes()), Value::Real(2.5));
    }

    #[test]
    fn decode_when_unknown_type_then_bytes() {
        assert_eq!(decode("MY_STRUCT", &[1, 2]), Value::Bytes(vec![1, 2]));
    }

    #[test]
    fn decode_when_bool_then_value() {
        assert_eq!(decode("BOOL", &[1]), Value::Bool(true));
        assert_eq!(decode("SINT", &[0xff]), Value::Int(-1));
    }
}
