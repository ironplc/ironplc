//! Symbol map model and encodings (`docs/spec/symbol-map.md`).

use ironplc_wasm_symbols::{flags, Leaf, Region, SymbolMap, Task};

fn root(path: &str) -> String {
    format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))
}

fn schema() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root("schemas/symbols-1.json")).unwrap()).unwrap()
}

fn spec_block(after: &str) -> String {
    let spec = std::fs::read_to_string(root("docs/spec/symbol-map.md")).unwrap();
    spec.split(after)
        .nth(1)
        .unwrap()
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap()
        .to_string()
}

fn sample() -> SymbolMap {
    let mut m = SymbolMap::new("ironplcc 0.0.1");
    m.files.push(ironplc_wasm_symbols::File {
        id: 0,
        name: "main.st".into(),
        sha256: "0".repeat(64),
    });
    m.regions.static_ = Region {
        base: 1024,
        size: 64,
    };
    m.regions.retain = Region {
        base: 1088,
        size: 4,
    };
    m.tasks.push(Task::cyclic(
        0,
        "DEFAULT",
        10_000_000,
        0,
        vec!["MAIN".into()],
    ));
    m.sites.push(ironplc_wasm_symbols::Site {
        file: 0,
        start: 22,
        end: 42,
    });
    m.leaves.push(Leaf {
        path: "MAIN.COUNT".into(),
        type_name: "DINT".into(),
        offset: 1088,
        size: 4,
        retain: true,
        flags: flags::RETAIN,
        location: None,
        enumeration: None,
        array: None,
        declared: 0,
    });
    m
}

#[test]
fn sym_002_schema_file_is_the_one_of_the_specification() {
    let from_spec: serde_json::Value =
        serde_json::from_str(&spec_block("## 4. JSON Schema")).unwrap();
    assert_eq!(from_spec, schema());
}

#[test]
fn sym_002_specification_example_validates() {
    let example: serde_json::Value =
        serde_json::from_str(&spec_block("## 5. Example").replace("…", &"0".repeat(64))).unwrap();
    let validator = jsonschema::validator_for(&schema()).unwrap();
    assert!(
        validator.is_valid(&example),
        "{:?}",
        validator
            .iter_errors(&example)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
    // and it parses into the model
    let m = SymbolMap::from_json(&example.to_string()).unwrap();
    assert_eq!(m.leaves[0].location.as_deref(), Some("%IX0.0"));
}

#[test]
fn sym_001_json_validates_and_round_trips() {
    let m = sample();
    let json = m.to_json();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let validator = jsonschema::validator_for(&schema()).unwrap();
    assert!(
        validator.is_valid(&value),
        "{:?}",
        validator
            .iter_errors(&value)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(SymbolMap::from_json(&json).unwrap(), m);
}

#[test]
fn sym_003_serialisation_is_deterministic_and_ordered() {
    let json = sample().to_json();
    assert_eq!(json, sample().to_json());
    let keys = [
        "\"format\"",
        "\"version\"",
        "\"abi\"",
        "\"compiler\"",
        "\"files\"",
        "\"regions\"",
        "\"tasks\"",
        "\"leaves\"",
        "\"sites\"",
    ];
    let positions: Vec<usize> = keys.iter().map(|k| json.find(k).unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{json}");
}

#[test]
fn sym_001_cbor_round_trips_with_the_same_keys() {
    let m = sample();
    let cbor = m.to_cbor();
    assert_eq!(SymbolMap::from_cbor(&cbor).unwrap(), m);
    let value: ciborium::value::Value = ciborium::from_reader(cbor.as_slice()).unwrap();
    let keys: Vec<String> = value
        .as_map()
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_text().unwrap().to_string())
        .collect();
    assert_eq!(
        keys,
        ["format", "version", "abi", "compiler", "files", "regions", "tasks", "leaves", "sites"]
    );
}

// ABI-004: the symbol map is read back from the plc.meta section of a module.
#[test]
fn abi_004_read_from_plc_meta_section() {
    let m = sample();
    let mut module = wasm_encoder::Module::new();
    module.section(&wasm_encoder::CustomSection {
        name: "plc.meta".into(),
        data: m.to_cbor().into(),
    });
    let wasm = module.finish();
    assert_eq!(SymbolMap::from_module(&wasm).unwrap(), m);
    let empty = wasm_encoder::Module::new().finish();
    assert!(SymbolMap::from_module(&empty).is_err());
}

#[test]
fn sym_010_leaf_lookup_is_case_insensitive() {
    let m = sample();
    assert_eq!(m.leaf("main.count").map(|l| l.offset), Some(1088));
    assert!(m.leaf("MAIN.OTHER").is_none());
}
