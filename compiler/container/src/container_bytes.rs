use std::vec;
use std::vec::Vec;

use crate::container::Container;
use crate::container_ref::ContainerRef;
use crate::error::ContainerError;

/// A serialized container together with the constant-offset scratch that
/// [`ContainerRef::from_slice`] fills, owned so a host can keep them beside
/// the VM buffers that borrow them.
///
/// Construction parses the bytes once, so [`container_ref`](Self::container_ref)
/// cannot fail and takes only a shared borrow: each call is a fresh, cheap
/// view over the same bytes, and a session or benchmark can hold one
/// `ContainerBytes` and build a view per run. Embedded targets have no use
/// for this type: the bytes live in flash and the offsets array is sized by
/// the programmer, as the `no_std` VM design describes.
#[derive(Clone, Debug)]
pub struct ContainerBytes {
    bytes: Vec<u8>,
    const_offsets: Vec<u32>,
}

impl ContainerBytes {
    /// Takes ownership of serialized container bytes, validating them.
    pub fn new(bytes: Vec<u8>) -> Result<Self, ContainerError> {
        let mut const_offsets = vec![0u32; ContainerRef::const_count(&bytes)? as usize];
        ContainerRef::from_slice(&bytes, &mut const_offsets)?;
        Ok(ContainerBytes {
            bytes,
            const_offsets,
        })
    }

    /// Serializes `container` into its wire bytes.
    pub fn from_container(container: &Container) -> Result<Self, ContainerError> {
        let mut bytes = Vec::new();
        container.write_to(&mut bytes)?;
        Self::new(bytes)
    }

    /// The serialized container.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// A zero-copy view over the bytes.
    pub fn container_ref(&self) -> ContainerRef<'_> {
        ContainerRef::from_parts(&self.bytes, &self.const_offsets)
            .expect("bytes were validated by ContainerBytes::new")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id_types::ConstantIndex;
    use crate::test_support::{container_bytes, steel_thread_single_function_container};

    #[test]
    fn container_bytes_new_when_valid_then_container_ref_reads_constants() {
        let image = ContainerBytes::new(container_bytes(&steel_thread_single_function_container()))
            .unwrap();

        let cref = image.container_ref();

        assert_eq!(cref.get_i32_constant(ConstantIndex::new(1)).unwrap(), 32);
    }

    #[test]
    fn container_bytes_new_when_corrupt_then_error() {
        let result = ContainerBytes::new(vec![0u8; 100]);
        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }

    #[test]
    fn container_bytes_from_container_when_valid_then_bytes_match_write_to() {
        let container = steel_thread_single_function_container();

        let image = ContainerBytes::from_container(&container).unwrap();

        assert_eq!(image.bytes(), container_bytes(&container).as_slice());
    }

    #[test]
    fn container_bytes_container_ref_when_called_twice_then_both_views_coexist() {
        let image =
            ContainerBytes::from_container(&steel_thread_single_function_container()).unwrap();

        let first = image.container_ref();
        let second = image.container_ref();

        assert_eq!(first.num_tasks(), second.num_tasks());
    }
}
