# PLCopen XML Support Implementation Plan

This document describes the implementation plan for adding PLCopen XML (IEC 61131-3 TC6 XML) file support to IronPLC.

## Overview

PLCopen XML is an XML-based interchange format for IEC 61131-3 programs. This implementation uses the PLCopen XSD schema to generate Rust parsing code, then transforms the parsed structures into IronPLC's DSL AST.

### Architecture

```
PLCopen XML File
       ↓
xsd-parser generated structs (XML → Schema Structs)
       ↓
transform.rs (Schema Structs → DSL Library)
  └─ For <ST> bodies: reuse ironplc-parser
       ↓
ironplc_dsl::Library (existing shared AST)
       ↓
ironplc_analyzer (existing semantic analysis)
```

### New Problem Codes

| Code | Name | Message |
|------|------|---------|
| P0006 | XmlMalformed | XML file is malformed |
| P0007 | XmlSchemaViolation | XML violates PLCopen schema |
| P6008 | XmlUnsupportedVersion | PLCopen XML version is not supported |
| P9003 | XmlBodyTypeNotSupported | POU body language is not supported |

---

## Phase 0: XSD Tooling Setup

**Goal**: Set up XSD-based code generation infrastructure.

### 0.1 Obtain and Store XSD

- [ ] Download PLCopen TC6 XML schema v2.01 from plcopen.org
- [ ] Create directory `compiler/resources/schemas/`
- [ ] Store XSD as `compiler/resources/schemas/tc6_xml_v201.xsd`
- [ ] Document XSD version and source in a README

### 0.2 Evaluate xsd-parser

- [ ] Create test project to evaluate xsd-parser crate
- [ ] Test XSD parsing on PLCopen schema
- [ ] Identify any schema features xsd-parser doesn't support
- [ ] Document workarounds if needed
- [ ] Decide: build-time generation vs one-time generation

### 0.3 Configure Code Generation

- [ ] Add xsd-parser dependency to `compiler/sources/Cargo.toml`
- [ ] Create `compiler/sources/build.rs` for build-time generation OR
- [ ] Create `compiler/sources/generate_schema.rs` binary for one-time generation
- [ ] Configure output path: `compiler/sources/src/xml/generated.rs`
- [ ] Configure serializer: quick-xml
- [ ] Configure naming conventions to match Rust idioms

### 0.4 Initial Generation

- [ ] Run generator on PLCopen XSD
- [ ] Review generated structs for correctness
- [ ] Verify all major elements present (Project, Pou, DataType, etc.)
- [ ] Check Option<T> usage matches XSD required/optional
- [ ] Fix any generation issues or file bugs upstream

### 0.5 Smoke Test

- [ ] Create minimal valid PLCopen XML test file
- [ ] Write test that deserializes test file to generated structs
- [ ] Verify round-trip: deserialize → serialize → compare
- [ ] Document any XSD elements that don't round-trip cleanly

**Phase 0 Milestone**: Can parse valid PLCopen XML into generated Rust structs.

---

## Phase 1: Foundation

**Goal**: Basic XML parsing infrastructure and data type support.

### 1.1 Module Structure

- [ ] Create `compiler/sources/src/xml/mod.rs`
- [ ] Create `compiler/sources/src/xml/error.rs`
- [ ] Create `compiler/sources/src/xml/transform.rs`
- [ ] Create `compiler/sources/src/xml/body_parser.rs`
- [ ] Export modules from `xml/mod.rs`
- [ ] Import generated module in `xml/mod.rs`

### 1.2 Error Types

- [ ] Define `XmlError` enum in `error.rs`
- [ ] Add variant for malformed XML (wraps quick-xml error)
- [ ] Add variant for schema violation
- [ ] Add variant for transformation error
- [ ] Implement `From<XmlError>` for `Diagnostic`

### 1.3 Problem Codes - Phase 1

- [ ] Add P0006 (XmlMalformed) to `problem-codes.csv`
- [ ] Add P0007 (XmlSchemaViolation) to `problem-codes.csv`
- [ ] Run `just compile` to regenerate Problem enum
- [ ] Create `docs/compiler/problems/P0006.rst`
- [ ] Create `docs/compiler/problems/P0007.rst`

### 1.4 Parser Entry Point

- [ ] Update `xml_parser.rs` to call XML deserialization
- [ ] Handle deserialization errors → P0006/P0007 diagnostics
- [ ] Extract source positions from quick-xml errors
- [ ] Return `Diagnostic` with file position on error

### 1.5 Transform - Project Level

- [ ] Implement `transform_project()` function signature
- [ ] Create empty `Library` from `Project`
- [ ] Extract file metadata (for future use)
- [ ] Write test: empty project → empty library

### 1.6 Transform - Elementary Types

- [ ] Map PLCopen elementary types to IronPLC types
- [ ] Handle BOOL, BYTE, WORD, DWORD, LWORD
- [ ] Handle SINT, INT, DINT, LINT
- [ ] Handle USINT, UINT, UDINT, ULINT
- [ ] Handle REAL, LREAL
- [ ] Handle TIME, DATE, TIME_OF_DAY, DATE_AND_TIME
- [ ] Handle STRING, WSTRING
- [ ] Write unit tests for each elementary type

### 1.7 Transform - Enumeration Types

- [ ] Implement `transform_enum_type()` function
- [ ] Extract enumeration name
- [ ] Extract enumeration values
- [ ] Handle optional initial value
- [ ] Create `EnumerationDeclaration` in DSL
- [ ] Write unit tests for enumeration transformation

### 1.8 Transform - Array Types

- [ ] Implement `transform_array_type()` function
- [ ] Extract base type
- [ ] Extract dimensions (subranges)
- [ ] Handle multi-dimensional arrays
- [ ] Create `ArrayTypeDeclaration` in DSL
- [ ] Write unit tests for array transformation

### 1.9 Transform - Structure Types

- [ ] Implement `transform_struct_type()` function
- [ ] Extract structure name
- [ ] Extract field names and types
- [ ] Handle optional field initial values
- [ ] Create `StructureDeclaration` in DSL
- [ ] Write unit tests for structure transformation

### 1.10 Transform - Subrange Types

- [ ] Implement `transform_subrange_type()` function
- [ ] Extract base type
- [ ] Extract min/max values
- [ ] Create subrange type in DSL
- [ ] Write unit tests for subrange transformation

### 1.11 Transform - Type Aliases

- [ ] Implement `transform_derived_type()` function
- [ ] Handle simple type aliases
- [ ] Create type alias in DSL
- [ ] Write unit tests for alias transformation

### 1.12 Integration Test - Phase 1

- [ ] Create `compiler/resources/test/xml/data_types.xml`
- [ ] Include enumeration, array, structure, subrange examples
- [ ] Write integration test: parse XML → Library
- [ ] Verify Library contains expected type declarations
- [ ] Compare behavior with equivalent ST file

**Phase 1 Milestone**: Parse XML with data types, produce valid Library.

---

## Phase 2: POU Support

**Goal**: Parse function blocks, functions, and programs with ST bodies.

### 2.1 Transform - POU Container

- [ ] Implement `transform_pous()` function
- [ ] Iterate over POU elements
- [ ] Dispatch by pouType attribute
- [ ] Collect transformed POUs into Library

### 2.2 Transform - Function Declaration

- [ ] Implement `transform_function()` function
- [ ] Extract function name
- [ ] Extract return type
- [ ] Create `FunctionDeclaration` in DSL
- [ ] Write unit tests for function transformation

### 2.3 Transform - Function Block Declaration

- [ ] Implement `transform_function_block()` function
- [ ] Extract function block name
- [ ] Create `FunctionBlockDeclaration` in DSL
- [ ] Write unit tests for FB transformation

### 2.4 Transform - Program Declaration

- [ ] Implement `transform_program()` function
- [ ] Extract program name
- [ ] Create `ProgramDeclaration` in DSL
- [ ] Write unit tests for program transformation

### 2.5 Transform - Interface (Variables)

- [ ] Implement `transform_interface()` function
- [ ] Handle inputVars → VAR_INPUT
- [ ] Handle outputVars → VAR_OUTPUT
- [ ] Handle inOutVars → VAR_IN_OUT
- [ ] Handle localVars → VAR
- [ ] Handle tempVars → VAR_TEMP
- [ ] Handle externalVars → VAR_EXTERNAL

### 2.6 Transform - Variable Declaration

- [ ] Implement `transform_variable()` function
- [ ] Extract variable name
- [ ] Extract variable type
- [ ] Handle optional initial value
- [ ] Handle optional address (AT)
- [ ] Handle CONSTANT qualifier
- [ ] Handle RETAIN qualifier
- [ ] Write unit tests for variable transformation

### 2.7 Body Parser - ST Extraction

- [ ] Implement `extract_st_body()` function
- [ ] Handle `<ST>` element with text content
- [ ] Handle `<ST>` element with XHTML wrapper
- [ ] Handle CDATA sections
- [ ] Preserve whitespace correctly
- [ ] Calculate byte offset of ST content within XML

### 2.8 Body Parser - ST Parsing Integration

- [ ] Call `ironplc_parser::parse_program()` on ST content
- [ ] Map parser errors to XML file positions
- [ ] Adjust `SourceSpan` by XML offset
- [ ] Return statements for POU body
- [ ] Write tests for ST body parsing

### 2.9 Body Parser - Position Mapping

- [ ] Track XML element start positions during parsing
- [ ] Store position info in transformation context
- [ ] Create `SourceSpan` with XML file positions
- [ ] Test error positions point to correct XML lines

### 2.10 Problem Codes - Phase 2

- [ ] Add P9003 (XmlBodyTypeNotSupported) to `problem-codes.csv`
- [ ] Run `just compile` to regenerate Problem enum
- [ ] Create `docs/compiler/problems/P9003.rst`

### 2.11 Unsupported Body Handling

- [ ] Detect FBD body → return P9003 diagnostic
- [ ] Detect LD body → return P9003 diagnostic
- [ ] Detect IL body → return P9003 diagnostic
- [ ] Include helpful message about ST support
- [ ] Write tests for unsupported body errors

### 2.12 Integration Test - Phase 2

- [ ] Create `compiler/resources/test/xml/function.xml`
- [ ] Create `compiler/resources/test/xml/function_block.xml`
- [ ] Create `compiler/resources/test/xml/program.xml`
- [ ] Write integration tests for each POU type
- [ ] Run semantic analysis on parsed POUs
- [ ] Verify no false errors from analyzer

### 2.13 Error Position Test

- [ ] Create XML with intentional ST syntax error
- [ ] Parse and capture diagnostic
- [ ] Verify diagnostic points to correct line/column in XML
- [ ] Verify error message is helpful

**Phase 2 Milestone**: Parse XML POUs with ST bodies through semantic analysis.

---

## Phase 3: Configuration and SFC

**Goal**: Full project support with configurations and SFC bodies.

### 3.1 Transform - Instances Container

- [ ] Implement `transform_instances()` function
- [ ] Handle configurations container
- [ ] Dispatch to configuration transformation

### 3.2 Transform - Configuration

- [ ] Implement `transform_configuration()` function
- [ ] Extract configuration name
- [ ] Handle global variables at configuration level
- [ ] Create `ConfigurationDeclaration` in DSL
- [ ] Write unit tests

### 3.3 Transform - Resource

- [ ] Implement `transform_resource()` function
- [ ] Extract resource name
- [ ] Handle global variables at resource level
- [ ] Create `ResourceDeclaration` in DSL
- [ ] Write unit tests

### 3.4 Transform - Task

- [ ] Implement `transform_task()` function
- [ ] Extract task name
- [ ] Extract priority
- [ ] Extract interval (TIME literal)
- [ ] Handle single vs cyclic tasks
- [ ] Write unit tests

### 3.5 Transform - Program Instance

- [ ] Implement `transform_pou_instance()` function
- [ ] Extract instance name
- [ ] Extract type name reference
- [ ] Handle task association
- [ ] Write unit tests

### 3.6 Transform - Global Variables

- [ ] Implement `transform_global_vars()` function
- [ ] Handle configuration-level globals
- [ ] Handle resource-level globals
- [ ] Handle access paths
- [ ] Write unit tests

### 3.7 SFC Body - Structure

- [ ] Implement `transform_sfc_body()` function
- [ ] Extract steps from SFC element
- [ ] Extract transitions from SFC element
- [ ] Extract actions from SFC element

### 3.8 SFC Body - Steps

- [ ] Implement `transform_step()` function
- [ ] Extract step name
- [ ] Handle initial step flag
- [ ] Handle step actions (action associations)
- [ ] Write unit tests

### 3.9 SFC Body - Transitions

- [ ] Implement `transform_transition()` function
- [ ] Extract transition name
- [ ] Extract source/target connections
- [ ] Extract condition (inline ST or reference)
- [ ] Parse ST condition expression
- [ ] Write unit tests

### 3.10 SFC Body - Actions

- [ ] Implement `transform_action()` function
- [ ] Extract action name
- [ ] Extract action body (ST)
- [ ] Parse ST action body
- [ ] Write unit tests

### 3.11 SFC Body - Action Qualifiers

- [ ] Handle N (non-stored) qualifier
- [ ] Handle R (reset) qualifier
- [ ] Handle S (set) qualifier
- [ ] Handle P (pulse) qualifier
- [ ] Handle L (time limited) qualifier
- [ ] Handle D (time delayed) qualifier
- [ ] Handle SD, DS, SL qualifiers
- [ ] Write unit tests for each qualifier

### 3.12 Integration Test - Phase 3

- [ ] Create `compiler/resources/test/xml/configuration.xml`
- [ ] Create `compiler/resources/test/xml/sfc_program.xml`
- [ ] Create `compiler/resources/test/xml/full_project.xml`
- [ ] Write integration tests for configuration
- [ ] Write integration tests for SFC
- [ ] Run full semantic analysis pipeline

**Phase 3 Milestone**: Parse complete PLCopen XML projects.

---

## Phase 4: Refinement

**Goal**: Production readiness, compatibility, documentation.

### 4.1 Version Detection

- [ ] Extract PLCopen version from XML namespace
- [ ] Implement version detection logic
- [ ] Add P6008 (XmlUnsupportedVersion) to `problem-codes.csv`
- [ ] Create `docs/compiler/problems/P6008.rst`
- [ ] Support TC6 v2.01 (primary)
- [ ] Evaluate TC6 v2.0 compatibility
- [ ] Write version detection tests

### 4.2 Error Message Review

- [ ] Review all XML error messages for clarity
- [ ] Ensure consistent message format
- [ ] Add context hints where helpful
- [ ] Test error messages with real-world mistakes

### 4.3 Edge Cases

- [ ] Test empty project file
- [ ] Test minimal valid project
- [ ] Test project with only types
- [ ] Test project with only POUs
- [ ] Test very large XML files
- [ ] Test deeply nested structures

### 4.5 Mixed Project Support

- [ ] Test project with both XML and ST files
- [ ] Verify cross-file references work
- [ ] Test XML POU calling ST POU
- [ ] Test ST POU using XML type

### 4.6 Test Coverage

- [ ] Run coverage report
- [ ] Identify untested paths
- [ ] Add tests to reach 85% minimum
- [ ] Document any intentionally untested code

### 4.7 Documentation - User

- [ ] Add XML support section to compiler docs
- [ ] Document supported PLCopen XML features
- [ ] Document unsupported features (FBD, LD, IL)

### 4.8 Documentation - Developer

- [ ] Document XML module architecture
- [ ] Document transformation patterns
- [ ] Document how to add new element support
- [ ] Update CLAUDE.md if needed

**Phase 4 Milestone**: Production-ready XML support.

---

## Test Resources

### XML Test Files to Create

```
compiler/resources/test/xml/
├── minimal_project.xml          # Smallest valid project
├── data_types.xml               # All data type variants
├── function.xml                 # Function with ST body
├── function_block.xml           # Function block with ST body
├── program.xml                  # Program with ST body
├── sfc_program.xml              # Program with SFC body
├── configuration.xml            # Configuration with resources
├── full_project.xml             # Complete project example
├── malformed/
│   ├── not_xml.xml              # Invalid XML syntax
│   ├── wrong_root.xml           # Valid XML, wrong root element
│   ├── missing_name.xml         # POU without name attribute
│   └── invalid_type.xml         # Invalid pouType value
└── versions/
    ├── v2_01.xml                # TC6 v2.01 format
    └── v2_00.xml                # TC6 v2.0 format (if supported)
```

---

## Dependencies

### Crates to Add

```toml
# compiler/sources/Cargo.toml
[dependencies]
quick-xml = { version = "0.31", features = ["serialize"] }
serde = { version = "1.0", features = ["derive"] }

[build-dependencies]
xsd-parser = "0.4"  # Or latest stable version
```

### Files to Modify

- `compiler/sources/Cargo.toml` - Add dependencies
- `compiler/sources/build.rs` - Add XSD generation (new file)
- `compiler/sources/src/parsers/xml_parser.rs` - Implement parser
- `compiler/sources/src/parsers/mod.rs` - May need updates
- `compiler/problems/resources/problem-codes.csv` - Add new codes

### Files to Create

- `compiler/resources/schemas/tc6_xml_v201.xsd`
- `compiler/sources/src/xml/mod.rs`
- `compiler/sources/src/xml/generated.rs` (or build-generated)
- `compiler/sources/src/xml/error.rs`
- `compiler/sources/src/xml/transform.rs`
- `compiler/sources/src/xml/body_parser.rs`
- `docs/compiler/problems/P0006.rst`
- `docs/compiler/problems/P0007.rst`
- `docs/compiler/problems/P6008.rst`
- `docs/compiler/problems/P9003.rst`

---

## Summary

| Phase | Tasks | Goal |
|-------|-------|------|
| 0 | 17 | XSD tooling setup |
| 1 | 35 | Data types working |
| 2 | 39 | POUs with ST bodies |
| 3 | 36 | Configuration and SFC |
| 4 | 24 | Production ready |
| **Total** | **151** | Complete XML support |
