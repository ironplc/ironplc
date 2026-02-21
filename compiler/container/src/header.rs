use std::io::{Read, Write};

use crate::ContainerError;

/// Magic number "IPLC" in little-endian.
pub const MAGIC: u32 = 0x49504C43;

/// Current container format version.
pub const FORMAT_VERSION: u16 = 1;

/// Fixed size of the file header in bytes.
pub const HEADER_SIZE: usize = 256;

/// File header for a bytecode container (256 bytes, fixed layout).
///
/// All multi-byte values are little-endian.
#[derive(Clone, Debug)]
pub struct FileHeader {
    pub magic: u32,
    pub format_version: u16,
    pub profile: u8,
    pub flags: u8,
    pub content_hash: [u8; 32],
    pub source_hash: [u8; 32],
    pub debug_hash: [u8; 32],
    pub layout_hash: [u8; 32],
    pub max_stack_depth: u16,
    pub max_call_depth: u16,
    pub num_variables: u16,
    pub num_fb_instances: u16,
    pub total_fb_instance_bytes: u32,
    pub total_str_var_bytes: u32,
    pub total_wstr_var_bytes: u32,
    pub num_temp_str_bufs: u16,
    pub num_temp_wstr_bufs: u16,
    pub max_str_length: u16,
    pub max_wstr_length: u16,
    pub num_functions: u16,
    pub num_fb_types: u16,
    pub num_arrays: u16,
    pub sig_section_offset: u32,
    pub sig_section_size: u32,
    pub debug_sig_offset: u32,
    pub debug_sig_size: u32,
    pub type_section_offset: u32,
    pub type_section_size: u32,
    pub const_section_offset: u32,
    pub const_section_size: u32,
    pub code_section_offset: u32,
    pub code_section_size: u32,
    pub debug_section_offset: u32,
    pub debug_section_size: u32,
    pub input_image_bytes: u16,
    pub output_image_bytes: u16,
    pub memory_image_bytes: u16,
    pub entry_function_id: u16,
    pub reserved: [u8; 30],
}

impl Default for FileHeader {
    fn default() -> Self {
        FileHeader {
            magic: MAGIC,
            format_version: FORMAT_VERSION,
            profile: 0,
            flags: 0,
            content_hash: [0; 32],
            source_hash: [0; 32],
            debug_hash: [0; 32],
            layout_hash: [0; 32],
            max_stack_depth: 0,
            max_call_depth: 0,
            num_variables: 0,
            num_fb_instances: 0,
            total_fb_instance_bytes: 0,
            total_str_var_bytes: 0,
            total_wstr_var_bytes: 0,
            num_temp_str_bufs: 0,
            num_temp_wstr_bufs: 0,
            max_str_length: 0,
            max_wstr_length: 0,
            num_functions: 0,
            num_fb_types: 0,
            num_arrays: 0,
            sig_section_offset: 0,
            sig_section_size: 0,
            debug_sig_offset: 0,
            debug_sig_size: 0,
            type_section_offset: 0,
            type_section_size: 0,
            const_section_offset: 0,
            const_section_size: 0,
            code_section_offset: 0,
            code_section_size: 0,
            debug_section_offset: 0,
            debug_section_size: 0,
            input_image_bytes: 0,
            output_image_bytes: 0,
            memory_image_bytes: 0,
            entry_function_id: 0,
            reserved: [0; 30],
        }
    }
}

impl FileHeader {
    /// Writes the header to the given writer as exactly 256 bytes.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        w.write_all(&self.magic.to_le_bytes())?;
        w.write_all(&self.format_version.to_le_bytes())?;
        w.write_all(&[self.profile])?;
        w.write_all(&[self.flags])?;
        w.write_all(&self.content_hash)?;
        w.write_all(&self.source_hash)?;
        w.write_all(&self.debug_hash)?;
        w.write_all(&self.layout_hash)?;
        w.write_all(&self.max_stack_depth.to_le_bytes())?;
        w.write_all(&self.max_call_depth.to_le_bytes())?;
        w.write_all(&self.num_variables.to_le_bytes())?;
        w.write_all(&self.num_fb_instances.to_le_bytes())?;
        w.write_all(&self.total_fb_instance_bytes.to_le_bytes())?;
        w.write_all(&self.total_str_var_bytes.to_le_bytes())?;
        w.write_all(&self.total_wstr_var_bytes.to_le_bytes())?;
        w.write_all(&self.num_temp_str_bufs.to_le_bytes())?;
        w.write_all(&self.num_temp_wstr_bufs.to_le_bytes())?;
        w.write_all(&self.max_str_length.to_le_bytes())?;
        w.write_all(&self.max_wstr_length.to_le_bytes())?;
        w.write_all(&self.num_functions.to_le_bytes())?;
        w.write_all(&self.num_fb_types.to_le_bytes())?;
        w.write_all(&self.num_arrays.to_le_bytes())?;
        w.write_all(&self.sig_section_offset.to_le_bytes())?;
        w.write_all(&self.sig_section_size.to_le_bytes())?;
        w.write_all(&self.debug_sig_offset.to_le_bytes())?;
        w.write_all(&self.debug_sig_size.to_le_bytes())?;
        w.write_all(&self.type_section_offset.to_le_bytes())?;
        w.write_all(&self.type_section_size.to_le_bytes())?;
        w.write_all(&self.const_section_offset.to_le_bytes())?;
        w.write_all(&self.const_section_size.to_le_bytes())?;
        w.write_all(&self.code_section_offset.to_le_bytes())?;
        w.write_all(&self.code_section_size.to_le_bytes())?;
        w.write_all(&self.debug_section_offset.to_le_bytes())?;
        w.write_all(&self.debug_section_size.to_le_bytes())?;
        w.write_all(&self.input_image_bytes.to_le_bytes())?;
        w.write_all(&self.output_image_bytes.to_le_bytes())?;
        w.write_all(&self.memory_image_bytes.to_le_bytes())?;
        w.write_all(&self.entry_function_id.to_le_bytes())?;
        w.write_all(&self.reserved)?;
        Ok(())
    }

    /// Reads a header from the given reader, consuming exactly 256 bytes.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut buf = [0u8; HEADER_SIZE];
        r.read_exact(&mut buf)?;

        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic != MAGIC {
            return Err(ContainerError::InvalidMagic);
        }

        let format_version = u16::from_le_bytes([buf[4], buf[5]]);
        if format_version != FORMAT_VERSION {
            return Err(ContainerError::UnsupportedVersion);
        }

        let profile = buf[6];
        let flags = buf[7];

        let mut content_hash = [0u8; 32];
        content_hash.copy_from_slice(&buf[8..40]);

        let mut source_hash = [0u8; 32];
        source_hash.copy_from_slice(&buf[40..72]);

        let mut debug_hash = [0u8; 32];
        debug_hash.copy_from_slice(&buf[72..104]);

        let mut layout_hash = [0u8; 32];
        layout_hash.copy_from_slice(&buf[104..136]);

        let max_stack_depth = u16::from_le_bytes([buf[136], buf[137]]);
        let max_call_depth = u16::from_le_bytes([buf[138], buf[139]]);
        let num_variables = u16::from_le_bytes([buf[140], buf[141]]);
        let num_fb_instances = u16::from_le_bytes([buf[142], buf[143]]);
        let total_fb_instance_bytes = u32::from_le_bytes([buf[144], buf[145], buf[146], buf[147]]);
        let total_str_var_bytes = u32::from_le_bytes([buf[148], buf[149], buf[150], buf[151]]);
        let total_wstr_var_bytes = u32::from_le_bytes([buf[152], buf[153], buf[154], buf[155]]);
        let num_temp_str_bufs = u16::from_le_bytes([buf[156], buf[157]]);
        let num_temp_wstr_bufs = u16::from_le_bytes([buf[158], buf[159]]);
        let max_str_length = u16::from_le_bytes([buf[160], buf[161]]);
        let max_wstr_length = u16::from_le_bytes([buf[162], buf[163]]);
        let num_functions = u16::from_le_bytes([buf[164], buf[165]]);
        let num_fb_types = u16::from_le_bytes([buf[166], buf[167]]);
        let num_arrays = u16::from_le_bytes([buf[168], buf[169]]);
        let sig_section_offset = u32::from_le_bytes([buf[170], buf[171], buf[172], buf[173]]);
        let sig_section_size = u32::from_le_bytes([buf[174], buf[175], buf[176], buf[177]]);
        let debug_sig_offset = u32::from_le_bytes([buf[178], buf[179], buf[180], buf[181]]);
        let debug_sig_size = u32::from_le_bytes([buf[182], buf[183], buf[184], buf[185]]);
        let type_section_offset = u32::from_le_bytes([buf[186], buf[187], buf[188], buf[189]]);
        let type_section_size = u32::from_le_bytes([buf[190], buf[191], buf[192], buf[193]]);
        let const_section_offset = u32::from_le_bytes([buf[194], buf[195], buf[196], buf[197]]);
        let const_section_size = u32::from_le_bytes([buf[198], buf[199], buf[200], buf[201]]);
        let code_section_offset = u32::from_le_bytes([buf[202], buf[203], buf[204], buf[205]]);
        let code_section_size = u32::from_le_bytes([buf[206], buf[207], buf[208], buf[209]]);
        let debug_section_offset = u32::from_le_bytes([buf[210], buf[211], buf[212], buf[213]]);
        let debug_section_size = u32::from_le_bytes([buf[214], buf[215], buf[216], buf[217]]);
        let input_image_bytes = u16::from_le_bytes([buf[218], buf[219]]);
        let output_image_bytes = u16::from_le_bytes([buf[220], buf[221]]);
        let memory_image_bytes = u16::from_le_bytes([buf[222], buf[223]]);
        let entry_function_id = u16::from_le_bytes([buf[224], buf[225]]);

        let mut reserved = [0u8; 30];
        reserved.copy_from_slice(&buf[226..256]);

        Ok(FileHeader {
            magic,
            format_version,
            profile,
            flags,
            content_hash,
            source_hash,
            debug_hash,
            layout_hash,
            max_stack_depth,
            max_call_depth,
            num_variables,
            num_fb_instances,
            total_fb_instance_bytes,
            total_str_var_bytes,
            total_wstr_var_bytes,
            num_temp_str_bufs,
            num_temp_wstr_bufs,
            max_str_length,
            max_wstr_length,
            num_functions,
            num_fb_types,
            num_arrays,
            sig_section_offset,
            sig_section_size,
            debug_sig_offset,
            debug_sig_size,
            type_section_offset,
            type_section_size,
            const_section_offset,
            const_section_size,
            code_section_offset,
            code_section_size,
            debug_section_offset,
            debug_section_size,
            input_image_bytes,
            output_image_bytes,
            memory_image_bytes,
            entry_function_id,
            reserved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn header_write_read_when_default_then_roundtrips() {
        let original = FileHeader::default();
        let mut buf = Vec::new();
        original.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = FileHeader::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.magic, MAGIC);
        assert_eq!(decoded.format_version, FORMAT_VERSION);
        assert_eq!(decoded.profile, 0);
        assert_eq!(decoded.flags, 0);
        assert_eq!(decoded.content_hash, [0; 32]);
        assert_eq!(decoded.num_variables, 0);
        assert_eq!(decoded.entry_function_id, 0);
        assert_eq!(decoded.reserved, [0; 30]);
    }

    #[test]
    fn header_read_when_invalid_magic_then_error() {
        let mut buf = vec![0u8; HEADER_SIZE];
        // Write wrong magic
        buf[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        // Write valid version so we can confirm magic is checked first
        buf[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());

        let mut cursor = Cursor::new(&buf);
        let result = FileHeader::read_from(&mut cursor);

        assert!(matches!(result, Err(ContainerError::InvalidMagic)));
    }

    #[test]
    fn header_write_when_default_then_exactly_256_bytes() {
        let header = FileHeader::default();
        let mut buf = Vec::new();
        header.write_to(&mut buf).unwrap();

        assert_eq!(buf.len(), HEADER_SIZE);
    }
}
