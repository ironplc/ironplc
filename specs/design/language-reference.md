# Language Reference Manual Design

Date: 2026-03-01

## Goal

Create a comprehensive IEC 61131-3 language reference manual for IronPLC covering the full standard scope with clear support status for each element. Individual pages per element enable direct linking from compiler error messages.

## Audience

Reference manual for lookup, not teaching. Readers have some programming familiarity. Standard compliance noted where relevant. Explanatory/tutorial content lives in other doc sections.

## Structure

Two new top-level sections under `reference/`:

- **`language/`** — Language syntax and semantics (shared elements + language-specific subsections)
- **`standard-library/`** — Standard functions and function blocks (shared across all languages)

### Shared vs Language-Specific

Elements shared across all IEC 61131-3 languages (data types, variables, POUs, configuration) live at the top level under `language/`. Language-specific notation (ST statements, LD contacts/coils) lives under language-specific subdirectories. Future languages (FBD, SFC, IL) add new subdirectories without touching existing pages.

## Directory Layout

```
reference/
├── language/
│   ├── index.rst                    (hub: language overview + support matrix)
│   ├── data-types/
│   │   ├── index.rst                (hub: type system overview)
│   │   ├── bool.rst
│   │   ├── sint.rst
│   │   ├── int.rst
│   │   ├── dint.rst
│   │   ├── lint.rst
│   │   ├── usint.rst
│   │   ├── uint.rst
│   │   ├── udint.rst
│   │   ├── ulint.rst
│   │   ├── real.rst
│   │   ├── lreal.rst
│   │   ├── byte.rst
│   │   ├── word.rst
│   │   ├── dword.rst
│   │   ├── lword.rst
│   │   ├── string.rst
│   │   ├── wstring.rst
│   │   ├── time.rst
│   │   ├── date.rst
│   │   ├── time-of-day.rst
│   │   ├── date-and-time.rst
│   │   ├── enumerated-types.rst
│   │   ├── subrange-types.rst
│   │   ├── array-types.rst
│   │   └── structure-types.rst
│   ├── variables/
│   │   ├── index.rst                (hub: variable system overview)
│   │   ├── declarations.rst
│   │   ├── io-qualifiers.rst        (%I, %Q, %M, AT)
│   │   ├── scope.rst               (VAR, VAR_GLOBAL, VAR_EXTERNAL)
│   │   ├── retention.rst           (RETAIN, NON_RETAIN, CONSTANT)
│   │   └── initial-values.rst
│   ├── pous/
│   │   ├── index.rst                (hub: program organization)
│   │   ├── program.rst
│   │   ├── function.rst
│   │   ├── function-block.rst
│   │   ├── configuration.rst
│   │   ├── resource.rst
│   │   └── task.rst
│   ├── structured-text/
│   │   ├── index.rst                (hub: ST overview + operator precedence)
│   │   ├── assignment.rst
│   │   ├── if.rst
│   │   ├── case.rst
│   │   ├── for.rst
│   │   ├── while.rst
│   │   ├── repeat.rst
│   │   ├── exit.rst
│   │   ├── return.rst
│   │   ├── arithmetic-operators.rst (+, -, *, /, MOD, **)
│   │   ├── comparison-operators.rst (=, <>, <, >, <=, >=)
│   │   ├── logical-operators.rst    (AND, OR, XOR, NOT)
│   │   └── function-call.rst
│   └── ladder-diagram/
│       ├── index.rst                (hub: LD overview)
│       ├── contacts.rst
│       ├── coils.rst
│       ├── rungs.rst
│       └── branches.rst
│
└── standard-library/
    ├── index.rst                    (hub: all functions/FBs at a glance)
    ├── functions/
    │   ├── index.rst                (hub: function categories)
    │   ├── abs.rst
    │   ├── sqrt.rst
    │   ├── ln.rst
    │   ├── log.rst
    │   ├── exp.rst
    │   ├── expt.rst
    │   ├── sin.rst
    │   ├── cos.rst
    │   ├── tan.rst
    │   ├── asin.rst
    │   ├── acos.rst
    │   ├── atan.rst
    │   ├── add.rst
    │   ├── sub.rst
    │   ├── mul.rst
    │   ├── div.rst
    │   ├── mod.rst
    │   ├── gt.rst
    │   ├── ge.rst
    │   ├── eq.rst
    │   ├── le.rst
    │   ├── lt.rst
    │   ├── ne.rst
    │   ├── sel.rst
    │   ├── max.rst
    │   ├── min.rst
    │   ├── limit.rst
    │   ├── mux.rst
    │   ├── shl.rst
    │   ├── shr.rst
    │   ├── rol.rst
    │   ├── ror.rst
    │   ├── len.rst
    │   ├── left.rst
    │   ├── right.rst
    │   ├── mid.rst
    │   ├── concat.rst
    │   ├── insert.rst
    │   ├── delete.rst
    │   ├── replace.rst
    │   ├── find.rst
    │   └── type-conversions.rst     (*_TO_* grouped on one page)
    └── function-blocks/
        ├── index.rst                (hub: FB categories)
        ├── ton.rst
        ├── tof.rst
        ├── tp.rst
        ├── ctu.rst
        ├── ctd.rst
        ├── ctud.rst
        ├── r-trig.rst
        ├── f-trig.rst
        ├── sr.rst
        └── rs.rst
```

~85 individual pages + ~10 hub/index pages.

## Page Templates

### Data Type Page

Metadata table with: Size, Range, Default value, IEC 61131-3 section, Support status. Followed by Literals section showing literal syntax, then See Also linking to related types.

### Statement Page (Structured Text)

Metadata table with: IEC 61131-3 section, Support status. Followed by Syntax (BNF), Example (complete code), Related Problem Codes (links to P#### pages), See Also.

### Standard Function Page

Metadata table with: IEC 61131-3 section, Support status. Followed by Signatures table (numbered overloads showing input types, return type, and per-overload support status), Description, Example, See Also.

### Standard Function Block Page

Metadata table with: IEC 61131-3 section, Support status. Followed by Inputs table (name, type, description), Outputs table, Behavior description, Example, See Also.

## Hub Pages

- **`language/index.rst`** — Support status matrix, links to subsections
- **`standard-library/index.rst`** — Two tables (functions, FBs) with name, description, status
- **Category hubs** (e.g., `data-types/index.rst`) — Table of all elements with name, description, status; hidden toctree

## Cross-Linking Strategy

- Problem code pages (P####) gain "See Also" links to language reference pages
- Language reference pages link back via "Related Problem Codes" sections
- Standard library pages link to data type pages for parameter types
- Hub pages link to all children

## Future Extensibility

- New IEC 61131-3 languages (FBD, SFC) add a subdirectory under `language/`
- Vendor dialect features can add an "Availability" row to the metadata table (e.g., "Siemens S7 dialect, requires `--dialect s7`")
- New standard functions/FBs add individual pages under the appropriate directory
