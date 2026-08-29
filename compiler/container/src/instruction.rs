//! The shape of a bytecode instruction: what operands follow an opcode byte,
//! and how the instruction set is declared.

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

/// Declares the instruction set: one row per opcode, giving its name, its
/// `(op_class, type_tag)` encoding, and the operands that follow the opcode
/// byte.
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
        $name:ident = ($op_class:expr, $type_tag:expr) => [$($operand:ident),* $(,)?];
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
                    }),)*
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use declare_instruction_set;
