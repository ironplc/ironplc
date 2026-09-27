//! Code generation of the WebAssembly target:
//! layout of the memory regions (ABI section 5 and 6), WebAssembly emission
//! with `wasm-encoder`, instrumentation (fuel, debug hooks, division checks,
//! ABI sections 9 and 10), the symbol map and the custom sections
//! `plc.meta` and `plc.build`, and validation with `wasmparser` (OV-004).
#![forbid(unsafe_code)]

mod emit;
mod math;
mod strings;

use ironplc_wasm_ir::{Module, Region};
use ironplc_wasm_symbols::{
    flags, ArrayInfo, EnumValue, File, Leaf as SymLeaf, Region as SymRegion, Regions, Site,
    SymbolMap, Task as SymTask,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use wasm_encoder::{
    CodeSection, ConstExpr, CustomSection, DataCountSection, DataSection, EntityType, ExportKind,
    ExportSection, FunctionSection, GlobalSection, GlobalType, ImportSection, MemorySection,
    MemoryType, TypeSection, ValType,
};

/// First address that may hold a variable (ABI-040).
pub const DATA_START: u32 = 1024;

/// Options of code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenOptions {
    /// Fuel instrumentation (ABI-070).
    pub fuel: bool,
    /// Debug hooks (ABI-030, ABI-032).
    pub debug_hooks: bool,
    /// Bounds and subrange checks (ABI-082); recorded in `plc.build`.
    pub bounds_checks: bool,
    /// Maximum static data in bytes (API-023).
    pub static_limit: u32,
    /// Compiler name and version.
    pub compiler: String,
    /// Source files: names and texts.
    pub files: Vec<(String, String)>,
}

/// Failure of code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Static data larger than the limit (G001).
    StaticLimit {
        /// Size needed.
        size: u64,
        /// Limit.
        limit: u32,
    },
    /// Internal error (I001): the module failed validation.
    Internal(String),
}

/// A generated logic module.
#[derive(Debug, Clone)]
pub struct Output {
    /// The validated module.
    pub wasm: Vec<u8>,
    /// Its symbol map.
    pub symbols: SymbolMap,
}

/// Addresses of the objects and regions.
pub(crate) struct Layout {
    pub addr: Vec<u32>,
    pub regions: [(u32, u32); 5],
    pub end: u32,
}

fn region_index(r: Region) -> usize {
    match r {
        Region::Static => 0,
        Region::Retain => 1,
        Region::Input => 2,
        Region::Output => 3,
        Region::Marker => 4,
    }
}

fn align_to(x: u64, a: u64) -> u64 {
    x.div_ceil(a) * a
}

/// Regions in the order static, retain, input, output, marker, each object
/// at its alignment (ABI-041, ABI-047, ABI-050).
fn layout(m: &Module) -> (Layout, u64) {
    let mut addr = vec![0; m.objects.len()];
    let mut regions = [(0u32, 0u32); 5];
    let mut pos = DATA_START as u64;
    for (ri, region) in [
        Region::Static,
        Region::Retain,
        Region::Input,
        Region::Output,
        Region::Marker,
    ]
    .into_iter()
    .enumerate()
    {
        pos = align_to(pos, 8);
        let base = pos;
        for (i, o) in m.objects.iter().enumerate() {
            if o.region == region {
                pos = align_to(pos, o.align.max(1) as u64);
                addr[i] = pos.min(u32::MAX as u64) as u32;
                pos += o.init.len() as u64;
            }
        }
        regions[ri] = (
            base.min(u32::MAX as u64) as u32,
            (pos - base).min(u32::MAX as u64) as u32,
        );
    }
    let end = pos;
    (
        Layout {
            addr,
            regions,
            end: end.min(u32::MAX as u64) as u32,
        },
        end,
    )
}

/// Symbol map entries of the source files.
pub fn file_entries(files: &[(String, String)]) -> Vec<File> {
    files
        .iter()
        .enumerate()
        .map(|(i, (name, text))| File {
            id: i as u32,
            name: name.clone(),
            sha256: hex(&Sha256::digest(text.as_bytes())),
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Serialize)]
struct BuildOptions {
    fuel: bool,
    debug: bool,
    checks: bool,
}

/// The `plc.build` section (ABI section 8).
#[derive(Serialize)]
struct Build {
    abi_major: u32,
    abi_minor: u32,
    source_sha256: String,
    compiler: String,
    retain_layout: String,
    options: BuildOptions,
}

/// Generates the logic module of an IR module.
pub fn generate(m: &Module, opts: &CodegenOptions) -> Result<Output, CodegenError> {
    let (lay, end) = layout(m);
    let static_size = end - DATA_START as u64;
    if static_size > opts.static_limit as u64 {
        return Err(CodegenError::StaticLimit {
            size: static_size,
            limit: opts.static_limit,
        });
    }
    let mut sites: Vec<Site> = m
        .sites
        .iter()
        .map(|s| Site {
            file: s.file,
            start: s.start,
            end: s.end,
        })
        .collect();
    let mut symbols = symbol_map(m, &lay, opts);
    let wasm = module(m, &lay, opts, &mut sites, &mut symbols)?;
    validate(&wasm)?;
    Ok(Output { wasm, symbols })
}

fn symbol_map(m: &Module, lay: &Layout, opts: &CodegenOptions) -> SymbolMap {
    let mut s = SymbolMap::new(&opts.compiler);
    s.files = file_entries(&opts.files);
    let r = |i: usize| SymRegion {
        base: lay.regions[i].0,
        size: lay.regions[i].1,
    };
    s.regions = Regions {
        static_: r(0),
        retain: r(1),
        input: r(2),
        output: r(3),
        marker: r(4),
    };
    s.tasks = m
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            SymTask::cyclic(
                i as u32,
                &t.name,
                t.interval_ns.max(1),
                t.priority,
                t.programs.iter().map(|(_, p)| p.clone()).collect(),
            )
        })
        .collect();
    s.leaves = m
        .leaves
        .iter()
        .map(|l| {
            let retain = m.objects[l.object as usize].region == Region::Retain;
            SymLeaf {
                path: l.path.clone(),
                type_name: l.type_name.clone(),
                offset: lay.addr[l.object as usize] + l.offset,
                size: l.size,
                retain,
                flags: l.flags | if retain { flags::RETAIN } else { 0 },
                location: l.location.clone(),
                enumeration: l.enumeration.as_ref().map(|values| {
                    values
                        .iter()
                        .map(|(name, value)| EnumValue {
                            name: name.clone(),
                            value: *value,
                        })
                        .collect()
                }),
                array: l.array.as_ref().map(|(dims, stride)| ArrayInfo {
                    dims: dims.iter().map(|(l, h)| [*l, *h]).collect(),
                    stride: *stride,
                }),
                declared: l.declared,
            }
        })
        .collect();
    s
}

fn build_section(m: &Module, _lay: &Layout, opts: &CodegenOptions, s: &SymbolMap) -> Vec<u8> {
    let mut src = Sha256::new();
    for (name, text) in &opts.files {
        src.update(name.as_bytes());
        src.update([0]);
        src.update(text.replace("\r\n", "\n").as_bytes());
        src.update([0]);
    }
    let mut retain = Sha256::new();
    for l in s.leaves.iter().filter(|l| l.retain) {
        retain.update(format!("{} {} {}\n", l.path, l.type_name, l.offset).as_bytes());
    }
    let _ = m;
    let b = Build {
        abi_major: 1,
        abi_minor: 1,
        source_sha256: hex(&src.finalize()),
        compiler: opts.compiler.clone(),
        retain_layout: hex(&retain.finalize()),
        options: BuildOptions {
            fuel: opts.fuel,
            debug: opts.debug_hooks,
            checks: opts.bounds_checks,
        },
    };
    let mut out = vec![];
    ciborium::into_writer(&b, &mut out).expect("CBOR encoding to memory cannot fail");
    out
}

/// Validates with the features of ABI-001 only (OV-004).
pub fn validate(wasm: &[u8]) -> Result<(), CodegenError> {
    use wasmparser::WasmFeatures as F;
    let features = F::MVP
        | F::MUTABLE_GLOBAL
        | F::SATURATING_FLOAT_TO_INT
        | F::SIGN_EXTENSION
        | F::MULTI_VALUE
        | F::BULK_MEMORY
        | F::REFERENCE_TYPES;
    wasmparser::Validator::new_with_features(features)
        .validate_all(wasm)
        .map(|_| ())
        .map_err(|e| CodegenError::Internal(format!("invalid module: {e}")))
}

/// Indexes of the module-level entities.
pub(crate) struct Indexes {
    pub debug_hook: Option<u32>,
    pub now: Option<u32>,
    pub trap_code: u32,
    pub trap_site: u32,
    pub fuel: Option<u32>,
    pub first_ir_func: u32,
    pub helpers: math::HelperIndexes,
    pub strings: strings::StrIndexes,
    /// Passive segment of each object that needs one (reset or init).
    pub segments: Vec<Option<u32>>,
}

fn module(
    m: &Module,
    lay: &Layout,
    opts: &CodegenOptions,
    sites: &mut Vec<Site>,
    symbols: &mut SymbolMap,
) -> Result<Vec<u8>, CodegenError> {
    let mut types = TypeSection::new();
    // 0: () -> (), 1: (i32 i32 i32) -> (), 2: () -> i32, 3: (i32) -> i32, 4: (i32) -> ()
    types.ty().function([], []);
    types
        .ty()
        .function([ValType::I32, ValType::I32, ValType::I32], []);
    types.ty().function([], [ValType::I32]);
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32], []);
    // 5: () -> i64 (now_ns)
    types.ty().function([], [ValType::I64]);
    let helper_types = math::add_types(&mut types);
    let string_types = strings::add_types(&mut types);

    let mut imports = ImportSection::new();
    let mut n_imports = 0;
    let used = math::used(m);
    let debug_hook = if opts.debug_hooks {
        imports.import("plc_rt", "debug_hook", EntityType::Function(4));
        n_imports += 1;
        Some(n_imports - 1)
    } else {
        None
    };
    // ABI-030: the time only when the module reads it (timers).
    let now = if used.now {
        imports.import("plc_rt", "now_ns", EntityType::Function(5));
        n_imports += 1;
        Some(n_imports - 1)
    } else {
        None
    };

    // Functions: plc_abi_version, plc_init, plc_task_run, helpers, IR.
    let helpers = math::HelperIndexes::assign(n_imports + 3, used);
    let string_helpers =
        strings::StrIndexes::assign(n_imports + 3 + helpers.count, strings::used(m));
    let first_ir_func = n_imports + 3 + helpers.count + string_helpers.count;

    // Globals.
    let mut globals = GlobalSection::new();
    let mut g = 0;
    let mut global = |globals: &mut GlobalSection, ty: ValType, mutable: bool, init: ConstExpr| {
        globals.global(
            GlobalType {
                val_type: ty,
                mutable,
                shared: false,
            },
            &init,
        );
        g += 1;
        g - 1
    };
    let trap_code = global(&mut globals, ValType::I32, true, ConstExpr::i32_const(0));
    let trap_site = global(&mut globals, ValType::I32, true, ConstExpr::i32_const(0));
    let fuel = opts
        .fuel
        .then(|| global(&mut globals, ValType::I64, true, ConstExpr::i64_const(0)));
    let mut region_globals = vec![];
    for (name, ri) in [("retain", 1), ("input", 2), ("output", 3), ("marker", 4)] {
        let (base, size) = lay.regions[ri];
        let b = global(
            &mut globals,
            ValType::I32,
            false,
            ConstExpr::i32_const(base as i32),
        );
        let s = global(
            &mut globals,
            ValType::I32,
            false,
            ConstExpr::i32_const(size as i32),
        );
        region_globals.push((name, b, s));
    }

    // Data: one passive segment per region image (plc_init), one per
    // object reset during execution with a non-zero image.
    let mut data = DataSection::new();
    let mut n_segments = 0u32;
    let mut region_segments = [None; 5];
    for (ri, seg) in region_segments.iter_mut().enumerate() {
        let (base, size) = lay.regions[ri];
        let mut image = vec![0u8; size as usize];
        for (i, o) in m.objects.iter().enumerate() {
            if region_index(o.region) == ri {
                let off = (lay.addr[i] - base) as usize;
                image[off..off + o.init.len()].copy_from_slice(&o.init);
            }
        }
        if image.iter().any(|b| *b != 0) {
            data.passive(image);
            *seg = Some(n_segments);
            n_segments += 1;
        }
    }
    let mut resettable = vec![false; m.objects.len()];
    emit::resettable(m, &mut resettable);
    let mut segments = vec![None; m.objects.len()];
    for (i, o) in m.objects.iter().enumerate() {
        if resettable[i] && o.init.iter().any(|b| *b != 0) {
            data.passive(o.init.iter().copied());
            segments[i] = Some(n_segments);
            n_segments += 1;
        }
    }

    let ix = Indexes {
        debug_hook,
        now,
        trap_code,
        trap_site,
        fuel,
        first_ir_func,
        helpers,
        strings: string_helpers,
        segments,
    };

    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    // plc_abi_version
    funcs.function(2);
    code.function(&emit::simple(&[wasm_encoder::Instruction::I32Const(1)]));
    // plc_init (ABI-011, ABI-081)
    funcs.function(2);
    code.function(&emit::init(lay, &region_segments, &ix));
    // plc_task_run (ABI-012, ABI-081)
    funcs.function(3);
    code.function(&emit::task_run(m, &ix));
    for f in math::functions(&ix.helpers, &helper_types)
        .into_iter()
        .chain(strings::functions(&ix.strings, &string_types))
    {
        funcs.function(f.0);
        code.function(&f.1);
    }
    for f in &m.functions {
        let ty = match f.kind {
            ironplc_wasm_ir::FuncKind::FunctionBlock => 1,
            _ => 0,
        };
        funcs.function(ty);
        let body = emit::function(m, f, lay, &ix, sites).map_err(CodegenError::Internal)?;
        code.function(&body);
    }

    let mut memory = MemorySection::new();
    let pages = (lay.end as u64).div_ceil(65536).max(1);
    memory.memory(MemoryType {
        minimum: pages,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("plc_abi_version", ExportKind::Func, n_imports);
    exports.export("plc_init", ExportKind::Func, n_imports + 1);
    exports.export("plc_task_run", ExportKind::Func, n_imports + 2);
    exports.export("plc_trap_code", ExportKind::Global, trap_code);
    exports.export("plc_trap_site", ExportKind::Global, trap_site);
    if let Some(f) = fuel {
        exports.export("plc_fuel", ExportKind::Global, f);
    }
    // ABI-014: one entry point per program, for tools that run it alone.
    for (i, f) in m.functions.iter().enumerate() {
        if f.kind == ironplc_wasm_ir::FuncKind::Program {
            exports.export(
                &format!("plc_program_{}", f.name),
                ExportKind::Func,
                first_ir_func + i as u32,
            );
        }
    }
    for (name, b, s) in region_globals {
        exports.export(&format!("plc_{name}_base"), ExportKind::Global, b);
        exports.export(&format!("plc_{name}_size"), ExportKind::Global, s);
    }

    let mut module = wasm_encoder::Module::new();
    module.section(&types);
    if n_imports > 0 {
        module.section(&imports);
    }
    module.section(&funcs);
    module.section(&memory);
    module.section(&globals);
    module.section(&exports);
    module.section(&DataCountSection { count: n_segments });
    module.section(&code);
    module.section(&data);
    // The sites are complete once the bodies are emitted (SYM-021).
    symbols.sites = sites.clone();
    let meta = symbols.to_cbor();
    let build = build_section(m, lay, opts, symbols);
    module.section(&CustomSection {
        name: ironplc_wasm_symbols::SECTION.into(),
        data: meta.into(),
    });
    module.section(&CustomSection {
        name: "plc.build".into(),
        data: build.into(),
    });
    Ok(module.finish())
}
