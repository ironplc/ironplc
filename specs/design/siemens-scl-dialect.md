# Design: Siemens SCL Dialect Support

## Overview

This document describes the design for parsing Siemens SCL (Structured Control Language) files as used in TIA Portal for S7-1200/S7-1500 PLCs. The goal is to parse `.scl` files from real-world Siemens projects — such as the [open-process-library](https://github.com/SASE-Space/open-process-library) — without errors.

This design implements [ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md) for the Siemens dialect.

## Scope

**In scope (parsing):** Accept all syntactically valid Siemens SCL constructs and represent them in the AST without parse errors.

**Out of scope (future):** Semantic analysis of Siemens-specific constructs (e.g., resolving data block references, analyzing `ORGANIZATION_BLOCK` scheduling). Standard IEC 61131-3 semantic analysis continues to run on the standard-compliant portions of SCL files.

## File Detection

Siemens source files are detected by the `FileType` enum in `compiler/sources/src/file_type.rs`:

```
FileType::SiemensSCL  // new variant
```

Extension mappings:

| Extension | Content |
|-----------|---------|
| `.scl` | SCL source files (FBs, FCs, OBs — may contain multiple blocks) |
| `.udt` | User-defined type (PLC data type) definitions |

Both extensions map to `FileType::SiemensSCL`. These are plain text files with no XML wrapper.

**`.db` extension — deferred.** TIA Portal uses `.db` for data block source files, but `.db` is an extremely common extension for other purposes (SQLite databases, Berkeley DB, caches). Blindly mapping `.db` → `FileType::SiemensSCL` would cause IronPLC to attempt parsing binary files as SCL, producing confusing errors. Options for future support include content sniffing (checking if the file starts with `DATA_BLOCK`), requiring explicit opt-in via project configuration, or only recognizing `.db` files when sibling `.scl` files are present. For now, `.db` is not auto-detected; users can rename data block files to `.scl`.

The SCL parser delegates to the standard ST parser with Siemens-specific parse options enabled.

## SCL Extensions to Parse

The extensions are listed in priority order based on how frequently they appear in real SCL files.

### Priority 1: Appears on Nearly Every File

#### 1.1 `#` Variable Prefix

All block-local variables in SCL use a `#` prefix:

```
#error := #setpoint - #processValue;
#output := #error * #gain;
```

**Design:** The lexer produces `Hash` + `Identifier` tokens. A dialect token filter (see [dialect-token-transforms.md](dialect-token-transforms.md#category-3-token-filtering)) removes `Hash` tokens that are immediately followed by `Identifier`. The `Identifier` token retains its own span and text — the parser sees a normal variable reference. The `#` is not part of the variable name.

This filter is safe because `Hash` + `Identifier` never occurs in standard IEC 61131-3 — the standard uses `#` only between type keywords and literal values (e.g., `INT#5` is `Int` + `Hash` + `Digits`).

#### 1.2 Curly-Brace Pragmas

SCL uses `{ }` for compile-time directives:

```
{S7_Optimized_Access := 'TRUE'}
{InstructionName := 'FB_PID'}
{DB_Specific := 'FALSE'}
```

**Design:** A dialect token transform collapses `LeftBrace ... RightBrace` sequences into a single `Pragma` token (see [dialect-token-transforms.md](dialect-token-transforms.md#category-4-token-collapsing)). The parser skips `Pragma` tokens like whitespace.

The pragma content is opaque — the parser does not interpret key-value pairs inside pragmas. Future work could extract structured metadata from pragmas for use in analysis. This transform is shared with the Beckhoff TwinCAT dialect.

#### 1.3 `REGION` / `END_REGION`

Code folding markers with no semantic meaning:

```
REGION Initialization
    #counter := 0;
END_REGION
```

**Design:** After keyword promotion, `REGION` and `END_REGION` identifiers become `Region` and `EndRegion` token types. A dialect token filter (see [dialect-token-transforms.md](dialect-token-transforms.md#what-about-region--end_region)) removes `Region` tokens plus the following tokens up to the next `Newline` (the region name text), and removes standalone `EndRegion` tokens. The tokens between regions are parsed normally.

#### 1.4 Double-Quoted Block Names

Siemens puts block names in double quotes:

```
FUNCTION_BLOCK "FB_Motor_Control"
FUNCTION "FC_Calculate" : Real
```

And in call expressions:

```
"FB_Motor_Control_DB"(Enable := TRUE);
"FC_Calculate"(Input := 5.0);
```

**Design:** The lexer produces `DoubleByteString` tokens for `"..."`. In SCL, double-quoted text is always an identifier — never a string literal (string literals use single quotes). A dialect token rewrite (see [dialect-token-transforms.md](dialect-token-transforms.md#category-2-token-rewriting)) converts ALL `DoubleByteString` tokens to `Identifier` tokens, stripping the quotes from `text` while preserving the original `span`. This is a blanket rule — no context sensitivity needed.

The span still covers the full `"FB_Motor"` range in the source file including quotes, so error messages highlight the quoted name in context.

#### 1.5 `VERSION` Declaration

Block version metadata appearing after pragmas and before variable declarations:

```
FUNCTION_BLOCK "FB_PID"
{ S7_Optimized_Access := 'TRUE' }
VERSION : 0.1
```

**Design:** Add `Version` as a new keyword token (promoted from `Identifier` only in SCL dialect). The parser grammar includes an optional `VERSION : <literal>` clause after the POU header and before variable sections. This rule is self-gating: in standard mode, `VERSION` remains an `Identifier` token and the `Version` rule never matches (see [Dialect Gating](#dialect-gating-token-transforms-as-the-gate)). The version value (a fixed-point literal like `0.1`) is stored as metadata in the AST but not used in semantic analysis.

#### 1.6 `BEGIN` Keyword

In SCL, the `BEGIN` keyword separates variable declarations from the body in function blocks and data blocks:

```
FUNCTION_BLOCK "FB_Example"
VAR
    counter : INT;
END_VAR

BEGIN
    #counter := #counter + 1;
END_FUNCTION_BLOCK
```

In standard IEC 61131-3 ST, the body follows immediately after the last `END_VAR` with no explicit marker. The `BEGIN` keyword is optional in SCL — files work with or without it.

**Design:** Add `Begin` as a keyword token (promoted from `Identifier` only in SCL dialect). The parser grammar includes an optional `Begin` token between variable declarations and the body. This rule is self-gating: in standard mode, `BEGIN` remains an `Identifier` and the `Begin` rule never matches. The `Begin` token is consumed and discarded — no AST representation needed.

#### 1.7 `VAR CONSTANT` Compound Keyword

SCL uses `VAR CONSTANT` (two keywords) for constant declarations:

```
VAR CONSTANT
    MAX_SPEED : REAL := 3000.0;
    RAMP_TIME : TIME := T#5s;
END_VAR
```

**Design:** The parser already recognizes `CONSTANT` as a qualifier on variable sections. In SCL mode, `VAR CONSTANT` is accepted as equivalent to `VAR CONSTANT` in standard IEC 61131-3 (which uses `VAR CONSTANT` as well — this is actually standard behavior). No change needed if the parser already handles this.

#### 1.8 `CONTINUE` Statement

SCL supports `CONTINUE` to skip to the next loop iteration, which is standard practice in TIA Portal:

```
FOR #i := 0 TO 100 DO
    IF #arr[#i] = 0 THEN
        CONTINUE;
    END_IF;
    // process arr[i]
END_FOR;
```

**Design:** Add `Continue` as a keyword token. The parser recognizes `CONTINUE` as a statement keyword, parallel to `EXIT`. This keyword is shared across multiple dialects — see the `DIALECT_KEYWORDS` table in the [Extension Origin Model](#extension-origin-model) where it has `origins: &[Iec61131Ed3, BeckhoffCodesys, SiemensSCL]`.

### Priority 2: Common in Real Projects

#### 2.1 `VAR_STAT` (Static Variables)

Static local variables in functions that persist across calls:

```
FUNCTION "FC_Counter" : Int
VAR_STAT
    count : Int := 0;
END_VAR
```

**Design:** Add `VarStat` as a new keyword token. The parser accepts `VAR_STAT ... END_VAR` as a variable section, parallel to `VAR_TEMP`, `VAR_INPUT`, etc. In the AST, static variables are represented with a new variable qualifier or section type. For semantic analysis, `VAR_STAT` in a `FUNCTION` is similar to instance variables in a function block — they persist across calls.

#### 2.2 `//` Line Comments

Already handled. SCL universally uses `//` line comments, which the IronPLC lexer already recognizes (though they are not standard IEC 61131-3). The existing `ParseOptions::allow_c_style_comments` flag should be automatically enabled for the SCL dialect.

### Priority 3: Present in Some Projects

#### 2.3 `DATA_BLOCK` / `END_DATA_BLOCK`

Siemens-specific data storage construct:

```
DATA_BLOCK "DB_Settings"
{ S7_Optimized_Access := 'TRUE' }
VERSION : 0.1
NON_RETAIN

STRUCT
    maxSpeed : Real := 100.0;
    timeout : Time := T#5s;
END_STRUCT;

BEGIN
    maxSpeed := 150.0;
END_DATA_BLOCK
```

Instance data blocks reference an FB type:

```
DATA_BLOCK "DB_MotorInstance"
"FB_Motor_Control"
BEGIN
END_DATA_BLOCK
```

**Design:** Add `DataBlock` and `EndDataBlock` as new keyword tokens. The parser recognizes `DATA_BLOCK "name" ... END_DATA_BLOCK` as a new top-level declaration type. In the AST, a data block is represented as a new `LibraryElementKind::DataBlockDeclaration` variant (see [AST Extensions](#ast-extensions-dsl-crate)).

Data blocks have two forms:

1. **Global data blocks** — contain a `STRUCT ... END_STRUCT` definition followed by an optional `BEGIN` initialization section. The struct fields are parsed using the existing `StructureDeclaration` mechanism. The `NON_RETAIN` keyword may appear on its own line as a block-level modifier (not inside a `VAR` section). `NON_RETAIN` is already a standard token (`TokenType::NonRetain`), so the parser recognizes it in this context without promotion.

2. **Instance data blocks** — reference an FB type name (a double-quoted identifier that becomes `Identifier` after token rewriting) followed by an optional `BEGIN ... END_DATA_BLOCK` section.

The initialization section after `BEGIN` (assignments like `maxSpeed := 150.0;`) is parsed as a `Vec<StmtKind>` using the existing statement parser. Semantic validation of these assignments (e.g., verifying they match the struct fields) is out of scope for initial parsing support.

#### 2.4 `ORGANIZATION_BLOCK` / `END_ORGANIZATION_BLOCK`

System-level program blocks:

```
ORGANIZATION_BLOCK "Main"
{ S7_Optimized_Access := 'TRUE' }
VERSION : 0.1

VAR_TEMP
    tempInfo : Int;
END_VAR

// body
END_ORGANIZATION_BLOCK
```

**Design:** Add `OrganizationBlock` and `EndOrganizationBlock` as keyword tokens. The parser treats an organization block structurally like a `PROGRAM` — it has a name, optional pragmas, a version, variable sections, and a body.

In the AST, organization blocks are represented as a new `LibraryElementKind::OrganizationBlockDeclaration` variant rather than overloading `ProgramDeclaration` with a flag. This follows the same pattern used for `InterfaceDeclaration` in the [Beckhoff design](beckhoff-twincat-dialect.md) — distinct declaration types get distinct AST variants. See [AST Extensions](#ast-extensions-dsl-crate) for the concrete struct definition.

#### 2.5 Classic Block Attributes (STEP 7 format)

Older SCL files (and some TIA Portal exports) use keyword-based block attributes:

```
FUNCTION_BLOCK FB10
TITLE = 'Mean_Value'
AUTHOR : AUT_1
FAMILY : Control
NAME : MeanVal
VERSION : '2.1'
KNOW_HOW_PROTECT
```

**Design:** Add keyword tokens for `Title`, `Author`, `Family`, `Name`, and `KnowHowProtect`. These appear between the block header and variable declarations.

The syntax uses two separator forms: `TITLE = 'value'` (with `=` and quoted string) and `AUTHOR : value` (with `:` and unquoted identifier). The parser consumes each attribute keyword and then reads tokens until the next `Newline`, treating the attribute value as opaque text. `KNOW_HOW_PROTECT` is a standalone keyword with no value.

`NAME` is safe to promote as a keyword because it only appears in this block-attribute context (between the POU header and `VAR` sections), and it is only promoted in SCL mode. In standard mode, `NAME` remains an `Identifier`.

For initial support, these attributes are recognized and skipped — the parser consumes them but does not add them to the AST. The TIA Portal format (`VERSION` only) is more common than the classic STEP 7 format.

#### 2.6 `GOTO` and Labels

SCL supports `GOTO` (from its Pascal heritage), which is not in IEC 61131-3 ST:

```
GOTO MyLabel;
// ...
MyLabel:
    // code
```

**Design:** Add `Goto` as a keyword token. The parser recognizes `GOTO label` as a statement and `label:` as a label statement. These are represented in the AST as new statement variants but not analyzed semantically.

#### 2.7 Siemens-Specific Data Types

Types like `S5Time`/`S5TIME`, `DTL`, `DB_ANY`, `HW_IO`, `HW_DEVICE`, `Variant`, `REF_TO`, etc.

**Design:** These are not added as keyword tokens (except `REF_TO` which has syntactic impact as a type constructor). The rest are resolved during semantic analysis. Since IEC 61131-3 identifiers are case-insensitive and user-defined types are valid, the parser already accepts these as identifiers.

`REF_TO` is semantically equivalent to Beckhoff's `REFERENCE TO` but syntactically different — it is a single keyword (`REF_TO`) rather than two keywords (`REFERENCE` + `TO`). Both should map to the same AST representation (`TypeSpec::ReferenceTo`) as defined in the [Beckhoff design](beckhoff-twincat-dialect.md#new-type-variants). In the `DIALECT_KEYWORDS` table, `REF_TO` has `origins: &[SiemensSCL]` while `REFERENCE` has `origins: &[BeckhoffCodesys]`.

#### 2.8 Slice-Based Bit Access

Direct bit/byte/word access on integer variables:

```
#MyWord.%X0 := TRUE;     // Access bit 0 of a WORD
#MyDWord.%B2 := 16#FF;   // Access byte 2 of a DWORD
```

**Design:** The lexer already produces tokens for `%` and the access patterns. In SCL mode, the parser recognizes `.%Xn`, `.%Bn`, `.%Wn` as member access expressions on integer-typed variables. This can be deferred to a later phase since it's less common in application-level SCL code.

## Parse Options Extension

The existing `ParseOptions` struct is extended:

```rust
pub struct ParseOptions {
    pub allow_c_style_comments: bool,
    pub dialect: Dialect,
}

pub enum Dialect {
    Standard,
    SiemensSCL,
    BeckhoffTwinCAT,
}
```

The `Dialect` enum is shared infrastructure defined once and used by both the [Siemens SCL](siemens-scl-dialect.md) and [Beckhoff TwinCAT](beckhoff-twincat-dialect.md) designs. It controls which token transforms are applied and which `ExtensionOrigin` values are active for keyword promotion (see [Extension Origin Model](#extension-origin-model)). The `SiemensSCL` dialect implies `allow_c_style_comments: true`.

## Parser Integration

### Dialect Gating: Token Transforms as the Gate

The dialect gate lives in the **token transform layer**, not in the parser. The parser grammar rules for SCL constructs (e.g., `VERSION : <literal>`, optional `BEGIN`, `DATA_BLOCK ... END_DATA_BLOCK`) are always present in the PEG grammar. They simply never fire in standard mode because the tokens that trigger them (`Version`, `Begin`, `DataBlock`, etc.) are only produced by the SCL keyword promotion transform.

This means the parser itself needs no dialect awareness or `ParseOptions` access for most features. A grammar rule like `tok(TokenType::Begin)` will never match when parsing standard IEC 61131-3 because `BEGIN` remains an `Identifier` token — it is never promoted to `Begin`.

This is the same gating mechanism described in the [Beckhoff design](beckhoff-twincat-dialect.md#dialect-gating-token-transforms-as-the-gate). All SCL grammar extensions use tokens that only exist after promotion.

### Token Transform Pipeline

SCL-specific syntax normalization is handled by the dialect token transform pipeline described in [dialect-token-transforms.md](dialect-token-transforms.md). The SCL dialect uses the shared `promote_keywords` function driven by the `DIALECT_KEYWORDS` table (see [Extension Origin Model](#extension-origin-model)) — no separate `promote_scl_keywords` function is needed. The pipeline applies these transforms in order:

1. **Keyword promotion** — promote `Identifier` tokens matching entries in `DIALECT_KEYWORDS` where `origins` intersects `SiemensSCL`. Promoted keywords: `REGION`, `END_REGION`, `VERSION`, `BEGIN`, `DATA_BLOCK`, `END_DATA_BLOCK`, `ORGANIZATION_BLOCK`, `END_ORGANIZATION_BLOCK`, `VAR_STAT`, `GOTO`, `CONTINUE`, `TITLE`, `AUTHOR`, `FAMILY`, `NAME`, `KNOW_HOW_PROTECT`, `REF_TO`
2. **Token rewriting** — all `DoubleByteString` → `Identifier` (strip quotes, SCL only)
3. **Pragma collapsing** — `{ ... }` → single `Pragma` token (shared with Beckhoff)
4. **Token filtering** — remove `Hash` before `Identifier`; remove `Region`...`Newline` and `EndRegion`

See the transform pipeline design for the full architecture, ordering constraints, and span preservation invariants.

### Hash Filtering and Keyword Promotion Interaction

The hash filter rule is "remove `Hash` when immediately followed by `Identifier`". Because hash filtering runs **after** keyword promotion, a `#` prefix before a promoted keyword will NOT be removed. For example, `#REGION` becomes `Hash` + `Region` after promotion, and the hash filter sees `Hash` + `Region` (not `Hash` + `Identifier`), so the `Hash` remains.

This is **correct behavior**: in Siemens TIA Portal, promoted keywords like `REGION`, `BEGIN`, `VERSION`, and `GOTO` are reserved words — they cannot be used as variable names. Writing `#REGION` as a variable reference is already invalid in TIA Portal and will correctly produce a parse error in IronPLC.

### REGION Filtering Edge Cases

The REGION filter removes `Region` token + all tokens until the next `Newline` (the region name text), and standalone `EndRegion` tokens. Edge cases:

- **No `Newline` after `REGION`** (end of file): the filter removes `Region` and all remaining tokens. This is a degenerate case — a `REGION` at the end of a file with no body is meaningless.
- **Empty region name** (`REGION` followed immediately by `Newline`): the filter removes just the `Region` token and the `Newline`.
- **Region name with spaces** (`REGION Initialization Phase`): all tokens between `Region` and `Newline` are removed, regardless of count.

## Extension Origin Model

Every vendor-specific construct is tagged with its origin using the shared `ExtensionOrigin` enum defined in the [Beckhoff design](beckhoff-twincat-dialect.md#extension-origin-model). This enum is the **single source of truth** that drives both the token transform pipeline and the semantic diagnostic (`P9004 UnsupportedExtension`).

SCL-specific keywords are added to the shared `DIALECT_KEYWORDS` table. Keywords shared between SCL and Beckhoff/CODESYS have multiple origins:

### SCL Entries in `DIALECT_KEYWORDS`

| Text | `TokenType` | Origins | Priority |
|------|------------|---------|----------|
| `REGION` | `Region` | `SiemensSCL` | 1 |
| `END_REGION` | `EndRegion` | `SiemensSCL` | 1 |
| `VERSION` | `Version` | `SiemensSCL` | 1 |
| `BEGIN` | `Begin` | `SiemensSCL` | 1 |
| `CONTINUE` | `Continue` | `Iec61131Ed3, BeckhoffCodesys, SiemensSCL` | 1 |
| `DATA_BLOCK` | `DataBlock` | `SiemensSCL` | 3 |
| `END_DATA_BLOCK` | `EndDataBlock` | `SiemensSCL` | 3 |
| `ORGANIZATION_BLOCK` | `OrganizationBlock` | `SiemensSCL` | 3 |
| `END_ORGANIZATION_BLOCK` | `EndOrganizationBlock` | `SiemensSCL` | 3 |
| `GOTO` | `Goto` | `SiemensSCL` | 4 |
| `TITLE` | `Title` | `SiemensSCL` | 2 |
| `AUTHOR` | `Author` | `SiemensSCL` | 2 |
| `FAMILY` | `Family` | `SiemensSCL` | 2 |
| `NAME` | `Name` | `SiemensSCL` | 2 |
| `KNOW_HOW_PROTECT` | `KnowHowProtect` | `SiemensSCL` | 2 |
| `REF_TO` | `RefTo` | `SiemensSCL` | 4 |

Keywords shared with Beckhoff (already in the table from the [Beckhoff design](beckhoff-twincat-dialect.md#new-tokentype-variants)):

| Text | `TokenType` | Origins |
|------|------------|---------|
| `VAR_STAT` | `VarStat` | `BeckhoffCodesys, SiemensSCL` |
| `CONTINUE` | `Continue` | `Iec61131Ed3, BeckhoffCodesys, SiemensSCL` |

### `VendorExtension` Implementations for SCL Nodes

Every new AST node representing an SCL-specific construct implements the `VendorExtension` trait defined in the [Beckhoff design](beckhoff-twincat-dialect.md#the-vendorextension-trait). This enables the `rule_unsupported_extension.rs` semantic rule to emit `P9004` diagnostics:

```rust
// Siemens SCL extension — DATA_BLOCK declaration
// Extension: Siemens SCL
impl VendorExtension for DataBlockDeclaration {
    fn extension_name(&self) -> &'static str { "DATA_BLOCK declaration" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] { &[ExtensionOrigin::SiemensSCL] }
    fn extension_span(&self) -> SourceSpan { self.span }
}

// Siemens SCL extension — ORGANIZATION_BLOCK declaration
// Extension: Siemens SCL
impl VendorExtension for OrganizationBlockDeclaration {
    fn extension_name(&self) -> &'static str { "ORGANIZATION_BLOCK declaration" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] { &[ExtensionOrigin::SiemensSCL] }
    fn extension_span(&self) -> SourceSpan { self.span }
}

// Siemens SCL extension — GOTO statement
// Extension: Siemens SCL
impl VendorExtension for GotoStatement {
    fn extension_name(&self) -> &'static str { "GOTO statement" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] { &[ExtensionOrigin::SiemensSCL] }
    fn extension_span(&self) -> SourceSpan { self.span }
}
```

`VAR_STAT` and `CONTINUE` are shared with Beckhoff — their `VendorExtension` implementations are defined in the [Beckhoff design](beckhoff-twincat-dialect.md#the-vendorextension-trait) with multiple origins.

## AST Extensions (DSL Crate)

These are the concrete changes needed in the `compiler/dsl/` crate to represent SCL constructs. The DSL must represent parsed constructs, not just parse them — downstream analysis and future code generation depend on having a complete AST.

### Summary: Token-Level vs AST-Level

| SCL construct | Handling |
|---|---|
| `#var` prefix | Stripped at token level — no AST change |
| `{ pragma }` | Stripped at token level — no AST change (future: `Pragma` AST node) |
| `REGION`/`END_REGION` | Stripped at token level — no AST change |
| `"quoted name"` | Rewritten at token level — no AST change |
| `BEGIN` | Consumed at parser level — no AST change |
| Classic block attributes | Consumed at parser level — no AST change (initially) |
| `VERSION : 0.1` | New optional `version` field on POU declarations |
| `CONTINUE` | New `StmtKind::Continue` (shared with Beckhoff) |
| `VAR_STAT` | New `VariableType::Static` (shared with Beckhoff) |
| `DATA_BLOCK` | New `LibraryElementKind::DataBlockDeclaration` |
| `ORGANIZATION_BLOCK` | New `LibraryElementKind::OrganizationBlockDeclaration` |
| `GOTO` / labels | New `StmtKind::Goto` and `StmtKind::Label` |
| `REF_TO` | Shared `TypeSpec::ReferenceTo` (same AST as Beckhoff's `REFERENCE TO`) |

### New Declaration Types

These types are added to `compiler/dsl/src/common.rs`.

**Top-level: `LibraryElementKind` extension**

```rust
pub enum LibraryElementKind {
    // ... existing variants ...
    DataBlockDeclaration(DataBlockDeclaration),          // NEW (Siemens)
    OrganizationBlockDeclaration(OrganizationBlockDeclaration),  // NEW (Siemens)
}
```

**New structs**

```rust
/// A Siemens data block declaration: DATA_BLOCK "name" ... END_DATA_BLOCK
pub struct DataBlockDeclaration {
    pub name: Id,
    pub version: Option<String>,
    pub retain: DeclarationQualifier,       // NON_RETAIN or Unspecified
    /// For global data blocks: the struct fields
    pub fields: Vec<StructureElementDeclaration>,
    /// For instance data blocks: the referenced FB type
    pub instance_of: Option<TypeName>,
    /// Initialization assignments after BEGIN
    pub initializations: Vec<StmtKind>,
    pub span: SourceSpan,
}

/// A Siemens organization block: ORGANIZATION_BLOCK "name" ... END_ORGANIZATION_BLOCK
///
/// Structurally identical to a PROGRAM but distinct in the AST so that
/// semantic analysis can treat it differently (e.g., scheduling metadata,
/// different variable section rules).
pub struct OrganizationBlockDeclaration {
    pub name: Id,
    pub version: Option<String>,
    pub variables: Vec<VarDecl>,
    pub body: FunctionBlockBodyKind,
    pub span: SourceSpan,
}
```

**VERSION field on existing POU declarations**

The version applies to `FunctionBlockDeclaration`, `FunctionDeclaration`, and `ProgramDeclaration`. Add an optional `version` field to each:

```rust
pub struct FunctionBlockDeclaration {
    // ... existing fields ...
    pub version: Option<String>,   // NEW: VERSION : 0.1
}

pub struct FunctionDeclaration {
    // ... existing fields ...
    pub version: Option<String>,   // NEW: VERSION : 0.1
}

pub struct ProgramDeclaration {
    // ... existing fields ...
    pub version: Option<String>,   // NEW: VERSION : 0.1
}
```

### New Variable Section Type

Shared with the [Beckhoff design](beckhoff-twincat-dialect.md#new-variable-section-type):

```rust
pub enum VariableType {
    // ... existing variants ...
    Static,     // VAR_STAT (NEW) — static vars in functions
}
```

### New Statement Variants

```rust
pub enum StmtKind {
    // ... existing variants ...
    Goto(GotoStatement),       // GOTO label;
    Label(LabelStatement),     // label:
    Continue(SourceSpan),      // CONTINUE; (shared with Beckhoff)
}

/// A GOTO statement: GOTO MyLabel;
pub struct GotoStatement {
    pub target: Id,
    pub span: SourceSpan,
}

/// A label statement: MyLabel:
pub struct LabelStatement {
    pub name: Id,
    pub span: SourceSpan,
}
```

### New Type Variant

Shared AST representation with Beckhoff's `REFERENCE TO`:

```rust
// REF_TO in Siemens SCL maps to the same AST as REFERENCE TO in Beckhoff:
TypeSpec:
    + ReferenceTo(Box<TypeSpec>)   // REF_TO type (Siemens) / REFERENCE TO type (Beckhoff)
```

### New TokenType Variants

These are added to the `TokenType` enum **without** `#[token(...)]` attributes — they have no logos lexer rules. They are populated exclusively by the dialect keyword promotion transform.

| Token | Promoted from `Identifier` text | Priority |
|-------|-------------------------------|----------|
| `Region` | `REGION` | 1 |
| `EndRegion` | `END_REGION` | 1 |
| `Version` | `VERSION` | 1 |
| `Begin` | `BEGIN` | 1 |
| `Continue` | `CONTINUE` | 1 |
| `VarStat` | `VAR_STAT` | 2 |
| `Title` | `TITLE` | 2 |
| `Author` | `AUTHOR` | 2 |
| `Family` | `FAMILY` | 2 |
| `Name` | `NAME` | 2 |
| `KnowHowProtect` | `KNOW_HOW_PROTECT` | 2 |
| `DataBlock` | `DATA_BLOCK` | 3 |
| `EndDataBlock` | `END_DATA_BLOCK` | 3 |
| `OrganizationBlock` | `ORGANIZATION_BLOCK` | 3 |
| `EndOrganizationBlock` | `END_ORGANIZATION_BLOCK` | 3 |
| `Goto` | `GOTO` | 4 |
| `RefTo` | `REF_TO` | 4 |

Note: `VarStat` and `Continue` are shared with the [Beckhoff design](beckhoff-twincat-dialect.md#new-tokentype-variants) — they appear in both keyword lists with multiple origins in the `DIALECT_KEYWORDS` table.

## File Type Integration

In `compiler/sources/src/file_type.rs`:

```rust
pub enum FileType {
    StructuredText,  // .st, .iec
    Xml,             // .xml
    TwinCat,         // .TcPOU, .TcGVL, .TcDUT
    SiemensSCL,      // .scl, .udt (NEW)
    Unknown,
}
```

Extension mappings added to `from_path()`:
- `"scl"` → `FileType::SiemensSCL`
- `"udt"` → `FileType::SiemensSCL`

In `compiler/sources/src/parsers/mod.rs`, add a new `scl_parser` module that:
1. Creates `ParseOptions` with `dialect: Dialect::SiemensSCL` (which implies `allow_c_style_comments: true`)
2. Delegates to `ironplc_parser::parse_program` with those options

In `parse_source()`, add the routing:
```rust
FileType::SiemensSCL => scl_parser::parse(content, file_id),
```

## Testing Strategy

### Keyword safety regression test (shared with Beckhoff — MUST exist before any keyword promotion)

The shared keyword safety test defined in the [Beckhoff design](beckhoff-twincat-dialect.md#keyword-safety-regression-test-must-be-added-before-any-keyword-promotion) must include ALL SCL-specific keywords. The test defines a function block where every planned keyword is used as a variable name, and verifies it parses successfully in standard mode.

SCL-specific keywords that must be added to the shared test (in addition to the Beckhoff keywords already listed):

```
REGION, VERSION, BEGIN, DATA_BLOCK, ORGANIZATION_BLOCK, GOTO,
TITLE, AUTHOR, FAMILY, NAME, KNOW_HOW_PROTECT, REF_TO
```

Note: `END_REGION`, `END_DATA_BLOCK`, `END_ORGANIZATION_BLOCK` contain underscores followed by standard keywords — they cannot appear as simple identifiers in a VAR block. However, they are safe because the logos lexer only recognizes `END_` compound keywords that are defined with `#[token(...)]` attributes. Since these new `END_*` variants have no logos attributes, the lexer produces them as `Identifier` tokens (split at tokenization boundaries).

### Validation fixtures

1. **Minimal SCL files** — one test file per extension (e.g., `region.scl`, `hash_prefix.scl`, `pragmas.scl`, `quoted_names.scl`, `var_stat.scl`, `data_block.scl`, `organization_block.scl`, `continue.scl`)
2. **Combined SCL file** — a single file using all extensions together, representative of real-world SCL
3. **open-process-library files** — use representative `.scl` files from the SASE-Space/open-process-library as integration tests (licensing permitting, or create equivalent fixtures)

### ST-level tests (parser)

- `#` prefix stripping with various identifier types
- `{ }` pragma collapsing (shared with Beckhoff tests)
- `REGION`/`END_REGION` filtering with region names
- Double-quoted block names in declarations and call expressions
- `VERSION : 0.1` after POU header
- Optional `BEGIN` before body
- `VAR_STAT` sections
- `CONTINUE` in loops
- `DATA_BLOCK` with `STRUCT` body and `BEGIN` initializations
- Instance `DATA_BLOCK` referencing an FB type
- `DATA_BLOCK` with `NON_RETAIN` modifier
- `ORGANIZATION_BLOCK` with variable sections and body
- Classic block attributes (`TITLE`, `AUTHOR`, `FAMILY`, `NAME`, `VERSION`, `KNOW_HOW_PROTECT`)
- `GOTO` and labels
- `REF_TO` type constructor

### Regression tests

- All existing standard ST tests must continue to pass unchanged
- Standard ST files parsed with the SCL dialect must produce identical results to parsing with the standard dialect

### Position fidelity tests

- Diagnostics on SCL files must point to positions in the original `.scl` file
- Token transforms must preserve correct `SourceSpan` values

### Integration tests

- Parse representative open-source SCL projects (e.g., open-process-library) without errors

## Phased Implementation

Phase 0 is shared infrastructure with the [Beckhoff design](beckhoff-twincat-dialect.md#phased-implementation). If Beckhoff work ships first, Phase 0 will already be complete. If SCL ships first, this phase must be implemented here.

0. **Phase 0 — Prerequisites** (before any dialect code, shared with Beckhoff):
   - Keyword safety regression test: function block with all planned keywords (both Beckhoff AND Siemens) as variable names, parsed in standard mode
   - `ExtensionOrigin` enum in the DSL crate
   - `VendorExtension` trait in the DSL crate
   - `P9004 UnsupportedExtension` problem code in CSV and documentation
   - `rule_unsupported_extension.rs` semantic rule (empty initially — no extension nodes exist yet)
   - `Dialect` enum and `ParseOptions` extension (shared infrastructure)
   - Token transform pipeline: `promote_keywords` + `collapse_pragmas` (shared with Beckhoff)

1. **Phase 1 — Core syntax** (enables parsing most open-process-library files):
   - `FileType::SiemensSCL` with `.scl`, `.udt` extensions
   - `#` prefix stripping (hash filtering transform)
   - `{ }` pragma collapsing (shared with Beckhoff)
   - `REGION`/`END_REGION` skipping (REGION filtering transform)
   - `"quoted name"` normalization (double-quote rewriting transform)
   - `BEGIN` keyword (optional body separator)
   - `CONTINUE` statement (shared with Beckhoff)
   - DSL: `VariableType::Static` (shared), `StmtKind::Continue` (shared)
   - `VendorExtension` impls on new AST nodes; `rule_unsupported_extension` visitor overrides

2. **Phase 2 — Declarations**:
   - `VERSION : 0.1` parsing and `version` field on POU declarations
   - `VAR_STAT` support (shared with Beckhoff)
   - Classic block attributes (`TITLE`, `AUTHOR`, `FAMILY`, `NAME`, `KNOW_HOW_PROTECT`)
   - DSL: `version` field on `FunctionBlockDeclaration`, `FunctionDeclaration`, `ProgramDeclaration`
   - `VendorExtension` impls on new nodes

3. **Phase 3 — Siemens-specific POU types**:
   - `DATA_BLOCK` / `END_DATA_BLOCK` with `STRUCT` body, `NON_RETAIN`, and `BEGIN` initialization section
   - Instance data blocks with FB type reference
   - `ORGANIZATION_BLOCK` / `END_ORGANIZATION_BLOCK`
   - DSL: `DataBlockDeclaration`, `OrganizationBlockDeclaration`
   - `VendorExtension` impls on new nodes

4. **Phase 4 — Advanced syntax**:
   - `GOTO` and labels
   - Slice-based bit access (`.%X0`, `.%B2`, `.%W3`)
   - `REF_TO` type constructor (maps to shared `TypeSpec::ReferenceTo` from Beckhoff design)
   - Assign-attempt operator `?=` (also a Beckhoff/CODESYS construct — shared AST representation)
   - DSL: `GotoStatement`, `LabelStatement`, `RefTo` → `ReferenceTo`
   - `VendorExtension` impls on new nodes
