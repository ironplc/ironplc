use crate::error::Trap;
use crate::value::Slot;

/// Defines which variable indices a program instance may access.
///
/// Each program instance can access shared globals (indices 0..shared_globals_size)
/// and its own partition (indices instance_offset..instance_offset+instance_count).
pub struct VariableScope {
    pub shared_globals_size: u16,
    pub instance_offset: u16,
    pub instance_count: u16,
}

impl VariableScope {
    /// Creates a permissive scope that allows access to all `num_variables` slots.
    #[cfg(test)]
    pub fn permissive(num_variables: u16) -> Self {
        VariableScope {
            shared_globals_size: num_variables,
            instance_offset: 0,
            instance_count: num_variables,
        }
    }

    /// Checks whether a variable index is within this scope's allowed range.
    pub fn check_access(&self, index: u16) -> Result<(), Trap> {
        if index < self.shared_globals_size
            || (index >= self.instance_offset && index < self.instance_offset + self.instance_count)
        {
            Ok(())
        } else {
            Err(Trap::InvalidVariableIndex(index))
        }
    }
}

/// Variable table: indexed storage for program variables.
pub struct VariableTable<'a> {
    slots: &'a mut [Slot],
}

impl<'a> VariableTable<'a> {
    /// Creates a new variable table backed by the given slice.
    pub fn new(backing: &'a mut [Slot]) -> Self {
        VariableTable { slots: backing }
    }

    /// Returns the number of variable slots.
    pub fn len(&self) -> u16 {
        self.slots.len() as u16
    }

    /// Loads the slot at the given index.
    pub fn load(&self, index: u16) -> Result<Slot, Trap> {
        self.slots
            .get(index as usize)
            .copied()
            .ok_or(Trap::InvalidVariableIndex(index))
    }

    /// Stores a slot at the given index.
    pub fn store(&mut self, index: u16, value: Slot) -> Result<(), Trap> {
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(Trap::InvalidVariableIndex(index))?;
        *slot = value;
        Ok(())
    }

    /// Copies pre-computed Slot values from a template byte slice into
    /// consecutive variable slots starting at `start`.
    ///
    /// The template is a sequence of raw Slot bytes (8 bytes per slot).
    /// Uses a single memcopy for performance. This is sound because `Slot`
    /// is `#[repr(transparent)]` over `u64`.
    pub fn copy_template(&mut self, start: u16, template: &[u8]) -> Result<(), Trap> {
        let num_slots = template.len() / 8;
        if num_slots == 0 {
            return Ok(());
        }
        let start_idx = start as usize;
        let end_idx = start_idx + num_slots;
        let dest = self
            .slots
            .get_mut(start_idx..end_idx)
            .ok_or(Trap::InvalidVariableIndex(start + num_slots as u16 - 1))?;
        // Safety: Slot is #[repr(transparent)] over u64, so &mut [Slot]
        // has the same layout as &mut [u64], which is just bytes.
        let dest_bytes: &mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, num_slots * 8) };
        dest_bytes.copy_from_slice(&template[..num_slots * 8]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_table_new_when_created_then_all_slots_zero() {
        let mut buf = [Slot::default(); 3];
        let table = VariableTable::new(&mut buf);

        assert_eq!(table.load(0).unwrap().as_i32(), 0);
        assert_eq!(table.load(1).unwrap().as_i32(), 0);
        assert_eq!(table.load(2).unwrap().as_i32(), 0);
    }

    #[test]
    fn variable_table_load_when_out_of_bounds_then_error() {
        let mut buf = [Slot::default(); 2];
        let table = VariableTable::new(&mut buf);

        assert_eq!(table.load(2), Err(Trap::InvalidVariableIndex(2)));
    }

    #[test]
    fn variable_table_store_load_when_value_stored_then_loads_correctly() {
        let mut buf = [Slot::default(); 2];
        let mut table = VariableTable::new(&mut buf);
        table.store(1, Slot::from_i32(42)).unwrap();

        assert_eq!(table.load(1).unwrap().as_i32(), 42);
    }

    #[test]
    fn variable_table_len_when_created_then_returns_count() {
        let mut buf = [Slot::default(); 5];
        let table = VariableTable::new(&mut buf);

        assert_eq!(table.len(), 5);
    }

    #[test]
    fn scope_check_when_index_in_shared_globals_then_ok() {
        let scope = VariableScope {
            shared_globals_size: 4,
            instance_offset: 10,
            instance_count: 5,
        };
        assert!(scope.check_access(0).is_ok());
        assert!(scope.check_access(3).is_ok());
    }

    #[test]
    fn scope_check_when_index_in_instance_range_then_ok() {
        let scope = VariableScope {
            shared_globals_size: 4,
            instance_offset: 10,
            instance_count: 5,
        };
        assert!(scope.check_access(10).is_ok());
        assert!(scope.check_access(14).is_ok());
    }

    #[test]
    fn scope_check_when_index_between_globals_and_instance_then_error() {
        let scope = VariableScope {
            shared_globals_size: 4,
            instance_offset: 10,
            instance_count: 5,
        };
        assert!(scope.check_access(5).is_err());
        assert!(scope.check_access(9).is_err());
    }

    #[test]
    fn scope_check_when_index_past_instance_then_error() {
        let scope = VariableScope {
            shared_globals_size: 4,
            instance_offset: 10,
            instance_count: 5,
        };
        assert!(scope.check_access(15).is_err());
    }

    #[test]
    fn scope_check_when_permissive_then_all_ok() {
        let scope = VariableScope::permissive(10);
        for i in 0..10 {
            assert!(scope.check_access(i).is_ok());
        }
    }

    #[test]
    fn variable_table_copy_template_when_valid_then_sets_slots() {
        let mut buf = [Slot::default(); 5];
        let mut table = VariableTable::new(&mut buf);

        let mut template = Vec::new();
        template.extend_from_slice(&42u64.to_le_bytes());
        template.extend_from_slice(&(-1i32 as i64 as u64).to_le_bytes());
        table.copy_template(2, &template).unwrap();

        assert_eq!(table.load(2).unwrap(), Slot::from_u64(42));
        assert_eq!(table.load(3).unwrap().as_i32(), -1);
        // Slots before and after remain unchanged
        assert_eq!(table.load(0).unwrap().as_i32(), 0);
        assert_eq!(table.load(4).unwrap().as_i32(), 0);
    }

    #[test]
    fn variable_table_copy_template_when_out_of_bounds_then_error() {
        let mut buf = [Slot::default(); 2];
        let mut table = VariableTable::new(&mut buf);
        let template = 42u64.to_le_bytes();
        assert!(table.copy_template(2, &template).is_err());
    }

    #[test]
    fn variable_table_copy_template_when_empty_then_noop() {
        let mut buf = [Slot::default(); 3];
        let mut table = VariableTable::new(&mut buf);
        table.copy_template(0, &[]).unwrap();
        assert_eq!(table.load(0).unwrap().as_i32(), 0);
    }
}
