# Language Reference Manual Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a comprehensive IEC 61131-3 language reference manual with ~95 individual pages organized under `reference/language/` and `reference/standard-library/`, following the design in `docs/plans/2026-03-01-language-reference-design.md`.

**Architecture:** Two new top-level sections under `reference/`: `language/` for syntax and semantics (shared elements + language-specific subsections for ST and LD), and `standard-library/` for standard functions and function blocks. Each element gets its own page using consistent templates. Hub/index pages provide navigation and support-status overviews.

**Tech Stack:** Sphinx (reStructuredText), Furo theme, sphinx_design extension for tables/grids.

---

### Task 1: Create directory structure and wire up navigation

**Files:**
- Create: `docs/reference/language/index.rst`
- Create: `docs/reference/standard-library/index.rst`
- Modify: `docs/reference/index.rst`

**Step 1: Create directories**

```bash
mkdir -p docs/reference/language/data-types
mkdir -p docs/reference/language/variables
mkdir -p docs/reference/language/pous
mkdir -p docs/reference/language/structured-text
mkdir -p docs/reference/language/ladder-diagram
mkdir -p docs/reference/standard-library/functions
mkdir -p docs/reference/standard-library/function-blocks
```

**Step 2: Create `docs/reference/language/index.rst`**

```rst
==================
Language Reference
==================

IronPLC implements the IEC 61131-3 standard for programmable logic controller
programming. This reference covers the language elements shared across all
IEC 61131-3 programming languages and the language-specific syntax for each
supported language.

Support Status
--------------

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Feature Area
     - Status
     - Section
   * - Elementary data types
     - Partial
     - :doc:`data-types/index`
   * - Derived data types
     - Partial
     - :doc:`data-types/index`
   * - Variables and declarations
     - Partial
     - :doc:`variables/index`
   * - Program organization units
     - Partial
     - :doc:`pous/index`
   * - Structured Text
     - Partial
     - :doc:`structured-text/index`
   * - Ladder Diagram
     - Not yet supported
     - :doc:`ladder-diagram/index`

.. toctree::
   :maxdepth: 2
   :hidden:

   data-types/index
   variables/index
   pous/index
   structured-text/index
   ladder-diagram/index
```

**Step 3: Create `docs/reference/standard-library/index.rst`**

```rst
================
Standard Library
================

IronPLC provides the standard functions and function blocks defined by
IEC 61131-3. These are available in all programming languages.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`ABS <functions/abs>`
     - Absolute value
     - Not yet supported
   * - :doc:`SQRT <functions/sqrt>`
     - Square root
     - Not yet supported
   * - :doc:`LN <functions/ln>`
     - Natural logarithm
     - Not yet supported
   * - :doc:`LOG <functions/log>`
     - Base-10 logarithm
     - Not yet supported
   * - :doc:`EXP <functions/exp>`
     - Natural exponential
     - Not yet supported
   * - :doc:`EXPT <functions/expt>`
     - Exponentiation
     - Supported (INT)
   * - :doc:`SIN <functions/sin>`
     - Sine
     - Not yet supported
   * - :doc:`COS <functions/cos>`
     - Cosine
     - Not yet supported
   * - :doc:`TAN <functions/tan>`
     - Tangent
     - Not yet supported
   * - :doc:`ASIN <functions/asin>`
     - Arc sine
     - Not yet supported
   * - :doc:`ACOS <functions/acos>`
     - Arc cosine
     - Not yet supported
   * - :doc:`ATAN <functions/atan>`
     - Arc tangent
     - Not yet supported
   * - :doc:`ADD <functions/add>`
     - Addition
     - Supported (INT)
   * - :doc:`SUB <functions/sub>`
     - Subtraction
     - Supported (INT)
   * - :doc:`MUL <functions/mul>`
     - Multiplication
     - Supported (INT)
   * - :doc:`DIV <functions/div>`
     - Division
     - Supported (INT)
   * - :doc:`MOD <functions/mod>`
     - Modulo
     - Supported (INT)
   * - :doc:`GT <functions/gt>`
     - Greater than
     - Supported (INT)
   * - :doc:`GE <functions/ge>`
     - Greater than or equal
     - Supported (INT)
   * - :doc:`EQ <functions/eq>`
     - Equal
     - Supported (INT)
   * - :doc:`LE <functions/le>`
     - Less than or equal
     - Supported (INT)
   * - :doc:`LT <functions/lt>`
     - Less than
     - Supported (INT)
   * - :doc:`NE <functions/ne>`
     - Not equal
     - Supported (INT)
   * - :doc:`SEL <functions/sel>`
     - Binary selection
     - Not yet supported
   * - :doc:`MAX <functions/max>`
     - Maximum
     - Not yet supported
   * - :doc:`MIN <functions/min>`
     - Minimum
     - Not yet supported
   * - :doc:`LIMIT <functions/limit>`
     - Clamp to range
     - Not yet supported
   * - :doc:`MUX <functions/mux>`
     - Multiplexer
     - Not yet supported
   * - :doc:`SHL <functions/shl>`
     - Shift left
     - Not yet supported
   * - :doc:`SHR <functions/shr>`
     - Shift right
     - Not yet supported
   * - :doc:`ROL <functions/rol>`
     - Rotate left
     - Not yet supported
   * - :doc:`ROR <functions/ror>`
     - Rotate right
     - Not yet supported
   * - :doc:`LEN <functions/len>`
     - String length
     - Not yet supported
   * - :doc:`LEFT <functions/left>`
     - Left substring
     - Not yet supported
   * - :doc:`RIGHT <functions/right>`
     - Right substring
     - Not yet supported
   * - :doc:`MID <functions/mid>`
     - Middle substring
     - Not yet supported
   * - :doc:`CONCAT <functions/concat>`
     - String concatenation
     - Not yet supported
   * - :doc:`INSERT <functions/insert>`
     - String insertion
     - Not yet supported
   * - :doc:`DELETE <functions/delete>`
     - String deletion
     - Not yet supported
   * - :doc:`REPLACE <functions/replace>`
     - String replacement
     - Not yet supported
   * - :doc:`FIND <functions/find>`
     - String search
     - Not yet supported
   * - :doc:`Type conversions <functions/type-conversions>`
     - Type conversion functions
     - Not yet supported

Function Blocks
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`TON <function-blocks/ton>`
     - On-delay timer
     - Not yet supported
   * - :doc:`TOF <function-blocks/tof>`
     - Off-delay timer
     - Not yet supported
   * - :doc:`TP <function-blocks/tp>`
     - Pulse timer
     - Not yet supported
   * - :doc:`CTU <function-blocks/ctu>`
     - Count up
     - Not yet supported
   * - :doc:`CTD <function-blocks/ctd>`
     - Count down
     - Not yet supported
   * - :doc:`CTUD <function-blocks/ctud>`
     - Count up/down
     - Not yet supported
   * - :doc:`R_TRIG <function-blocks/r-trig>`
     - Rising edge detection
     - Not yet supported
   * - :doc:`F_TRIG <function-blocks/f-trig>`
     - Falling edge detection
     - Not yet supported
   * - :doc:`SR <function-blocks/sr>`
     - Set/reset flip-flop
     - Not yet supported
   * - :doc:`RS <function-blocks/rs>`
     - Reset/set flip-flop
     - Not yet supported

.. toctree::
   :maxdepth: 1
   :hidden:

   functions/index
   function-blocks/index
```

**Step 4: Update `docs/reference/index.rst`**

Add the two new sections to the toctree:

```rst
=========
Reference
=========

Technical reference material for IronPLC tools.

.. toctree::
   :maxdepth: 1

   Language <language/index>
   Standard Library <standard-library/index>
   Compiler <compiler/index>
   Runtime <runtime/index>
   Editor Extension <editor/index>
```

**Step 5: Build docs to verify navigation works**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

Expected: Build fails because child pages don't exist yet. That's OK — this task just wires up the skeleton. Later tasks create the child pages.

**Step 6: Commit**

```bash
git add docs/reference/language/ docs/reference/standard-library/ docs/reference/index.rst
git commit -m "docs: add language reference and standard library navigation skeleton"
```

---

### Task 2: Data types — hub page and elementary integer types

**Files:**
- Create: `docs/reference/language/data-types/index.rst`
- Create: `docs/reference/language/data-types/bool.rst`
- Create: `docs/reference/language/data-types/sint.rst`
- Create: `docs/reference/language/data-types/int.rst`
- Create: `docs/reference/language/data-types/dint.rst`
- Create: `docs/reference/language/data-types/lint.rst`
- Create: `docs/reference/language/data-types/usint.rst`
- Create: `docs/reference/language/data-types/uint.rst`
- Create: `docs/reference/language/data-types/udint.rst`
- Create: `docs/reference/language/data-types/ulint.rst`

**Step 1: Create `data-types/index.rst` hub page**

The hub page lists all data types in a table with name, size, description, and support status. It includes a hidden toctree linking to all child pages.

Categories in the table:
- Boolean: BOOL
- Signed integers: SINT, INT, DINT, LINT
- Unsigned integers: USINT, UINT, UDINT, ULINT
- Real numbers: REAL, LREAL
- Bit strings: BYTE, WORD, DWORD, LWORD
- Strings: STRING, WSTRING
- Date and time: TIME, DATE, TIME_OF_DAY, DATE_AND_TIME
- Derived types: Enumerated, Subrange, Array, Structure

**Step 2: Create integer type pages**

Each integer type page follows the data type template:

```rst
====
DINT
====

32-bit signed integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits
   * - **Range**
     - -2,147,483,648 to 2,147,483,647
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block:: iec61131

   DINT#42
   DINT#-100
   DINT#16#1A

See Also
--------

- :doc:`int` — 16-bit signed integer
- :doc:`lint` — 64-bit signed integer
- :doc:`udint` — 32-bit unsigned integer
```

Create pages for: BOOL (1-bit, TRUE/FALSE literals), SINT (8-bit signed, -128..127), INT (16-bit signed, -32768..32767), DINT (32-bit signed), LINT (64-bit signed), USINT (8-bit unsigned, 0..255), UINT (16-bit unsigned, 0..65535), UDINT (32-bit unsigned), ULINT (64-bit unsigned).

Support status: BOOL = Supported, all integer types = Supported.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

Expected: Warnings about missing pages (REAL, LREAL, etc.) but no errors for the pages we created.

**Step 4: Commit**

```bash
git add docs/reference/language/data-types/
git commit -m "docs: add data type hub and elementary integer type reference pages"
```

---

### Task 3: Data types — real, bit string, string, and date/time types

**Files:**
- Create: `docs/reference/language/data-types/real.rst`
- Create: `docs/reference/language/data-types/lreal.rst`
- Create: `docs/reference/language/data-types/byte.rst`
- Create: `docs/reference/language/data-types/word.rst`
- Create: `docs/reference/language/data-types/dword.rst`
- Create: `docs/reference/language/data-types/lword.rst`
- Create: `docs/reference/language/data-types/string.rst`
- Create: `docs/reference/language/data-types/wstring.rst`
- Create: `docs/reference/language/data-types/time.rst`
- Create: `docs/reference/language/data-types/date.rst`
- Create: `docs/reference/language/data-types/time-of-day.rst`
- Create: `docs/reference/language/data-types/date-and-time.rst`

**Step 1: Create real type pages**

REAL: 32-bit IEEE 754 float, LREAL: 64-bit IEEE 754 float. Support status: Not yet supported. Include literal syntax (e.g., `REAL#3.14`, `1.0E+10`).

**Step 2: Create bit string type pages**

BYTE (8-bit), WORD (16-bit), DWORD (32-bit), LWORD (64-bit). Support status: Not yet supported. Literals use `16#FF` hex notation.

**Step 3: Create string type pages**

STRING (single-byte), WSTRING (double-byte/Unicode). Support status: Not yet supported. Literals use `'hello'` and `"hello"`.

**Step 4: Create date/time type pages**

TIME (duration, `T#1s`, `T#100ms`), DATE (`D#2024-01-15`), TIME_OF_DAY (`TOD#14:30:00`), DATE_AND_TIME (`DT#2024-01-15-14:30:00`). Support status: TIME = Not yet supported, others = Not yet supported.

**Step 5: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 6: Commit**

```bash
git add docs/reference/language/data-types/
git commit -m "docs: add real, bit string, string, and date/time type reference pages"
```

---

### Task 4: Data types — derived types

**Files:**
- Create: `docs/reference/language/data-types/enumerated-types.rst`
- Create: `docs/reference/language/data-types/subrange-types.rst`
- Create: `docs/reference/language/data-types/array-types.rst`
- Create: `docs/reference/language/data-types/structure-types.rst`

**Step 1: Create derived type pages**

Each page follows the template but with syntax sections showing declaration syntax instead of literal syntax.

Enumerated types: `TYPE Color : (Red, Green, Blue); END_TYPE`. Support: Partial.
Subrange types: `TYPE Percent : INT (0..100); END_TYPE`. Support: Partial.
Array types: `ARRAY [1..10] OF INT`. Support: Not yet supported.
Structure types: `TYPE Point : STRUCT X: REAL; Y: REAL; END_STRUCT; END_TYPE`. Support: Partial.

Include Related Problem Codes where applicable (e.g., P2001 for structure types).

**Step 2: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 3: Commit**

```bash
git add docs/reference/language/data-types/
git commit -m "docs: add derived type reference pages"
```

---

### Task 5: Variables section

**Files:**
- Create: `docs/reference/language/variables/index.rst`
- Create: `docs/reference/language/variables/declarations.rst`
- Create: `docs/reference/language/variables/io-qualifiers.rst`
- Create: `docs/reference/language/variables/scope.rst`
- Create: `docs/reference/language/variables/retention.rst`
- Create: `docs/reference/language/variables/initial-values.rst`

**Step 1: Create variables hub page**

Table listing all variable topics with status.

**Step 2: Create individual variable pages**

- `declarations.rst`: VAR/END_VAR blocks, multiple declarations, type specification. Support: Supported.
- `io-qualifiers.rst`: %I (input), %Q (output), %M (memory), AT keyword for direct addressing. Support: Partial.
- `scope.rst`: VAR (local), VAR_GLOBAL, VAR_EXTERNAL, VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT. Support: Partial.
- `retention.rst`: RETAIN, NON_RETAIN, CONSTANT qualifiers. Support: Not yet supported.
- `initial-values.rst`: `:= value` initialization syntax. Support: Supported.

Each page uses the statement template (metadata table, syntax, example, related problem codes, see also).

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/language/variables/
git commit -m "docs: add variable declaration reference pages"
```

---

### Task 6: Program organization units section

**Files:**
- Create: `docs/reference/language/pous/index.rst`
- Create: `docs/reference/language/pous/program.rst`
- Create: `docs/reference/language/pous/function.rst`
- Create: `docs/reference/language/pous/function-block.rst`
- Create: `docs/reference/language/pous/configuration.rst`
- Create: `docs/reference/language/pous/resource.rst`
- Create: `docs/reference/language/pous/task.rst`

**Step 1: Create POU hub page**

Table listing PROGRAM, FUNCTION, FUNCTION_BLOCK, CONFIGURATION, RESOURCE, TASK with status.

**Step 2: Create individual POU pages**

- `program.rst`: PROGRAM/END_PROGRAM declaration, variable blocks, body. Support: Supported.
- `function.rst`: FUNCTION/END_FUNCTION, return type, parameters. Support: Partial.
- `function-block.rst`: FUNCTION_BLOCK/END_FUNCTION_BLOCK, instance variables, inputs/outputs. Support: Partial.
- `configuration.rst`: CONFIGURATION/END_CONFIGURATION declaration. Support: Supported.
- `resource.rst`: RESOURCE/END_RESOURCE declaration. Support: Supported.
- `task.rst`: TASK declaration with INTERVAL and PRIORITY. Support: Supported.

Include syntax, example, related problem codes.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/language/pous/
git commit -m "docs: add program organization unit reference pages"
```

---

### Task 7: Structured Text section — hub and statements

**Files:**
- Create: `docs/reference/language/structured-text/index.rst`
- Create: `docs/reference/language/structured-text/assignment.rst`
- Create: `docs/reference/language/structured-text/if.rst`
- Create: `docs/reference/language/structured-text/case.rst`
- Create: `docs/reference/language/structured-text/for.rst`
- Create: `docs/reference/language/structured-text/while.rst`
- Create: `docs/reference/language/structured-text/repeat.rst`
- Create: `docs/reference/language/structured-text/exit.rst`
- Create: `docs/reference/language/structured-text/return.rst`

**Step 1: Create ST hub page**

Overview of Structured Text with operator precedence table and links to all elements.

Operator precedence table (highest to lowest):
1. Parentheses `()`
2. Function calls
3. Negation `-`, `NOT`
4. Exponentiation `**`
5. Multiply `*`, Divide `/`, Modulo `MOD`
6. Add `+`, Subtract `-`
7. Comparison `<`, `>`, `<=`, `>=`
8. Equality `=`, `<>`
9. `AND`, `&`
10. `XOR`
11. `OR`

**Step 2: Create statement pages**

Each follows the statement template (metadata, syntax BNF, description, example, related problem codes, see also).

- `assignment.rst`: `:=` operator, variable := expression. Support: Supported.
- `if.rst`: IF/ELSIF/ELSE/END_IF. Support: Supported.
- `case.rst`: CASE/OF/ELSE/END_CASE with integer and subrange selectors. Support: Supported.
- `for.rst`: FOR/TO/BY/DO/END_FOR. Support: Supported.
- `while.rst`: WHILE/DO/END_WHILE. Support: Supported.
- `repeat.rst`: REPEAT/UNTIL/END_REPEAT. Support: Supported.
- `exit.rst`: EXIT (break from innermost loop). Support: Supported.
- `return.rst`: RETURN (early exit from POU). Support: Supported.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/language/structured-text/
git commit -m "docs: add structured text hub and statement reference pages"
```

---

### Task 8: Structured Text — operators and function calls

**Files:**
- Create: `docs/reference/language/structured-text/arithmetic-operators.rst`
- Create: `docs/reference/language/structured-text/comparison-operators.rst`
- Create: `docs/reference/language/structured-text/logical-operators.rst`
- Create: `docs/reference/language/structured-text/function-call.rst`

**Step 1: Create operator pages**

- `arithmetic-operators.rst`: `+`, `-`, `*`, `/`, `MOD`, `**` (power). Each with syntax, applicable types, and examples. Support: Supported for integer types.
- `comparison-operators.rst`: `=`, `<>`, `<`, `>`, `<=`, `>=`. Support: Supported for integer types.
- `logical-operators.rst`: `AND` / `&`, `OR`, `XOR`, `NOT`. Support: Supported.

**Step 2: Create function call page**

- `function-call.rst`: Calling syntax for standard functions and user functions. Formal (named) parameters vs. positional parameters. FB instance call syntax `instance(IN := value)` and output access `instance.Q`. Related problem codes: P4001. Support: Partial.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/language/structured-text/
git commit -m "docs: add operator and function call reference pages"
```

---

### Task 9: Ladder Diagram section (placeholder pages)

**Files:**
- Create: `docs/reference/language/ladder-diagram/index.rst`
- Create: `docs/reference/language/ladder-diagram/contacts.rst`
- Create: `docs/reference/language/ladder-diagram/coils.rst`
- Create: `docs/reference/language/ladder-diagram/rungs.rst`
- Create: `docs/reference/language/ladder-diagram/branches.rst`

**Step 1: Create LD hub page**

Brief overview of Ladder Diagram. All elements marked "Not yet supported".

**Step 2: Create placeholder element pages**

Each page has the metadata table (IEC 61131-3 section, Support: Not yet supported), a brief description of the element, and a note that support is planned. Include enough information for someone to understand what the element will do.

- `contacts.rst`: Normally open `--| |--` and normally closed `--|/|--` contacts. IEC 61131-3 Section 3.2.
- `coils.rst`: Output coil `--( )--`, set coil `--(S)--`, reset coil `--(R)--`. IEC 61131-3 Section 3.2.
- `rungs.rst`: Horizontal logic lines connecting power rails. IEC 61131-3 Section 3.2.
- `branches.rst`: Parallel and series connections. IEC 61131-3 Section 3.2.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/language/ladder-diagram/
git commit -m "docs: add ladder diagram placeholder reference pages"
```

---

### Task 10: Standard library — function hub and numeric functions

**Files:**
- Create: `docs/reference/standard-library/functions/index.rst`
- Create: `docs/reference/standard-library/functions/abs.rst`
- Create: `docs/reference/standard-library/functions/sqrt.rst`
- Create: `docs/reference/standard-library/functions/ln.rst`
- Create: `docs/reference/standard-library/functions/log.rst`
- Create: `docs/reference/standard-library/functions/exp.rst`
- Create: `docs/reference/standard-library/functions/expt.rst`
- Create: `docs/reference/standard-library/functions/sin.rst`
- Create: `docs/reference/standard-library/functions/cos.rst`
- Create: `docs/reference/standard-library/functions/tan.rst`
- Create: `docs/reference/standard-library/functions/asin.rst`
- Create: `docs/reference/standard-library/functions/acos.rst`
- Create: `docs/reference/standard-library/functions/atan.rst`

**Step 1: Create function hub page**

Table grouping functions by category: Numeric, Arithmetic, Comparison, Selection, Bit String, String, Type Conversion. Each with name, description, status.

**Step 2: Create numeric function pages**

Each follows the standard function template with signatures table showing all overloads. Example for ABS:

```rst
===
ABS
===

Returns the absolute value of a numeric input.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.2
   * - **Support**
     - Not yet supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``SINT``
     - ``SINT``
     - Not yet supported
   * - 2
     - ``INT``
     - ``INT``
     - Not yet supported
   * - 3
     - ``DINT``
     - ``DINT``
     - Not yet supported
   * - 4
     - ``LINT``
     - ``LINT``
     - Not yet supported
   * - 5
     - ``REAL``
     - ``REAL``
     - Not yet supported
   * - 6
     - ``LREAL``
     - ``LREAL``
     - Not yet supported

Description
-----------

Returns the absolute value of *IN*. For signed integer types, the result
of ``ABS`` applied to the most negative value is undefined because
the positive value cannot be represented.

Example
-------

.. code-block:: iec61131

   result := ABS(-42);    (* result = 42 *)
   value := ABS(REAL#-3.14);  (* value = 3.14 *)

See Also
--------

- :doc:`sqrt` — square root
- :doc:`expt` — exponentiation
```

EXPT is the only numeric function with Support = Supported (for INT types).

Trig functions (SIN, COS, TAN, ASIN, ACOS, ATAN) take REAL/LREAL and return REAL/LREAL. All not yet supported.

**Step 3: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 4: Commit**

```bash
git add docs/reference/standard-library/functions/
git commit -m "docs: add function hub and numeric function reference pages"
```

---

### Task 11: Standard library — arithmetic, comparison, and selection functions

**Files:**
- Create: `docs/reference/standard-library/functions/add.rst`
- Create: `docs/reference/standard-library/functions/sub.rst`
- Create: `docs/reference/standard-library/functions/mul.rst`
- Create: `docs/reference/standard-library/functions/div.rst`
- Create: `docs/reference/standard-library/functions/mod.rst`
- Create: `docs/reference/standard-library/functions/gt.rst`
- Create: `docs/reference/standard-library/functions/ge.rst`
- Create: `docs/reference/standard-library/functions/eq.rst`
- Create: `docs/reference/standard-library/functions/le.rst`
- Create: `docs/reference/standard-library/functions/lt.rst`
- Create: `docs/reference/standard-library/functions/ne.rst`
- Create: `docs/reference/standard-library/functions/sel.rst`
- Create: `docs/reference/standard-library/functions/max.rst`
- Create: `docs/reference/standard-library/functions/min.rst`
- Create: `docs/reference/standard-library/functions/limit.rst`
- Create: `docs/reference/standard-library/functions/mux.rst`

**Step 1: Create arithmetic function pages**

ADD, SUB, MUL, DIV, MOD — each with overloads for all integer types, REAL, LREAL, and (for ADD/SUB) TIME. Support: Supported for integer types via operator syntax.

Note: These functions are the functional form of the operators. `ADD(a, b)` is equivalent to `a + b`. Document both forms.

DIV note: Integer division truncates toward zero. Division by zero causes a runtime fault.

**Step 2: Create comparison function pages**

GT, GE, EQ, LE, LT, NE — each with overloads for integer types, REAL, LREAL, STRING, WSTRING, TIME, DATE, TIME_OF_DAY, DATE_AND_TIME. Return BOOL. Support: Supported for integer types.

**Step 3: Create selection function pages**

- SEL: Binary selection `SEL(G, IN0, IN1)` — returns IN0 if G=FALSE, IN1 if G=TRUE. Polymorphic across all types.
- MAX/MIN: Maximum/minimum of two inputs. Polymorphic across numeric types.
- LIMIT: `LIMIT(MN, IN, MX)` — clamps IN to range [MN, MX]. Polymorphic across numeric types.
- MUX: `MUX(K, IN0, IN1, ...)` — selects input K from list. Polymorphic across all types.

All selection functions: Not yet supported.

**Step 4: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 5: Commit**

```bash
git add docs/reference/standard-library/functions/
git commit -m "docs: add arithmetic, comparison, and selection function reference pages"
```

---

### Task 12: Standard library — bit string, string, and type conversion functions

**Files:**
- Create: `docs/reference/standard-library/functions/shl.rst`
- Create: `docs/reference/standard-library/functions/shr.rst`
- Create: `docs/reference/standard-library/functions/rol.rst`
- Create: `docs/reference/standard-library/functions/ror.rst`
- Create: `docs/reference/standard-library/functions/len.rst`
- Create: `docs/reference/standard-library/functions/left.rst`
- Create: `docs/reference/standard-library/functions/right.rst`
- Create: `docs/reference/standard-library/functions/mid.rst`
- Create: `docs/reference/standard-library/functions/concat.rst`
- Create: `docs/reference/standard-library/functions/insert.rst`
- Create: `docs/reference/standard-library/functions/delete.rst`
- Create: `docs/reference/standard-library/functions/replace.rst`
- Create: `docs/reference/standard-library/functions/find.rst`
- Create: `docs/reference/standard-library/functions/type-conversions.rst`

**Step 1: Create bit string function pages**

SHL, SHR (shift left/right), ROL, ROR (rotate left/right). Operate on BYTE, WORD, DWORD, LWORD. Take a shift count parameter N. All not yet supported.

**Step 2: Create string function pages**

- LEN(IN): Returns INT length. Works for STRING and WSTRING.
- LEFT(IN, L): Returns leftmost L characters.
- RIGHT(IN, L): Returns rightmost L characters.
- MID(IN, L, P): Returns L characters starting at position P.
- CONCAT(IN1, IN2): Concatenates two strings.
- INSERT(IN1, IN2, P): Inserts IN2 into IN1 at position P.
- DELETE(IN, L, P): Deletes L characters from IN starting at P.
- REPLACE(IN1, IN2, L, P): Replaces L characters in IN1 with IN2 at P.
- FIND(IN1, IN2): Returns position of IN2 in IN1 (0 if not found).

All not yet supported.

**Step 3: Create type conversions page**

Single grouped page for all `*_TO_*` conversion functions (e.g., INT_TO_REAL, BOOL_TO_INT, DINT_TO_STRING). Include a compatibility matrix table showing which conversions exist. All not yet supported.

**Step 4: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 5: Commit**

```bash
git add docs/reference/standard-library/functions/
git commit -m "docs: add bit string, string, and type conversion function reference pages"
```

---

### Task 13: Standard library — function blocks

**Files:**
- Create: `docs/reference/standard-library/function-blocks/index.rst`
- Create: `docs/reference/standard-library/function-blocks/ton.rst`
- Create: `docs/reference/standard-library/function-blocks/tof.rst`
- Create: `docs/reference/standard-library/function-blocks/tp.rst`
- Create: `docs/reference/standard-library/function-blocks/ctu.rst`
- Create: `docs/reference/standard-library/function-blocks/ctd.rst`
- Create: `docs/reference/standard-library/function-blocks/ctud.rst`
- Create: `docs/reference/standard-library/function-blocks/r-trig.rst`
- Create: `docs/reference/standard-library/function-blocks/f-trig.rst`
- Create: `docs/reference/standard-library/function-blocks/sr.rst`
- Create: `docs/reference/standard-library/function-blocks/rs.rst`

**Step 1: Create function block hub page**

Table grouped by category: Timers (TON, TOF, TP), Counters (CTU, CTD, CTUD), Edge Detection (R_TRIG, F_TRIG), Bistable (SR, RS).

**Step 2: Create timer pages**

Each follows the FB template with inputs table, outputs table, behavior description, example.

- TON: On-delay timer. IN (BOOL), PT (TIME) → Q (BOOL), ET (TIME). When IN is TRUE for PT duration, Q becomes TRUE.
- TOF: Off-delay timer. IN (BOOL), PT (TIME) → Q (BOOL), ET (TIME). Q stays TRUE for PT duration after IN goes FALSE.
- TP: Pulse timer. IN (BOOL), PT (TIME) → Q (BOOL), ET (TIME). Generates a pulse of duration PT on rising edge of IN.

**Step 3: Create counter pages**

- CTU: Count up. CU (BOOL), R (BOOL), PV (INT) → Q (BOOL), CV (INT). Increments CV on rising edge of CU. Q = TRUE when CV >= PV.
- CTD: Count down. CD (BOOL), LD (BOOL), PV (INT) → Q (BOOL), CV (INT). Decrements CV on rising edge of CD. Q = TRUE when CV <= 0.
- CTUD: Count up/down. CU (BOOL), CD (BOOL), R (BOOL), LD (BOOL), PV (INT) → QU (BOOL), QD (BOOL), CV (INT).

**Step 4: Create edge detection pages**

- R_TRIG: Rising edge. CLK (BOOL) → Q (BOOL). Q is TRUE for one scan when CLK transitions FALSE→TRUE.
- F_TRIG: Falling edge. CLK (BOOL) → Q (BOOL). Q is TRUE for one scan when CLK transitions TRUE→FALSE.

**Step 5: Create bistable pages**

- SR: Set-dominant. S1 (BOOL), R (BOOL) → Q1 (BOOL). Set-dominant flip-flop.
- RS: Reset-dominant. S (BOOL), R1 (BOOL) → Q1 (BOOL). Reset-dominant flip-flop.

All function blocks: Not yet supported.

**Step 6: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 7: Commit**

```bash
git add docs/reference/standard-library/function-blocks/
git commit -m "docs: add standard function block reference pages"
```

---

### Task 14: Full build verification and fix any issues

**Files:**
- Possibly modify any `.rst` files with broken references

**Step 1: Run full Sphinx build with strict warnings**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

Expected: Clean build with no warnings or errors.

**Step 2: Fix any broken cross-references**

If the build reports broken references, fix them. Common issues:
- Duplicate section labels (due to `autosectionlabel`)
- Missing toctree entries
- Broken `:doc:` references

**Step 3: Verify navigation**

Open `_build/index.html` and navigate through:
- Reference → Language Reference hub
- Reference → Standard Library hub
- Click through to individual pages
- Verify breadcrumbs and sidebar work correctly

**Step 4: Commit fixes**

```bash
git add docs/
git commit -m "docs: fix build warnings in language reference"
```

---

### Task 15: Update cross-references from explanation pages

**Files:**
- Modify: `docs/explanation/structured-text-basics.rst`

**Step 1: Update references in structured-text-basics.rst**

Line 78 currently references `/reference/compiler/index` for derived types. Update to point to the new language reference pages:

Change: "These are covered in the :doc:`/reference/compiler/index`."
To: "These are covered in the :doc:`/reference/language/data-types/index`."

**Step 2: Build and verify**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

**Step 3: Commit**

```bash
git add docs/explanation/structured-text-basics.rst
git commit -m "docs: update cross-references to point to new language reference"
```

---

### Task 16: Final build and squash into clean commits

**Step 1: Run the full docs build one more time**

Run: `cd /workspaces/ironplc/docs && sphinx-build -a -W -n -b html . _build`

Expected: Clean build, zero warnings.

**Step 2: Review all changes**

Run: `git diff main --stat`

Verify the file list matches expectations (~95 new files under `reference/language/` and `reference/standard-library/`).

**Step 3: Push branch and create PR**

```bash
git push -u origin feature/language-reference
gh pr create --title "Add IEC 61131-3 language reference manual" --body "..."
```
