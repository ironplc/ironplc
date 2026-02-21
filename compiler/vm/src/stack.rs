use crate::error::Trap;
use crate::value::Slot;

/// Fixed-capacity operand stack for VM execution.
pub struct OperandStack {
    data: Vec<Slot>,
    max_depth: usize,
}

impl OperandStack {
    /// Creates a new operand stack with the given maximum depth.
    pub fn new(max_depth: u16) -> Self {
        OperandStack {
            data: Vec::with_capacity(max_depth as usize),
            max_depth: max_depth as usize,
        }
    }

    /// Pushes a slot onto the stack.
    pub fn push(&mut self, slot: Slot) -> Result<(), Trap> {
        if self.data.len() >= self.max_depth {
            return Err(Trap::StackOverflow);
        }
        self.data.push(slot);
        Ok(())
    }

    /// Pops a slot from the stack.
    pub fn pop(&mut self) -> Result<Slot, Trap> {
        self.data.pop().ok_or(Trap::StackUnderflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_push_when_exceeds_max_depth_then_stack_overflow() {
        let mut stack = OperandStack::new(1);
        stack.push(Slot::from_i32(1)).unwrap();

        assert_eq!(stack.push(Slot::from_i32(2)), Err(Trap::StackOverflow));
    }

    #[test]
    fn stack_pop_when_empty_then_stack_underflow() {
        let mut stack = OperandStack::new(4);

        assert_eq!(stack.pop(), Err(Trap::StackUnderflow));
    }

    #[test]
    fn stack_push_pop_when_values_pushed_then_lifo_order() {
        let mut stack = OperandStack::new(4);
        stack.push(Slot::from_i32(10)).unwrap();
        stack.push(Slot::from_i32(20)).unwrap();

        assert_eq!(stack.pop().unwrap().as_i32(), 20);
        assert_eq!(stack.pop().unwrap().as_i32(), 10);
    }
}
