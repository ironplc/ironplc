//! The shape of a bytecode instruction: what operands follow an opcode byte,
//! how a function body is walked instruction by instruction, and how the
//! instruction set is declared.

use crate::opcode::Opcode;

/// What one opcode byte means: its mnemonic and the operands that follow it.
///
/// Obtained from [`Instruction::decode`], which answers `None` for a byte
/// that is not an assigned opcode. Both facts come from the same
/// `declare_instruction_set!` row, so a byte that has one always has the
/// other.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// The opcode's name, as written in the instruction set.
    pub mnemonic: &'static str,
    /// The operands that follow the opcode byte, in order.
    pub operands: &'static [Operand],
    /// What an instruction's operands do not say -- where an implicit value
    /// comes from, say. Empty for most instructions, whose operands speak for
    /// themselves.
    pub note: &'static str,
}

/// The role one operand plays in an instruction.
///
/// Every operand of every instruction is one of these, and each variant fixes
/// both the operand's byte width -- which is where
/// [`instruction_size`](crate::opcode::instruction_size) comes from -- and
/// its meaning, which is what a disassembler needs in order to render it. A new operand shape is a new variant here, and a renderer that
/// matches on `Operand` without a catch-all then fails to compile until it
/// handles the new shape.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    /// u16 index into the constant pool.
    ConstIndex,
    /// u16 index into the variable table.
    VarIndex,
    /// u16 variable index whose slot holds another variable's index.
    RefIndex,
    /// u16 index into the array descriptor table.
    ArrayDescIndex,
    /// u32 byte offset into the data region.
    DataOffset,
    /// u8 field index within a function block instance.
    FieldIndex,
    /// u16 function block type ID (see [`fb_type`](crate::fb_type)).
    FbTypeId,
    /// u16 ID of the function being called.
    FunctionId,
    /// u16 ID of the built-in being called (see [`builtin`](crate::builtin)).
    BuiltinId,
    /// i16 branch offset, relative to the start of the next instruction.
    JumpOffset,
    /// u8 comparison operator (see [`cmp_op`](crate::opcode::cmp_op)).
    CmpOp,
    /// u16 maximum length of a string, in characters.
    MaxLength,
    /// u8 width of a string's characters, in bytes.
    CharWidth,
    /// u8 count of function block fields to copy.
    NumFields,
    /// u16 variable index where the owning FB type's fields start.
    FieldVarOffset,
    /// u16 variable index where the callee's parameters start.
    ParamVarOffset,
}

impl Operand {
    /// The operand's width in bytes.
    pub const fn width(self) -> usize {
        match self {
            Operand::FieldIndex | Operand::CmpOp | Operand::CharWidth | Operand::NumFields => 1,
            Operand::ConstIndex
            | Operand::VarIndex
            | Operand::RefIndex
            | Operand::ArrayDescIndex
            | Operand::FbTypeId
            | Operand::FunctionId
            | Operand::BuiltinId
            | Operand::JumpOffset
            | Operand::MaxLength
            | Operand::FieldVarOffset
            | Operand::ParamVarOffset => 2,
            Operand::DataOffset => 4,
        }
    }
}

/// One instruction read out of a function body: where it starts, what it is,
/// and the operand bytes that follow its opcode.
///
/// Produced by [`decode_body`], which is the one place a body is walked
/// instruction by instruction. Every pass that reads bytecode back — the
/// stack-balance verifier, the string-encoding verifier, the disassembler —
/// needs the same walk, and a walk that gets an instruction's length wrong
/// reads every later instruction at the wrong offset. Deriving it once, from
/// the operand layout each opcode declares, is what keeps them agreeing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DecodedInstruction<'a> {
    /// Byte offset of the opcode within the function body.
    pub offset: usize,
    /// The opcode byte.
    pub opcode: Opcode,
    /// What that byte means: mnemonic and operand layout.
    pub instruction: Instruction,
    /// The operand bytes following the opcode, exactly as long as the
    /// declared layout requires.
    pub operands: &'a [u8],
}

impl<'a> DecodedInstruction<'a> {
    /// Total encoded size: the opcode byte plus its operands.
    pub fn size(&self) -> usize {
        1 + self.operands.len()
    }

    /// Byte offset of the following instruction.
    pub fn next_offset(&self) -> usize {
        self.offset + self.size()
    }

    /// Each operand in declaration order, paired with its bytes.
    pub fn operands_with_kinds(&self) -> impl Iterator<Item = (Operand, &'a [u8])> + use<'a> {
        let operands = self.operands;
        let mut at = 0usize;
        self.instruction.operands.iter().map(move |&operand| {
            let width = operand.width();
            let bytes = &operands[at..at + width];
            at += width;
            (operand, bytes)
        })
    }

    /// The `n`th operand read as an unsigned integer, whatever its declared
    /// width. `None` when the instruction has no `n`th operand.
    ///
    /// Operands that are not unsigned (a `JumpOffset` is a signed `i16`) need
    /// their own bytes; take those from [`Self::operands_with_kinds`].
    pub fn operand(&self, n: usize) -> Option<u32> {
        let (_, bytes) = self.operands_with_kinds().nth(n)?;
        Some(match bytes.len() {
            1 => bytes[0] as u32,
            2 => u16::from_le_bytes([bytes[0], bytes[1]]) as u32,
            _ => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        })
    }
}

/// A byte a body walk could not read as an instruction.
///
/// Callers differ in what they make of one: the stack-balance verifier
/// rejects the container, the disassembler renders a row saying so, and a
/// pass that only looks for a pattern gives up on the rest of the body. The
/// walk itself takes no position — it reports what it found and lets the
/// caller decide.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DecodeStop {
    /// The byte at this offset is not an assigned opcode. The walk resumes at
    /// the next byte, since an unassigned byte has no length of its own.
    UnknownOpcode { offset: usize, byte: u8 },
    /// A known opcode whose operands run past the end of the body. Nothing
    /// follows it, so the walk ends here.
    Truncated { offset: usize, opcode: Opcode },
}

impl DecodeStop {
    /// Byte offset of the problem.
    pub fn offset(&self) -> usize {
        match self {
            DecodeStop::UnknownOpcode { offset, .. } | DecodeStop::Truncated { offset, .. } => {
                *offset
            }
        }
    }
}

/// Walks a function body from its first byte, yielding one item per
/// instruction.
///
/// Bodies are emitted as a contiguous instruction stream with no interleaved
/// data, so a linear walk from offset 0 enumerates exactly the instructions —
/// and exactly the valid branch targets.
///
/// An [`Err`] item is a byte the walk could not read as an instruction. After
/// an [`DecodeStop::UnknownOpcode`] the walk continues at the following byte;
/// after a [`DecodeStop::Truncated`] it ends. A caller that wants to stop at
/// the first problem says so with `map_while(Result::ok)`; one that wants to
/// report it reads the error.
pub fn decode_body(bytecode: &[u8]) -> BodyDecoder<'_> {
    BodyDecoder { bytecode, pc: 0 }
}

/// The iterator [`decode_body`] returns.
pub struct BodyDecoder<'a> {
    bytecode: &'a [u8],
    pc: usize,
}

impl<'a> Iterator for BodyDecoder<'a> {
    type Item = Result<DecodedInstruction<'a>, DecodeStop>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pc >= self.bytecode.len() {
            return None;
        }

        let offset = self.pc;
        let op = self.bytecode[offset];

        let Some(instruction) = Instruction::decode(op) else {
            self.pc = offset + 1;
            return Some(Err(DecodeStop::UnknownOpcode { offset, byte: op }));
        };

        let size = crate::opcode::instruction_size(op);
        if offset + size > self.bytecode.len() {
            self.pc = self.bytecode.len();
            return Some(Err(DecodeStop::Truncated { offset, opcode: op }));
        }

        self.pc = offset + size;
        Some(Ok(DecodedInstruction {
            offset,
            opcode: op,
            instruction,
            operands: &self.bytecode[offset + 1..offset + size],
        }))
    }
}

/// Declares the instruction set: one row per opcode, giving its name, its
/// `(op_class, type_tag)` encoding, the operands that follow the opcode byte,
/// and optionally a `note` for what those operands leave unsaid.
///
/// A row is the *only* declaration of an opcode. The opcode byte, the
/// mnemonic, the operand layout and -- because the layout gives each
/// operand's width -- [`instruction_size`](crate::opcode::instruction_size)
/// all come from it. Nothing about an opcode is written down twice, so no two
/// facts about one can drift apart,
/// and an opcode cannot come into existence without the layout that a
/// disassembler needs in order to render it.
macro_rules! declare_instruction_set {
    ($(
        $(#[$meta:meta])*
        $name:ident = ($op_class:expr, $type_tag:expr) => [$($operand:ident),* $(,)?]
            $(note $note:literal)?;
    )*) => {
        $(
            $(#[$meta])*
            pub const $name: Opcode = encode_opcode($op_class, $type_tag);
        )*

        impl Instruction {
            /// The instruction starting with the byte `op`, or `None` when
            /// `op` is not an assigned opcode.
            pub fn decode(op: Opcode) -> Option<Instruction> {
                match op {
                    $($name => Some(Instruction {
                        mnemonic: stringify!($name),
                        operands: &[$(Operand::$operand),*],
                        note: concat!("" $(, $note)?),
                    }),)*
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use declare_instruction_set;

#[cfg(test)]
mod tests {
    use std::vec;
    use std::vec::Vec;

    use super::*;
    use crate::opcode;

    /// `STR_INIT data_offset=32, max_len=4, char_width=2` — one operand of
    /// each width the instruction set uses.
    fn str_init() -> Vec<u8> {
        let mut bytes = vec![opcode::STR_INIT];
        bytes.extend_from_slice(&32u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.push(2);
        bytes
    }

    #[test]
    fn decode_body_when_two_instructions_then_yields_both_with_offsets() {
        let mut bytecode = str_init();
        bytecode.push(opcode::RET_VOID);

        let decoded: Vec<_> = decode_body(&bytecode).map(Result::unwrap).collect();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].offset, 0);
        assert_eq!(decoded[0].opcode, opcode::STR_INIT);
        assert_eq!(decoded[0].size(), 8);
        assert_eq!(decoded[0].next_offset(), 8);
        assert_eq!(decoded[1].offset, 8);
        assert_eq!(decoded[1].opcode, opcode::RET_VOID);
    }

    #[test]
    fn operand_when_read_by_index_then_matches_declared_width() {
        let bytecode = str_init();
        let decoded = decode_body(&bytecode).next().unwrap().unwrap();

        assert_eq!(decoded.operand(0), Some(32)); // DataOffset (u32)
        assert_eq!(decoded.operand(1), Some(4)); // MaxLength (u16)
        assert_eq!(decoded.operand(2), Some(2)); // CharWidth (u8)
        assert_eq!(decoded.operand(3), None);
    }

    #[test]
    fn operands_with_kinds_when_walked_then_pairs_each_kind_with_its_bytes() {
        let bytecode = str_init();
        let decoded = decode_body(&bytecode).next().unwrap().unwrap();

        let kinds: Vec<_> = decoded.operands_with_kinds().collect();
        assert_eq!(
            kinds,
            vec![
                (Operand::DataOffset, &[32u8, 0, 0, 0][..]),
                (Operand::MaxLength, &[4u8, 0][..]),
                (Operand::CharWidth, &[2u8][..]),
            ]
        );
    }

    #[test]
    fn decode_body_when_unassigned_byte_then_reports_it_and_resumes() {
        // An unassigned byte has no length of its own, so the walk continues
        // at the next byte rather than ending -- which is what lets a
        // disassembler render every row of a corrupt body.
        let bytecode = vec![0xFE, opcode::RET_VOID];

        let decoded: Vec<_> = decode_body(&bytecode).collect();

        assert_eq!(
            decoded[0],
            Err(DecodeStop::UnknownOpcode {
                offset: 0,
                byte: 0xFE
            })
        );
        assert_eq!(decoded[1].unwrap().opcode, opcode::RET_VOID);
    }

    #[test]
    fn decode_body_when_operands_run_past_the_end_then_truncated_and_ends() {
        let mut bytecode = str_init();
        bytecode.truncate(bytecode.len() - 1);

        let decoded: Vec<_> = decode_body(&bytecode).collect();

        assert_eq!(
            decoded,
            vec![Err(DecodeStop::Truncated {
                offset: 0,
                opcode: opcode::STR_INIT
            })]
        );
    }

    #[test]
    fn decode_stop_when_asked_for_offset_then_reports_where_it_stopped() {
        assert_eq!(
            DecodeStop::UnknownOpcode {
                offset: 7,
                byte: 0xFE
            }
            .offset(),
            7
        );
        assert_eq!(
            DecodeStop::Truncated {
                offset: 9,
                opcode: opcode::STR_INIT
            }
            .offset(),
            9
        );
    }

    #[test]
    fn decode_body_when_empty_then_yields_nothing() {
        assert_eq!(decode_body(&[]).count(), 0);
    }
}
