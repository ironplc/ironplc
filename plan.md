# Plan: Multi-File Project Support Without New Project Files

## Problem Statement

Real PLC projects span multiple files with cross-file references. Users migrating from Beremiz, TwinCAT, CODESYS, and other environments already have project structures on disk. IronPLC should work with these existing structures without requiring users to create IronPLC-specific project manifest files.

## Current State

IronPLC **already supports multi-file compilation**:

- The CLI accepts multiple file paths and directories: `ironplcc check file1.st file2.st dir/`
- Directory input enumerates all files in the directory
- All parsed files are merged into a single `Library` via `Library::extend()` before semantic analysis
- The LSP loads all files from the VS Code workspace folder
- Supported file types: `.st`, `.iec` (Structured Text), `.xml` (PLCopen TC6 XML v2.01)
- PLCopen XML support is thorough (POUs, data types, configurations, SFC)

**What's missing**: IronPLC treats all directories the same (flat enumeration of all files). It doesn't understand existing PLC project structures, and it doesn't support file formats from major PLC environments beyond plain ST and PLCopen XML.

## Design Principles

1. **No new files required**: The user should never need to create an `ironplc.toml` or similar manifest
2. **Work with what exists**: Detect and understand existing project structures automatically
3. **Graceful fallback**: If no recognizable project structure is found, fall back to current behavior (enumerate all recognized files in directory)
4. **Incremental value**: Each phase delivers standalone value; later phases are not prerequisites

---

## Phase 1: Recognize More File Formats From Existing Environments

**Goal**: Users can point IronPLC at files from TwinCAT or Siemens projects and have them parsed.

### 1a. TwinCAT File Support (`.TcPOU`, `.TcGVL`, `.TcDUT`)

TwinCAT 3 stores each POU as a separate XML file wrapping standard ST code in CDATA sections:

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

**Changes needed:**

- **`compiler/sources/src/file_type.rs`**: Add `TwinCatPou`, `TwinCatGvl`, `TwinCatDut` variants to `FileType` (or a single `TwinCat` variant), recognizing `.TcPOU`, `.TcGVL`, `.TcDUT` extensions (case-insensitive)
- **`compiler/sources/src/parsers/`**: New `twincat_parser.rs` module (~200-300 lines):
  - Use `roxmltree` (already a dependency) to parse the XML
  - Extract `<Declaration>` and `<Implementation><ST>` CDATA content
  - Concatenate declaration + implementation into a single ST text block
  - Feed the result into the existing `st_parser::parse()` to produce a `Library`
  - Handle `.TcGVL` files (which contain `<Declaration>` only with `VAR_GLOBAL` blocks)
  - Handle `.TcDUT` files (which contain `<Declaration>` only with `TYPE` blocks)
- **`compiler/sources/src/parsers/mod.rs`**: Add dispatch case for TwinCAT file types
- **Problem codes**: Add a problem code for malformed TwinCAT XML (e.g., missing CDATA sections)
- **Tests**: ST extracted from TwinCAT XML parses identically to equivalent plain `.st` files

**Estimated scope**: ~400-500 lines of new code plus tests.

### 1b. Siemens `.scl` Extension Recognition

Siemens SCL (Structured Control Language) is syntactically close to IEC 61131-3 Structured Text. Many `.scl` files will parse as valid ST.

**Changes needed:**

- **`compiler/sources/src/file_type.rs`**: Map `.scl` extension to `FileType::StructuredText`
- That's it for the first pass. SCL-specific dialect differences (if any are encountered) can be handled later as parser extensions.

**Estimated scope**: ~5 lines of code plus a test.

---

## Phase 2: Automatic Project Structure Detection

**Goal**: When pointed at a directory, IronPLC intelligently detects the project type and loads files accordingly, using existing project metadata from other environments.

### Architecture: Project Discovery Pipeline

Add a **project discovery** step that runs before file enumeration. This is a chain-of-responsibility pattern where each detector gets a chance to recognize the directory:

```
Directory path
    ↓
[Project Detector Chain]
    ├─ BeremizDetector: looks for plc.xml
    ├─ TwinCatDetector: looks for *.plcproj
    ├─ CodesysFbsDetector: looks for characteristic FBS structure
    └─ FallbackDetector: enumerate all recognized files (current behavior)
    ↓
List of files to compile (ordered)
```

**Location**: New module `compiler/sources/src/discovery.rs` (or `compiler/sources/src/discovery/` directory if multiple detectors warrant it).

### 2a. Beremiz/OpenPLC Project Detection

A Beremiz project directory contains `plc.xml` (the PLCopen XML project file) and optionally `beremiz.xml` (IDE settings).

**Detection**: If the directory contains a file named `plc.xml`, treat it as a Beremiz project.

**Behavior**: Parse only `plc.xml` as the project source. Ignore other files (they are build artifacts or IDE config). The PLCopen XML parser already handles this file format completely.

**Why this matters**: Without detection, pointing IronPLC at a Beremiz project directory would try to parse every file, including `beremiz.xml` (which is not PLCopen XML) and build artifacts. With detection, it correctly selects only the program file.

**Changes needed:**
- Detection function: check for `plc.xml` existence
- Return `vec![dir.join("plc.xml")]` as the file list
- ~30 lines plus tests

### 2b. TwinCAT Project Detection

A TwinCAT 3 project has a `.plcproj` file that references all POU/GVL/DUT files in XML:

```xml
<Project>
  <ItemGroup>
    <Compile Include="POUs\MAIN.TcPOU" />
    <Compile Include="DUTs\ST_MyStruct.TcDUT" />
    <Compile Include="GVLs\GVL_Global.TcGVL" />
  </ItemGroup>
</Project>
```

**Detection**: If the directory (or a subdirectory) contains a `.plcproj` file.

**Behavior**: Parse the `.plcproj` XML to extract `<Compile Include="...">` paths. Resolve them relative to the `.plcproj` location. Return the ordered file list. This respects the project's intended compilation set rather than blindly including everything.

**Changes needed:**
- Detection function: glob for `**/*.plcproj`
- Parse `.plcproj` to extract Compile Include paths
- Resolve relative paths
- ~100-150 lines plus tests

### 2c. Fallback: Directory Enumeration (Current Behavior)

If no project structure is detected, fall back to the current behavior: enumerate all files with recognized extensions in the directory.

**Enhancement opportunity**: Sort files in a deterministic order (alphabetical, or types-before-programs) so that compilation order is predictable. Currently `enumerate_files` does not guarantee order.

### Integration Points

- **CLI (`plc2x/src/cli.rs`)**: Replace `enumerate_files()` with the new discovery pipeline for directory arguments. File arguments bypass discovery and are used directly.
- **LSP (`plc2x/src/lsp_project.rs`)**: Use discovery when initializing from a workspace folder. The `initialize()` method currently calls `initialize_from_directory()` on `SourceProject` — this should be routed through the discovery pipeline.
- **`SourceProject` (`sources/src/project.rs`)**: Add a `from_discovered_files(files: Vec<PathBuf>)` constructor or similar.

---

## Phase 3: Cross-File Diagnostic Improvements

**Goal**: When errors involve cross-file references, the diagnostics clearly indicate both the reference site and the definition site.

### Current Limitation

All files are merged into a single `Library` via concatenation (`Library::extend()`). When analysis runs, it operates on this merged library. Source locations (`SourceSpan`) include `FileId`, so errors can already be attributed to specific files. However, some cross-file scenarios produce confusing diagnostics:

- A `VAR_EXTERNAL` in `file_a.st` references a `VAR_GLOBAL` in `file_b.st`. If the types don't match, the error only points to one location.
- Duplicate symbol definitions across files show the second definition but not the first.

### Proposed Improvements

#### 3a. Multi-Span Diagnostics

Enhance the `Diagnostic` type to support **secondary labels** (related spans). This is a common pattern in compilers (Rust's own diagnostics use this):

```
error[P0XXX]: VAR_EXTERNAL type mismatch
  --> file_a.st:5:3
   |
5  |     myVar : INT;
   |     ^^^^^ declared as INT here
   |
  --> file_b.st:2:3
   |
2  |     myVar : REAL;
   |     ^^^^^ but VAR_GLOBAL is REAL here
```

**Changes needed:**
- Extend `Diagnostic` (in `compiler/problems/`) to support a list of secondary `Label`s
- Update relevant analyzer rules to attach secondary labels for cross-file errors
- Update the LSP diagnostic mapper to include `relatedInformation` (LSP supports this natively via `DiagnosticRelatedInformation`)
- Update the CLI diagnostic printer to show secondary locations

#### 3b. Track Element Origin Files

Currently, after `Library::extend()`, there is no way to know which file an element came from except through the `SourceSpan` on individual AST nodes. Consider enriching the merged `Library` or the `SemanticContext` with a file-origin index so that "go to definition" across files becomes straightforward in the LSP.

---

## Phase 4: LSP Multi-Root Workspace Support

**Goal**: The VS Code extension properly handles workspaces with multiple PLC projects (e.g., a monorepo with several PLC programs).

### Current State

The LSP initializes with workspace folders and loads all files from each folder. It appears to treat each workspace folder as a single project.

### Proposed Improvements

- **Auto-detect sub-projects**: If a workspace folder contains multiple project structures (e.g., multiple `plc.xml` files in subdirectories, or multiple `.plcproj` files), detect each as a separate compilation unit
- **Separate analysis per project**: Each detected project gets its own `SourceProject` and independent semantic analysis, avoiding false cross-project symbol conflicts
- **Per-project diagnostics**: Errors are scoped to their project, not the entire workspace

This depends on Phase 2 (project discovery) being in place.

---

## Phase Summary

| Phase | Description | Depends On | Key Deliverable |
|-------|-------------|------------|-----------------|
| 1a | TwinCAT file format support | Nothing | Parse `.TcPOU`, `.TcGVL`, `.TcDUT` files |
| 1b | `.scl` extension recognition | Nothing | Siemens SCL files recognized as ST |
| 2a | Beremiz project detection | Nothing | `ironplcc check beremiz-project/` just works |
| 2b | TwinCAT project detection | Phase 1a | `ironplcc check twincat-project/` just works |
| 2c | Fallback enhancement | Nothing | Deterministic file ordering |
| 3a | Multi-span diagnostics | Nothing | Better cross-file error messages |
| 3b | Element origin tracking | Nothing | Foundation for cross-file go-to-definition |
| 4 | Multi-root workspace | Phase 2 | Multiple PLC projects in one VS Code workspace |

## Files Likely Modified

| File | Phases | Nature of Change |
|------|--------|-----------------|
| `compiler/sources/src/file_type.rs` | 1a, 1b | Add TwinCAT and SCL file types |
| `compiler/sources/src/parsers/mod.rs` | 1a | Add TwinCAT parser dispatch |
| `compiler/sources/src/parsers/twincat_parser.rs` | 1a | **New file**: TwinCAT XML → ST extraction |
| `compiler/sources/src/discovery.rs` | 2a, 2b, 2c | **New file**: Project structure detection |
| `compiler/sources/src/project.rs` | 2 | Integration with discovery pipeline |
| `compiler/plc2x/src/cli.rs` | 2 | Use discovery pipeline for directory args |
| `compiler/plc2x/src/lsp_project.rs` | 2, 4 | Use discovery for workspace initialization |
| `compiler/plc2x/src/lsp.rs` | 4 | Multi-root workspace handling |
| `compiler/problems/resources/problem-codes.csv` | 1a, 3a | New problem codes |
| `compiler/problems/src/problem.rs` | 3a | Secondary labels support |
| Various `compiler/analyzer/src/rule_*.rs` | 3a | Add secondary labels to cross-file errors |

## Open Questions

1. **TwinCAT nested POUs**: TwinCAT supports methods and properties as child objects of function blocks, stored in sub-files within a `ParentName^/` directory. Should Phase 1a handle these, or defer to a later iteration?
   - **Recommendation**: Defer. Start with top-level POUs, GVLs, and DUTs. Methods in sub-files are an advanced case.

2. **CODESYS File-Based Storage**: Should we add detection for CODESYS FBS projects? The FBS format stores `.st` files that are already parseable, but the directory structure has metadata files that might confuse enumeration.
   - **Recommendation**: Defer to a later phase. FBS is a paid add-on with limited adoption. The fallback detector (enumerate all `.st` files) already handles the common case.

3. **PLCopen XML version support**: Currently only TC6 v2.01 namespace is supported. Should we add v1.0 support?
   - **Recommendation**: Evaluate demand. V2.01 is what Beremiz and modern tools use. Add v1.0 only if users request it.

4. **Compilation ordering within a project**: When merging multiple files, should IronPLC attempt to order them (data types first, then function blocks, then programs)? Or is the current "merge everything, then resolve" approach sufficient?
   - **Recommendation**: The current approach is sufficient for now. IEC 61131-3 does not define a compilation order — all declarations are visible throughout the project. The analyzer already resolves types after merging. Ordering would only matter for forward-reference error messages, which is a UX concern addressable in Phase 3.

5. **Should the discovery pipeline be configurable?** E.g., allow users to exclude certain files or override detection.
   - **Recommendation**: Not initially. If needed later, this could be handled via a lightweight `.ironplcignore` file (similar to `.gitignore`), but this contradicts the "no new files" principle. Better to rely on the CLI's explicit file arguments for override: `ironplcc check specific_file.st` bypasses discovery entirely.
