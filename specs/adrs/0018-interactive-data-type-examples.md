# Interactive Playground Examples for Elementary Data Type Pages

status: proposed
date: 2026-03-08

## Context and Problem Statement

The IronPLC documentation site integrates an interactive playground (`playground.ironplc.com`) that lets users edit and run IEC 61131-3 code directly in the browser. The standard library reference and structured text pages already embed playground examples using the `playground-with-program` directive, but the 15 elementary data type reference pages (BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL, LREAL, BYTE, WORD, DWORD, LWORD) only show static literal syntax in `code-block` directives. Users can see *what* to type (`INT#42`) but not *how* the type behaves in a running program.

Should we add interactive playground examples to the elementary data type pages, and if so, what should those examples demonstrate?

## Decision Drivers

* **Learning by doing** — research on programming education consistently shows that editable, runnable examples produce better learning outcomes than static code listings; a reader who can change `INT#42` to `INT#-42` and see the result immediately builds intuition faster than one who reads a range table
* **Consistency across documentation** — the standard library pages (e.g., ABS, ADD, MUL) already use `playground-with-program` examples; a user who follows a cross-reference from `ABS` to the `DINT` type page expects the same interactive experience, and the gap breaks the documentation's internal consistency
* **Newcomer onboarding** — IEC 61131-3 newcomers coming from C, Python, or JavaScript encounter unfamiliar syntax (typed literals, VAR blocks, `:=` assignment); a runnable example that shows declaration-through-use in a single view eliminates the "how do I even try this?" barrier without requiring IronPLC installation
* **Discoverability of type behaviors** — static documentation cannot demonstrate edge cases (what happens near the range boundary? how does floating-point precision manifest?); an interactive example invites experimentation that static text discourages
* **Minimal maintenance cost** — the `playground-with-program` directive and playground infrastructure already exist; adding examples to existing pages requires only RST content, no new code

## Considered Options

* **Add interactive examples** — embed a `playground-with-program` example on each supported elementary type page, demonstrating the type's defining characteristic
* **Keep static-only** — leave the pages as they are, with only literal syntax in `code-block` directives
* **Link to external playground** — add a "Try it in the playground" link instead of embedding an example

## Decision Outcome

Chosen option: "Add interactive examples", because the infrastructure already exists, the cost is purely content authoring, and the benefit — consistent, interactive documentation across all supported types — directly serves the project's goal of being the most accessible IEC 61131-3 reference.

### Example design principles

Each example follows four principles that ensure the examples are useful without bloating the reference pages:

1. **One concept per type family** — each example demonstrates the defining characteristic of its type family, not a generic "assign and read" pattern:
   - *Integer types* (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT): arithmetic operations, because integers exist to compute with
   - *Floating-point types* (REAL, LREAL): scaling/conversion, because precision and range are what distinguish floats from integers
   - *Bit string types* (BYTE, WORD, DWORD, LWORD): bitwise masking, because bit-level manipulation is the reason these types exist separately from unsigned integers
   - *BOOL*: logical combination (AND/OR), because boolean logic is the foundation of PLC control flow

2. **Minimal but complete** — each example uses 2–3 variables and 2–4 statements. This is enough to show declaration, assignment, and a meaningful operation without overwhelming the compact reference format. Users who want more complex examples can find them in the structured text and standard library pages.

3. **Typed literals** — examples use typed literal syntax (e.g., `SINT#10`, `REAL#1.8`) rather than untyped literals (e.g., `10`, `1.8`). This reinforces the literal format documented in the Literals section immediately above the example, creating a natural reading flow: "here's the syntax, now here's how you use it."

4. **Practical relevance** — variable names and operations reflect real PLC programming scenarios (temperature scaling, bit masking for status registers, batch counting) rather than abstract math (`x := a + b`). This helps readers connect the type to its typical use case.

### Why these specific examples

| Type family | Example pattern | Why this pattern |
|---|---|---|
| BOOL | `run := sensor AND enabled` | Boolean logic gates are the most fundamental PLC operation; AND/OR with named signals mirrors ladder logic thinking |
| Signed integers | Addition/multiplication with domain names (`temperature + offset`, `count * batch_size`) | Arithmetic is the primary use; domain-specific names show readers where these types appear in real programs |
| Unsigned integers | Addition with domain names (`position + step_size`, `total_units + batch`) | Same rationale as signed; unsigned types appear in counting, addressing, and accumulation scenarios |
| Floating-point | Multiplication for unit conversion (`raw_temp * scale`) | Scaling and conversion are the dominant use of floats in PLC programs; this also naturally demonstrates precision |
| Bit strings | AND/OR masking (`flags AND mask`) | Bit masking is the defining operation that distinguishes bit strings from unsigned integers; status register patterns are universally recognizable |

### Consequences

* Good, because all 15 supported elementary type pages now have the same interactive experience as the standard library pages — no more inconsistency
* Good, because newcomers can experiment with each type immediately without installing IronPLC
* Good, because the examples reinforce the literal syntax documented on the same page
* Good, because the examples use domain-relevant variable names that connect types to real PLC use cases
* Good, because no new code or infrastructure is needed — the `playground-with-program` directive handles everything
* Bad, because each example adds ~10 lines to the RST file, making the reference pages slightly longer
* Bad, because the examples require the playground service to be available — if the playground is down, the iframes show empty
* Neutral, because the examples are intentionally minimal and do not replace tutorials or guides — they are a stepping stone, not a destination

### Confirmation

Verify that:
1. Each of the 15 supported elementary type pages has a `playground-with-program` example
2. Each example uses typed literals matching the Literals section on the same page
3. Each example compiles and runs successfully in the IronPLC playground
4. The data types index page includes a tip about interactive examples (matching the standard library index pattern)

## Pros and Cons of the Options

### Add Interactive Examples (chosen)

Embed a `playground-with-program` example on each supported elementary type page.

* Good, because users can immediately experiment with each type
* Good, because the documentation is internally consistent (all reference pages use the same format)
* Good, because the infrastructure cost is zero — the directive and playground already exist
* Good, because examples serve as implicit integration tests for the playground's type support
* Bad, because the playground service is a runtime dependency for the documentation experience
* Bad, because examples may need updating if the playground's supported syntax changes

### Keep Static-Only

Leave the pages with only `code-block` literal examples.

* Good, because the pages are simple and self-contained
* Good, because there is no dependency on the playground service
* Bad, because users cannot experiment without installing IronPLC
* Bad, because the documentation is inconsistent — function pages have interactive examples but type pages do not
* Bad, because static literal syntax (`INT#42`) does not show how to declare, assign, or operate on the type

### Link to External Playground

Add a "Try it in the playground" link instead of embedding.

* Good, because the reference page stays compact
* Good, because the playground loads only when the user chooses to visit it
* Bad, because the extra click significantly reduces engagement — most users will not follow the link
* Bad, because the linked playground opens without a pre-loaded example, so the user must write code from scratch
* Bad, because there is no connection between the type being documented and the playground experience — the user must figure out what to type

## More Information

### Relationship to existing playground usage

The playground is already embedded in 48+ standard library function pages and several structured text reference pages. The elementary data type pages are the last major section of the supported reference documentation without interactive examples. After this change, every page that documents a supported language feature will have a runnable example.

### Why not wait for unsupported types?

The unsupported type pages (STRING, WSTRING, DATE, TIME_OF_DAY, DATE_AND_TIME) do not get examples because the playground cannot run code using those types. Adding examples to the supported types now does not create a commitment to add examples to unsupported types — those will get examples when their type support ships.
