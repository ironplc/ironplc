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
| `.db` | Data block source files |
| `.udt` | User-defined type (PLC data type) definitions |

All three extensions map to `FileType::SiemensSCL`. These are plain text files with no XML wrapper.

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

**Design:** The lexer already produces a `Hash` token for `#`. The parser, when in SCL mode, treats `Hash` followed by `Identifier` as a variable reference. The `#` prefix is syntactic sugar — it does not change the variable's identity. The AST stores the variable name without the `#` prefix.

At the token transform level (in `xform_tokens.rs` or a new SCL-specific transform), a `Hash + Identifier` pair is merged into a single `Identifier` token with the text set to the identifier name (stripping the `#`). This keeps the parser grammar unchanged — it sees an `Identifier` token in expression positions regardless of dialect.

#### 1.2 Curly-Brace Pragmas

SCL uses `{ }` for compile-time directives:

```
{S7_Optimized_Access := 'TRUE'}
{InstructionName := 'FB_PID'}
{DB_Specific := 'FALSE'}
```

**Design:** Add a token transform that, when in SCL mode, collapses a sequence of `LeftBrace ... RightBrace` tokens into a single `Pragma` token (new token type). The parser treats `Pragma` tokens as whitespace/comments — they are preserved in the token stream for position fidelity but skipped during parsing.

The pragma content is opaque — the parser does not interpret key-value pairs inside pragmas. Future work could extract structured metadata from pragmas for use in analysis.

#### 1.3 `REGION` / `END_REGION`

Code folding markers with no semantic meaning:

```
REGION Initialization
    #counter := 0;
END_REGION
```

**Design:** Add `Region` and `EndRegion` as new token types in the lexer. The region name (the text after `REGION` up to the newline) is captured as part of the `Region` token text but not parsed further.

In the parser, `Region` and `EndRegion` tokens are treated as whitespace — they are allowed at any statement boundary and discarded. The tokens between `REGION` and `END_REGION` are parsed normally.

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

**Design:** The lexer already produces `DoubleByteString` tokens for `"..."`. In SCL mode, when a `DoubleByteString` appears in a name position (after `FUNCTION_BLOCK`, `FUNCTION`, `PROGRAM`, `DATA_BLOCK`, `ORGANIZATION_BLOCK`, or in a call expression), the parser accepts it as an identifier. The quotes are stripped and the content becomes the identifier name.

This is implemented as a token transform: in SCL mode, `DoubleByteString` tokens are rewritten to `Identifier` tokens with the quotes removed from the text. This avoids grammar changes in the parser.

#### 1.5 `VERSION` Declaration

Block version metadata appearing after pragmas and before variable declarations:

```
FUNCTION_BLOCK "FB_PID"
{ S7_Optimized_Access := 'TRUE' }
VERSION : 0.1
```

**Design:** Add `Version` as a new keyword token. The parser, in SCL mode, recognizes an optional `VERSION : <literal>` clause after the POU header and before variable sections. The version value (a fixed-point literal like `0.1`) is stored as metadata in the AST but not used in semantic analysis.

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

**Design:** Add `Begin` as a keyword token. In SCL mode, the parser accepts an optional `BEGIN` token between variable declarations and the body. It is consumed and discarded — no AST representation needed.

#### 1.7 `VAR CONSTANT` Compound Keyword

SCL uses `VAR CONSTANT` (two keywords) for constant declarations:

```
VAR CONSTANT
    MAX_SPEED : REAL := 3000.0;
    RAMP_TIME : TIME := T#5s;
END_VAR
```

**Design:** The parser already recognizes `CONSTANT` as a qualifier on variable sections. In SCL mode, `VAR CONSTANT` is accepted as equivalent to `VAR CONSTANT` in standard IEC 61131-3 (which uses `VAR CONSTANT` as well — this is actually standard behavior). No change needed if the parser already handles this.

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

**Design:** Add `DataBlock` and `EndDataBlock` as new keyword tokens, along with `Begin`. The parser recognizes `DATA_BLOCK "name" ... END_DATA_BLOCK` as a new top-level declaration type. In the AST, a data block is represented as a new `LibraryElementKind` variant.

For the initial implementation, the data block body is parsed structurally (recognizing `STRUCT`/`END_STRUCT` and `BEGIN`/`END_DATA_BLOCK` boundaries) but the initialization section after `BEGIN` is stored as opaque content. Full semantic analysis of data blocks requires understanding Siemens-specific instance memory layout, which is out of scope.

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

**Design:** Add `OrganizationBlock` and `EndOrganizationBlock` as keyword tokens. The parser treats an organization block like a `PROGRAM` — it has a name, optional pragmas, a version, variable sections, and a body. In the AST, it maps to the existing program representation with a flag or variant indicating it's an organization block.

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

**Design:** Add keyword tokens for `Title`, `Author`, `Family`, `Name` (note: `Name` may conflict — use context), and `KnowHowProtect`. These appear between the block header and variable declarations. They are stored as metadata in the AST. For initial support, these can be recognized and skipped, since the TIA Portal format (`VERSION` only) is more common.

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

**Design:** These are not added as keyword tokens (except `REF_TO` which has syntactic impact as a type constructor like `POINTER TO`). The rest are resolved during semantic analysis. Since IEC 61131-3 identifiers are case-insensitive and user-defined types are valid, the parser already accepts these as identifiers.

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
    // future: BeckhoffTwinCAT (for ST inside CDATA), Codesys, etc.
}
```

The `Dialect` enum controls which token transforms and parser grammar extensions are active. The `SiemensSCL` dialect implies `allow_c_style_comments: true`.

## Token Transform Pipeline

SCL-specific syntax normalization happens at the token transform level, between lexing and parsing. This keeps the parser grammar as close to standard as possible.

### Keyword promotion: Identifier → keyword TokenType

The logos lexer in `token.rs` only knows IEC 61131-3 keywords. Vendor-specific keywords like `REGION`, `END_REGION`, `VERSION`, `BEGIN`, `DATA_BLOCK`, `ORGANIZATION_BLOCK`, `VAR_STAT`, `GOTO`, etc. are **not** added to the logos grammar. If they were, they would always be keywords — even in standard mode where `REGION` or `BEGIN` are valid identifiers.

Instead, when the dialect is `SiemensSCL`, a token transform promotes `Identifier` tokens whose text matches SCL keywords into the appropriate `TokenType` variant:

```rust
fn promote_scl_keywords(tokens: Vec<Token>) -> Vec<Token> {
    tokens.into_iter().map(|mut tok| {
        if tok.token_type == TokenType::Identifier {
            tok.token_type = match tok.text.to_uppercase().as_str() {
                "REGION" => TokenType::Region,
                "END_REGION" => TokenType::EndRegion,
                "VERSION" => TokenType::Version,
                "BEGIN" => TokenType::Begin,
                "DATA_BLOCK" => TokenType::DataBlock,
                "END_DATA_BLOCK" => TokenType::EndDataBlock,
                "ORGANIZATION_BLOCK" => TokenType::OrganizationBlock,
                "END_ORGANIZATION_BLOCK" => TokenType::EndOrganizationBlock,
                "VAR_STAT" => TokenType::VarStat,
                "GOTO" => TokenType::Goto,
                _ => tok.token_type,
            };
        }
        tok
    }).collect()
}
```

The `TokenType` enum gains new variants for these keywords **without** `#[token(...)]` attributes — they have no logos lexer rules and are populated exclusively by this promotion transform. This ensures standard mode is unaffected: `BEGIN` remains a valid `Identifier` when parsing `.st` files.

Note that `VAR_STAT`, `DATA_BLOCK`, `END_DATA_BLOCK`, `ORGANIZATION_BLOCK`, and `END_ORGANIZATION_BLOCK` contain underscores, so the logos `[A-Za-z_][A-Za-z0-9_]*` regex matches them as single `Identifier` tokens — they do not need special multi-token handling.

### Full transform pipeline

```
Source text
  → Logos lexer (standard IEC 61131-3 keywords only — same for all dialects)
  → Preprocessor (OSCAT comments — same for all dialects)
  → SCL keyword promotion (when dialect == SiemensSCL):
      Promote Identifier tokens to SCL keyword TokenTypes
  → SCL syntax transforms (when dialect == SiemensSCL):
      1. Collapse { ... } into Pragma tokens (skip during parsing)
      2. Merge Hash + Identifier into Identifier (strip #)
      3. Rewrite DoubleByteString to Identifier in name positions (strip quotes)
      4. Mark Region/EndRegion for skip during parsing
  → Standard token transforms (insert keyword terminators, etc.)
  → Parser
```

This approach has a precedent in the codebase: `xform_tokens.rs` already transforms the token stream before parsing. The SCL transforms follow the same pattern. The Beckhoff TwinCAT dialect uses an identical keyword promotion mechanism (see [beckhoff-twincat-dialect.md](beckhoff-twincat-dialect.md#keyword-promotion-via-token-transform-not-lexer)).

## AST Representation

New AST constructs needed:

| SCL construct | AST representation |
|---|---|
| `#var` prefix | Stripped at token level — no AST change |
| `{ pragma }` | Stripped at token level — no AST change (future: `Pragma` AST node) |
| `REGION`/`END_REGION` | Stripped at token level — no AST change |
| `"quoted name"` | Stripped at token level — no AST change |
| `VERSION : 0.1` | New optional `version` field on POU declarations |
| `VAR_STAT` | New `VarQualifier::Static` or `VariableListKind::Static` |
| `DATA_BLOCK` | New `LibraryElementKind::DataBlock` |
| `ORGANIZATION_BLOCK` | Maps to existing `Program` with metadata flag |

The minimal AST additions are `VERSION`, `VAR_STAT`, and `DATA_BLOCK`. Everything else is handled at the token transform level.

## File Type Integration

In `compiler/sources/src/file_type.rs`:

```rust
pub enum FileType {
    StructuredText,
    Xml,
    TwinCat,
    SiemensSCL,  // new
    Unknown,
}
```

In `compiler/sources/src/parsers/mod.rs`, add a new `scl_parser` module that:
1. Creates `ParseOptions` with `dialect: Dialect::SiemensSCL`
2. Delegates to `ironplc_parser::parse_program` with those options

## Testing Strategy

### Validation fixtures

1. **Minimal SCL files** — one test file per extension (e.g., `region.scl`, `hash_prefix.scl`, `pragmas.scl`, `quoted_names.scl`, `var_stat.scl`, `data_block.scl`, `organization_block.scl`)
2. **Combined SCL file** — a single file using all extensions together, representative of real-world SCL
3. **open-process-library files** — use representative `.scl` files from the SASE-Space/open-process-library as integration tests (licensing permitting, or create equivalent fixtures)

### Regression tests

- All existing standard ST tests must continue to pass unchanged
- Standard ST files parsed with the SCL dialect must produce identical results to parsing with the standard dialect

### Position fidelity tests

- Diagnostics on SCL files must point to positions in the original `.scl` file
- Token transforms must preserve correct `SourceSpan` values

## Phased Implementation

1. **Phase 1 — Core syntax** (enables parsing most open-process-library files):
   - `#` prefix stripping
   - `{ }` pragma collapsing
   - `REGION`/`END_REGION` skipping
   - `"quoted name"` normalization
   - `BEGIN` keyword (optional body separator)
   - `FileType::SiemensSCL` with `.scl`, `.db`, `.udt` extensions
   - `Dialect` enum and `ParseOptions` extension

2. **Phase 2 — Declarations**:
   - `VERSION : 0.1` parsing
   - `VAR_STAT` support
   - Classic block attributes (`TITLE`, `AUTHOR`, `FAMILY`, `NAME`, `KNOW_HOW_PROTECT`)

3. **Phase 3 — Siemens-specific POU types**:
   - `DATA_BLOCK` / `END_DATA_BLOCK` with `BEGIN` initialization section
   - `ORGANIZATION_BLOCK` / `END_ORGANIZATION_BLOCK`

4. **Phase 4 — Advanced syntax**:
   - `GOTO` and labels
   - Slice-based bit access (`.%X0`, `.%B2`, `.%W3`)
   - `REF_TO` type constructor
   - Assign-attempt operator `?=`
