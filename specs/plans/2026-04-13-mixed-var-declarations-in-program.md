# Mixed Located/Non-Located Variable Declarations in PROGRAM VAR Blocks

## Goal

Allow `PROGRAM` declarations to contain `VAR` blocks with a mix of located
(e.g. `xStart AT %IX0.0 : BOOL`) and non-located (e.g. `Motor : FB_MotorControl`)
variable declarations.

Currently the parser treats `var_declarations` (non-located) and
`located_var_declarations` (located) as mutually exclusive alternatives per
VAR block. When both kinds appear in the same block, the parser fails.

## Architecture

Add a new PEG rule `program_var_decl` that tries `located_var_decl` first
(unambiguous due to `AT` keyword), then falls back to `var_init_decl`. A
wrapper rule `program_var_declarations` uses this to parse a full
`VAR ... END_VAR` block and separates the results into `Located` and `Var`
`VarDeclarations` variants.

The `program_declaration` rule is updated to try `program_var_declarations`
before the existing `other_var_declarations` and `located_var_declarations`
alternatives. The existing rules remain as fallbacks for blocks that contain
incomplete locations or other specialized forms.

## File map

| File | Change |
|------|--------|
| `compiler/parser/src/parser.rs` | Add `program_var_decl` and `program_var_declarations` rules; update `program_declaration` |
| `compiler/parser/src/tests.rs` | Add tests for mixed VAR blocks in PROGRAM |

## Tasks

- [ ] Add `program_var_decl` rule (tries located then non-located)
- [ ] Add `program_var_declarations` rule (VAR block wrapper)
- [ ] Update `program_declaration` to use new rule
- [ ] Add test: mixed located and non-located in same VAR block
- [ ] Add test: full motor-control-style program parses successfully
- [ ] Run CI pipeline
