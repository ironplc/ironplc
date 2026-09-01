//! String-encoding verification for emitted bytecode (verifier rule R0304).
//!
//! Every string value carries its encoding with it — a `char_width` of 1 for
//! `STRING` (Latin-1) or 2 for `WSTRING` (UTF-16LE) — in the data-region
//! header of its slot, in the tag of its constant-pool entry, and in the temp
//! buffer it passes through (ADR-0034, ADR-0035). Every string operation
//! checks those against each other and traps (`V9014`) when they disagree.
//!
//! That trap is unattributable. It fires one scan into the run, names two
//! numbers, and points at no instruction — and, being a trap, it fires on the
//! customer's machine rather than on the machine that built the container.
//! Nothing in a program *should* reach it: an encoding mismatch is not
//! something a program can ask for, it is something codegen has to have
//! emitted wrong.
//!
//! So this pass rejects it where it is attributable. It reads back the widths
//! the bytecode itself declares and checks the operations that require them to
//! agree, at the point where codegen hands a container over — the same place,
//! and for the same reason, as [`verify_stack_balance`](crate::verify).
//!
//! # What it checks
//!
//! Every slot's width comes from the `STR_INIT` that declared it, gathered
//! across all functions because data-region offsets are allocated once for the
//! whole program. Against those:
//!
//! - the two-slot string operations (`FIND_STR`, `CONCAT_STR`, `REPLACE_STR`,
//!   `INSERT_STR`), whose operand slots must share an encoding;
//! - `CMP_STR`, whose two slots arrive as the constants pushed by the two
//!   instructions before it;
//! - a string constant stored straight into a slot, where the constant's pool
//!   tag must match the slot's declared width.
//!
//! # What it does not check
//!
//! Anything whose width is not written down statically: a slot addressed
//! through a computed offset, a value that reaches a store through a temp
//! buffer some builtin produced, a slot whose declarations disagree (which is
//! a reused offset, not a mistake). The pass reports what it can prove and is
//! silent otherwise, so it never turns bytecode it cannot follow into a
//! compiler error.

use std::collections::BTreeMap;
use std::fmt;
use std::vec::Vec;

use crate::char_width::CharWidth;
use crate::code_section::CodeSection;
use crate::constant_pool::ConstantPool;
use crate::container::Container;
use crate::id_types::{ConstantIndex, FunctionId};
use crate::instruction::Instruction;
use crate::opcode::{self, Opcode};

/// R0304: an operation whose operands do not agree on a string encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StringEncodingViolation {
    /// Two data-region slots addressed by one operation declare different
    /// encodings, so the operation traps whichever way it runs.
    SlotDisagreement {
        function_id: FunctionId,
        offset: usize,
        opcode: Opcode,
        left: CharWidth,
        right: CharWidth,
    },
    /// A string constant is stored into a slot declared at the other
    /// encoding — a literal interned at the wrong width for its destination.
    ConstantDisagreement {
        function_id: FunctionId,
        offset: usize,
        constant: CharWidth,
        destination: CharWidth,
    },
}

impl StringEncodingViolation {
    /// The function whose body contains the violation.
    pub fn function_id(&self) -> FunctionId {
        match self {
            StringEncodingViolation::SlotDisagreement { function_id, .. }
            | StringEncodingViolation::ConstantDisagreement { function_id, .. } => *function_id,
        }
    }

    /// Byte offset within that function's body.
    pub fn offset(&self) -> usize {
        match self {
            StringEncodingViolation::SlotDisagreement { offset, .. }
            | StringEncodingViolation::ConstantDisagreement { offset, .. } => *offset,
        }
    }

    /// The `R####` rule code from `bytecode-verifier-rules.md`.
    pub fn rule(&self) -> &'static str {
        "R0304"
    }
}

/// Renders a width the way the runtime names it, so a verifier message and a
/// `V9014` trap message describe the same thing the same way.
fn width_name(width: CharWidth) -> &'static str {
    match width {
        CharWidth::Narrow => "STRING (char_width 1)",
        CharWidth::Wide => "WSTRING (char_width 2)",
    }
}

impl fmt::Display for StringEncodingViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let func = self.function_id();
        let at = self.offset();
        match self {
            StringEncodingViolation::SlotDisagreement {
                opcode,
                left,
                right,
                ..
            } => write!(
                f,
                "function {func} offset {at}: opcode 0x{opcode:02X} addresses one operand as \
                 {} and the other as {}",
                width_name(*left),
                width_name(*right)
            ),
            StringEncodingViolation::ConstantDisagreement {
                constant,
                destination,
                ..
            } => write!(
                f,
                "function {func} offset {at}: a {} constant is stored into a slot declared as {}",
                width_name(*constant),
                width_name(*destination)
            ),
        }
    }
}

/// One decoded instruction of a function body.
struct Decoded<'a> {
    offset: usize,
    opcode: Opcode,
    instruction: Instruction,
    operands: &'a [u8],
}

impl Decoded<'_> {
    /// Reads the `n`th operand of this instruction as an unsigned integer,
    /// whatever its declared width.
    fn operand(&self, n: usize) -> Option<u32> {
        let mut at = 0usize;
        for (index, operand) in self.instruction.operands.iter().enumerate() {
            let width = operand.width();
            if index == n {
                let bytes = self.operands.get(at..at + width)?;
                return Some(match width {
                    1 => bytes[0] as u32,
                    2 => u16::from_le_bytes([bytes[0], bytes[1]]) as u32,
                    _ => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                });
            }
            at += width;
        }
        None
    }
}

/// Verifies string-encoding agreement across every function in `container`.
///
/// Returns the first violation found, scanning functions in directory order
/// and each body from its first instruction.
pub fn verify_string_encoding(container: &Container) -> Result<(), StringEncodingViolation> {
    let widths = declared_slot_widths(&container.code);

    for entry in &container.code.functions {
        let bytecode = container
            .code
            .get_function_bytecode(entry.function_id)
            .unwrap_or_default();
        verify_function(
            entry.function_id,
            bytecode,
            &widths,
            &container.constant_pool,
        )?;
    }
    Ok(())
}

/// The width each data-region slot was initialized at, across the whole
/// program.
///
/// Data-region offsets are allocated once for the program, not per function,
/// so a slot initialized by `INIT` is the same slot the scan function then
/// addresses. An offset declared twice at different widths is recorded as
/// `None` — nothing is claimed about it, and nothing is reported against it.
fn declared_slot_widths(code: &CodeSection) -> BTreeMap<u32, Option<CharWidth>> {
    let mut widths: BTreeMap<u32, Option<CharWidth>> = BTreeMap::new();

    for entry in &code.functions {
        let bytecode = code
            .get_function_bytecode(entry.function_id)
            .unwrap_or_default();
        for decoded in decode_body(bytecode) {
            if decoded.opcode != opcode::STR_INIT {
                continue;
            }
            let (Some(data_offset), Some(width_byte)) = (decoded.operand(0), decoded.operand(2))
            else {
                continue;
            };
            let Ok(width) = CharWidth::from_u8(width_byte as u8) else {
                continue;
            };
            widths
                .entry(data_offset)
                .and_modify(|known| {
                    if *known != Some(width) {
                        *known = None;
                    }
                })
                .or_insert(Some(width));
        }
    }

    widths
}

/// Decodes a function body linearly into its instructions, stopping at the
/// first byte that is not an assigned opcode or whose operands run past the
/// end. Both of those are what `verify_stack_balance` reports; this pass just
/// stops looking.
fn decode_body(bytecode: &[u8]) -> Vec<Decoded<'_>> {
    let mut decoded = Vec::new();
    let mut pc = 0usize;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        let Some(instruction) = Instruction::decode(op) else {
            break;
        };
        let size = opcode::instruction_size(op);
        if pc + size > bytecode.len() {
            break;
        }
        decoded.push(Decoded {
            offset: pc,
            opcode: op,
            instruction,
            operands: &bytecode[pc + 1..pc + size],
        });
        pc += size;
    }

    decoded
}

/// Reports a violation when two known widths differ.
fn widths_agree(
    function_id: FunctionId,
    decoded: &Decoded<'_>,
    left: Option<CharWidth>,
    right: Option<CharWidth>,
) -> Result<(), StringEncodingViolation> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => {
            Err(StringEncodingViolation::SlotDisagreement {
                function_id,
                offset: decoded.offset,
                opcode: decoded.opcode,
                left,
                right,
            })
        }
        _ => Ok(()),
    }
}

fn verify_function(
    function_id: FunctionId,
    bytecode: &[u8],
    widths: &BTreeMap<u32, Option<CharWidth>>,
    constants: &ConstantPool,
) -> Result<(), StringEncodingViolation> {
    let body = decode_body(bytecode);
    let width_of = |offset: u32| widths.get(&offset).copied().flatten();

    for (index, decoded) in body.iter().enumerate() {
        match decoded.opcode {
            // Two data-region slots named as immediates.
            opcode::FIND_STR | opcode::CONCAT_STR | opcode::REPLACE_STR | opcode::INSERT_STR => {
                let (Some(first), Some(second)) = (decoded.operand(0), decoded.operand(1)) else {
                    continue;
                };
                widths_agree(function_id, decoded, width_of(first), width_of(second))?;
            }

            // CMP_STR takes its two slots from the stack. Codegen pushes them
            // as the two instructions before it; when it did not, there is
            // nothing static to check.
            opcode::BUILTIN => {
                if decoded.operand(0) != Some(opcode::builtin::CMP_STR as u32) {
                    continue;
                }
                let (Some(left), Some(right)) = (
                    pushed_constant_offset(&body, index, 2, constants),
                    pushed_constant_offset(&body, index, 1, constants),
                ) else {
                    continue;
                };
                widths_agree(function_id, decoded, width_of(left), width_of(right))?;
            }

            // A string constant stored straight into a slot: the pool tag and
            // the slot's declared width are both known.
            opcode::STR_STORE_VAR => {
                let Some(destination) = decoded.operand(0).and_then(width_of) else {
                    continue;
                };
                let Some(previous) = index.checked_sub(1).and_then(|i| body.get(i)) else {
                    continue;
                };
                if previous.opcode != opcode::LOAD_CONST_STR {
                    continue;
                }
                let Some(constant) = previous
                    .operand(0)
                    .and_then(|i| constants.char_width(ConstantIndex::new(i as u16)).ok())
                else {
                    continue;
                };
                if constant != destination {
                    return Err(StringEncodingViolation::ConstantDisagreement {
                        function_id,
                        offset: decoded.offset,
                        constant,
                        destination,
                    });
                }
            }

            _ => {}
        }
    }

    Ok(())
}

/// The data-region offset pushed `back` instructions before `index`, when that
/// instruction is a `LOAD_CONST_I32` naming an i32 constant.
fn pushed_constant_offset(
    body: &[Decoded<'_>],
    index: usize,
    back: usize,
    constants: &ConstantPool,
) -> Option<u32> {
    let previous = body.get(index.checked_sub(back)?)?;
    if previous.opcode != opcode::LOAD_CONST_I32 {
        return None;
    }
    let pool_index = ConstantIndex::new(previous.operand(0)? as u16);
    let value = constants.get_i32(pool_index).ok()?;
    u32::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use std::vec;
    use std::vec::Vec;

    use super::*;
    use crate::builder::ContainerBuilder;
    use crate::opcode::builtin;

    /// A `WSTRING[4]` slot at offset 0 and a `STRING[4]` slot after it.
    const WIDE_SLOT: u32 = 0;
    const NARROW_SLOT: u32 = 32;

    fn str_init(data_offset: u32, char_width: CharWidth) -> Vec<u8> {
        let mut bytes = vec![opcode::STR_INIT];
        bytes.extend_from_slice(&data_offset.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.push(char_width.byte_width());
        bytes
    }

    fn two_offset_op(op: Opcode, first: u32, second: u32) -> Vec<u8> {
        let mut bytes = vec![op];
        bytes.extend_from_slice(&first.to_le_bytes());
        bytes.extend_from_slice(&second.to_le_bytes());
        bytes
    }

    fn one_operand_op(op: Opcode, operand: u32) -> Vec<u8> {
        let mut bytes = vec![op];
        bytes.extend_from_slice(&operand.to_le_bytes());
        bytes
    }

    fn load_const(op: Opcode, pool_index: u16) -> Vec<u8> {
        let mut bytes = vec![op];
        bytes.extend_from_slice(&pool_index.to_le_bytes());
        bytes
    }

    fn builtin_call(func_id: u16) -> Vec<u8> {
        let mut bytes = vec![opcode::BUILTIN];
        bytes.extend_from_slice(&func_id.to_le_bytes());
        bytes
    }

    /// Builds a single-function container from `body`, with a constant pool
    /// holding the two slot offsets as i32 (indices 0 and 1), a narrow string
    /// (index 2) and a wide string (index 3).
    fn container_from(body: Vec<u8>) -> Container {
        ContainerBuilder::new()
            .num_variables(1)
            .max_call_depth(1)
            .data_region_bytes(64)
            .add_i32_constant(WIDE_SLOT as i32)
            .add_i32_constant(NARROW_SLOT as i32)
            .add_str_constant(b"ab")
            .add_wstr_constant(&[0x61, 0x00, 0x62, 0x00])
            // Function bodies are looked up by position, so a single-function
            // container has to occupy slot 0.
            .add_function(FunctionId::INIT, &body, 4, 1, 0)
            .build()
    }

    fn body(parts: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes: Vec<u8> = parts.iter().flatten().copied().collect();
        bytes.push(opcode::RET_VOID);
        bytes
    }

    #[test]
    fn verify_when_concat_operands_share_width_then_ok() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Wide),
            two_offset_op(opcode::CONCAT_STR, WIDE_SLOT, NARROW_SLOT),
        ]));

        assert_eq!(verify_string_encoding(&container), Ok(()));
    }

    #[test]
    fn verify_when_concat_mixes_widths_then_slot_disagreement() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Narrow),
            two_offset_op(opcode::CONCAT_STR, WIDE_SLOT, NARROW_SLOT),
        ]));

        let violation = verify_string_encoding(&container).unwrap_err();
        assert!(matches!(
            violation,
            StringEncodingViolation::SlotDisagreement {
                opcode: opcode::CONCAT_STR,
                left: CharWidth::Wide,
                right: CharWidth::Narrow,
                ..
            }
        ));
        assert_eq!(violation.rule(), "R0304");
    }

    #[test]
    fn verify_when_find_mixes_widths_then_slot_disagreement() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Narrow),
            two_offset_op(opcode::FIND_STR, WIDE_SLOT, NARROW_SLOT),
        ]));

        assert!(matches!(
            verify_string_encoding(&container).unwrap_err(),
            StringEncodingViolation::SlotDisagreement { .. }
        ));
    }

    #[test]
    fn verify_when_cmp_str_mixes_widths_then_slot_disagreement() {
        // The instruction sequence issue #1550 reported: a WSTRING variable
        // compared against a scratch slot codegen initialized narrow because
        // the operand was a literal.
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Narrow),
            load_const(opcode::LOAD_CONST_I32, 0),
            load_const(opcode::LOAD_CONST_I32, 1),
            builtin_call(builtin::CMP_STR),
        ]));

        assert!(matches!(
            verify_string_encoding(&container).unwrap_err(),
            StringEncodingViolation::SlotDisagreement {
                left: CharWidth::Wide,
                right: CharWidth::Narrow,
                ..
            }
        ));
    }

    #[test]
    fn verify_when_cmp_str_operands_share_width_then_ok() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Wide),
            load_const(opcode::LOAD_CONST_I32, 0),
            load_const(opcode::LOAD_CONST_I32, 1),
            builtin_call(builtin::CMP_STR),
        ]));

        assert_eq!(verify_string_encoding(&container), Ok(()));
    }

    #[test]
    fn verify_when_narrow_constant_stored_into_wide_slot_then_constant_disagreement() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            load_const(opcode::LOAD_CONST_STR, 2),
            one_operand_op(opcode::STR_STORE_VAR, WIDE_SLOT),
        ]));

        let violation = verify_string_encoding(&container).unwrap_err();
        assert!(matches!(
            violation,
            StringEncodingViolation::ConstantDisagreement {
                constant: CharWidth::Narrow,
                destination: CharWidth::Wide,
                ..
            }
        ));
    }

    #[test]
    fn verify_when_wide_constant_stored_into_wide_slot_then_ok() {
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            load_const(opcode::LOAD_CONST_STR, 3),
            one_operand_op(opcode::STR_STORE_VAR, WIDE_SLOT),
        ]));

        assert_eq!(verify_string_encoding(&container), Ok(()));
    }

    #[test]
    fn verify_when_slot_width_undeclared_then_nothing_reported() {
        // No STR_INIT for either slot: the pass claims nothing about widths it
        // cannot read back, rather than guessing.
        let container = container_from(body(&[two_offset_op(
            opcode::CONCAT_STR,
            WIDE_SLOT,
            NARROW_SLOT,
        )]));

        assert_eq!(verify_string_encoding(&container), Ok(()));
    }

    #[test]
    fn verify_when_slot_declared_at_both_widths_then_nothing_reported() {
        // A reused offset is not a mistake, so a slot whose declarations
        // disagree is left unchecked rather than reported.
        let container = container_from(body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(WIDE_SLOT, CharWidth::Narrow),
            str_init(NARROW_SLOT, CharWidth::Narrow),
            two_offset_op(opcode::CONCAT_STR, WIDE_SLOT, NARROW_SLOT),
        ]));

        assert_eq!(verify_string_encoding(&container), Ok(()));
    }

    #[test]
    fn verify_when_slot_declared_in_another_function_then_still_checked() {
        // Program variables are initialized by INIT and used by SCAN; the
        // widths have to be gathered across the whole container.
        let init = body(&[
            str_init(WIDE_SLOT, CharWidth::Wide),
            str_init(NARROW_SLOT, CharWidth::Narrow),
        ]);
        let scan = body(&[two_offset_op(opcode::CONCAT_STR, WIDE_SLOT, NARROW_SLOT)]);

        let container = ContainerBuilder::new()
            .num_variables(1)
            .max_call_depth(1)
            .data_region_bytes(64)
            .add_function(FunctionId::INIT, &init, 4, 1, 0)
            .add_function(FunctionId::SCAN, &scan, 4, 1, 0)
            .build();

        assert!(matches!(
            verify_string_encoding(&container).unwrap_err(),
            StringEncodingViolation::SlotDisagreement { .. }
        ));
    }

    #[test]
    fn violation_when_displayed_then_names_both_encodings() {
        let violation = StringEncodingViolation::SlotDisagreement {
            function_id: FunctionId::SCAN,
            offset: 12,
            opcode: opcode::CONCAT_STR,
            left: CharWidth::Wide,
            right: CharWidth::Narrow,
        };

        let message = std::format!("{violation}");
        assert!(message.contains("offset 12"), "{message}");
        assert!(message.contains("WSTRING (char_width 2)"), "{message}");
        assert!(message.contains("STRING (char_width 1)"), "{message}");
    }
}
