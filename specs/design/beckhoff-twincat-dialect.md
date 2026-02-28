# Design: Beckhoff TwinCAT Dialect Support

## Overview

This document describes the design for parsing Beckhoff TwinCAT 3 extensions to IEC 61131-3 Structured Text. TwinCAT already has partial support in IronPLC — the XML file wrapper (`.TcPOU`, `.TcGVL`, `.TcDUT`) is parsed and ST is extracted from CDATA sections. This design extends support to the Structured Text dialect inside those files: object-oriented extensions, pragma attributes, pointer/reference types, and additional variable sections.

This design implements [ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md) for the Beckhoff TwinCAT dialect.

## Scope

**In scope (parsing):** Accept all syntactically valid TwinCAT ST constructs within `.TcPOU`, `.TcGVL`, and `.TcDUT` files and represent them in the AST without parse errors.

**Out of scope (future):** Semantic analysis of TwinCAT-specific constructs (e.g., resolving `EXTENDS` hierarchies, type checking `POINTER TO` dereferences, method dispatch). Standard IEC 61131-3 semantic analysis continues to run on the standard-compliant portions.

## Current State

IronPLC already handles TwinCAT files at the file format level:

- `FileType::TwinCat` detects `.TcPOU`, `.TcGVL`, `.TcDUT` extensions
- `twincat_parser.rs` extracts ST from XML CDATA sections
- Position adjustment maps parse errors back to the original XML file
- POU, GVL, and DUT XML structures are recognized

The gap: the ST content inside CDATA is parsed with the standard IEC 61131-3 grammar, which rejects TwinCAT-specific syntax. The `st_parser::parse` call in `twincat_parser.rs` needs to use TwinCAT dialect options.

## TwinCAT Extensions to Parse

### Priority 1: Object-Oriented Programming

These are the most impactful TwinCAT extensions — they fundamentally extend the grammar with new declaration types and new keywords.

#### 1.1 `METHOD` / `END_METHOD`

Methods are defined within function blocks:

```
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR

METHOD Start : BOOL
VAR_INPUT
    speed : INT;
END_VAR
    bRunning := TRUE;
    Start := TRUE;
END_METHOD

METHOD Stop : BOOL
    bRunning := FALSE;
    Stop := TRUE;
END_METHOD
END_FUNCTION_BLOCK
```

**Design:** Add `Method` and `EndMethod` as keyword tokens. The parser recognizes `METHOD name : return_type ... END_METHOD` as a sub-declaration within a `FUNCTION_BLOCK`. Methods have their own variable sections and body, structurally identical to a `FUNCTION`.

In the TwinCAT XML format, methods are separate XML elements within the POU:

```xml
<POU Name="FB_Motor">
  <Declaration><![CDATA[...]]></Declaration>
  <Implementation><ST><![CDATA[...]]></ST></Implementation>
  <Method Name="Start" Id="{guid}">
    <Declaration><![CDATA[METHOD Start : BOOL]]></Declaration>
    <Implementation><ST><![CDATA[...]]></ST></Implementation>
  </Method>
</POU>
```

The `twincat_parser.rs` module needs to iterate over `Method` child elements and parse each one, then attach the parsed methods to the parent FB in the AST.

**AST representation:** New `MethodDeclaration` variant within function block declarations, containing the method name, return type, variable sections, and body.

#### 1.2 `PROPERTY` / `END_PROPERTY` with `Get`/`Set`

Properties have separate getter and setter methods:

```
PROPERTY Value : INT

// Getter
GET
VAR
END_VAR
    Value := nInternalValue;
END_GET

// Setter
SET
VAR
END_VAR
    nInternalValue := Value;
END_SET
END_PROPERTY
```

In TwinCAT XML:

```xml
<Property Name="Value" Id="{guid}">
  <Declaration><![CDATA[PROPERTY Value : INT]]></Declaration>
  <Get Name="Get" Id="{guid}">
    <Declaration><![CDATA[]]></Declaration>
    <Implementation><ST><![CDATA[Value := nValue;]]></ST></Implementation>
  </Get>
  <Set Name="Set" Id="{guid}">
    <Declaration><![CDATA[]]></Declaration>
    <Implementation><ST><![CDATA[nValue := Value;]]></ST></Implementation>
  </Set>
</Property>
```

**Design:** Add `Property`, `EndProperty`, `Get`, `EndGet`, `Set`, `EndSet` as keyword tokens. The `twincat_parser.rs` module handles `Property`, `Get`, and `Set` XML elements. In the ST parser, property declarations and get/set bodies are recognized within function blocks.

**AST representation:** New `PropertyDeclaration` variant containing the property name, type, and optional get/set bodies.

#### 1.3 `INTERFACE` / `END_INTERFACE`

Abstract interface declarations:

```
INTERFACE I_Drivable
    METHOD Start : BOOL
    END_METHOD
    METHOD Stop : BOOL
    END_METHOD
END_INTERFACE
```

In TwinCAT XML, interfaces are a separate object type (`<Itf>` element instead of `<POU>`).

**Design:** Add `Interface` and `EndInterface` as keyword tokens. The parser recognizes `INTERFACE name ... END_INTERFACE` as a top-level declaration containing method signatures (method declarations without bodies). In the AST, interfaces are a new `LibraryElementKind::Interface`.

The `twincat_parser.rs` module needs to handle `<Itf>` elements in addition to `<POU>`, `<GVL>`, and `<DUT>`.

#### 1.4 `EXTENDS` and `IMPLEMENTS`

Inheritance and interface implementation on function blocks:

```
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable, I_Loggable
```

**Design:** Add `Extends` and `Implements` as keyword tokens. The parser recognizes an optional `EXTENDS base_name` clause and an optional `IMPLEMENTS interface_list` clause after the function block name. These are stored as metadata on the function block AST node.

#### 1.5 Access Modifiers

Access modifiers in TwinCAT apply to **methods and properties only**, not to individual variables or VAR sections. This is confirmed by the [Beckhoff documentation](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/3537661579.html) and [community references](https://stefanhenneken.net/2017/04/23/iec-61131-3-methods-properties-and-inheritance/).

```
FUNCTION_BLOCK FB_Example

PUBLIC METHOD DoWork : BOOL
    ...
END_METHOD

PRIVATE METHOD InternalHelper : BOOL
    ...
END_METHOD

PROTECTED METHOD ForSubclasses : BOOL
    ...
END_METHOD

ABSTRACT METHOD MustOverride : BOOL
END_METHOD

FINAL METHOD CannotOverride : BOOL
    ...
END_METHOD
END_FUNCTION_BLOCK
```

The default access modifier is `PUBLIC` when none is specified.

**Design:** Add `Public`, `Private`, `Protected`, `Internal`, `Abstract`, `Final` as keyword tokens. These appear as optional modifiers before method and property declarations. The parser accepts them in modifier positions and stores them as metadata on the `MethodDeclaration` and `PropertyDeclaration` AST nodes. No semantic enforcement initially.

#### 1.6 `THIS^` and `SUPER^`

Special expressions for self-reference and parent-reference:

```
THIS^.myMethod();
SUPER^.Start();
```

**Design:** Add `This` and `Super` as keyword tokens. Both are promoted from identifiers by the token transform, ensuring consistent treatment. The parser recognizes `THIS^` and `SUPER^` as primary expressions (like variable references) that can be followed by member access (`.`). The `^` is the existing dereference operator in IEC 61131-3. In the AST, these map to new expression variants.

### Priority 2: Type System Extensions

#### 2.1 `POINTER TO` / `REFERENCE TO`

Typed pointer and reference types:

```
VAR
    pMotor : POINTER TO FB_Motor;
    refValue : REFERENCE TO INT;
END_VAR

pMotor^.Start();
refValue REF= someInt;
```

**Design:** Add `Pointer` and `Reference` as keyword tokens (note: `To` already exists). The parser recognizes `POINTER TO type` and `REFERENCE TO type` as type specifiers in variable declarations. The `REF=` operator is handled in assignment context at the parser level (see section 3.6).

**AST representation:** New type wrapper variants: `TypeSpec::PointerTo(Box<TypeSpec>)` and `TypeSpec::ReferenceTo(Box<TypeSpec>)`.

#### 2.2 `UNION`

Union type declarations:

```
TYPE U_Data :
UNION
    intVal : INT;
    realVal : REAL;
    boolArray : ARRAY[0..31] OF BOOL;
END_UNION;
END_TYPE
```

**Design:** Add `Union` and `EndUnion` as keyword tokens. The parser recognizes `UNION ... END_UNION` as a type body within a `TYPE` declaration, parallel to `STRUCT ... END_STRUCT`. In the AST, unions are a new type declaration variant.

#### 2.3 Additional Time Types

`LTIME`, `LDATE`, `LTOD`/`LTIME_OF_DAY`, `LDT`/`LDATE_AND_TIME`:

```
VAR
    tHighRes : LTIME := LTIME#500ns;
    dtLong : LDT;
END_VAR
```

**Design:** Add these as keyword tokens. They are handled identically to the existing `TIME`, `DATE`, `TOD`, `DT` types — the parser recognizes them as elementary type names. Their semantics (64-bit resolution) are a future semantic analysis concern.

### Priority 3: Variable Sections and Misc

#### 3.1 `VAR_INST`

Instance variables in methods (persist across calls, per FB instance):

```
METHOD DoWork : BOOL
VAR_INST
    callCount : INT;
END_VAR
    callCount := callCount + 1;
END_METHOD
```

**Design:** Add `VarInst` as a keyword token. The parser accepts `VAR_INST ... END_VAR` as a variable section within methods. In the AST, instance variables are represented with a new qualifier.

#### 3.2 `VAR_STAT` (also in Siemens)

Static variables in functions:

```
FUNCTION FC_Counter : INT
VAR_STAT
    count : INT;
END_VAR
    count := count + 1;
    FC_Counter := count;
END_FUNCTION
```

**Design:** Same as described in the [Siemens SCL design](siemens-scl-dialect.md#21-var_stat-static-variables). Add `VarStat` as a keyword token.

#### 3.3 Pragma Attributes

TwinCAT uses `{attribute 'name'}` syntax:

```
{attribute 'qualified_only'}
{attribute 'strict'}
{attribute 'pack_mode' := '1'}
```

And conditional compilation:

```
{IF defined (variable)}
    // conditional code
{END_IF}
```

And region markers:

```
{region 'My Section'}
{endregion}
```

**Design:** Same approach as the [Siemens SCL pragma design](siemens-scl-dialect.md#12-curly-brace-pragmas). A token transform collapses `LeftBrace ... RightBrace` sequences into `Pragma` tokens that are skipped during parsing. The content inside pragmas is opaque.

For TwinCAT, pragmas are always present (they appear in virtually every `.TcPOU` file), so the pragma collapsing transform should be automatically enabled for the TwinCAT dialect.

#### 3.4 Short-Circuit Boolean Operators

TwinCAT adds `AND_THEN` and `OR_ELSE` for short-circuit evaluation, critical for safe pointer checks:

```
IF (ptr <> 0 AND_THEN ptr^ = 99) THEN
    // safe: ptr^ is only evaluated if ptr <> 0
END_IF;
```

**Design:** Add `AndThen` and `OrElse` as keyword tokens. These are binary operators with the same precedence as `AND` and `OR` respectively. In the AST, they map to new boolean operator variants. Semantically they are identical to `AND`/`OR` except for evaluation order, which is a code generation concern.

#### 3.5 `OVERRIDE` and `CONTINUE` Keywords

`OVERRIDE` marks a method as overriding a base class method:

```
METHOD OVERRIDE Start : BOOL
    // ...
END_METHOD
```

`CONTINUE` skips to the next loop iteration (common extension):

```
FOR i := 0 TO 100 DO
    IF arr[i] = 0 THEN
        CONTINUE;
    END_IF;
    // process arr[i]
END_FOR;
```

**Design:** Add `Override` and `Continue` as keyword tokens. `OVERRIDE` is a method modifier alongside `ABSTRACT`/`FINAL`. `CONTINUE` is a statement keyword, parallel to `EXIT`.

#### 3.6 Extended Assignment Operators

TwinCAT adds `S=` (latching set), `R=` (latching reset), and `REF=` (reference assignment). See the [Beckhoff S= documentation](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528254091.html), [R= documentation](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528259467.html), and [REF= documentation](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/4978163979.html).

```
bMotorRunning S= bStartButton;   // once TRUE, stays TRUE until R=
bMotorRunning R= bStopButton;    // once TRUE, resets to FALSE
refA REF= stA;                   // assign reference (address of stA to refA)
```

Both operands of `S=` and `R=` must be `BOOL`. These operators can be chained on a single line, where all assignments refer to the operand at the end:

```
bSetVariable S= bResetVariable R= F_Sample(bIn := bVar);
```

**Design:** These operators **cannot be handled at the token transform level**. The tokens `S`, `R`, and `REF` are all valid variable names, so collapsing `Identifier("S") + Equal` into a single token at the transform stage would incorrectly transform expressions like `IF S = 5 THEN` (where `S` is a variable being compared with `=`).

Instead, these are handled at the **parser level in assignment/statement context**. After parsing the LHS expression of a statement, the parser checks for the assignment operator:
- `:=` → standard assignment
- `Identifier("S")` + `Equal` → set-assignment (`S=`)
- `Identifier("R")` + `Equal` → reset-assignment (`R=`)
- `Identifier("REF")` + `Equal` → reference-assignment (`REF=`)

This is unambiguous because in IEC 61131-3, `:=` is the assignment operator and `=` is only used for comparison. In statement position after an LHS expression, `Identifier + Equal` cannot be a comparison — comparisons appear inside expressions, not as statements.

**AST representation:** New assignment statement variants: `SetAssign`, `ResetAssign`, `RefAssign`. See AST Extensions section.

#### 3.7 Address and Size Operators

```
pAddr := ADR(myVariable);      // get pointer to variable
nSize := SIZEOF(myStruct);     // get size in bytes
```

**Design:** `ADR`, `SIZEOF`, `BITADR`, and `INDEXOF` are function-like operators. Since they look syntactically identical to function calls (identifier followed by parenthesized arguments), the parser already handles them as function call expressions. They only need special handling during semantic analysis, not parsing. No parser changes needed.

#### 3.8 Diagnostic Pseudo-Variables

```
sName := __POUNAME;      // returns name of current POU as STRING
sPos  := __POSITION;     // returns source position as STRING
```

**Design:** These are identifiers that start with `__`. The lexer already accepts `__POUNAME` and `__POSITION` as valid identifiers (they match the `[A-Za-z_][A-Za-z0-9_]*` pattern). No lexer changes needed. Semantic analysis would recognize them as built-in pseudo-variables.

#### 3.9 `__NEW`, `__DELETE`, `__TRY`/`__CATCH`/`__FINALLY`

Advanced runtime features:

```
pMotor := __NEW(FB_Motor);
__DELETE(pMotor);

__TRY
    riskyOperation();
__CATCH(exceptionCode)
    handleError();
__FINALLY
    cleanup();
__ENDTRY
```

**Design:** These are low priority. `__NEW` and `__DELETE` look like function calls to the parser (identifier + parenthesized args) so need no grammar changes. `__TRY`/`__CATCH`/`__FINALLY`/`__ENDTRY` require new keyword tokens and a new statement block structure.

#### 3.10 Enum with Underlying Type

```
TYPE E_Color :
(
    Red := 0,
    Green := 1,
    Blue := 2
) UINT;
END_TYPE
```

**Design:** After the closing `)` of an enum value list, the parser optionally accepts a type name specifying the underlying type. This is stored as metadata on the enum declaration AST node.

## Extension Origin Model

Every vendor-specific construct in IronPLC is tagged with its origin — which vendor dialects introduced it. This enum is the **single source of truth** that drives both the token transform pipeline (which keywords to promote) and the semantic diagnostic (which extensions to flag as unsupported).

### `ExtensionOrigin` Enum

```rust
/// Identifies the vendor or standards origin of a language extension.
///
/// A single extension may have multiple origins. For example, `VAR_STAT`
/// appears in both Beckhoff TwinCAT and Siemens SCL. `CONTINUE` is part
/// of IEC 61131-3 3rd edition AND appears in Beckhoff/CODESYS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtensionOrigin {
    /// IEC 61131-3 3rd edition (2013) features not yet supported by IronPLC.
    Iec61131Ed3,
    /// Beckhoff TwinCAT / CODESYS OOP and type system extensions.
    BeckhoffCodesys,
    /// Siemens SCL-specific extensions.
    SiemensSCL,
}
```

The origin is a **static property of each extension type**, not determined per-instance at parse time. `VAR_STAT` always returns `&[BeckhoffCodesys, SiemensSCL]` regardless of which file it was parsed from.

### How `ExtensionOrigin` Drives the Token Transform Pipeline

The keyword promotion table is indexed by `ExtensionOrigin`. When the dialect is `BeckhoffTwinCAT`, the transform enables all keywords tagged with `BeckhoffCodesys`. When the dialect is `SiemensSCL`, it enables all keywords tagged with `SiemensSCL`. Keywords tagged with both (like `VAR_STAT`) are promoted in both dialects.

```rust
/// Associates each promotable keyword with its extension origins.
struct KeywordEntry {
    text: &'static str,         // e.g., "METHOD"
    token_type: TokenType,      // e.g., TokenType::Method
    origins: &'static [ExtensionOrigin],  // e.g., &[BeckhoffCodesys]
}

const DIALECT_KEYWORDS: &[KeywordEntry] = &[
    // Beckhoff/CODESYS only
    KeywordEntry { text: "METHOD",     token_type: TokenType::Method,     origins: &[BeckhoffCodesys] },
    KeywordEntry { text: "INTERFACE",  token_type: TokenType::Interface,  origins: &[BeckhoffCodesys] },
    KeywordEntry { text: "EXTENDS",    token_type: TokenType::Extends,    origins: &[BeckhoffCodesys] },
    // ...

    // Shared between Beckhoff and Siemens
    KeywordEntry { text: "VAR_STAT",   token_type: TokenType::VarStat,    origins: &[BeckhoffCodesys, SiemensSCL] },
    KeywordEntry { text: "CONTINUE",   token_type: TokenType::Continue,   origins: &[Iec61131Ed3, BeckhoffCodesys] },
    // ...
];

fn promote_keywords(tokens: Vec<Token>, dialect: Dialect) -> Vec<Token> {
    let active_origins = dialect.extension_origins(); // e.g., &[BeckhoffCodesys]
    // Promote Identifier tokens whose text matches an entry
    // where entry.origins intersects active_origins
}
```

This design means:
- Adding a new keyword requires one entry in `DIALECT_KEYWORDS`
- The same table drives both `promote_twincat_keywords` and `promote_scl_keywords`
- No duplication between the promotion logic and the AST metadata

## Parser Integration

### Dialect Gating: Token Transforms as the Gate

The dialect gate lives in the **token transform layer**, not in the parser. The parser grammar rules for TwinCAT constructs (e.g., `METHOD ... END_METHOD`, `EXTENDS`, `INTERFACE`) are always present in the PEG grammar. They simply never fire in standard mode because the tokens that trigger them (`Method`, `Extends`, `Interface`, etc.) are only produced by the TwinCAT keyword promotion transform.

This means the parser itself needs no dialect awareness or `ParseOptions` access for most features. A grammar rule like `tok(TokenType::Method) _ name() _ ...` will never match when parsing standard IEC 61131-3 because `METHOD` remains an `Identifier` token — it is never promoted to `Method`.

The only exception would be grammar rules that must distinguish between standard and TwinCAT behavior for the *same* token. No such cases have been identified in the current design — all TwinCAT grammar extensions use tokens that only exist after promotion.

### Token Transform Pipeline

TwinCAT dialect tokens are handled by the shared transform pipeline described in [dialect-token-transforms.md](dialect-token-transforms.md). The TwinCAT dialect applies:

1. **Keyword promotion** — all OOP keywords, type keywords, and variable section keywords (see table below)
2. **Pragma collapsing** — `{ ... }` → single `Pragma` token (shared with Siemens SCL)

No token rewriting or filtering is needed for TwinCAT — double-quoted strings remain `DoubleByteString` (WSTRING literals), and there is no `#` variable prefix.

Multi-token constructs (`POINTER TO`, `REFERENCE TO`) are composed by the parser, not by token promotion. The parser sees `Pointer` + `To` (where `To` is already a standard keyword) and recognizes the type constructor. Extended assignment operators (`S=`, `R=`, `REF=`) are also handled at the parser level in assignment context (see section 3.6).

### TwinCAT XML Parser Changes

The `twincat_parser.rs` module currently handles `POU`, `GVL`, and `DUT` XML elements. It needs to be extended:

1. **Method elements** — iterate over `<Method>` children of a POU and parse each as a **standalone declaration**
2. **Property elements** — iterate over `<Property>` children; parse each `<Get>` and `<Set>` body as a standalone statement list
3. **Interface elements** — handle `<Itf>` as a new top-level object type alongside POU/GVL/DUT

Each sub-element is parsed independently following the existing CDATA extraction pattern: extract Declaration CDATA, extract Implementation/ST CDATA (if present), concatenate with closing keyword, parse, adjust positions. The parsed results are then attached to the parent FB's AST node.

In real TwinCAT projects, methods and properties are **always separate XML elements** — they never appear inline in the function block's ST body. The FB's Declaration CDATA has the header + VARs, its Implementation CDATA has the body, and methods are sibling `<Method>` elements. The `twincat_parser.rs` module orchestrates parsing each piece and assembling the final AST.

### ST Parser Changes

The ST parser (`compiler/parser/`) needs:

1. **New `TokenType` variants** — approximately 30 new variants (see full list below), but these are only added to the enum, **not** to the logos lexer grammar
2. **Grammar extensions** — method/property declarations, interface declarations, `EXTENDS`/`IMPLEMENTS` clauses, pointer/reference types, union types, access modifiers on methods/properties, extended assignment operators
3. **No dialect-aware parsing needed** — grammar rules are self-gating through token promotion (see "Dialect Gating" above)

### New TokenType Variants

These are added to the `TokenType` enum **without** `#[token(...)]` attributes — they have no logos lexer rules. They are populated exclusively by the dialect keyword promotion transform.

| Token | Promoted from `Identifier` text | Priority |
|-------|-------------------------------|----------|
| `Method` | `METHOD` | 1 |
| `EndMethod` | `END_METHOD` | 1 |
| `Property` | `PROPERTY` | 1 |
| `EndProperty` | `END_PROPERTY` | 1 |
| `GetAccessor` | `GET` | 1 |
| `EndGet` | `END_GET` | 1 |
| `SetAccessor` | `SET` | 1 |
| `EndSet` | `END_SET` | 1 |
| `Interface` | `INTERFACE` | 1 |
| `EndInterface` | `END_INTERFACE` | 1 |
| `Extends` | `EXTENDS` | 1 |
| `Implements` | `IMPLEMENTS` | 1 |
| `Public` | `PUBLIC` | 1 |
| `Private` | `PRIVATE` | 1 |
| `Protected` | `PROTECTED` | 1 |
| `Internal` | `INTERNAL` | 1 |
| `Abstract` | `ABSTRACT` | 1 |
| `Final` | `FINAL` | 1 |
| `This` | `THIS` | 1 |
| `Super` | `SUPER` | 1 |
| `Pointer` | `POINTER` | 2 |
| `Reference` | `REFERENCE` | 2 |
| `Union` | `UNION` | 2 |
| `EndUnion` | `END_UNION` | 2 |
| `Ltime` | `LTIME` | 2 |
| `Ldate` | `LDATE` | 2 |
| `LtimeOfDay` | `LTOD` / `LTIME_OF_DAY` | 2 |
| `LdateAndTime` | `LDT` / `LDATE_AND_TIME` | 2 |
| `VarInst` | `VAR_INST` | 3 |
| `VarStat` | `VAR_STAT` | 3 |
| `AndThen` | `AND_THEN` | 3 |
| `OrElse` | `OR_ELSE` | 3 |
| `Override` | `OVERRIDE` | 3 |
| `Continue` | `CONTINUE` | 3 |

Multi-token constructs handled by parser context (not promotion):
- `POINTER TO` — parser sees `Pointer` + `To` (standard keyword)
- `REFERENCE TO` — parser sees `Reference` + `To`
- `REF=` — parser sees `Identifier("REF")` + `Equal` in assignment context (see section 3.6)
- `S=` / `R=` — parser sees `Identifier("S"|"R")` + `Equal` in assignment context (see section 3.6)

Note: Identifiers starting with `__` (`__NEW`, `__DELETE`, `__POUNAME`, `__POSITION`, `__QUERYINTERFACE`, `__ISVALIDREF`, `__VARINFO`) are already valid identifiers — they need no promotion, only semantic recognition.

## AST Extensions (DSL Crate)

These are the concrete changes needed in the `compiler/dsl/` crate to represent TwinCAT constructs. The DSL must represent parsed constructs, not just parse them — downstream analysis and future code generation depend on having a complete AST.

### New Declaration Types

These types are added to `compiler/dsl/src/common.rs`.

**Top-level: `LibraryElementKind` extension**

```rust
pub enum LibraryElementKind {
    // ... existing variants ...
    InterfaceDeclaration(InterfaceDeclaration),  // NEW
}
```

**`FunctionBlockDeclaration` extension**

```rust
pub struct FunctionBlockDeclaration {
    pub name: TypeName,
    pub variables: Vec<VarDecl>,
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: FunctionBlockBodyKind,
    pub span: SourceSpan,
    // NEW fields:
    pub extends: Option<TypeName>,          // EXTENDS base_name
    pub implements: Vec<TypeName>,          // IMPLEMENTS I_Foo, I_Bar
    pub methods: Vec<MethodDeclaration>,    // METHOD ... END_METHOD
    pub properties: Vec<PropertyDeclaration>, // PROPERTY ... END_PROPERTY
}
```

**New structs**

```rust
/// An interface declaration: INTERFACE name ... END_INTERFACE
pub struct InterfaceDeclaration {
    pub name: Id,
    pub extends: Option<TypeName>,         // interfaces can extend other interfaces
    pub methods: Vec<MethodSignature>,     // method signatures (no bodies)
    pub properties: Vec<PropertySignature>, // property signatures (no bodies)
    pub span: SourceSpan,
}

/// A method signature (interface context — no body).
pub struct MethodSignature {
    pub name: Id,
    pub return_type: Option<TypeName>,
    pub variables: Vec<VarDecl>,           // VAR_INPUT, VAR_OUTPUT params
    pub span: SourceSpan,
}

/// A property signature (interface context — no body).
pub struct PropertySignature {
    pub name: Id,
    pub prop_type: TypeName,
    pub span: SourceSpan,
}

/// A method declaration with optional body (function block context).
pub struct MethodDeclaration {
    pub name: Id,
    pub return_type: Option<TypeName>,
    pub access: Option<AccessModifier>,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_override: bool,
    pub variables: Vec<VarDecl>,
    pub body: Vec<StmtKind>,
    pub span: SourceSpan,
}

/// A property declaration (function block context).
pub struct PropertyDeclaration {
    pub name: Id,
    pub prop_type: TypeName,
    pub access: Option<AccessModifier>,
    pub getter: Option<PropertyAccessor>,
    pub setter: Option<PropertyAccessor>,
    pub span: SourceSpan,
}

/// A property getter or setter body.
pub struct PropertyAccessor {
    pub variables: Vec<VarDecl>,
    pub body: Vec<StmtKind>,
    pub span: SourceSpan,
}

/// Access modifier for methods and properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessModifier {
    Public,
    Private,
    Protected,
    Internal,
}
```

### New Variable Section Type

```rust
pub enum VariableType {
    // ... existing variants ...
    Instance,   // VAR_INST (NEW) — instance vars in methods
    Static,     // VAR_STAT (NEW) — static vars in functions
}
```

### New Type Variants

```rust
// In the type specification representation:
TypeSpec:
    + PointerTo(Box<TypeSpec>)     // POINTER TO type
    + ReferenceTo(Box<TypeSpec>)   // REFERENCE TO type

// In data type declarations:
DataTypeDeclarationKind:
    + Union(UnionDeclaration)      // UNION ... END_UNION

pub struct UnionDeclaration {
    pub type_name: TypeName,
    pub elements: Vec<StructureElementDeclaration>,  // reuses struct field representation
}

// Enum extension for underlying type:
pub struct EnumerationDeclaration {
    // ... existing fields ...
    pub underlying_type: Option<TypeName>,  // NEW: the optional UINT/INT after )
}
```

### New Expression Variants

```rust
// In compiler/dsl/src/textual.rs:
pub enum ExprKind {
    // ... existing variants ...
    ThisRef(SourceSpan),           // THIS — self-reference
    SuperRef(SourceSpan),          // SUPER — parent-reference
    Dereference(Box<ExprKind>),    // expr^ — pointer dereference
}
```

Note: `THIS^` and `SUPER^` are parsed as `ThisRef`/`SuperRef` + dereference. The `^` applies the existing dereference operator to the self/parent reference. Member access (`THIS^.method()`) uses the existing member access expression with a `ThisRef` or `SuperRef` as the base.

### New Statement Variants

```rust
pub enum StmtKind {
    // ... existing variants ...
    SetAssign {                    // bVar S= bOperand;
        target: Variable,
        operand: ExprKind,
        span: SourceSpan,
    },
    ResetAssign {                  // bVar R= bOperand;
        target: Variable,
        operand: ExprKind,
        span: SourceSpan,
    },
    RefAssign {                    // refVar REF= sourceVar;
        target: Variable,
        source: ExprKind,
        span: SourceSpan,
    },
    Continue(SourceSpan),          // CONTINUE;
}
```

### New Operator Variants

```rust
// For AND_THEN / OR_ELSE short-circuit operators:
pub enum Operator {
    // ... existing variants ...
    AndThen,   // AND_THEN — short-circuit AND
    OrElse,    // OR_ELSE — short-circuit OR
}
```

## Vendor Extension Trait and Semantic Rule

### The `VendorExtension` Trait

Every AST node representing a vendor-specific construct implements this trait. It provides the metadata needed for the `P9004` diagnostic without any per-instance runtime data — origins are static per type.

```rust
/// Marker trait for AST nodes representing vendor-specific language extensions.
///
/// Nodes implementing this trait are parsed and represented in the AST but
/// not yet semantically analyzed or supported in code generation. The semantic
/// rule `rule_unsupported_extension` walks the AST and emits P9004 for every
/// node that implements this trait.
///
/// As each extension graduates to full support, remove its VendorExtension
/// impl. The semantic rule automatically stops flagging it.
pub trait VendorExtension {
    /// Human-readable name of this extension (e.g., "METHOD declaration").
    fn extension_name(&self) -> &'static str;

    /// Which vendor dialects introduced this extension. A single extension
    /// may originate from multiple vendors (e.g., VAR_STAT is both
    /// BeckhoffCodesys and SiemensSCL).
    fn extension_origins(&self) -> &'static [ExtensionOrigin];

    /// The source span for diagnostic reporting.
    fn extension_span(&self) -> SourceSpan;
}
```

**Example implementations:**

```rust
// Beckhoff/CODESYS extension — METHOD declaration
// Extension: Beckhoff/CODESYS OOP
impl VendorExtension for MethodDeclaration {
    fn extension_name(&self) -> &'static str { "METHOD declaration" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] { &[ExtensionOrigin::BeckhoffCodesys] }
    fn extension_span(&self) -> SourceSpan { self.span }
}

// Shared extension — VAR_STAT
// Extension: Beckhoff/CODESYS, Siemens SCL
impl VendorExtension for VarStatSection {
    fn extension_name(&self) -> &'static str { "VAR_STAT section" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] {
        &[ExtensionOrigin::BeckhoffCodesys, ExtensionOrigin::SiemensSCL]
    }
    fn extension_span(&self) -> SourceSpan { self.span }
}

// IEC 61131-3 3rd edition + Beckhoff — CONTINUE statement
// Extension: IEC 61131-3 3rd edition, Beckhoff/CODESYS
impl VendorExtension for ContinueStatement {
    fn extension_name(&self) -> &'static str { "CONTINUE statement" }
    fn extension_origins(&self) -> &'static [ExtensionOrigin] {
        &[ExtensionOrigin::Iec61131Ed3, ExtensionOrigin::BeckhoffCodesys]
    }
    fn extension_span(&self) -> SourceSpan { self.span }
}
```

### Problem Code: `P9004 — UnsupportedExtension`

One problem code covers all unsupported extensions. The diagnostic message identifies the specific extension and its origin(s):

```
P9004 - Recognized extension not supported
  --> project/FB_Motor.TcPOU:15:1
   |
15 | METHOD Start : BOOL
   | ^^^^^^^^^^^^^^^^^^^^ METHOD declaration (Beckhoff/CODESYS extension) is recognized
   |                      but not yet supported by IronPLC
```

For shared extensions:

```
P9004 - Recognized extension not supported
  --> project/FC_Counter.scl:8:1
   |
 8 | VAR_STAT
   | ^^^^^^^^ VAR_STAT section (Beckhoff/CODESYS, Siemens SCL extension) is recognized
   |          but not yet supported by IronPLC
```

### Semantic Rule: `rule_unsupported_extension.rs`

A single visitor walks the AST and emits `P9004` for every `VendorExtension` node. The visitor checks each AST node type that could be a vendor extension:

```rust
pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleUnsupportedExtension { diagnostics: Vec::new() };
    visitor.walk(lib).map_err(|e| vec![e])?;
    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

impl RuleUnsupportedExtension {
    fn check_extension(&mut self, ext: &dyn VendorExtension) {
        let origins: Vec<&str> = ext.extension_origins().iter().map(|o| o.as_str()).collect();
        let origin_text = origins.join(", ");
        self.diagnostics.push(Diagnostic::problem(
            Problem::UnsupportedExtension,
            Label::span(
                ext.extension_span(),
                format!(
                    "{} ({} extension) is recognized but not yet supported by IronPLC",
                    ext.extension_name(),
                    origin_text,
                ),
            ),
        ));
    }
}
```

The visitor overrides `visit_*` for each extension node type (MethodDeclaration, PropertyDeclaration, InterfaceDeclaration, etc.) and calls `check_extension`. As extensions graduate to full support, their `visit_*` override is removed (or their `VendorExtension` impl is removed) and the rule stops flagging them.

### Graduation Path

When an extension moves from "parsed but unsupported" to "fully supported":

1. Remove the `VendorExtension` impl from the AST node
2. Remove the `visit_*` override in `rule_unsupported_extension.rs`
3. Add real semantic rules for the construct
4. The `P9004` diagnostic automatically stops appearing for that construct

## Testing Strategy

### XML-level tests (twincat_parser.rs)

- POU with Method child elements
- POU with Property child elements (Get, Set, both, Get-only)
- Interface (`<Itf>`) elements
- Verify position adjustment works for method/property CDATA sections

### ST-level tests (parser)

- `FUNCTION_BLOCK` with `EXTENDS` and `IMPLEMENTS`
- `METHOD` and `END_METHOD` within function blocks
- `PROPERTY` with `GET`/`SET` accessors
- `INTERFACE` with method signatures
- Access modifiers on methods and properties
- `POINTER TO` and `REFERENCE TO` type declarations
- `UNION` type declarations
- `VAR_INST` and `VAR_STAT` sections
- `{attribute}` pragmas (verify they're skipped cleanly)
- Additional time types (`LTIME`, `LDT`, etc.)
- Enum with underlying type
- `S=` / `R=` / `REF=` extended assignment operators
- `S=`/`R=` chained on a single line
- `AND_THEN` / `OR_ELSE` short-circuit operators
- `CONTINUE` statement in loops

### Keyword safety regression test (MUST be added before any keyword promotion)

Before adding any keyword promotion code, add a test that proves every planned keyword is a valid identifier in standard mode. This is a **prerequisite** — it must exist and pass before any dialect transforms are implemented.

The test defines a function block where every planned TwinCAT/Siemens keyword is used as a variable name, and verifies it parses successfully in standard mode:

```rust
/// Proves that all planned dialect keywords remain valid identifiers
/// in standard IEC 61131-3 mode. If keyword promotion accidentally
/// leaks into standard parsing, this test will fail.
#[test]
fn parse_when_standard_mode_then_dialect_keywords_are_valid_identifiers() {
    let program = "
FUNCTION_BLOCK ALL_KEYWORDS_AS_VARS
VAR
    METHOD : INT;
    PROPERTY : INT;
    INTERFACE : INT;
    EXTENDS : INT;
    IMPLEMENTS : INT;
    PUBLIC : INT;
    PRIVATE : INT;
    PROTECTED : INT;
    INTERNAL : INT;
    ABSTRACT : INT;
    FINAL : INT;
    OVERRIDE : INT;
    THIS : INT;
    SUPER : INT;
    POINTER : INT;
    REFERENCE : INT;
    UNION : INT;
    CONTINUE : INT;
    LTIME : INT;
    LDATE : INT;
    GET : INT;
    SET : INT;
END_VAR

METHOD := 1;
PROPERTY := 2;
INTERFACE := 3;
EXTENDS := 4;
IMPLEMENTS := 5;
PUBLIC := 6;
PRIVATE := 7;
PROTECTED := 8;
INTERNAL := 9;
ABSTRACT := 10;
FINAL := 11;
OVERRIDE := 12;
THIS := 13;
SUPER := 14;
POINTER := 15;
REFERENCE := 16;
UNION := 17;
CONTINUE := 18;
LTIME := 19;
LDATE := 20;
GET := 21;
SET := 22;
END_FUNCTION_BLOCK
";
    // Parse with default (standard) options — no dialect
    let result = parse_program(program, &FileId::default(), &ParseOptions::default());
    assert!(result.is_ok(), "Dialect keywords must remain valid identifiers in standard mode");
}
```

This test lives in `compiler/parser/src/tests.rs` alongside the other parser tests. It must be added **before** any `TokenType` variants for dialect keywords are created, and must continue to pass after they are added.

### Regression tests

- All existing TwinCAT XML tests continue to pass
- All existing standard ST tests continue to pass
- Standard ST files parsed with TwinCAT dialect produce identical results

### Integration tests

- Parse representative open-source TwinCAT projects without errors

## Phased Implementation

0. **Phase 0 — Prerequisites** (before any dialect code):
   - Keyword safety regression test: function block with all planned keywords as variable names, parsed in standard mode
   - `ExtensionOrigin` enum in the DSL crate
   - `VendorExtension` trait in the DSL crate
   - `P9004 UnsupportedExtension` problem code in CSV and documentation
   - `rule_unsupported_extension.rs` semantic rule (empty initially — no extension nodes exist yet)

1. **Phase 1 — Core OOP and pragmas** (enables parsing most real TwinCAT projects):
   - `Dialect` enum and `ParseOptions` extension (shared infrastructure)
   - Token transform pipeline: keyword promotion + pragma collapsing (shared with Siemens SCL)
   - `{ }` pragma collapsing
   - `METHOD` / `END_METHOD` parsing in ST
   - `PROPERTY` / `END_PROPERTY` with `GET`/`SET` parsing in ST
   - `EXTENDS` / `IMPLEMENTS` on function blocks (virtually every FB uses these)
   - Method, Property, and Interface XML element handling in `twincat_parser.rs`
   - `INTERFACE` / `END_INTERFACE`
   - DSL: `MethodDeclaration`, `PropertyDeclaration`, `InterfaceDeclaration`, `extends`/`implements` fields
   - `VendorExtension` impls on all new AST nodes; `rule_unsupported_extension` visitor overrides

2. **Phase 2 — Access modifiers and expressions**:
   - Access modifiers on methods/properties (`PUBLIC`, `PRIVATE`, `PROTECTED`, `INTERNAL`)
   - `ABSTRACT` / `FINAL` / `OVERRIDE` method modifiers
   - `THIS^` / `SUPER^` expressions
   - DSL: `AccessModifier`, `ThisRef`, `SuperRef`
   - `VendorExtension` impls on new nodes

3. **Phase 3 — Type system extensions**:
   - `POINTER TO` / `REFERENCE TO`
   - `REF=` operator (parser-level, assignment context)
   - `UNION` / `END_UNION`
   - Additional time types (`LTIME`, `LDT`, `LTOD`, `LDATE`)
   - `VAR_INST`, `VAR_STAT`
   - Enum underlying types
   - `AND_THEN` / `OR_ELSE` short-circuit operators
   - `CONTINUE` statement
   - DSL: `PointerTo`, `ReferenceTo`, `UnionDeclaration`, `RefAssign`, `Continue`, `AndThen`/`OrElse`
   - `VendorExtension` impls on new nodes

4. **Phase 4 — Advanced features**:
   - `S=` / `R=` extended assignment operators (parser-level, assignment context)
   - `__TRY` / `__CATCH` / `__FINALLY` / `__ENDTRY`
   - Conditional compilation pragmas (`{IF defined(...)}` / `{END_IF}`)
   - DSL: `SetAssign`, `ResetAssign`
   - `VendorExtension` impls on new nodes
