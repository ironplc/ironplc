# Plan: PLC Open XML Syntax Highlighting

## Overview

Add VS Code syntax highlighting support for PLC Open TC6 XML files (`.xml`). This enables users editing PLC Open XML project files to benefit from colorized elements, making the XML structure and embedded Structured Text code easier to read.

## Current State

### What Exists
- **Structured Text grammar**: `integrations/vscode/syntaxes/61131-3-st.tmLanguage.json` provides highlighting for `.st` and `.iec` files
- **ST language configuration**: `integrations/vscode/decl/61131-3-st-language-configuration.json` provides bracket matching, auto-close pairs
- **XML parser**: The compiler fully parses PLC Open XML files via `compiler/sources/src/xml/`
- **XSD schema**: `compiler/resources/schemas/tc6_xml_v201.xsd` defines the official PLC Open TC6 v2.01 schema

### What's Missing
- TextMate grammar for PLC Open XML files
- Language configuration for XML editing
- VS Code extension registration for the new language
- Embedded ST highlighting within `<ST><xhtml>...</xhtml></ST>` blocks

## Implementation Plan

### Phase 1: Basic PLC Open XML Grammar

Create a TextMate grammar that highlights the XML structure with PLC Open-specific awareness.

#### 1.1 Create TextMate Grammar File

**File**: `integrations/vscode/syntaxes/plcopen-xml.tmLanguage.json`

**Scope name**: `source.plcopen-xml`

**Required patterns**:

| Pattern | Scope | Purpose |
|---------|-------|---------|
| XML declaration | `meta.tag.preprocessor.xml` | `<?xml ... ?>` |
| XML comments | `comment.block.xml` | `<!-- ... -->` |
| CDATA sections | `string.unquoted.cdata.xml` | `<![CDATA[ ... ]]>` |
| Element tags | `entity.name.tag.xml` | Opening/closing tags |
| Attributes | `entity.other.attribute-name.xml` | Attribute names |
| Attribute values | `string.quoted.double.xml` | `"value"` |
| Namespaces | `entity.name.tag.namespace.xml` | `xmlns:...` |

**PLC Open-specific highlighting**:

| Element | Scope | Examples |
|---------|-------|----------|
| Project structure | `entity.name.tag.structure.plcopen` | `project`, `fileHeader`, `contentHeader`, `types` |
| POU definitions | `entity.name.tag.pou.plcopen` | `pou`, `interface`, `body` |
| POU types | `support.type.pou.plcopen` | `pouType="function"`, `pouType="functionBlock"`, `pouType="program"` |
| Variable sections | `entity.name.tag.variable.plcopen` | `inputVars`, `outputVars`, `inOutVars`, `localVars` |
| Data types | `entity.name.tag.datatype.plcopen` | `dataTypes`, `dataType`, `baseType`, `type` |
| Type references | `storage.type.plcopen` | `BOOL`, `INT`, `REAL`, `derived` |
| SFC elements | `entity.name.tag.sfc.plcopen` | `step`, `transition`, `action`, `actionBlock` |
| Body languages | `entity.name.tag.body.plcopen` | `ST`, `SFC`, `FBD`, `LD`, `IL` |
| Names/identifiers | `entity.name.plcopen` | `name="..."` attribute values |

#### 1.2 Create Language Configuration

**File**: `integrations/vscode/decl/plcopen-xml-language-configuration.json`

```json
{
    "comments": {
        "blockComment": ["<!--", "-->"]
    },
    "brackets": [
        ["<", ">"],
        ["[", "]"],
        ["(", ")"]
    ],
    "autoClosingPairs": [
        { "open": "<", "close": ">", "notIn": ["string", "comment"] },
        { "open": "\"", "close": "\"", "notIn": ["string"] },
        { "open": "'", "close": "'", "notIn": ["string"] },
        { "open": "<!--", "close": "-->", "notIn": ["string"] },
        { "open": "<![CDATA[", "close": "]]>", "notIn": ["string", "comment"] }
    ],
    "surroundingPairs": [
        ["<", ">"],
        ["\"", "\""],
        ["'", "'"]
    ],
    "folding": {
        "markers": {
            "start": "^\\s*<(?!\\?|!--|!\\[CDATA\\[|/)[^/>]*>",
            "end": "^\\s*</[^>]+>"
        }
    }
}
```

### Phase 2: VS Code Extension Integration

#### 2.1 Update package.json

Add new language and grammar registrations:

```json
{
  "languages": [
    {
      "id": "plcopen-xml",
      "aliases": ["PLCopen XML", "IEC 61131-3 XML", "TC6 XML"],
      "extensions": [],
      "filenames": [],
      "firstLine": "^<\\?xml[^>]*\\?>\\s*<project\\s+xmlns=\"http://www\\.plcopen\\.org/xml/tc6",
      "configuration": "./decl/plcopen-xml-language-configuration.json"
    }
  ],
  "grammars": [
    {
      "language": "plcopen-xml",
      "scopeName": "source.plcopen-xml",
      "path": "./syntaxes/plcopen-xml.tmLanguage.json"
    }
  ]
}
```

**Note**: We use `firstLine` pattern detection rather than `.xml` extension to avoid conflicting with other XML files. Users can also manually set language mode.

#### 2.2 File Association Strategy

**Option A: First-line detection only** (recommended)
- Uses `firstLine` regex to detect PLC Open namespace
- No extension conflicts
- Requires XML declaration + project element to trigger

**Option B: Add manual association command**
- Add command "IronPLC: Set language to PLCopen XML"
- Allows explicit user control

### Phase 3: Embedded ST Highlighting

Embed the existing ST grammar within `<xhtml>` blocks to highlight Structured Text code inside the XML.

#### 3.1 Grammar Embedding Pattern

In `plcopen-xml.tmLanguage.json`, add injection for ST content:

```json
{
  "begin": "(<)(xhtml)(\\s+xmlns=\"http://www\\.w3\\.org/1999/xhtml\")?(>)",
  "end": "(</)(xhtml)(>)",
  "contentName": "meta.embedded.block.st",
  "patterns": [
    { "include": "source.61131-3-st" }
  ]
}
```

This allows ST code within `<ST><xhtml>...</xhtml></ST>` to be highlighted using the existing ST grammar.

#### 3.2 Alternative: Injection Grammar

Create a separate injection grammar to add ST highlighting:

**File**: `integrations/vscode/syntaxes/plcopen-xml-st-injection.json`

```json
{
  "scopeName": "plcopen-xml.st-injection",
  "injectionSelector": "L:source.plcopen-xml",
  "patterns": [...]
}
```

Register in package.json:

```json
{
  "grammars": [
    {
      "scopeName": "plcopen-xml.st-injection",
      "path": "./syntaxes/plcopen-xml-st-injection.json",
      "injectTo": ["source.plcopen-xml"]
    }
  ]
}
```

### Phase 4: Testing and Validation

#### 4.1 Manual Testing Checklist

- [ ] XML declaration highlighted correctly
- [ ] PLC Open-specific elements use distinct colors
- [ ] Attributes and values highlighted
- [ ] Embedded ST code in `<xhtml>` blocks highlighted
- [ ] Comments highlighted
- [ ] Folding works for major elements
- [ ] Auto-closing pairs work
- [ ] File detection triggers on valid PLC Open XML files
- [ ] File detection does NOT trigger on non-PLC Open XML files

#### 4.2 Test Files

Use existing test file: `compiler/plc2x/resources/test/simple.xml`

Create additional test files for:
- Complex data type declarations (enum, array, struct)
- Multiple POUs with different body types
- SFC with steps and transitions
- Configuration elements

### Phase 5: Documentation (Optional)

#### 5.1 Update User Documentation

If deemed necessary, document the new syntax highlighting in the IronPLC documentation site.

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `integrations/vscode/syntaxes/plcopen-xml.tmLanguage.json` | Create | TextMate grammar |
| `integrations/vscode/decl/plcopen-xml-language-configuration.json` | Create | Editor configuration |
| `integrations/vscode/package.json` | Modify | Register language and grammar |

## Dependencies

- No new npm dependencies required
- Uses existing TextMate grammar format
- Embeds existing ST grammar (`source.61131-3-st`)

## Scope Exclusions

The following are explicitly **out of scope** for this plan:

1. **LSP enhancements** - Semantic tokens, go-to-definition, or diagnostics for XML files
2. **Schema validation** - Real-time validation against tc6_xml_v201.xsd
3. **Code completion** - IntelliSense for XML elements or attributes
4. **Snippets** - Template insertions for PLC Open structures
5. **Outline view** - Document symbols for POUs and types

These could be addressed in future enhancements.

## Risks and Considerations

### Risk 1: XML Extension Conflicts
**Issue**: Registering `.xml` extension would affect all XML files.
**Mitigation**: Use `firstLine` detection instead of extension.

### Risk 2: Embedded ST Position Tracking
**Issue**: Errors in ST code within XML need accurate position mapping.
**Status**: Already handled by compiler's roxmltree integration. Syntax highlighting is display-only and doesn't affect error reporting.

### Risk 3: Grammar Complexity
**Issue**: PLC Open XML has many elements; complete coverage is verbose.
**Mitigation**: Start with high-value elements, expand coverage iteratively.

## Estimated Effort

| Phase | Complexity | Notes |
|-------|------------|-------|
| Phase 1 | Medium | Core grammar creation |
| Phase 2 | Low | Extension configuration |
| Phase 3 | Medium | Embedded grammar requires careful scoping |
| Phase 4 | Low | Manual testing |
| Phase 5 | Low | Optional documentation |

## Success Criteria

1. PLC Open XML files are automatically detected in VS Code (via first-line pattern)
2. XML structure elements are colorized with appropriate semantic scopes
3. PLC Open-specific elements (pou, interface, variable sections) are distinctly highlighted
4. Embedded Structured Text code within `<xhtml>` blocks receives full ST highlighting
5. Editor features (bracket matching, folding, auto-close) work correctly
6. No interference with other XML files or languages
