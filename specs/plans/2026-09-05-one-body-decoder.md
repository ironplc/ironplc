# Decode a function body in one place

## Goal

Four places walk a function body instruction by instruction, each re-deriving
instruction lengths from the opcode tables. A walk that gets one length wrong
reads every later instruction at the wrong offset, so this is exactly the kind
of thing that should exist once.

## The four

| Where | What it does with a byte it cannot read |
|---|---|
| `container/src/verify.rs::instruction_boundaries` | Rejects the container (`UnknownOpcode` / `TruncatedInstruction`) |
| `container/src/verify.rs::method_return_depth` | Stops looking for `RET` |
| `project/src/disassemble.rs::decode_instructions` | Renders an `UNKNOWN(0x..)` or `<truncated>` row and carries on |
| `codegen/src/optimize/rewrite.rs::decode` | Keeps the clamped bytes so it can re-emit them |

They differ *only* in that last column. The walk itself — decode the opcode,
take its declared operand layout, slice the operands, advance — is the same
four times.

## Change

`container/src/instruction.rs` gains the walk, beside the `Instruction` and
`Operand` definitions whose layouts it reads:

```rust
pub struct DecodedInstruction<'a> {
    pub offset: usize,
    pub opcode: Opcode,
    pub instruction: Instruction,
    pub operands: &'a [u8],
}

pub enum DecodeStop {
    UnknownOpcode { offset: usize, byte: u8 },
    Truncated { offset: usize, opcode: Opcode },
}

pub fn decode_body(bytecode: &[u8]) -> BodyDecoder<'_>;  // Iterator<Item = Result<..>>
```

The walk takes no position on a byte it cannot read: it reports one and lets
the caller decide. An unassigned byte is reported and the walk resumes at the
next byte — which is what preserves the disassembler's ability to render every
row of a corrupt body — while a truncated instruction ends it, because nothing
follows.

Two accessors come with it: `operand(n)` reads the nth operand at its declared
width, and `operands_with_kinds()` pairs each operand with its own bytes.

## Callers

- `instruction_boundaries` maps a `DecodeStop` onto the `StackImbalance` it
  already reports.
- `method_return_depth` becomes `decode_body(..).map_while(Result::ok).any(..)`,
  which keeps its "stop at a byte that does not decode" behaviour.
- `decode_instructions` renders one row per item, `Err` included, and
  `format_operand` takes an operand's own bytes rather than an absolute offset
  into the body.

`optimize/rewrite.rs` keeps its own decoder. It holds owned bytes so it can
re-emit them and deliberately keeps the clamped tail of a truncated
instruction; this walk yields an error and no bytes there, so adopting it would
change what that pass emits.

## Prefactor

This *is* the prefactor — it is the shape the string-encoding verifier (a
later PR) needs, extracted first and on its own so it can be reviewed as what
it is: no behaviour change, existing tests unedited.

## Tests

`instruction.rs` unit tests: offsets and sizes across a multi-operand
instruction, `operand(n)` at each declared width, `operands_with_kinds`
pairing, an unassigned byte reported and the walk resuming, a truncated
instruction ending it, and an empty body.

The existing `verify.rs` and `disassemble.rs` tests are the regression check
for the three adopted callers and are not edited.
