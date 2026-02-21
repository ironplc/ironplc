use crate::error::Trap;
use crate::value::Slot;

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
}
