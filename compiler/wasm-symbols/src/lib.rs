//! Symbol map of the WebAssembly target (`docs/spec/symbol-map.md`, schema version 1): the
//! model, its JSON form (sidecar file) and its CBOR form (custom section
//! `plc.meta` of a logic module).

use serde::{Deserialize, Serialize};

/// Leaf flags (`symbol-map.md`, section 3).
pub mod flags {
    /// Retentive variable.
    pub const RETAIN: u32 = 1;
    /// `CONSTANT` variable.
    pub const CONSTANT: u32 = 2;
    /// `VAR_INPUT`.
    pub const INPUT: u32 = 4;
    /// `VAR_OUTPUT`.
    pub const OUTPUT: u32 = 8;
    /// Located variable.
    pub const LOCATED: u32 = 16;
    /// SFC step state.
    pub const SFC_STEP: u32 = 32;
    /// `VAR_IN_OUT`.
    pub const IN_OUT: u32 = 64;
    /// `VAR_EXTERNAL`.
    pub const EXTERNAL: u32 = 128;
    /// `VAR_GLOBAL`.
    pub const GLOBAL: u32 = 256;
    /// `VAR_STAT`.
    pub const STAT: u32 = 512;
}

/// Name of the custom section holding the CBOR symbol map.
pub const SECTION: &str = "plc.meta";

/// ABI version of the modules described.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Abi {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
}

/// A source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    /// Index in the list of sources.
    pub id: u32,
    /// File name.
    pub name: String,
    /// SHA-256 of the text, in lower-case hexadecimal.
    pub sha256: String,
}

/// A memory region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// First address.
    pub base: u32,
    /// Size in bytes.
    pub size: u32,
}

/// The memory regions of a module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regions {
    /// Static data.
    #[serde(rename = "static")]
    pub static_: Region,
    /// Retentive data.
    pub retain: Region,
    /// Located inputs.
    pub input: Region,
    /// Located outputs.
    pub output: Region,
    /// Located markers.
    pub marker: Region,
}

/// A task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Argument of `plc_task_run`.
    pub id: u32,
    /// Name.
    pub name: String,
    /// Kind: `"cyclic"`.
    pub kind: String,
    /// Interval in nanoseconds.
    pub interval_ns: u64,
    /// Priority, 0 being the highest.
    pub priority: u32,
    /// Program instance paths, in execution order.
    pub programs: Vec<String>,
}

impl Task {
    /// A cyclic task.
    pub fn cyclic(
        id: u32,
        name: &str,
        interval_ns: u64,
        priority: u32,
        programs: Vec<String>,
    ) -> Self {
        Task {
            id,
            name: name.to_string(),
            kind: "cyclic".into(),
            interval_ns,
            priority,
            programs,
        }
    }
}

/// A value of an enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumValue {
    /// Name.
    pub name: String,
    /// Value.
    pub value: i64,
}

/// Array descriptor of a leaf (SYM-013).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrayInfo {
    /// Dimensions: low and high index.
    pub dims: Vec<[i64; 2]>,
    /// Element stride in bytes.
    pub stride: u32,
}

/// A variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leaf {
    /// Upper-case path (SYM-010).
    pub path: String,
    /// Type name.
    #[serde(rename = "type")]
    pub type_name: String,
    /// Address.
    pub offset: u32,
    /// Size in bytes.
    pub size: u32,
    /// Stored in the retain region.
    pub retain: bool,
    /// Flags (see [`flags`]).
    pub flags: u32,
    /// Direct address of a located variable.
    pub location: Option<String>,
    /// Values of an enumeration.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enumeration: Option<Vec<EnumValue>>,
    /// Array descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array: Option<ArrayInfo>,
    /// Index into `sites` of the declaration.
    pub declared: u32,
}

/// A source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// File index.
    pub file: u32,
    /// Start byte offset.
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

/// The symbol map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolMap {
    /// `"ironplc-symbols"`.
    pub format: String,
    /// Schema version (1).
    pub version: u32,
    /// ABI version.
    pub abi: Abi,
    /// Compiler name and version.
    pub compiler: String,
    /// Source files.
    pub files: Vec<File>,
    /// Memory regions.
    pub regions: Regions,
    /// Tasks.
    pub tasks: Vec<Task>,
    /// Variables.
    pub leaves: Vec<Leaf>,
    /// Source sites.
    pub sites: Vec<Site>,
}

/// Error when reading a symbol map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid symbol map: {}", self.0)
    }
}

impl std::error::Error for Error {}

impl SymbolMap {
    /// Empty symbol map for ABI 1.1.
    pub fn new(compiler: &str) -> Self {
        SymbolMap {
            format: "ironplc-symbols".into(),
            version: 1,
            abi: Abi { major: 1, minor: 1 },
            compiler: compiler.to_string(),
            files: vec![],
            regions: Regions::default(),
            tasks: vec![],
            leaves: vec![],
            sites: vec![],
        }
    }

    /// Finds a leaf by path, ignoring case.
    pub fn leaf(&self, path: &str) -> Option<&Leaf> {
        self.leaves
            .iter()
            .find(|l| l.path.eq_ignore_ascii_case(path))
    }

    /// Compact JSON, keys in schema order (SYM-003).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("serialisable")
    }

    /// Indented JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }

    /// Reads the JSON form.
    pub fn from_json(text: &str) -> Result<Self, Error> {
        serde_json::from_str(text).map_err(|e| Error(e.to_string()))
    }

    /// CBOR form, for the `plc.meta` section.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).expect("CBOR encoding to memory cannot fail");
        out
    }

    /// Reads the CBOR form.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, Error> {
        ciborium::from_reader(bytes).map_err(|e| Error(format!("{e:?}")))
    }

    /// Reads the `plc.meta` section of a logic module.
    pub fn from_module(wasm: &[u8]) -> Result<Self, Error> {
        for payload in wasmparser::Parser::new(0).parse_all(wasm) {
            let payload = payload.map_err(|e| Error(e.to_string()))?;
            if let wasmparser::Payload::CustomSection(reader) = payload {
                if reader.name() == SECTION {
                    return Self::from_cbor(reader.data());
                }
            }
        }
        Err(Error(format!("no {SECTION} section")))
    }
}
