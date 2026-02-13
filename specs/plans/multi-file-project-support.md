# Multi-File Project Support Without New Project Files

This document describes the implementation plan for enabling IronPLC to work with existing PLC project structures from Beremiz, TwinCAT, and other environments — without requiring users to create IronPLC-specific manifest files.

## Overview

Real PLC projects span multiple files with cross-file references. Users migrating from existing PLC environments already have project structures on disk. IronPLC should detect and understand these structures automatically, loading the correct set of files and analyzing them together.

### Design Principles

1. **No new files required**: Users must never create an `ironplc.toml`, `ironplc.json`, or similar manifest
2. **Work with what exists**: Detect existing project structures (Beremiz `plc.xml`, TwinCAT `.plcproj`, etc.) automatically
3. **Graceful fallback**: Unrecognized directories fall back to current behavior (enumerate all recognized files)
4. **Compiler stays file-oriented**: The CLI's core interface remains `ironplcc check file1.st file2.st`; directory arguments trigger discovery as a convenience
5. **Discovery lives in the library, not the editor**: Both CLI and LSP consume the same discovery logic from `compiler/sources/`

### Architectural Layering

Discovery logic is a **library concern** in `compiler/sources/`, not a VS Code extension concern. This follows the pattern established by other language toolchains:

| Layer | IronPLC | Analog |
|-------|---------|--------|
| Compiler (file-oriented) | `ironplcc check file1.st file2.st` | `rustc`, `gcc` |
| Project discovery (directory-oriented) | `compiler/sources/src/discovery/` | `cargo`, `go mod` |
| LSP (consumes discovery) | `ironplcc lsp` + workspace folders | `rust-analyzer`, `gopls` |
| Editor extension (thin, standard) | VS Code extension via File > Open Folder | rust-analyzer extension |

The VS Code extension requires **no discovery logic**. It uses standard VS Code behaviors (File > Open Folder), and the LSP protocol sends workspace folders to the server. The LSP server uses the discovery library to determine which files to load.

### Current State

IronPLC already supports multi-file compilation:

- **CLI** (`cli.rs`): Accepts multiple file paths and directories via `ironplcc check path1 path2 ...`
- **File enumeration** (`cli.rs:enumerate_files`): For directories, returns all entries (flat, non-recursive, no type filtering)
- **Project creation** (`cli.rs:create_project`): Pushes all enumerated files into a `FileBackedProject`
- **LSP** (`lsp.rs`): Receives workspace folders via standard LSP `initialize` protocol, takes first folder only
- **LSP project initialization** (`project.rs:FileBackedProject::initialize`): Delegates to `SourceProject::initialize_from_directory`
- **Directory initialization** (`sources/project.rs:initialize_from_directory`): Flat directory read, filters to supported file types (`.st`, `.iec`, `.xml`)
- **Semantic analysis** (`project.rs:semantic`): Parses all source files, passes all libraries to `analyze(&all_libraries)`
- **Supported file types** (`file_type.rs`): `.st`, `.iec` (StructuredText), `.xml` (PLCopen XML)
- **Diagnostics** (`dsl/diagnostic.rs`): Already support primary + secondary labels with `with_secondary()`

### What's Missing

- No understanding of existing PLC project structures (Beremiz, TwinCAT)
- No support for TwinCAT file formats (`.TcPOU`, `.TcGVL`, `.TcDUT`)
- No recognition of `.scl` files (Siemens Structured Control Language)
- LSP `related_information` is always `None` — secondary labels not surfaced to VS Code
- Directory enumeration is non-recursive and order-undefined

---

## Phase 1: Additional File Format Support

**Goal**: Parse files from TwinCAT and Siemens environments using existing parsers.

### 1.1 TwinCAT File Type Recognition

Add TwinCAT file extensions to `FileType`.

- [ ] Add `TwinCat` variant to `FileType` enum in `compiler/sources/src/file_type.rs`
- [ ] Map `.TcPOU` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Map `.TcGVL` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Map `.TcDUT` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Mark `TwinCat` as supported in `is_supported()`
- [ ] Add extensions to `extensions()` method
- [ ] Write unit tests for each extension and case-insensitivity

### 1.2 TwinCAT Parser

TwinCAT 3 stores each POU as an XML file wrapping standard ST code in CDATA sections:

```xml
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{...}" SpecialFunc="None">
    <Declaration><![CDATA[
PROGRAM MAIN
VAR
    myVar : INT;
END_VAR
    ]]></Declaration>
    <Implementation>
      <ST><![CDATA[
myVar := myVar + 1;
      ]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>
```

The parser extracts CDATA text and feeds it to the existing ST parser.

- [ ] Create `compiler/sources/src/parsers/twincat_parser.rs`
- [ ] Implement `parse(content: &str, file_id: &FileId) -> Result<Library, Diagnostic>`
- [ ] Use `roxmltree` (already a dependency) to parse the XML envelope
- [ ] Detect the object type from root child element: `POU`, `GVL`, or `DUT`
- [ ] Extract `<Declaration>` CDATA content
- [ ] For POU files: extract `<Implementation><ST>` CDATA content
- [ ] Concatenate declaration + implementation into a single ST text
- [ ] Feed concatenated text to `st_parser::parse()`
- [ ] For GVL files: extract `<Declaration>` only (contains `VAR_GLOBAL` blocks)
- [ ] For DUT files: extract `<Declaration>` only (contains `TYPE` blocks)
- [ ] Handle missing CDATA gracefully → return `Diagnostic` with appropriate problem code
- [ ] Handle non-ST implementation bodies (e.g., `<FBD>`) → return P9003 diagnostic
- [ ] Write unit tests: POU with declaration + ST implementation
- [ ] Write unit tests: GVL with global variables only
- [ ] Write unit tests: DUT with type declarations only
- [ ] Write unit tests: missing CDATA returns error diagnostic
- [ ] Write unit tests: extracted ST parses identically to equivalent plain `.st` content

### 1.3 TwinCAT Parser Integration

- [ ] Add `twincat_parser` module to `compiler/sources/src/parsers/mod.rs`
- [ ] Add dispatch case for `FileType::TwinCat` in `parse_source()`
- [ ] Write integration test: `FileType::TwinCat` dispatches to TwinCAT parser

### 1.4 Problem Code for TwinCAT Errors

- [ ] Add problem code for malformed TwinCAT XML (missing expected elements)
- [ ] Add entry to `compiler/problems/resources/problem-codes.csv`
- [ ] Create corresponding `docs/compiler/problems/PXXXX.rst` documentation file
- [ ] Run `just compile` to regenerate Problem enum

### 1.5 SCL Extension Recognition

Siemens SCL (Structured Control Language) is syntactically close to IEC 61131-3 Structured Text. Map `.scl` to the existing ST parser.

- [ ] Add `.scl` extension mapping to `FileType::StructuredText` in `from_path()`
- [ ] Add `"scl"` to the extensions list for `StructuredText`
- [ ] Write unit test for `.scl` extension recognition

**Phase 1 Milestone**: `ironplcc check my_pou.TcPOU` and `ironplcc check my_program.scl` work.

---

## Phase 2: Project Discovery Pipeline

**Goal**: When given a directory, IronPLC detects existing project structures and loads the correct set of files.

### Architecture

A chain of project detectors, each checking for a specific project structure. The first match wins; if none match, fall back to current behavior.

```
Directory path
    ↓
discovery::discover_files(dir) -> Vec<PathBuf>
    ├─ detect_beremiz(dir)    → Some(vec![dir/plc.xml])
    ├─ detect_twincat(dir)    → Some(vec![from .plcproj])
    └─ detect_fallback(dir)   → vec![all supported files]
```

Each detector is a function `fn(dir: &Path) -> Option<Vec<PathBuf>>` returning `None` if the project type is not detected, or `Some(files)` with the ordered file list.

### 2.1 Discovery Module Structure

- [ ] Create `compiler/sources/src/discovery/mod.rs`
- [ ] Define public function `discover_files(dir: &Path) -> Result<Vec<PathBuf>, Diagnostic>`
- [ ] Implement detector chain: try each detector in order, use first `Some` result
- [ ] Export module from `compiler/sources/src/lib.rs`
- [ ] Write unit test: empty directory returns empty vec
- [ ] Write unit test: directory with unknown files returns empty vec

### 2.2 Beremiz Project Detection

A Beremiz project directory contains `plc.xml` (PLCopen TC6 XML) and optionally `beremiz.xml` (IDE settings). Without detection, pointing at a Beremiz directory would also try to parse `beremiz.xml` and build artifacts, producing errors.

- [ ] Implement `detect_beremiz(dir: &Path) -> Option<Vec<PathBuf>>`
- [ ] Check for existence of `plc.xml` in the directory
- [ ] Return `Some(vec![dir.join("plc.xml")])` if found
- [ ] Write unit test with mock directory containing `plc.xml`
- [ ] Write unit test: directory without `plc.xml` returns `None`
- [ ] Write integration test: Beremiz-like directory with `plc.xml` + `beremiz.xml` only loads `plc.xml`

### 2.3 TwinCAT Project Detection

A TwinCAT 3 project has a `.plcproj` file referencing all source files:

```xml
<Project>
  <ItemGroup>
    <Compile Include="POUs\MAIN.TcPOU" />
    <Compile Include="DUTs\ST_MyStruct.TcDUT" />
  </ItemGroup>
</Project>
```

- [ ] Implement `detect_twincat(dir: &Path) -> Option<Vec<PathBuf>>`
- [ ] Search for a `.plcproj` file in the directory (non-recursive first pass)
- [ ] Parse `.plcproj` XML to extract `<Compile Include="...">` paths
- [ ] Resolve Include paths relative to the `.plcproj` file location
- [ ] Return the ordered file list
- [ ] Handle missing referenced files gracefully (warn and skip)
- [ ] Write unit test with mock `.plcproj` and referenced files
- [ ] Write unit test: directory without `.plcproj` returns `None`
- [ ] Write unit test: `.plcproj` with missing referenced file produces warning

### 2.4 Fallback Detection

When no specific project structure is detected, enumerate all supported files. Enhance current behavior with deterministic ordering.

- [ ] Implement `detect_fallback(dir: &Path) -> Option<Vec<PathBuf>>`
- [ ] Enumerate all files with supported extensions (`.st`, `.iec`, `.xml`, `.scl`, `.TcPOU`, `.TcGVL`, `.TcDUT`)
- [ ] Sort files in deterministic order (alphabetical by path)
- [ ] Always return `Some` (this is the catch-all)
- [ ] Write unit test: files are returned in alphabetical order

### 2.5 Integrate Discovery into CLI

Replace the flat enumeration in `cli.rs` with the discovery pipeline for directory arguments. File arguments continue to bypass discovery.

- [ ] In `cli.rs:enumerate_files`: when the path is a directory, call `discovery::discover_files()`
- [ ] When the path is a file, continue returning it directly (no discovery)
- [ ] Write integration test: `ironplcc check beremiz-dir/` loads only `plc.xml`
- [ ] Write integration test: `ironplcc check file.st` bypasses discovery

### 2.6 Integrate Discovery into LSP

Replace `SourceProject::initialize_from_directory`'s flat directory scan with the discovery pipeline.

- [ ] In `sources/project.rs:initialize_from_directory`: call `discovery::discover_files()` instead of `fs::read_dir` with manual filtering
- [ ] Preserve existing error handling for unreadable directories
- [ ] Write integration test: LSP workspace initialization uses discovery

**Phase 2 Milestone**: `ironplcc check my-beremiz-project/` detects `plc.xml` and analyzes only the project file. `ironplcc check my-twincat-project/` reads the `.plcproj` and loads referenced files.

---

## Phase 3: Cross-File Diagnostic Improvements

**Goal**: When errors involve cross-file references, diagnostics clearly indicate both locations.

### Current State

The `Diagnostic` type already supports secondary labels (`secondary: Vec<Label>` with `with_secondary()` builder). The CLI already renders secondary labels via codespan-reporting's `map_diagnostic` function. However, the LSP mapping in `lsp_project.rs:map_diagnostic` always sets `related_information: None`, so secondary labels are invisible in VS Code.

### 3.1 Surface Secondary Labels in LSP

- [ ] In `lsp_project.rs:map_diagnostic`: map `diagnostic.secondary` labels to `DiagnosticRelatedInformation`
- [ ] Convert each secondary `Label` to a `DiagnosticRelatedInformation` with the correct URI and range
- [ ] Set `related_information` on the LSP diagnostic instead of `None`
- [ ] Write unit test: diagnostic with secondary labels produces `related_information`

### 3.2 Add Cross-File Secondary Labels to Analyzer

Identify analyzer rules that would benefit from pointing to related locations across files. Add secondary labels where they improve the user experience.

- [ ] Audit existing analyzer rules for cross-file scenarios (e.g., `VAR_EXTERNAL`/`VAR_GLOBAL` mismatches, duplicate definitions)
- [ ] Add secondary labels to the highest-value cross-file error scenarios
- [ ] Write tests for each new secondary label

**Phase 3 Milestone**: Cross-file errors in VS Code show "Related information" linking to the other file.

---

## Phase 4: LSP Multi-Root Workspace Support

**Goal**: Handle VS Code workspaces containing multiple PLC projects.

### Current State

The LSP takes the **first** workspace folder only (`lsp.rs:start_with_connection` uses `folders.first()`). Multi-root workspaces and subdirectory projects are not supported.

### 4.1 Support Multiple Workspace Folders

- [ ] In `lsp.rs`: iterate over all workspace folders, not just the first
- [ ] Create a separate `SourceProject` per workspace folder
- [ ] Route file change notifications to the correct project by matching file paths
- [ ] Run semantic analysis per project independently
- [ ] Scope diagnostics to their respective project
- [ ] Write tests for multi-folder workspace initialization

### 4.2 Sub-Project Detection

When a single workspace folder contains multiple PLC project structures (e.g., a monorepo), detect each as a separate compilation unit.

- [ ] Extend discovery to scan for sub-projects within a directory
- [ ] Each detected sub-project gets its own `SourceProject`
- [ ] Avoid false cross-project symbol conflicts
- [ ] Write tests for sub-project detection

**Phase 4 Milestone**: A VS Code workspace with multiple PLC project folders analyzes each independently.

---

## Files to Create

| File | Phase | Description |
|------|-------|-------------|
| `compiler/sources/src/parsers/twincat_parser.rs` | 1 | TwinCAT XML → ST extraction and parsing |
| `compiler/sources/src/discovery/mod.rs` | 2 | Project discovery pipeline |
| `docs/compiler/problems/PXXXX.rst` | 1 | TwinCAT malformed XML problem documentation |

## Files to Modify

| File | Phase | Nature of Change |
|------|-------|-----------------|
| `compiler/sources/src/file_type.rs` | 1 | Add `TwinCat` variant, `.scl` extension |
| `compiler/sources/src/parsers/mod.rs` | 1 | Add TwinCAT parser dispatch |
| `compiler/sources/src/lib.rs` | 2 | Export discovery module |
| `compiler/sources/src/project.rs` | 2 | Use discovery in `initialize_from_directory` |
| `compiler/plc2x/src/cli.rs` | 2 | Use discovery in `enumerate_files` for directories |
| `compiler/plc2x/src/lsp_project.rs` | 3 | Map secondary labels to LSP `related_information` |
| `compiler/plc2x/src/lsp.rs` | 4 | Support multiple workspace folders |
| `compiler/problems/resources/problem-codes.csv` | 1 | New problem code for TwinCAT errors |

## Dependencies

### Existing Dependencies (No Changes)

- `roxmltree` — already used for PLCopen XML parsing; reused for TwinCAT XML and `.plcproj` parsing

### No New Dependencies Required

The TwinCAT parser and project discovery both use `roxmltree` (already in `compiler/sources/Cargo.toml`) and `std::fs` for directory operations.

## Test Resources to Create

```
compiler/plc2x/resources/test/
├── twincat/
│   ├── main.TcPOU          # POU with declaration + ST implementation
│   ├── globals.TcGVL       # Global variable list
│   ├── my_type.TcDUT       # Data unit type
│   └── project.plcproj     # TwinCAT project file referencing above
├── beremiz/
│   ├── plc.xml             # PLCopen XML project file
│   └── beremiz.xml         # IDE config (should be ignored by discovery)
└── mixed/
    ├── types.st            # ST type definitions
    └── main.st             # ST program
```

## Risks and Mitigations

### Risk 1: TwinCAT XML Schema Variations

**Issue**: Different TwinCAT versions may produce slightly different XML structures.
**Mitigation**: Parse defensively. Require only the elements we need (`<Declaration>`, `<Implementation><ST>`). Ignore unknown elements. Test against real TwinCAT exports.

### Risk 2: Discovery Conflicts

**Issue**: A directory might match multiple detectors (e.g., a Beremiz project that also contains `.st` files).
**Mitigation**: Detectors run in priority order; first match wins. The most specific detector (Beremiz, TwinCAT) runs before the generic fallback. Document the priority order.

### Risk 3: Large Directories

**Issue**: Discovery scanning very large directories could be slow.
**Mitigation**: Detection checks are lightweight (file existence, extension matching). Only the TwinCAT detector reads a file (the small `.plcproj` XML). No recursive scanning in the initial implementation.

### Risk 4: SCL Dialect Differences

**Issue**: Siemens SCL has extensions beyond IEC 61131-3 ST that may cause parse errors.
**Mitigation**: Start with extension recognition only. Parse errors from dialect differences will surface as normal diagnostics. Dialect-specific parser extensions can be added later based on user feedback.

## Scope Exclusions

The following are explicitly **out of scope** for this plan:

1. **CODESYS File-Based Storage detection** — FBS is a paid add-on with limited adoption; the fallback detector handles loose `.st` files
2. **PLCopen XML v1.0 support** — v2.01 is the current standard used by Beremiz and modern tools
3. **TwinCAT nested POUs** — Methods and properties in `ParentName^/` subdirectories are deferred
4. **Recursive directory scanning** — Not needed for any known project structure; can be added later
5. **Compilation ordering** — IEC 61131-3 does not define compilation order; the existing "merge everything, then resolve" approach is correct
6. **`.ironplcignore` or configuration files** — Contradicts the "no new files" principle; CLI explicit file arguments serve as the override mechanism

## Open Questions

1. **Should TwinCAT detection search subdirectories for `.plcproj`?** TwinCAT projects often nest the PLC project inside a solution directory structure (`Solution/ProjectName/PLCProject/PLCProject.plcproj`). Non-recursive detection would miss this. Recommendation: Start non-recursive; add a single level of subdirectory scanning if real users need it.

2. **Should discovery produce a structured result or just file paths?** The current plan returns `Vec<PathBuf>`. A richer result (`DiscoveredProject { project_type, files, root_dir }`) could be useful for the LSP to display project information. Recommendation: Start with `Vec<PathBuf>` for simplicity; enrich later if needed.

3. **How should the LSP handle file changes to non-ST files?** The VS Code extension currently registers only `61131-3-st` as the document selector. Files like `.TcPOU` or `.xml` won't trigger `didChange` notifications. Recommendation: Add additional file watchers or language registrations in a follow-up to the VS Code extension.

## Summary

| Phase | Goal | Key Deliverable |
|-------|------|-----------------|
| 1 | Parse more file formats | TwinCAT `.TcPOU`/`.TcGVL`/`.TcDUT` and Siemens `.scl` support |
| 2 | Auto-detect project structures | `ironplcc check beremiz-project/` and `ironplcc check twincat-project/` just work |
| 3 | Better cross-file diagnostics | Secondary labels visible in VS Code |
| 4 | Multi-root workspaces | Multiple PLC projects in one VS Code workspace |
