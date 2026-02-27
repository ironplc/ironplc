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

```
FUNCTION_BLOCK FB_Example
VAR
    PUBLIC myPublicVar : INT;
    PRIVATE myPrivateVar : INT;
    PROTECTED myProtectedVar : INT;
    INTERNAL myInternalVar : INT;
END_VAR

PUBLIC METHOD DoWork : BOOL
    ...
END_METHOD

ABSTRACT METHOD MustOverride : BOOL
END_METHOD

FINAL METHOD CannotOverride : BOOL
    ...
END_METHOD
END_FUNCTION_BLOCK
```

**Design:** Add `Public`, `Private`, `Protected`, `Internal`, `Abstract`, `Final` as keyword tokens. These appear as optional modifiers before variable declarations and method/property declarations. The parser accepts them in modifier positions and stores them as metadata. No semantic enforcement initially.

#### 1.6 `THIS^` and `SUPER^`

Special expressions for self-reference and parent-reference:

```
THIS^.myMethod();
SUPER^.Start();
```

**Design:** Add `This` and `Super` as keyword tokens. The parser recognizes `THIS^` and `SUPER^` as primary expressions (like variable references) that can be followed by member access (`.`). The `^` is the existing dereference operator in IEC 61131-3. In the AST, these map to new expression variants.

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

**Design:** Add `Pointer` and `Reference` as keyword tokens (note: `To` already exists). The parser recognizes `POINTER TO type` and `REFERENCE TO type` as type specifiers in variable declarations. The `REF=` operator is a new assignment operator token.

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

TwinCAT adds `S=` (latching set) and `R=` (latching reset):

```
bMotorRunning S= bStartButton;   // once TRUE, stays TRUE until R=
bMotorRunning R= bStopButton;    // once TRUE, resets to FALSE
```

**Design:** Add `SetAssign` (`S=`) and `ResetAssign` (`R=`) as operator tokens. These are assignment operators used in statement context. In the AST, they map to new assignment variants.

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

## Parser Integration

### Keyword Promotion via Token Transform (Not Lexer)

The logos lexer in `token.rs` declares IEC 61131-3 keywords with `#[token("KEYWORD", ignore(case))]` at higher priority than the `Identifier` regex. This means standard keywords always lex as their keyword token type — `FUNCTION_BLOCK` is always `TokenType::FunctionBlock`, never `Identifier`.

Vendor-specific keywords like `METHOD`, `EXTENDS`, `INTERFACE`, `PROPERTY` etc. **must not** be added to the logos lexer. If they were, they would always be keywords in every dialect, breaking standard mode where these are valid identifiers. For example, a standard IEC 61131-3 program can legally have a variable named `method` or `interface`.

Instead, vendor keywords are promoted from `Identifier` to the appropriate `TokenType` by a **dialect-aware token transform** that runs after lexing. This follows the existing pattern in `xform_tokens.rs`:

```
Source text
  → Logos lexer (standard IEC 61131-3 keywords only)
  → Preprocessor (OSCAT comments)
  → Dialect keyword promotion (when dialect != Standard):
      For each Identifier token, check if its text (case-insensitive)
      matches a vendor keyword for the active dialect.
      If so, replace the token_type while preserving span/line/col/text.
  → Other token transforms (pragma collapsing, # stripping, etc.)
  → Standard token transforms (insert keyword terminators)
  → Parser
```

The keyword promotion function is a simple lookup table:

```rust
fn promote_twincat_keywords(tokens: Vec<Token>) -> Vec<Token> {
    tokens.into_iter().map(|mut tok| {
        if tok.token_type == TokenType::Identifier {
            tok.token_type = match tok.text.to_uppercase().as_str() {
                "METHOD" => TokenType::Method,
                "EXTENDS" => TokenType::Extends,
                "IMPLEMENTS" => TokenType::Implements,
                "INTERFACE" => TokenType::Interface,
                "PROPERTY" => TokenType::Property,
                "ABSTRACT" => TokenType::Abstract,
                "FINAL" => TokenType::Final,
                "PUBLIC" => TokenType::Public,
                "PRIVATE" => TokenType::Private,
                "PROTECTED" => TokenType::Protected,
                "INTERNAL" => TokenType::Internal,
                "OVERRIDE" => TokenType::Override,
                "THIS" => TokenType::This,
                "POINTER" => TokenType::Pointer,
                "REFERENCE" => TokenType::Reference,
                "UNION" => TokenType::Union,
                "CONTINUE" => TokenType::Continue,
                // compound keywords like END_METHOD, END_INTERFACE
                // are already split by the lexer into separate tokens
                // and handled by the parser
                _ => tok.token_type,
            };
        }
        tok
    }).collect()
}
```

For compound keywords like `END_METHOD`, `END_INTERFACE`, etc., note that the lexer does not produce these as single tokens because they are not in the logos grammar. They arrive as two tokens: `Identifier("END_METHOD")` — wait, actually the `_` in `END_METHOD` makes this a single identifier token since `[A-Za-z_][A-Za-z0-9_]*` matches it. So these compound vendor keywords are also promoted from `Identifier` in the same lookup table:

```rust
"END_METHOD" => TokenType::EndMethod,
"END_PROPERTY" => TokenType::EndProperty,
"END_INTERFACE" => TokenType::EndInterface,
"END_UNION" => TokenType::EndUnion,
"VAR_INST" => TokenType::VarInst,
"VAR_STAT" => TokenType::VarStat,
"AND_THEN" => TokenType::AndThen,
"OR_ELSE" => TokenType::OrElse,
```

Multi-token operators like `POINTER TO`, `REFERENCE TO`, `REF=`, `S=`, and `R=` are handled by the parser, not by token promotion. The parser sees `Pointer` + `To` (where `To` is already a standard keyword) and recognizes the type constructor. For `REF=`, `S=`, `R=`, the parser recognizes the `Identifier("REF")` or `Identifier("S")` followed by `Equal` in the appropriate context — or alternatively, these can be collapsed in a separate token transform pass.

**Why this approach:**
- The logos lexer stays dialect-neutral — it only knows IEC 61131-3
- No conditional compilation or feature flags in the lexer
- Standard mode is unaffected — `METHOD` remains a valid `Identifier`
- The token transform is simple, testable, and composable
- Source spans are perfectly preserved — only `token_type` changes

### TwinCAT XML Parser Changes

The `twincat_parser.rs` module currently handles `POU`, `GVL`, and `DUT` XML elements. It needs to be extended:

1. **Method elements** — iterate over `<Method>` children of a POU and parse each one
2. **Property elements** — iterate over `<Property>` children, including their `<Get>` and `<Set>` sub-elements
3. **Interface elements** — handle `<Itf>` as a new top-level object type alongside POU/GVL/DUT

Each sub-element follows the same pattern as the existing POU parsing: extract Declaration CDATA, extract Implementation/ST CDATA, concatenate with closing keyword, parse, adjust positions.

### ST Parser Changes

The ST parser (`compiler/parser/`) needs:

1. **New `TokenType` variants** — approximately 30 new variants (see full list below), but these are only added to the enum, **not** to the logos lexer grammar
2. **Grammar extensions** — method/property declarations within function blocks, interface declarations, `EXTENDS`/`IMPLEMENTS` clauses, pointer/reference types, union types, access modifiers
3. **Dialect-aware parsing** — some grammar rules are only active when the dialect is TwinCAT (though most TwinCAT extensions don't conflict with standard syntax)

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
- `REF=` — parser sees `Identifier("REF")` + `Equal` or promoted in a separate pass
- `S=` / `R=` — parser sees `Identifier("S")` + `Equal`

Note: `Super` can be handled as an identifier rather than a keyword since it's only meaningful in expression context with `^`. Identifiers starting with `__` (`__NEW`, `__DELETE`, `__POUNAME`, `__POSITION`, `__QUERYINTERFACE`, `__ISVALIDREF`, `__VARINFO`) are already valid identifiers — they need no promotion, only semantic recognition.

## AST Extensions

### New Declaration Types

```
LibraryElementKind:
    + InterfaceDeclaration { name, methods: Vec<MethodSignature> }

// Within FunctionBlock:
    + methods: Vec<MethodDeclaration>
    + properties: Vec<PropertyDeclaration>
    + extends: Option<Identifier>
    + implements: Vec<Identifier>

MethodDeclaration:
    name: Identifier
    return_type: Option<TypeSpec>
    access: Option<AccessModifier>
    is_abstract: bool
    is_final: bool
    variables: Vec<VariableList>
    body: Vec<StmtKind>

PropertyDeclaration:
    name: Identifier
    prop_type: TypeSpec
    access: Option<AccessModifier>
    getter: Option<PropertyAccessor>
    setter: Option<PropertyAccessor>

PropertyAccessor:
    variables: Vec<VariableList>
    body: Vec<StmtKind>

AccessModifier: Public | Private | Protected | Internal
```

### New Type Variants

```
TypeSpec:
    + PointerTo(Box<TypeSpec>)
    + ReferenceTo(Box<TypeSpec>)

TypeDeclarationBody:
    + Union { fields: Vec<StructField> }
```

### New Expression Variants

```
ExprKind:
    + ThisRef          // THIS^
    + SuperRef         // SUPER^
    + Dereference(Box<ExprKind>)  // expr^  (may already exist)
```

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
- Access modifiers on variables and methods
- `POINTER TO` and `REFERENCE TO` type declarations
- `UNION` type declarations
- `VAR_INST` and `VAR_STAT` sections
- `{attribute}` pragmas (verify they're skipped cleanly)
- Additional time types (`LTIME`, `LDT`, etc.)
- Enum with underlying type

### Regression tests

- All existing TwinCAT XML tests continue to pass
- All existing standard ST tests continue to pass
- Standard ST files parsed with TwinCAT dialect produce identical results

### Integration tests

- Parse representative open-source TwinCAT projects without errors

## Phased Implementation

1. **Phase 1 — Pragmas and basic method/property recognition**:
   - `{ }` pragma collapsing (shared with Siemens SCL)
   - `METHOD` / `END_METHOD` parsing in ST
   - `PROPERTY` / `END_PROPERTY` with `GET`/`SET` parsing in ST
   - Method and Property XML element handling in `twincat_parser.rs`
   - `Dialect` integration with `ParseOptions`

2. **Phase 2 — OOP declarations**:
   - `INTERFACE` / `END_INTERFACE`
   - `EXTENDS` / `IMPLEMENTS` on function blocks
   - Access modifiers (`PUBLIC`, `PRIVATE`, `PROTECTED`, `INTERNAL`)
   - `ABSTRACT` / `FINAL`
   - `THIS^` / `SUPER^` expressions

3. **Phase 3 — Type system extensions**:
   - `POINTER TO` / `REFERENCE TO`
   - `REF=` operator
   - `UNION` / `END_UNION`
   - Additional time types (`LTIME`, `LDT`, `LTOD`, `LDATE`)
   - `VAR_INST`, `VAR_STAT`
   - Enum underlying types
   - `AND_THEN` / `OR_ELSE` short-circuit operators
   - `OVERRIDE` method modifier
   - `CONTINUE` statement

4. **Phase 4 — Advanced features**:
   - `S=` / `R=` extended assignment operators
   - `__TRY` / `__CATCH` / `__FINALLY` / `__ENDTRY`
   - Conditional compilation pragmas (`{IF defined(...)}` / `{END_IF}`)
