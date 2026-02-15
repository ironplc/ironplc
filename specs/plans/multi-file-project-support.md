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
- **PLCopen XML content detection**: The compiler's XML parser (`xml/position.rs`) validates by content — checks for `<project>` root element with namespace `http://www.plcopen.org/xml/tc6_0201`. The VS Code extension also has a `firstLine` regex that detects PLCopen XML by namespace in the opening tag. All PLCopen XML tools (Beremiz, TwinCAT, CODESYS, OpenPLC) use plain `.xml` as the file extension — no product produces `.plcxml` files
- **VS Code extension** (`package.json`): Registers `.st`, `.iec` (as `61131-3-st`) and a `plcopen-xml` language with `.plcxml` extension plus `firstLine` content detection. The `.plcxml` extension was added for test fixtures only — it should be removed since no product produces it
- **VS Code LSP document selector** (`extension.ts`): Only includes `61131-3-st` — PLCopen XML files are syntax-highlighted but never sent to the LSP for analysis
- **Diagnostics** (`dsl/diagnostic.rs`): Already support primary + secondary labels with `with_secondary()`

### What's Missing

- No understanding of existing PLC project structures (Beremiz, TwinCAT)
- No support for TwinCAT file formats (`.TcPOU`, `.TcGVL`, `.TcDUT`)
- LSP document selector only includes `61131-3-st` — PLCopen XML and TwinCAT files are not sent to the LSP for analysis
- LSP `related_information` is always `None` — secondary labels not surfaced to VS Code
- CLI `enumerate_files` returns all directory entries without type filtering — filtering happens later in `Source::library()` via `FileType`
- Directory enumeration is non-recursive and order-undefined

---

## Phase 1: Additional File Format Support

**Goal**: Parse TwinCAT file formats using existing parsers.

### 1.1 TwinCAT File Type Recognition

Add TwinCAT file extensions to `FileType`.

- [ ] Add `TwinCat` variant to `FileType` enum in `compiler/sources/src/file_type.rs`
- [ ] Map `.TcPOU` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Map `.TcGVL` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Map `.TcDUT` extension (case-insensitive) to `FileType::TwinCat`
- [ ] Mark `TwinCat` as supported in `is_supported()`
- [ ] Add extensions to `extensions()` method
- [ ] Write unit tests for each extension and case-insensitivity

**Note on PLCopen XML**: The `.xml` extension is correct — all PLCopen XML tools (Beremiz, TwinCAT, CODESYS, OpenPLC) produce plain `.xml` files. The compiler already recognizes `.xml` and validates PLCopen XML by content (checking for `<project>` root element with `http://www.plcopen.org/xml/tc6_0201` namespace). No extension changes needed for PLCopen XML.

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

### 1.5 VS Code Extension Updates

Register TwinCAT file types, fix the LSP document selector, and clean up the `.plcxml` extension.

**Register TwinCAT files:**

- [ ] In `integrations/vscode/package.json`: add `.TcPOU`, `.TcGVL`, `.TcDUT` as recognized file extensions (either as a new language entry or by extending an existing one)
- [ ] Verify that `didChange` notifications are sent to the LSP for `.TcPOU` files opened in VS Code

**Fix LSP document selector:**

The VS Code extension's LSP client (`extension.ts`) currently only includes `61131-3-st` in its `documentSelector`. This means PLCopen XML files (and TwinCAT files) are syntax-highlighted but never sent to the LSP for analysis.

- [ ] In `integrations/vscode/src/extension.ts`: add `plcopen-xml` to the LSP `documentSelector`
- [ ] Add the new TwinCAT language to the LSP `documentSelector`
- [ ] Verify PLCopen XML files are sent to the LSP for analysis when opened

**Remove `.plcxml` extension:**

No product (Beremiz, TwinCAT, CODESYS, OpenPLC) produces `.plcxml` files — all use plain `.xml`. The `.plcxml` extension was added for test fixtures only. The VS Code `plcopen-xml` language already has a `firstLine` content-based regex that detects PLCopen XML by namespace, which handles `.xml` files correctly.

- [ ] Remove `.plcxml` from the `extensions` array in the `plcopen-xml` language entry in `package.json`
- [ ] Rename grammar test fixtures from `.plcxml` to `.xml` and update test configuration
- [ ] Verify `firstLine` content detection still identifies PLCopen XML files with `.xml` extension

**Phase 1 Milestone**: `ironplcc check my_pou.TcPOU` works. VS Code sends change notifications and LSP analysis requests for TwinCAT and PLCopen XML files.

---

## Phase 2: Project Discovery Pipeline

**Goal**: When given a directory, IronPLC detects existing project structures and loads the correct set of files.

### Architecture

A chain of project detectors, each checking for a specific project structure. The first match wins; if none match, fall back to current behavior. Discovery returns a structured result so that consumers (LSP, CLI) can adapt behavior based on the detected project type.

```rust
/// The type of PLC project that was detected.
pub enum ProjectType {
    /// Beremiz project (plc.xml found in directory)
    Beremiz,
    /// TwinCAT 3 project (.plcproj found in directory)
    TwinCat,
    /// No specific project structure detected; all supported files enumerated
    Unstructured,
}

/// The result of project discovery.
pub struct DiscoveredProject {
    /// What kind of project was detected
    pub project_type: ProjectType,
    /// The root directory of the discovered project
    pub root_dir: PathBuf,
    /// The source files to load, in deterministic order
    pub files: Vec<PathBuf>,
}
```

```
Directory path
    ↓
discovery::discover(dir) -> Result<DiscoveredProject, Diagnostic>
    ├─ detect_beremiz(dir)    → Some(DiscoveredProject { Beremiz, ... })
    ├─ detect_twincat(dir)    → Some(DiscoveredProject { TwinCat, ... })
    └─ detect_fallback(dir)   → DiscoveredProject { Unstructured, ... }
```

Each detector is a function `fn(dir: &Path) -> Option<DiscoveredProject>` returning `None` if the project type is not detected, or `Some(project)` with the detected type and ordered file list.

### 2.1 Discovery Module Structure

- [ ] Create `compiler/sources/src/discovery/mod.rs`
- [ ] Define `ProjectType` enum with `Beremiz`, `TwinCat`, `Unstructured` variants
- [ ] Define `DiscoveredProject` struct with `project_type`, `root_dir`, `files` fields
- [ ] Define public function `discover(dir: &Path) -> Result<DiscoveredProject, Diagnostic>`
- [ ] Implement detector chain: try each detector in order, use first `Some` result
- [ ] Export module from `compiler/sources/src/lib.rs`
- [ ] Write unit test: empty directory returns `Unstructured` with empty files
- [ ] Write unit test: directory with unknown files returns `Unstructured` with empty files

### 2.2 Beremiz Project Detection

A Beremiz project directory contains `plc.xml` (PLCopen TC6 XML) and optionally `beremiz.xml` (IDE settings). Without detection, pointing at a Beremiz directory would also try to parse `beremiz.xml` and build artifacts, producing errors.

- [ ] Implement `detect_beremiz(dir: &Path) -> Option<DiscoveredProject>`
- [ ] Check for existence of `plc.xml` in the directory
- [ ] Return `Some(DiscoveredProject { project_type: Beremiz, files: vec![dir.join("plc.xml")] })` if found
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

- [ ] Implement `detect_twincat(dir: &Path) -> Option<DiscoveredProject>`
- [ ] Search for a `.plcproj` file in the directory (non-recursive)
- [ ] Parse `.plcproj` XML to extract `<Compile Include="...">` paths
- [ ] Resolve Include paths relative to the `.plcproj` file location
- [ ] Return `DiscoveredProject { project_type: TwinCat, files }` with the ordered file list
- [ ] When a referenced file does not exist: return a `Diagnostic` error (fail fast rather than silently skip)
- [ ] Write unit test with mock `.plcproj` and referenced files
- [ ] Write unit test: directory without `.plcproj` returns `None`
- [ ] Write unit test: `.plcproj` with missing referenced file returns diagnostic error

### 2.4 Fallback Detection

When no specific project structure is detected, enumerate all supported files. Enhance current behavior with deterministic ordering.

- [ ] Implement `detect_fallback(dir: &Path) -> DiscoveredProject`
- [ ] Enumerate all files with supported extensions (`.st`, `.iec`, `.xml`, `.TcPOU`, `.TcGVL`, `.TcDUT`) based on `FileType`
- [ ] Sort files in deterministic order (alphabetical by path)
- [ ] Return `DiscoveredProject { project_type: Unstructured, files }` (this is the catch-all)
- [ ] Write unit test: files are returned in alphabetical order

### 2.5 Integrate Discovery into CLI

Replace the flat enumeration in `cli.rs` with the discovery pipeline for directory arguments. File arguments continue to bypass discovery.

- [ ] In `cli.rs:enumerate_files`: when the path is a directory, call `discovery::discover()` and use `project.files`
- [ ] When the path is a file, continue returning it directly (no discovery)
- [ ] Write integration test: `ironplcc check beremiz-dir/` loads only `plc.xml`
- [ ] Write integration test: `ironplcc check file.st` bypasses discovery

### 2.6 Integrate Discovery into LSP

Replace `SourceProject::initialize_from_directory`'s flat directory scan with the discovery pipeline.

- [ ] In `sources/project.rs:initialize_from_directory`: call `discovery::discover()` instead of `fs::read_dir` with manual filtering
- [ ] Store `DiscoveredProject` on `SourceProject` so the LSP can access `project_type` for display
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

**Deferred.** Adding secondary labels to analyzer rules is a separate concern from surfacing existing ones. Many rules already emit secondary labels (38+ call sites for `with_secondary()` across the analyzer). Phase 3.1 makes all of these visible in VS Code immediately.

Identifying additional cross-file scenarios (e.g., `VAR_EXTERNAL`/`VAR_GLOBAL` mismatches, duplicate definitions across files) and adding secondary labels to them should be scoped in a separate plan once Phase 3.1 is complete and the value can be assessed.

**Phase 3 Milestone**: Existing secondary labels are visible in VS Code as "Related information".

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
| `compiler/sources/src/file_type.rs` | 1 | Add `TwinCat` variant and TwinCAT extensions |
| `compiler/sources/src/parsers/mod.rs` | 1 | Add TwinCAT parser dispatch |
| `compiler/sources/src/lib.rs` | 2 | Export discovery module |
| `compiler/sources/src/project.rs` | 2 | Use discovery in `initialize_from_directory` |
| `compiler/plc2x/src/cli.rs` | 2 | Use discovery in `enumerate_files` for directories |
| `compiler/plc2x/src/lsp_project.rs` | 3 | Map secondary labels to LSP `related_information` |
| `compiler/problems/resources/problem-codes.csv` | 1 | New problem code for TwinCAT errors |
| `integrations/vscode/package.json` | 1 | Register TwinCAT extensions; remove `.plcxml`; keep `firstLine` content detection |
| `integrations/vscode/src/extension.ts` | 1 | Add `plcopen-xml` and TwinCAT languages to LSP `documentSelector` |

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
**Mitigation**: Parse defensively. Require only the elements we need (`<Declaration>`, `<Implementation><ST>`). Ignore unknown elements. Test against files in TwinCAT's normal on-disk format (`.TcPOU` etc. are standard XML files — no TwinCAT installation or export step needed to create test fixtures).

### Risk 2: Discovery Conflicts

**Issue**: A directory might match multiple detectors (e.g., a Beremiz project that also contains `.st` files).
**Mitigation**: Detectors run in priority order; first match wins. The most specific detector (Beremiz, TwinCAT) runs before the generic fallback. Document the priority order.

### Risk 3: Large Directories

**Issue**: Discovery scanning very large directories could be slow.
**Mitigation**: Detection checks are lightweight (file existence, extension matching). Only the TwinCAT detector reads a file (the small `.plcproj` XML). No recursive scanning in the initial implementation.

## Scope Exclusions

The following are explicitly **out of scope** for this plan:

1. **Siemens SCL (`.scl`) support** — SCL has dialect differences from IEC 61131-3 ST that would cause confusing parse errors without a dedicated parser or dialect mode; Siemens project formats are proprietary binary
2. **CODESYS File-Based Storage detection** — FBS is a paid add-on with limited adoption; the fallback detector handles loose `.st` files
3. **PLCopen XML v1.0 support** — v2.01 is the current standard used by Beremiz and modern tools
4. **TwinCAT nested POUs** — Methods and properties in `ParentName^/` subdirectories are deferred
5. **Recursive directory scanning** — Not needed for any known project structure; can be added later
6. **Compilation ordering** — IEC 61131-3 does not define compilation order; the existing "merge everything, then resolve" approach is correct
7. **`.ironplcignore` or configuration files** — Contradicts the "no new files" principle; CLI explicit file arguments serve as the override mechanism
8. **LSP multi-root workspace support** — Significant architectural change; out of scope

## Design Decisions

1. **TwinCAT detection is non-recursive.** TwinCAT projects sometimes nest the PLC project inside a solution directory structure (`Solution/ProjectName/PLCProject/PLCProject.plcproj`). Non-recursive detection may miss this. Start simple; add subdirectory scanning later if real users need it.

2. **Discovery returns a structured result.** `DiscoveredProject { project_type, root_dir, files }` enables the LSP and VS Code extension to display different information based on the detected project type (e.g., showing "Beremiz project" vs. "TwinCAT project" in the status bar).

3. **VS Code extension uses content-based detection for XML.** No product produces `.plcxml` files — all PLCopen XML tools use plain `.xml`. The VS Code `plcopen-xml` language uses a `firstLine` regex to detect PLCopen XML by the `http://www.plcopen.org/xml/tc6` namespace in the opening tag. TwinCAT files (`.TcPOU`, `.TcGVL`, `.TcDUT`) use distinctive extensions and don't need content detection. The LSP document selector must include all registered languages so files are sent for analysis, not just syntax-highlighted.

4. **TwinCAT detection fails fast on missing files.** When a `.plcproj` references a file that doesn't exist on disk, discovery returns a `Diagnostic` error rather than silently skipping the file. This makes project misconfiguration immediately visible.

5. **XML files are identified by content, not extension.** The `.xml` extension is ambiguous — it could be PLCopen XML, TwinCAT project files, or unrelated XML. Both the compiler and VS Code use content-based detection: the compiler checks for `<project>` root element with the `http://www.plcopen.org/xml/tc6_0201` namespace; VS Code uses a `firstLine` regex matching the same namespace. TwinCAT files use distinctive extensions (`.TcPOU`, `.TcGVL`, `.TcDUT`) that don't need content detection. No product produces `.plcxml` files — the extension was removed.

## Summary

| Phase | Goal | Key Deliverable |
|-------|------|-----------------|
| 1 | Parse TwinCAT file formats | TwinCAT `.TcPOU`/`.TcGVL`/`.TcDUT` support; LSP receives all file types |
| 2 | Auto-detect project structures | `ironplcc check beremiz-project/` and `ironplcc check twincat-project/` just work |
| 3 | Surface secondary labels in LSP | Existing secondary labels visible in VS Code |
