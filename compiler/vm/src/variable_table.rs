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
            || (index >= self.instance_offset
                && index < self.instance_offset + self.instance_count)
        {
            Ok(())
        } else {
            Err(Trap::InvalidVariableIndex(index))
        }
    }
}

/// Variable table: indexed storage for program variables.
pub struct VariableTable {
    slots: Vec<Slot>,
}

impl VariableTable {
    /// Creates a new variable table with `count` slots, all initialized to zero.
    pub fn new(count: u16) -> Self {
        VariableTable {
            slots: vec![Slot::default(); count as usize],
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_table_new_when_created_then_all_slots_zero() {
        let table = VariableTable::new(3);

        assert_eq!(table.load(0).unwrap().as_i32(), 0);
        assert_eq!(table.load(1).unwrap().as_i32(), 0);
        assert_eq!(table.load(2).unwrap().as_i32(), 0);
    }

    #[test]
    fn variable_table_load_when_out_of_bounds_then_error() {
        let table = VariableTable::new(2);

        assert_eq!(table.load(2), Err(Trap::InvalidVariableIndex(2)));
    }

    #[test]
    fn variable_table_store_load_when_value_stored_then_loads_correctly() {
        let mut table = VariableTable::new(2);
        table.store(1, Slot::from_i32(42)).unwrap();

        assert_eq!(table.load(1).unwrap().as_i32(), 42);
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
}
