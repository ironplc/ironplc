# Rename `--allow-oop-extensions` to `--allow-fb-inheritance`

## Motivation

The flag introduced in #1287 was named `allow_oop_extensions` /
`--allow-oop-extensions`. Two problems (tracked in issue #1298):

1. **"extensions" is an ambiguous umbrella term.** Every vendor `--allow-*`
   flag is an extension, and OOP is a broad area (`EXTENDS`/`IMPLEMENTS`/
   `INTERFACE`/`ABSTRACT` today; `METHOD`/`PROPERTY`, `THIS^`/`SUPER^`, and
   dispatch in later PRs). The name gives no hint which OOP construct it gates
   and leaves no distinct name for future OOP flags.
2. **Vendor-specific framing.** The flag is described as "CODESYS/TwinCAT OOP
   extensions", but this syntax is not exclusive to those two tools. Feature
   descriptions should name the *syntax*; the dialect table is where the
   vendor mapping belongs.

## Decision

- New name: **`allow_fb_inheritance`** / **`--allow-fb-inheritance`**
  (LSP `allowFbInheritance`).
- **Clean break** — no deprecated alias for the old flag (it only just merged).
- **Full consistency** — rename internal test helpers and the
  `oop_extensions.rs` test files too; no `oop_extensions` left in code.
- Reword user-facing descriptions to drop "CODESYS/TwinCAT", keeping the
  "Enabled by `--dialect=rusty`/`--dialect=codesys`" dialect mapping.
- Leave dated `specs/plans/*.md` from #1287 untouched (historical record).
- Do **not** rename the AST `oop` field / `FunctionBlockOop` struct or the
  `VendorExtension`/`ExtensionOrigin`/`rule_unsupported_extension` mechanism —
  those are a separate, still-accurate concept, not the flag.

## Touch points

- `compiler/parser/src/options.rs` — `define_compiler_options!` field, CLI
  string, dialect preset assertions, reword description.
- `compiler/ironplc-cli/bin/main.rs` — clap arg, `compiler_options()` overlay,
  reword doc comment.
- `compiler/ironplc-cli/src/lsp.rs` — `allowFbInheritance` key + test name.
- `compiler/mcp/src/feature_flag_conformance.rs` — fixture `key`.
- `compiler/parser/src/{token.rs,parser.rs}` — comment references.
- `compiler/parser/src/tests/{mod.rs,oop_extensions.rs→fb_inheritance.rs,common.rs}`.
- `compiler/plc2plc/src/tests/{mod.rs,oop_extensions.rs→fb_inheritance.rs}`.
- `compiler/analyzer/src/*` — test-helper names in several rule/xform modules.
- `compiler/sources/src/parsers/twincat_parser.rs` — test-helper name.
- `docs/explanation/enabling-dialects-and-features.rst`,
  `docs/reference/compiler/ironplcc.rst`,
  `docs/reference/compiler/source-formats/twincat.rst`.

## Validation

`cd compiler && just` (build, coverage ≥85%, clippy, fmt).
