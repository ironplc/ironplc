# Symbol map

Status: approved. Schema version **1**.

The symbol map describes every variable of a logic module (fully qualified
path, offset, type, size, retain flag), the tasks, the memory regions and the
source sites used by traps and debugging. It is used by the runtime, the IDE,
the debugger and the supervision software.

## 1. Encodings

- **SYM-001 (M)** The compiler shall produce the symbol map as a data structure (API) and shall be able to serialise it as JSON (sidecar file, JavaScript API) and as CBOR (custom section `plc.meta` of the module, ABI-004).
- **SYM-002 (M)** Both encodings shall carry the same model, with the same key names; the JSON form shall validate against the schema of section 4.
- **SYM-003 (M)** The serialisation shall be deterministic: keys in the order of the schema, lists in the order defined below.

This model is the content of the `plc.meta` section in its version 1.

## 2. Paths

- **SYM-010 (M)** Paths shall be written in upper case: program instance name, then members separated by `.`; array elements as `[i]` or `[i,j]` with the declared index values; global variables by their name alone (`MAIN.CNT1.CV`, `MAIN.A[3]`, `LINE_SPEED`).
- **SYM-011 (M)** Without configuration elements, the instance of a program is named after the program type; with configuration elements, the instance name of `PROGRAM <instance> WITH ...` is used, prefixed by the resource name when there are several resources (`RES0.MAIN_INST.X`).
- **SYM-012 (M)** Internal variables of standard function blocks (edge memory, start time) shall not be listed; their visible inputs and outputs shall be.
- **SYM-013 (M)** An array of elementary type shall be listed as one leaf with an `array` descriptor (dimensions and stride), whose elements are the leaves `path[i,...]`; an array of structures or function blocks shall list the leaves of each element.

## 3. Model

| Key | Type | Content |
|---|---|---|
| `format` | text | `"ironplc-symbols"` |
| `version` | uint | 1 |
| `abi` | map | `{ "major": 1, "minor": 1 }` |
| `compiler` | text | Name and version of the compiler |
| `files` | list | Source files: `{ "id", "name", "sha256" }`, in the order given to the compiler |
| `regions` | map | `static`, `retain`, `input`, `output`, `marker`: `{ "base", "size" }` |
| `tasks` | list | `{ "id", "name", "kind": "cyclic", "interval_ns", "priority", "programs": [instance paths] }`; `id` is the argument of `plc_task_run` |
| `leaves` | list | Variables, depth first in declaration order: see below |
| `sites` | list | Source spans: `{ "file", "start", "end" }` (byte offsets, end exclusive), indexed by trap and debug sites |

Leaf:

| Key | Type | Content |
|---|---|---|
| `path` | text | SYM-010 |
| `type` | text | Elementary type name (`INT`, `STRING[20]`), enumeration name, or subrange base type |
| `offset` | uint | Address in linear memory |
| `size` | uint | Size in bytes |
| `retain` | bool | Stored in the retain region |
| `flags` | uint | Bits, the first six as in the existing `plc.meta`: 1 `RETAIN`, 2 `CONSTANT`, 4 `INPUT`, 8 `OUTPUT`, 16 `LOCATED`, 32 `SFC_STEP`, then 64 `IN_OUT`, 128 `EXTERNAL`, 256 `GLOBAL`, 512 `STAT` |
| `location` | text or null | Direct address of a located variable (`%IX0.3`) |
| `enum` | list or absent | For enumerations: `{ "name", "value" }` pairs |
| `array` | map or absent | SYM-013: `{ "dims": [[low, high], ...], "stride" }` |
| `declared` | uint | Index into `sites` of the declaration |

- **SYM-020 (M)** Every variable of every program instance, every global variable and every member of function block instances reachable from them shall appear as a leaf, except `VAR_TEMP` variables, function frames and the internal state of standard blocks.
- **SYM-021 (M)** The `sites` list shall contain the declaration of each leaf, each statement where a debug hook or a checked operation is inserted, and nothing else; its order is the order of first use by the compiler.

## 4. JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "urn:ironplc:schemas:symbols-1",
  "title": "IronPLC WebAssembly symbol map, version 1",
  "type": "object",
  "required": ["format", "version", "abi", "compiler", "files", "regions", "tasks", "leaves", "sites"],
  "additionalProperties": false,
  "properties": {
    "format": { "const": "ironplc-symbols" },
    "version": { "const": 1 },
    "abi": {
      "type": "object",
      "required": ["major", "minor"],
      "properties": { "major": { "const": 1 }, "minor": { "type": "integer", "minimum": 0 } }
    },
    "compiler": { "type": "string" },
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "name", "sha256"],
        "properties": {
          "id": { "type": "integer", "minimum": 0 },
          "name": { "type": "string" },
          "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
        }
      }
    },
    "regions": {
      "type": "object",
      "required": ["static", "retain", "input", "output", "marker"],
      "additionalProperties": { "$ref": "#/$defs/region" }
    },
    "tasks": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "name", "kind", "interval_ns", "priority", "programs"],
        "properties": {
          "id": { "type": "integer", "minimum": 0 },
          "name": { "type": "string" },
          "kind": { "const": "cyclic" },
          "interval_ns": { "type": "integer", "minimum": 1 },
          "priority": { "type": "integer", "minimum": 0 },
          "programs": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "leaves": { "type": "array", "items": { "$ref": "#/$defs/leaf" } },
    "sites": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["file", "start", "end"],
        "properties": {
          "file": { "type": "integer", "minimum": 0 },
          "start": { "type": "integer", "minimum": 0 },
          "end": { "type": "integer", "minimum": 0 }
        }
      }
    }
  },
  "$defs": {
    "region": {
      "type": "object",
      "required": ["base", "size"],
      "properties": { "base": { "type": "integer", "minimum": 0 }, "size": { "type": "integer", "minimum": 0 } }
    },
    "leaf": {
      "type": "object",
      "required": ["path", "type", "offset", "size", "retain", "flags", "location", "declared"],
      "additionalProperties": false,
      "properties": {
        "path": { "type": "string", "pattern": "^[A-Z_][A-Z0-9_]*(\\.[A-Z_][A-Z0-9_]*|\\[-?[0-9]+(,-?[0-9]+)*\\])*$" },
        "type": { "type": "string" },
        "offset": { "type": "integer", "minimum": 1024 },
        "size": { "type": "integer", "minimum": 1 },
        "retain": { "type": "boolean" },
        "flags": { "type": "integer", "minimum": 0 },
        "location": { "type": ["string", "null"], "pattern": "^%[IQM][XBWDL]?[0-9]+(\\.[0-9]+)?$" },
        "enum": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["name", "value"],
            "properties": { "name": { "type": "string" }, "value": { "type": "integer" } }
          }
        },
        "array": {
          "type": "object",
          "required": ["dims", "stride"],
          "properties": {
            "dims": {
              "type": "array",
              "minItems": 1,
              "maxItems": 8,
              "items": { "type": "array", "prefixItems": [{ "type": "integer" }, { "type": "integer" }], "minItems": 2, "maxItems": 2 }
            },
            "stride": { "type": "integer", "minimum": 1 }
          }
        },
        "declared": { "type": "integer", "minimum": 0 }
      }
    }
  }
}
```

## 5. Example

```json
{
  "format": "ironplc-symbols",
  "version": 1,
  "abi": { "major": 1, "minor": 1 },
  "compiler": "ironplcc 0.0.1",
  "files": [{ "id": 0, "name": "main.st", "sha256": "…" }],
  "regions": {
    "static": { "base": 1024, "size": 64 },
    "retain": { "base": 1088, "size": 4 },
    "input": { "base": 1092, "size": 1 },
    "output": { "base": 1093, "size": 1 },
    "marker": { "base": 1094, "size": 0 }
  },
  "tasks": [{ "id": 0, "name": "DEFAULT", "kind": "cyclic", "interval_ns": 10000000, "priority": 0, "programs": ["MAIN"] }],
  "leaves": [
    { "path": "MAIN.START", "type": "BOOL", "offset": 1092, "size": 1, "retain": false, "flags": 16, "location": "%IX0.0", "declared": 0 },
    { "path": "MAIN.COUNT", "type": "DINT", "offset": 1088, "size": 4, "retain": true, "flags": 1, "location": null, "declared": 1 }
  ],
  "sites": [{ "file": 0, "start": 22, "end": 42 }, { "file": 0, "start": 60, "end": 74 }]
}
```
