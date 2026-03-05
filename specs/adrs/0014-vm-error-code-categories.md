# VM Error Code Categories

status: proposed
date: 2026-03-04

## Context and Problem Statement

The compiler and editor already use categorized error codes where the thousands digit indicates the error category: P0xxx for parsing, P4xxx for semantic analysis, P6xxx for file system errors, P9xxx for internal errors. The VS Code extension uses E####. The VM runtime needs an equivalent system so that users can look up specific errors in the documentation.

The VM is a command-line executable, so its primary error reporting mechanism is the combination of a process exit code and a message on stderr. Should the V-code (the documentation-facing identifier) be the same as the exit code, or should they be separate?

## Decision Drivers

* **Unix exit codes are 0–255 (u8)** — thousands-digit codes like 4001 cannot be used directly as exit codes
* **Consistency across components** — the same logical categories (execution errors, file system errors, internal errors) should have the same first digit across compiler (P-prefix) and VM (V-prefix)
* **The VM will grow** — new trap types, new CLI modes, and new IO conditions will be added over time; the numbering scheme must not impose a hard ceiling on the number of distinct errors
* **Scripts need category-level branching** — CI pipelines and automation need to distinguish "the user's program faulted" from "the file was not found" from "the VM has a bug", but rarely need per-error granularity
* **Users need specific lookup** — a user who sees an error should be able to search for its code and find a documentation page explaining the cause and fix

## Considered Options

* V-code = exit code, with hundreds-digit categories (V0001–V0249) — the V-code number is the exit code
* Category-level exit codes (1, 2, 3) with thousands-digit V-codes (V4xxx, V6xxx, V9xxx) in stderr — the V-code is in the message only
* Flat sequential numbering (V0001–V0017) with no categories — all errors exit with code 1

## Decision Outcome

Chosen option: "Category-level exit codes with thousands-digit V-codes", because it preserves the compiler's category convention exactly while avoiding the 255-code ceiling.

Each error has two identifiers:

1. **V-code** (e.g., V4001) — printed to stderr, used for documentation lookup. Uses thousands-digit categories matching the compiler. No numeric ceiling.
2. **Exit code** (1, 2, or 3) — indicates the error *category* only. Used by scripts to branch.

### Category ranges

| V-Code Range | Exit Code | Category | Compiler Parallel |
|-------------|-----------|----------|------------------|
| V4001–V4999 | 1 | Runtime execution errors (user's program faulted) | P4xxx (semantic analysis) |
| V6001–V6999 | 2 | File system / IO errors | P6xxx (file system) |
| V9001–V9999 | 3 | Internal VM errors (compiler or VM bug) | P9xxx (internal) |

The first digit of the V-code carries the same meaning as the first digit of the corresponding P-code: 4 = the user's code did something wrong, 6 = a file system operation failed, 9 = something is wrong with the tooling itself.

### Error message format

```
Error: V4001 - VM trap: divide by zero (task 0, instance 0)
Error: V6001 - Unable to open /path/to/file.iplc: No such file or directory
Error: V9001 - VM trap: stack overflow (task 0, instance 0)
```

The format is `{v_code} - {message}`, matching the editor's `formatProblem` pattern.

### Consequences

* Good, because V-code numbering matches the compiler exactly — same first digit, same meaning — making the error code system learnable across components
* Good, because there is unlimited room for new codes within each category (up to 999 per category)
* Good, because scripts can branch on category via exit code (1 vs 2 vs 3) without parsing stderr
* Good, because the exit code contract is stable — adding new V-codes never changes the set of possible exit codes
* Bad, because the exit code alone does not identify the specific error — but in practice, scripts rarely need per-error granularity; the category is sufficient for automation, and the V-code in the message serves human lookup
* Neutral, because other tools (rustc, gcc, clang) also use a single non-zero exit code for all errors, with structured codes in the message — this is a well-established pattern

## Pros and Cons of the Options

### V-code = Exit Code (hundreds-digit categories)

The V-code number is the exit code. V0001 = exit code 1. Categories use hundreds-digit ranges (1–49, 100–149, 200–249).

* Good, because there is a single identifier — no indirection between what the user sees and what `$?` returns
* Bad, because each category is limited to ~49 codes — this is a hard ceiling that the VM may outgrow
* Bad, because the first digit does not match the compiler's convention (V02xx ≠ P9xxx), breaking cross-component consistency

### Flat Sequential Numbering

All errors are numbered V0001–V0017 with no category structure. All errors exit with code 1.

* Good, because the scheme is simple
* Bad, because there is no categorical structure — users cannot tell at a glance whether an error is their fault, a file issue, or a VM bug
* Bad, because exit code 1 for all errors prevents scripts from distinguishing categories

## More Information

### Why the compiler uses thousands-digit categories

The compiler's P-code ranges were chosen to group logically related errors and leave room for growth within each group. The same reasoning applies to the VM: grouping errors by category (execution, IO, internal) helps users understand what went wrong before they even read the documentation page.

### Relationship to ADR-0005 (Safety-First Design)

Internal VM errors (V9xxx) represent violations of invariants that the compiler and bytecode verifier should prevent. Their existence as a separate category reinforces the defense-in-depth principle from ADR-0005: even if the verifier has a bug, the VM detects and reports the violation with a specific code rather than silently corrupting state.
