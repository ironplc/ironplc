# PLCopen XML Module

This document describes the architecture and patterns used in the PLCopen XML parsing module (`compiler/sources/src/xml/`).

## Architecture Overview

```
PLCopen XML File
       ↓
position.rs (XML → Schema Structs)
  └─ Uses roxmltree for position-aware parsing
       ↓
schema.rs (Schema Struct Definitions)
       ↓
transform.rs (Schema Structs → DSL Library)
  └─ For <ST> bodies: reuses ironplc-parser
       ↓
ironplc_dsl::Library (shared AST)
```

## Module Structure

### `schema.rs`

Defines Rust structs that mirror the PLCopen TC6 XML schema structure:

- `Project` - Root element containing file/content headers and types
- `Types` - Container for data types and POUs
- `DataType` - Enumeration, array, structure, subrange, etc.
- `Pou` - Program Organization Unit (function, function_block, program)
- `Body` - POU body with ST, SFC, or other language content
- `Configuration` - Runtime configuration with resources and tasks

Key patterns:
- Use `Option<T>` for optional XML elements
- Use `Vec<T>` for repeating elements
- Use `bool` for empty element presence (e.g., `<FBD/>`)
- Include position info in `StBody` for accurate error reporting

### `position.rs`

Parses XML using roxmltree while preserving source positions:

```rust
pub fn parse_plcopen_xml(xml_content: &str, file_id: &FileId) -> Result<Project, Diagnostic>
```

Key patterns:
- Single-pass parsing extracts both data and positions
- Element tag names matched with `match child.tag_name().name()`
- Attributes accessed via `node.attribute("name")`
- Text content captured with position for ST body parsing

### `transform.rs`

Transforms schema structs into IronPLC DSL:

```rust
pub fn transform_project(project: &Project, file_id: &FileId) -> Result<Library, Diagnostic>
```

Key patterns:
- Each schema type has a corresponding `transform_*` function
- ST body text is parsed using `ironplc_parser::parse_st_statements()`
- Position offsets from XML are passed to ST parser for accurate error locations
- Returns `Diagnostic` on transformation errors

## Adding New Element Support

To add support for a new XML element:

### 1. Add Schema Structs (`schema.rs`)

```rust
#[derive(Debug, Clone, Default)]
pub struct NewElement {
    pub name: String,
    pub optional_field: Option<String>,
    pub children: Vec<ChildElement>,
}
```

### 2. Add Parsing (`position.rs`)

```rust
fn parse_new_element(node: roxmltree::Node) -> Result<NewElement, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let mut children = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "child" => children.push(parse_child_element(child)?),
            _ => {}  // Ignore unknown elements
        }
    }

    Ok(NewElement { name, optional_field: None, children })
}
```

### 3. Add Transformation (`transform.rs`)

```rust
fn transform_new_element(elem: &NewElement, file_id: &FileId) -> Result<DslType, Diagnostic> {
    // Transform schema struct to DSL type
    Ok(DslType {
        name: Id::from(elem.name.as_str()),
        // ...
    })
}
```

### 4. Add Tests

Add tests in the `#[cfg(test)]` module at the end of each file:

```rust
#[test]
fn transform_when_new_element_then_creates_dsl_type() {
    let xml = format!(r#"{}
  <types>
    <dataTypes>
      <newElement name="Test"/>
    </dataTypes>
    <pous/>
  </types>
</project>"#, minimal_project_header());

    let project = parse_project(&xml);
    let library = transform_project(&project, &test_file_id()).unwrap();
    // Assert expected DSL structure
}
```

## Error Handling

Use specific problem codes for XML-related errors:

| Code | Use Case |
|------|----------|
| P0006 | Malformed XML (syntax errors) |
| P0007 | Schema violations (wrong structure) |
| P0008 | SFC missing initial step |
| P6008 | Unsupported XML version |
| P9003 | Unsupported body language |

Create diagnostics with file context:

```rust
Diagnostic::problem(
    Problem::XmlSchemaViolation,
    Label::span(file_span(file_id), "Description of the issue"),
)
```

## ST Body Parsing

ST body text embedded in XML is parsed using the existing ST parser:

```rust
fn parse_st_body(
    st_text: &str,
    file_id: &FileId,
    line_offset: usize,   // XML line where ST starts
    col_offset: usize,    // XML column where ST starts
) -> Result<Vec<StmtKind>, Diagnostic>
```

The offsets ensure error positions in ST code point to correct XML file locations.

## Testing Patterns

Tests use inline XML with a helper for the standard header:

```rust
fn minimal_project_header() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>"#
}
```

Follow BDD naming: `transform_when_condition_then_result`
