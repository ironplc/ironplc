use crate::char_width::CharWidth;
use crate::code_section::FuncEntry;
use crate::const_type::ConstType;
use crate::error::ContainerError;
use crate::header::{FileHeader, FLAG_HAS_TYPE_SECTION, HEADER_SIZE};
use crate::id_types::{ConstantIndex, FbTypeId, FunctionId};
use crate::task_table::{ProgramInstanceEntry, TaskEntry, TASK_TABLE_HEADER_SIZE};
use crate::type_section::{
    ArrayDescriptor, FieldEntry, UserFbDescriptor, FB_TYPE_DESCRIPTOR_HEADER_SIZE,
};

/// Bytes of a constant pool entry before its value: type tag (u8),
/// char-width tag (u8) and value size (u16).
const CONST_ENTRY_HEADER_SIZE: usize = 4;

/// One constant pool entry as [`ContainerRef::from_slice`] decodes it into
/// the caller's constant table.
///
/// A primitive's value bytes are copied inline, so the dispatch loop reads a
/// constant with one indexed load rather than following an offset into the
/// pool; a string keeps only where its bytes lie. The table is the one piece
/// of scratch a `no_std` caller sizes (with [`ContainerRef::const_count`])
/// and owns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConstTableEntry {
    /// The primitive's little-endian value bytes, zero-extended; unused for
    /// strings.
    value: [u8; 8],
    /// Byte offset of a string's value within the pool's entry bytes.
    offset: u32,
    /// Byte length of a string's value.
    len: u16,
    /// The entry's [`ConstType`] tag.
    tag: u8,
}

impl ConstTableEntry {
    /// An entry `from_slice` has not filled yet; what a table starts as.
    pub const EMPTY: Self = ConstTableEntry {
        value: [0; 8],
        offset: 0,
        len: 0,
        tag: 0,
    };
}

/// Zero-copy, `no_std`-compatible view over a serialized bytecode container.
///
/// Borrows the underlying byte slice and provides O(1) accessors for every
/// section the VM reads. The caller provides the constant table that
/// [`from_slice`](Self::from_slice) decodes the pool into.
///
/// Construction validates every fixed-layout table the accessors index --
/// the function directory, the task table and the type section -- so an
/// entry inside a declared count can always be decoded. Out-of-range indices
/// still come back as `None` or an error, since the bytecode that supplies
/// them is not validated here.
#[derive(Clone, Debug)]
pub struct ContainerRef<'a> {
    header: FileHeader,
    const_pool_bytes: &'a [u8],
    constants: &'a [ConstTableEntry],
    code_bytes: &'a [u8],
    func_dir: &'a [u8],
    shared_globals_size: u16,
    /// The task entries, `TaskEntry::SIZE` bytes each.
    tasks: &'a [u8],
    /// The program instance entries, `ProgramInstanceEntry::SIZE` bytes each.
    programs: &'a [u8],
    /// The array descriptors, `ArrayDescriptor::SIZE` bytes each; empty when
    /// the container has no type section.
    array_descriptors: &'a [u8],
    /// The user FB descriptors, `UserFbDescriptor::SIZE` bytes each; empty
    /// when the container has no type section.
    user_fb_types: &'a [u8],
}

/// Reads a little-endian u16 from a byte slice at the given offset.
///
/// Returns `SectionSizeMismatch` if the slice is too short.
fn read_u16(data: &[u8], offset: usize) -> Result<u16, ContainerError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or(ContainerError::SectionSizeMismatch)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Returns the section at `offset` of `size` bytes, or `SectionSizeMismatch`
/// when it runs past the end of `data`.
fn section(data: &[u8], offset: u32, size: u32) -> Result<&[u8], ContainerError> {
    let start = offset as usize;
    let end = start
        .checked_add(size as usize)
        .ok_or(ContainerError::SectionSizeMismatch)?;
    data.get(start..end)
        .ok_or(ContainerError::SectionSizeMismatch)
}

/// Returns the first `count * N` bytes of `data` as the table at `data`'s
/// start, or `SectionSizeMismatch` when `data` is shorter than that.
fn table<const N: usize>(data: &[u8], count: usize) -> Result<&[u8], ContainerError> {
    let len = count
        .checked_mul(N)
        .ok_or(ContainerError::SectionSizeMismatch)?;
    data.get(..len).ok_or(ContainerError::SectionSizeMismatch)
}

/// Splits a table of fixed-size entries into `N`-byte arrays.
#[inline]
fn entries<const N: usize>(table: &[u8]) -> impl Iterator<Item = &[u8; N]> {
    table
        .chunks_exact(N)
        .filter_map(|chunk| chunk.try_into().ok())
}

/// Returns the `index`th `N`-byte entry of `table`, or `None` past the end.
#[inline]
fn entry<const N: usize>(table: &[u8], index: usize) -> Option<&[u8; N]> {
    let start = index.checked_mul(N)?;
    table.get(start..start + N)?.try_into().ok()
}

impl<'a> ContainerRef<'a> {
    /// Returns the number of constants in the constant pool without fully
    /// parsing the container.
    ///
    /// This is useful for sizing the constant table before calling
    /// [`from_slice`](Self::from_slice).
    pub fn const_count(data: &[u8]) -> Result<u16, ContainerError> {
        let header = Self::parse_header(data)?;
        if header.const_section_size == 0 {
            return Ok(0);
        }
        read_u16(data, header.const_section_offset as usize)
    }

    /// Parses a serialized container from a byte slice, decoding the
    /// constant pool into `const_table`.
    ///
    /// The caller must provide a `const_table` with at least
    /// [`const_count`](Self::const_count) elements.
    pub fn from_slice(
        data: &'a [u8],
        const_table: &'a mut [ConstTableEntry],
    ) -> Result<Self, ContainerError> {
        let header = Self::parse_header(data)?;
        let num_consts = Self::const_count(data)? as usize;
        let const_pool_bytes = Self::const_pool_bytes(&header, data)?;
        Self::decode_constants(const_pool_bytes, num_consts, const_table)?;
        let constants: &'a [ConstTableEntry] = const_table;
        Self::from_parts(data, constants)
    }

    /// Builds the view over `data` using a constant table that
    /// [`from_slice`](Self::from_slice) already decoded for the same bytes.
    ///
    /// This is what lets a host hand out many views over one buffer without
    /// a mutable scratch borrow per view. It is crate-private because a
    /// table from any other source would silently misread the pool.
    pub(crate) fn from_parts(
        data: &'a [u8],
        constants: &'a [ConstTableEntry],
    ) -> Result<Self, ContainerError> {
        let header = Self::parse_header(data)?;

        let const_pool_bytes = Self::const_pool_bytes(&header, data)?;
        let num_consts = Self::const_count(data)? as usize;
        let constants = constants
            .get(..num_consts)
            .ok_or(ContainerError::SectionSizeMismatch)?;

        let code_section = section(data, header.code_section_offset, header.code_section_size)?;
        let func_dir = table::<{ FuncEntry::SIZE }>(code_section, header.num_functions as usize)?;
        let code_bytes = &code_section[func_dir.len()..];

        let (shared_globals_size, tasks, programs) = Self::split_task_table(&header, data)?;
        let (array_descriptors, user_fb_types) = Self::split_type_section(&header, data)?;

        Ok(ContainerRef {
            header,
            const_pool_bytes,
            constants,
            code_bytes,
            func_dir,
            shared_globals_size,
            tasks,
            programs,
            array_descriptors,
            user_fb_types,
        })
    }

    fn parse_header(data: &[u8]) -> Result<FileHeader, ContainerError> {
        let header_bytes: &[u8; HEADER_SIZE] = data
            .get(..HEADER_SIZE)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(ContainerError::SectionSizeMismatch)?;
        FileHeader::from_bytes(header_bytes)
    }

    /// The constant pool's entry bytes: the section minus its leading count.
    fn const_pool_bytes(header: &FileHeader, data: &'a [u8]) -> Result<&'a [u8], ContainerError> {
        if header.const_section_size == 0 {
            return Ok(&[]);
        }
        let const_section = section(data, header.const_section_offset, header.const_section_size)?;
        const_section
            .get(2..)
            .ok_or(ContainerError::SectionSizeMismatch)
    }

    /// Decodes the first `num_consts` constant pool entries into
    /// `const_table`, which must hold at least that many elements.
    ///
    /// Every entry must lie within `const_pool_bytes` and carry a known type
    /// tag; a primitive's value may not exceed the eight bytes kept inline,
    /// which is the same limit the owned reader applies.
    fn decode_constants(
        const_pool_bytes: &[u8],
        num_consts: usize,
        const_table: &mut [ConstTableEntry],
    ) -> Result<(), ContainerError> {
        let entries = const_table
            .get_mut(..num_consts)
            .ok_or(ContainerError::SectionSizeMismatch)?;
        let mut pos: usize = 0;
        for entry in entries {
            let tag = *const_pool_bytes
                .get(pos)
                .ok_or(ContainerError::SectionSizeMismatch)?;
            let const_type = ConstType::from_u8(tag)?;
            let len = read_u16(const_pool_bytes, pos + 2)?;
            let value_offset = pos + CONST_ENTRY_HEADER_SIZE;
            let value = const_pool_bytes
                .get(value_offset..value_offset + len as usize)
                .ok_or(ContainerError::SectionSizeMismatch)?;

            let mut inline = [0u8; 8];
            if !const_type.is_string_like() {
                let width = value.len();
                if width > inline.len() {
                    return Err(ContainerError::InvalidConstantType(tag));
                }
                inline[..width].copy_from_slice(value);
            }
            *entry = ConstTableEntry {
                value: inline,
                offset: value_offset as u32,
                len,
                tag,
            };
            pos = value_offset + len as usize;
        }
        Ok(())
    }

    /// Splits the task table section into its shared-globals size, task
    /// entries and program entries, checking that the declared counts fit
    /// and that every task entry decodes.
    ///
    /// A container with no task section (size 0) has no tasks; a present
    /// section must at least carry its header.
    fn split_task_table(
        header: &FileHeader,
        data: &'a [u8],
    ) -> Result<(u16, &'a [u8], &'a [u8]), ContainerError> {
        if header.task_section_size == 0 {
            return Ok((0, &[], &[]));
        }
        let table_bytes = section(data, header.task_section_offset, header.task_section_size)?;
        let num_tasks = read_u16(table_bytes, 0)? as usize;
        let num_programs = read_u16(table_bytes, 2)? as usize;
        let shared_globals_size = read_u16(table_bytes, 4)?;

        let rest = &table_bytes[TASK_TABLE_HEADER_SIZE..];
        let tasks = table::<{ TaskEntry::SIZE }>(rest, num_tasks)?;
        let programs = table::<{ ProgramInstanceEntry::SIZE }>(&rest[tasks.len()..], num_programs)?;

        // Decode every task once so `task_entries` can promise each one
        // decodes; the only fallible byte is the task type tag.
        for bytes in entries::<{ TaskEntry::SIZE }>(tasks) {
            TaskEntry::from_bytes(bytes)?;
        }

        Ok((shared_globals_size, tasks, programs))
    }

    /// Splits the type section into its array descriptor and user FB
    /// descriptor tables, both empty when the container declares none.
    ///
    /// FB type descriptors are variable-length (a header plus one entry per
    /// field), so locating the tables behind them is a walk over their
    /// headers; the VM does not read them, so they are not kept.
    fn split_type_section(
        header: &FileHeader,
        data: &'a [u8],
    ) -> Result<(&'a [u8], &'a [u8]), ContainerError> {
        if header.flags & FLAG_HAS_TYPE_SECTION == 0 || header.type_section_size == 0 {
            return Ok((&[], &[]));
        }
        let section = section(data, header.type_section_offset, header.type_section_size)?;

        let fb_count = read_u16(section, 0)? as usize;
        let mut pos = 2;
        for _ in 0..fb_count {
            let num_fields = *section
                .get(pos + 2)
                .ok_or(ContainerError::SectionSizeMismatch)? as usize;
            pos += FB_TYPE_DESCRIPTOR_HEADER_SIZE + num_fields * FieldEntry::SIZE;
        }

        let array_count = read_u16(section, pos)? as usize;
        pos += 2;
        let array_descriptors = table::<{ ArrayDescriptor::SIZE }>(
            section
                .get(pos..)
                .ok_or(ContainerError::SectionSizeMismatch)?,
            array_count,
        )?;
        pos += array_descriptors.len();

        // The user FB table was added to the format after the array table;
        // a section that ends here has none, matching the owned reader.
        let user_fb_types = if pos + 2 <= section.len() {
            let user_fb_count = read_u16(section, pos)? as usize;
            table::<{ UserFbDescriptor::SIZE }>(&section[pos + 2..], user_fb_count)?
        } else {
            &[]
        };

        Ok((array_descriptors, user_fb_types))
    }

    /// Returns a reference to the parsed file header.
    #[inline]
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Returns the decoded constant at `index`.
    #[inline]
    fn constant(&self, index: ConstantIndex) -> Result<&ConstTableEntry, ContainerError> {
        self.constants
            .get(index.raw() as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))
    }

    /// Returns the `N` little-endian value bytes of the primitive constant
    /// at `index`, after checking its type tag is `expected`.
    ///
    /// This is the constant read on the dispatch loop's hot path: one
    /// indexed load of the decoded table, then a compare and a copy.
    #[inline]
    fn primitive_constant<const N: usize>(
        &self,
        index: ConstantIndex,
        expected: ConstType,
    ) -> Result<[u8; N], ContainerError> {
        let entry = self.constant(index)?;
        if entry.tag != expected as u8 {
            return Err(ContainerError::InvalidConstantType(entry.tag));
        }
        let mut bytes = [0u8; N];
        bytes.copy_from_slice(&entry.value[..N]);
        Ok(bytes)
    }

    /// Returns the i32 constant at the given pool index.
    #[inline]
    pub fn get_i32_constant(&self, index: ConstantIndex) -> Result<i32, ContainerError> {
        self.primitive_constant::<4>(index, ConstType::I32)
            .map(i32::from_le_bytes)
    }

    /// Returns the i64 constant at the given pool index.
    #[inline]
    pub fn get_i64_constant(&self, index: ConstantIndex) -> Result<i64, ContainerError> {
        self.primitive_constant::<8>(index, ConstType::I64)
            .map(i64::from_le_bytes)
    }

    /// Returns the f32 constant at the given pool index.
    #[inline]
    pub fn get_f32_constant(&self, index: ConstantIndex) -> Result<f32, ContainerError> {
        self.primitive_constant::<4>(index, ConstType::F32)
            .map(f32::from_le_bytes)
    }

    /// Returns the f64 constant at the given pool index.
    #[inline]
    pub fn get_f64_constant(&self, index: ConstantIndex) -> Result<f64, ContainerError> {
        self.primitive_constant::<8>(index, ConstType::F64)
            .map(f64::from_le_bytes)
    }

    /// Returns the raw bytes of the string constant at the given pool index.
    /// Accepts both [`ConstType::Str`] (Latin-1) and [`ConstType::WStr`]
    /// (UTF-16LE) entries.
    pub fn get_str_constant(&self, index: ConstantIndex) -> Result<&'a [u8], ContainerError> {
        let entry = self.constant(index)?;
        if !ConstType::from_u8(entry.tag)?.is_string_like() {
            return Err(ContainerError::InvalidConstantType(entry.tag));
        }
        let start = entry.offset as usize;
        self.const_pool_bytes
            .get(start..start + entry.len as usize)
            .ok_or(ContainerError::SectionSizeMismatch)
    }

    /// Returns the per-code-unit [`CharWidth`] of the string constant at
    /// `index`, or an error when the entry is not string-typed.
    pub fn constant_char_width(&self, index: ConstantIndex) -> Result<CharWidth, ContainerError> {
        let entry = self.constant(index)?;
        ConstType::from_u8(entry.tag)?
            .char_width()
            .ok_or(ContainerError::InvalidConstantType(entry.tag))
    }

    /// Returns the raw directory entry for the given function ID.
    ///
    /// Function IDs are compiler-assigned sequential indices, so the entry
    /// sits at `id * FuncEntry::SIZE` in the directory.
    #[inline]
    fn function_entry_bytes(&self, id: FunctionId) -> Option<&'a [u8; FuncEntry::SIZE]> {
        entry::<{ FuncEntry::SIZE }>(self.func_dir, id.raw() as usize)
    }

    /// Returns the function directory entry for the given function ID.
    #[inline]
    pub fn function_entry(&self, id: FunctionId) -> Option<FuncEntry> {
        self.function_entry_bytes(id).map(FuncEntry::from_bytes)
    }

    /// Returns the bytecode slice for the given function ID.
    ///
    /// The dispatch loop calls this once per instruction, so it reads the
    /// two directory fields it needs rather than decoding the whole entry.
    #[inline]
    pub fn get_function_bytecode(&self, id: FunctionId) -> Option<&'a [u8]> {
        let entry = self.function_entry_bytes(id)?;
        let start = u32::from_le_bytes([entry[2], entry[3], entry[4], entry[5]]) as usize;
        let length = u32::from_le_bytes([entry[6], entry[7], entry[8], entry[9]]) as usize;
        self.code_bytes.get(start..start.checked_add(length)?)
    }

    /// Returns the number of tasks in the task table.
    pub fn num_tasks(&self) -> u16 {
        (self.tasks.len() / TaskEntry::SIZE) as u16
    }

    /// Returns the number of program instances in the task table.
    pub fn num_programs(&self) -> u16 {
        (self.programs.len() / ProgramInstanceEntry::SIZE) as u16
    }

    /// Returns the shared globals size from the task table header.
    #[inline]
    pub fn shared_globals_size(&self) -> u16 {
        self.shared_globals_size
    }

    /// Returns the task entry at the given index.
    pub fn task_entry(&self, index: u16) -> Result<TaskEntry, ContainerError> {
        let bytes = entry::<{ TaskEntry::SIZE }>(self.tasks, index as usize)
            .ok_or(ContainerError::SectionSizeMismatch)?;
        TaskEntry::from_bytes(bytes)
    }

    /// Returns the program instance entry at the given index.
    pub fn program_entry(&self, index: u16) -> Result<ProgramInstanceEntry, ContainerError> {
        entry::<{ ProgramInstanceEntry::SIZE }>(self.programs, index as usize)
            .map(ProgramInstanceEntry::from_bytes)
            .ok_or(ContainerError::SectionSizeMismatch)
    }

    /// Iterates over the task table's task entries in declaration order.
    pub fn task_entries(&self) -> impl Iterator<Item = TaskEntry> + 'a {
        entries::<{ TaskEntry::SIZE }>(self.tasks).map(|bytes| {
            // `from_slice` decoded every entry once; the only byte that can
            // fail is the task type tag, and it already passed.
            TaskEntry::from_bytes(bytes).expect("task entries are validated at parse time")
        })
    }

    /// Iterates over the task table's program instance entries in
    /// declaration order.
    pub fn program_entries(&self) -> impl Iterator<Item = ProgramInstanceEntry> + 'a {
        entries::<{ ProgramInstanceEntry::SIZE }>(self.programs)
            .map(ProgramInstanceEntry::from_bytes)
    }

    /// Returns the array descriptor at the given type-section index, or
    /// `None` when the container has no such descriptor.
    #[inline]
    pub fn array_descriptor(&self, index: u16) -> Option<ArrayDescriptor> {
        entry::<{ ArrayDescriptor::SIZE }>(self.array_descriptors, index as usize)
            .map(ArrayDescriptor::from_bytes)
    }

    /// Returns the user-defined function block descriptor with the given
    /// type ID, or `None` when the container declares none.
    #[inline]
    pub fn user_fb_type(&self, type_id: FbTypeId) -> Option<UserFbDescriptor> {
        entries::<{ UserFbDescriptor::SIZE }>(self.user_fb_types)
            .map(UserFbDescriptor::from_bytes)
            .find(|desc| desc.type_id == type_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::vec;
    use std::vec::Vec;

    use crate::opcode;
    use crate::test_support::{container_bytes, steel_thread_single_function_container};
    use crate::{
        CharWidth, ContainerBuilder, ContainerBytes, FbTypeDescriptor, FbTypeId, FieldType,
        InstanceId, TaskId, TaskType, UserFbDescriptor,
    };

    fn steel_thread_bytes() -> Vec<u8> {
        container_bytes(&steel_thread_single_function_container())
    }

    #[test]
    fn container_ref_from_slice_when_valid_bytes_then_parses() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        assert_eq!(count, 2);
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert_eq!(cref.header().num_variables, 2);
        assert_eq!(cref.header().num_functions, 1);
        assert_eq!(cref.header().max_stack_depth, 2);
    }

    // A corruption applied to the steel-thread bytes, and a predicate that
    // recognizes the error `from_slice` should return for that corruption.
    #[rstest]
    #[case::invalid_magic(
        (|mut data: Vec<u8>| { data[0..4].copy_from_slice(&[0xFF; 4]); data }) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::InvalidMagic)) as fn(&ContainerError) -> bool
    )]
    #[case::truncated(
        (|_: Vec<u8>| vec![0u8; 100]) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::const_section_offset_past_end(
        (|data: Vec<u8>| { let n = data.len() as u32; with_tampered_header(&data, |h| h.const_section_size = n * 2) }) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::code_section_offset_past_end(
        (|data: Vec<u8>| { let n = data.len() as u32; with_tampered_header(&data, |h| h.code_section_size = n * 2) }) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::func_dir_larger_than_code(
        (|data: Vec<u8>| with_tampered_header(&data, |h| h.num_functions = 999)) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::task_section_offset_past_end(
        (|data: Vec<u8>| { let n = data.len() as u32; with_tampered_header(&data, |h| h.task_section_size = n * 2) }) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::const_section_only_one_byte(
        (|data: Vec<u8>| with_tampered_header(&data, |h| h.const_section_size = 1)) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::const_section_truncates_entry_header(
        (|data: Vec<u8>| with_tampered_header(&data, |h| h.const_section_size = 5)) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::const_section_entry_value_truncated(
        (|data: Vec<u8>| with_tampered_header(&data, |h| h.const_section_size = 6)) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::task_section_smaller_than_header(
        (|data: Vec<u8>| with_tampered_header(&data, |h| h.task_section_size = 3)) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    #[case::const_entry_value_size_bytes_corrupted(
        (|data: Vec<u8>| {
            let header =
                FileHeader::read_from(&mut std::io::Cursor::new(&data[..HEADER_SIZE])).unwrap();
            let const_start = header.const_section_offset as usize;
            let mut data = data;
            // Const section layout: [count: u16][entry0: type(1) reserved(1) size(2) value(n)].
            // Blow up the declared value size so it overruns the const pool.
            let size_offset = const_start + 2 + 2;
            data[size_offset] = 0xFF;
            data[size_offset + 1] = 0xFF;
            data
        }) as fn(Vec<u8>) -> Vec<u8>,
        (|e: &ContainerError| matches!(e, ContainerError::SectionSizeMismatch)) as fn(&ContainerError) -> bool
    )]
    fn container_ref_from_slice_when_corrupted_then_error(
        #[case] tamper: fn(Vec<u8>) -> Vec<u8>,
        #[case] expect_err: fn(&ContainerError) -> bool,
    ) {
        let data = tamper(steel_thread_bytes());
        let mut offsets = vec![ConstTableEntry::EMPTY; 16];
        let result = ContainerRef::from_slice(&data, &mut offsets);
        let err = result.expect_err("expected corruption to be rejected");
        assert!(expect_err(&err), "unexpected error: {err:?}");
    }

    #[test]
    fn container_ref_get_i32_constant_when_valid_index_then_returns_value() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert_eq!(cref.get_i32_constant(ConstantIndex::new(0)).unwrap(), 10);
        assert_eq!(cref.get_i32_constant(ConstantIndex::new(1)).unwrap(), 32);
    }

    #[test]
    fn container_ref_get_i32_constant_when_out_of_bounds_then_error() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let result = cref.get_i32_constant(ConstantIndex::new(99));
        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantIndex(idx)) if idx == ConstantIndex::new(99)
        ));
    }

    #[test]
    fn container_ref_get_function_bytecode_when_valid_id_then_returns_slice() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let bytecode = cref.get_function_bytecode(FunctionId::INIT).unwrap();
        // First byte: LOAD_CONST_I32 (0x00), last byte: RET_VOID (0xB5)
        assert_eq!(bytecode[0], 0x00);
        assert_eq!(*bytecode.last().unwrap(), 0x8C);
    }

    #[test]
    fn container_ref_task_entry_when_valid_index_then_returns_fields() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let task = cref.task_entry(0).unwrap();
        assert_eq!(task.task_id, TaskId::DEFAULT);
        assert_eq!(task.task_type, TaskType::Freewheeling);
        assert_eq!(task.flags, 0x01);
    }

    #[test]
    fn container_ref_program_entry_when_valid_index_then_returns_fields() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let prog = cref.program_entry(0).unwrap();
        assert_eq!(prog.instance_id, InstanceId::DEFAULT);
        assert_eq!(prog.task_id, TaskId::DEFAULT);
        assert_eq!(prog.var_table_count, 2);
    }

    /// A do-nothing program (`RET_VOID`) with whatever constant pool the
    /// caller's builder carries.
    fn ret_void_bytes(builder: ContainerBuilder) -> Vec<u8> {
        container_bytes(
            &builder
                .num_variables(0)
                .add_function(FunctionId::INIT, &[opcode::RET_VOID], 0, 0, 0)
                .build(),
        )
    }

    fn f32_constant_bytes() -> Vec<u8> {
        ret_void_bytes(ContainerBuilder::new().add_f32_constant(1.5))
    }

    fn empty_pool_bytes() -> Vec<u8> {
        ret_void_bytes(ContainerBuilder::new())
    }

    #[test]
    fn container_ref_const_count_when_header_truncated_then_errors() {
        let data = vec![0u8; HEADER_SIZE - 1];
        let result = ContainerRef::const_count(&data);
        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }

    #[test]
    fn container_ref_const_count_when_const_section_empty_then_returns_zero() {
        let data = empty_pool_bytes();
        assert_eq!(ContainerRef::const_count(&data).unwrap(), 0);
    }

    #[test]
    fn container_ref_from_slice_when_const_section_empty_then_const_table_empty() {
        let data = empty_pool_bytes();
        let mut offsets = vec![ConstTableEntry::EMPTY; 0];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();
        assert_eq!(cref.header().num_functions, 1);
    }

    #[test]
    fn container_ref_from_slice_when_const_table_too_small_then_errors() {
        // steel_thread_bytes has 2 constants; pass a buffer of length 1.
        let data = steel_thread_bytes();
        let mut offsets = vec![ConstTableEntry::EMPTY; 1];
        let result = ContainerRef::from_slice(&data, &mut offsets);
        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }

    #[test]
    fn container_ref_get_i32_constant_when_type_mismatch_then_errors() {
        let data = f32_constant_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let result = cref.get_i32_constant(ConstantIndex::new(0));
        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn container_ref_get_function_bytecode_when_id_out_of_bounds_then_returns_none() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert!(cref.get_function_bytecode(FunctionId::new(99)).is_none());
    }

    #[test]
    fn container_ref_get_function_bytecode_when_entry_length_exceeds_code_then_returns_none() {
        // Tamper the function directory so the entry claims a code_length
        // that runs past the end of code_bytes. from_slice validates the
        // outer section boundary but not individual func entries, so the
        // bounds check inside get_function_bytecode must catch it.
        let base = steel_thread_bytes();
        let header =
            FileHeader::read_from(&mut std::io::Cursor::new(&base[..HEADER_SIZE])).unwrap();
        let code_start = header.code_section_offset as usize;

        let mut data = base.clone();
        // Function directory entry layout (16 bytes):
        //   function_id(2) + code_offset(4, at bytes 2..6)
        //   + code_length(4, at bytes 6..10) + ...
        let length_offset = code_start + 6;
        data[length_offset..length_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());

        let mut offsets = vec![ConstTableEntry::EMPTY; 4];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();
        assert!(cref.get_function_bytecode(FunctionId::INIT).is_none());
    }

    #[test]
    fn container_ref_task_entry_when_index_out_of_bounds_then_errors() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert!(matches!(
            cref.task_entry(99),
            Err(ContainerError::SectionSizeMismatch)
        ));
    }

    #[test]
    fn container_ref_program_entry_when_index_out_of_bounds_then_errors() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert!(matches!(
            cref.program_entry(99),
            Err(ContainerError::SectionSizeMismatch)
        ));
    }

    #[test]
    fn read_u16_when_offset_past_end_then_errors() {
        let data = [1u8, 2, 3];
        assert!(matches!(
            read_u16(&data, 2),
            Err(ContainerError::SectionSizeMismatch)
        ));
    }

    /// Rewrites the header of `data` with `tamper` applied.
    fn with_tampered_header(data: &[u8], tamper: impl FnOnce(&mut FileHeader)) -> Vec<u8> {
        let mut header =
            FileHeader::read_from(&mut std::io::Cursor::new(&data[..HEADER_SIZE])).unwrap();
        tamper(&mut header);
        let mut tampered = Vec::with_capacity(data.len());
        header.write_to(&mut tampered).unwrap();
        tampered.extend_from_slice(&data[HEADER_SIZE..]);
        tampered
    }

    #[test]
    fn container_ref_const_count_when_const_section_size_is_zero_then_returns_zero() {
        // Tamper the header to set const_section_size = 0 so the early-exit
        // branch in const_count is exercised.
        let data = with_tampered_header(&steel_thread_bytes(), |h| {
            h.const_section_size = 0;
        });
        assert_eq!(ContainerRef::const_count(&data).unwrap(), 0);
    }

    #[test]
    fn container_ref_from_slice_when_const_section_size_is_zero_then_succeeds_with_empty_pool() {
        let data = with_tampered_header(&steel_thread_bytes(), |h| {
            h.const_section_size = 0;
        });
        let mut offsets = vec![ConstTableEntry::EMPTY; 0];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();
        assert_eq!(cref.header().num_functions, 1);
    }

    #[test]
    fn container_ref_num_programs_and_shared_globals_when_valid_then_return_fields() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![ConstTableEntry::EMPTY; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        // The builder synthesizes a single default program with shared_globals_size=0.
        assert_eq!(cref.num_programs(), 1);
        assert_eq!(cref.shared_globals_size(), 0);
    }

    #[test]
    fn container_ref_num_tasks_and_programs_when_task_section_zero_then_return_zero() {
        // When task_section_size is 0, from_slice accepts the container and
        // the runtime accessors fall back to zero rather than indexing an
        // empty slice.
        let data = with_tampered_header(&steel_thread_bytes(), |h| {
            h.task_section_size = 0;
        });
        let mut offsets = vec![ConstTableEntry::EMPTY; 4];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();
        assert_eq!(cref.num_tasks(), 0);
        assert_eq!(cref.num_programs(), 0);
        assert_eq!(cref.shared_globals_size(), 0);
    }

    /// A parsed view over `container`, owned so tests can build one in a line.
    fn image(container: &crate::Container) -> ContainerBytes {
        ContainerBytes::from_container(container).unwrap()
    }

    /// Serializes a builder's container: a do-nothing program with the
    /// constants and type section the builder carries.
    fn image_of(builder: ContainerBuilder) -> ContainerBytes {
        ContainerBytes::new(ret_void_bytes(builder)).unwrap()
    }

    #[test]
    fn container_ref_task_entries_when_default_table_then_yields_the_synthesized_task() {
        let image = image(&steel_thread_single_function_container());
        let cref = image.container_ref();

        let tasks: Vec<TaskEntry> = cref.task_entries().collect();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, TaskId::DEFAULT);
        assert_eq!(tasks[0].task_type, TaskType::Freewheeling);
    }

    #[test]
    fn container_ref_program_entries_when_default_table_then_carries_init_function_id() {
        let container = steel_thread_single_function_container();
        let expected = container.task_table.programs[0].init_function_id;
        let image = image(&container);
        let cref = image.container_ref();

        let programs: Vec<ProgramInstanceEntry> = cref.program_entries().collect();

        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0].init_function_id, expected);
        assert_eq!(programs[0].var_table_count, 2);
    }

    #[test]
    fn container_ref_from_slice_when_task_count_overruns_section_then_error() {
        let base = steel_thread_bytes();
        let header =
            FileHeader::read_from(&mut std::io::Cursor::new(&base[..HEADER_SIZE])).unwrap();
        let mut data = base;
        // num_tasks is the first u16 of the task table.
        let task_start = header.task_section_offset as usize;
        data[task_start..task_start + 2].copy_from_slice(&99u16.to_le_bytes());

        let mut offsets = vec![ConstTableEntry::EMPTY; 4];
        let result = ContainerRef::from_slice(&data, &mut offsets);

        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }

    #[test]
    fn container_ref_from_slice_when_task_type_invalid_then_error() {
        let base = steel_thread_bytes();
        let header =
            FileHeader::read_from(&mut std::io::Cursor::new(&base[..HEADER_SIZE])).unwrap();
        let mut data = base;
        // The task type tag is byte 4 of the first entry, after the 6-byte header.
        let type_tag = header.task_section_offset as usize + TASK_TABLE_HEADER_SIZE + 4;
        data[type_tag] = 0xFF;

        let mut offsets = vec![ConstTableEntry::EMPTY; 4];
        let result = ContainerRef::from_slice(&data, &mut offsets);

        assert!(matches!(result, Err(ContainerError::InvalidTaskType(0xFF))));
    }

    /// Corrupts the first constant pool entry of the steel-thread bytes,
    /// whose layout is `[count: u16][tag: u8][char_width: u8][size: u16][value]`.
    fn with_first_constant_tampered(tamper: impl FnOnce(&mut [u8])) -> Vec<u8> {
        let base = steel_thread_bytes();
        let header =
            FileHeader::read_from(&mut std::io::Cursor::new(&base[..HEADER_SIZE])).unwrap();
        let mut data = base;
        let entry = header.const_section_offset as usize + 2;
        tamper(&mut data[entry..entry + 8]);
        data
    }

    #[test]
    fn container_ref_from_slice_when_primitive_wider_than_eight_bytes_then_error() {
        let data = with_first_constant_tampered(|entry| {
            entry[2..4].copy_from_slice(&9u16.to_le_bytes());
        });

        let mut table = vec![ConstTableEntry::EMPTY; 4];
        let result = ContainerRef::from_slice(&data, &mut table);

        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantType(0))
        ));
    }

    #[test]
    fn container_ref_from_slice_when_constant_tag_unknown_then_error() {
        let data = with_first_constant_tampered(|entry| entry[0] = 0xEE);

        let mut table = vec![ConstTableEntry::EMPTY; 4];
        let result = ContainerRef::from_slice(&data, &mut table);

        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantType(0xEE))
        ));
    }

    #[test]
    fn container_ref_get_i64_constant_when_valid_then_returns_value() {
        let image = image_of(ContainerBuilder::new().add_i64_constant(-5_000_000_000));
        let cref = image.container_ref();

        assert_eq!(
            cref.get_i64_constant(ConstantIndex::new(0)).unwrap(),
            -5_000_000_000
        );
    }

    #[test]
    fn container_ref_get_f32_constant_when_valid_then_returns_value() {
        let image = image_of(ContainerBuilder::new().add_f32_constant(1.5));
        let cref = image.container_ref();

        assert_eq!(cref.get_f32_constant(ConstantIndex::new(0)).unwrap(), 1.5);
    }

    #[test]
    fn container_ref_get_f64_constant_when_valid_then_returns_value() {
        let image = image_of(ContainerBuilder::new().add_f64_constant(2.25));
        let cref = image.container_ref();

        assert_eq!(cref.get_f64_constant(ConstantIndex::new(0)).unwrap(), 2.25);
    }

    #[test]
    fn container_ref_get_i64_constant_when_i32_entry_then_type_error() {
        let image = image(&steel_thread_single_function_container());
        let cref = image.container_ref();

        let result = cref.get_i64_constant(ConstantIndex::new(0));

        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[rstest]
    #[case::narrow(ContainerBuilder::new().add_str_constant(b"hi"), CharWidth::Narrow)]
    #[case::wide(ContainerBuilder::new().add_wstr_constant(b"h\0i\0"), CharWidth::Wide)]
    fn container_ref_string_constant_when_string_entry_then_returns_bytes_and_width(
        #[case] builder: ContainerBuilder,
        #[case] width: CharWidth,
    ) {
        let image = image_of(builder);
        let cref = image.container_ref();

        let bytes = cref.get_str_constant(ConstantIndex::new(0)).unwrap();
        let char_width = cref.constant_char_width(ConstantIndex::new(0)).unwrap();

        assert_eq!(bytes.len(), 2 * width.as_usize());
        assert_eq!(bytes[0], b'h');
        assert_eq!(char_width, width);
    }

    #[test]
    fn container_ref_string_constant_when_primitive_entry_then_type_error() {
        let image = image(&steel_thread_single_function_container());
        let cref = image.container_ref();

        assert!(matches!(
            cref.get_str_constant(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
        assert!(matches!(
            cref.constant_char_width(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn container_ref_function_entry_when_valid_id_then_returns_fields() {
        let container = steel_thread_single_function_container();
        let expected = container.code.get_function(FunctionId::INIT).unwrap();
        let image = image(&container);
        let cref = image.container_ref();

        let entry = cref.function_entry(FunctionId::INIT).unwrap();

        assert_eq!(entry.function_id, FunctionId::INIT);
        assert_eq!(entry.code_length, expected.code_length);
        assert_eq!(entry.num_locals, expected.num_locals);
        assert_eq!(entry.num_params, expected.num_params);
    }

    #[test]
    fn container_ref_function_entry_when_id_out_of_bounds_then_returns_none() {
        let image = image(&steel_thread_single_function_container());
        let cref = image.container_ref();

        assert!(cref.function_entry(FunctionId::new(99)).is_none());
    }

    #[test]
    fn container_ref_array_descriptor_when_present_then_returns_descriptor() {
        let mut builder = ContainerBuilder::new();
        let index = builder.add_array_descriptor(FieldType::I32 as u8, 10, 0);
        let image = image_of(builder);
        let cref = image.container_ref();

        let desc = cref.array_descriptor(index).unwrap();

        assert_eq!(desc.element_type, FieldType::I32 as u8);
        assert_eq!(desc.total_elements, 10);
        assert_eq!(desc.element_extra, 0);
    }

    #[test]
    fn container_ref_array_descriptor_when_index_out_of_bounds_then_returns_none() {
        let mut builder = ContainerBuilder::new();
        builder.add_array_descriptor(FieldType::I32 as u8, 10, 0);
        let image = image_of(builder);
        let cref = image.container_ref();

        assert!(cref.array_descriptor(1).is_none());
    }

    #[test]
    fn container_ref_array_descriptor_when_no_type_section_then_returns_none() {
        let image = image(&steel_thread_single_function_container());
        let cref = image.container_ref();

        assert!(cref.array_descriptor(0).is_none());
    }

    #[test]
    fn container_ref_array_descriptor_when_behind_fb_type_descriptors_then_found() {
        // FB type descriptors are variable-length and precede the array
        // table, so the walk over their headers must land on it.
        let mut builder = ContainerBuilder::new().add_fb_type(FbTypeDescriptor {
            type_id: FbTypeId::new(0x0010),
            fields: vec![
                FieldEntry {
                    field_type: FieldType::I32,
                    field_extra: 0,
                },
                FieldEntry {
                    field_type: FieldType::Time,
                    field_extra: 0,
                },
                FieldEntry {
                    field_type: FieldType::String,
                    field_extra: 80,
                },
            ],
        });
        builder.add_array_descriptor(FieldType::F64 as u8, 7, 0);
        let image = image_of(builder);
        let cref = image.container_ref();

        let desc = cref.array_descriptor(0).unwrap();

        assert_eq!(desc.total_elements, 7);
    }

    #[test]
    fn container_ref_user_fb_type_when_present_then_returns_descriptor() {
        let builder = ContainerBuilder::new().add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1000),
            function_id: FunctionId::new(3),
            var_offset: 8,
            num_fields: 2,
        });
        let image = image_of(builder);
        let cref = image.container_ref();

        let desc = cref.user_fb_type(FbTypeId::new(0x1000)).unwrap();

        assert_eq!(desc.function_id, FunctionId::new(3));
        assert_eq!(desc.var_offset, 8);
        assert_eq!(desc.num_fields, 2);
    }

    #[test]
    fn container_ref_user_fb_type_when_unknown_id_then_returns_none() {
        let builder = ContainerBuilder::new().add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1000),
            function_id: FunctionId::new(3),
            var_offset: 8,
            num_fields: 2,
        });
        let image = image_of(builder);
        let cref = image.container_ref();

        assert!(cref.user_fb_type(FbTypeId::new(0x1001)).is_none());
    }

    #[test]
    fn container_ref_from_slice_when_type_section_ends_before_user_fb_table_then_none() {
        // A type section written before the user FB table existed ends after
        // the array descriptors; trimming the declared size reproduces one.
        let mut builder = ContainerBuilder::new();
        builder.add_array_descriptor(FieldType::I32 as u8, 4, 0);
        let data = with_tampered_header(&ret_void_bytes(builder), |h| {
            h.type_section_size -= 2;
        });

        let mut offsets = vec![ConstTableEntry::EMPTY; 0];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert_eq!(cref.array_descriptor(0).unwrap().total_elements, 4);
        assert!(cref.user_fb_type(FbTypeId::new(0)).is_none());
    }

    #[test]
    fn container_ref_from_slice_when_type_section_truncated_then_error() {
        let mut builder = ContainerBuilder::new();
        builder.add_array_descriptor(FieldType::I32 as u8, 4, 0);
        let data = with_tampered_header(&ret_void_bytes(builder), |h| {
            h.type_section_size = 3;
        });

        let mut offsets = vec![ConstTableEntry::EMPTY; 0];
        let result = ContainerRef::from_slice(&data, &mut offsets);

        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }
}
