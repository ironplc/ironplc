use crate::error::Trap;
use crate::value::Slot;

/// Fixed-capacity operand stack for VM execution.
pub struct OperandStack<'a> {
    data: &'a mut [Slot],
    len: usize,
}

impl<'a> OperandStack<'a> {
    /// Creates a new operand stack backed by the given slice.
    pub fn new(backing: &'a mut [Slot]) -> Self {
        OperandStack {
            data: backing,
            len: 0,
        }
    }

    /// Pushes a slot onto the stack.
    pub fn push(&mut self, slot: Slot) -> Result<(), Trap> {
        if self.len >= self.data.len() {
            return Err(Trap::StackOverflow);
        }
        self.data[self.len] = slot;
        self.len += 1;
        Ok(())
    }

    /// Pops a slot from the stack.
    pub fn pop(&mut self) -> Result<Slot, Trap> {
        if self.len == 0 {
            return Err(Trap::StackUnderflow);
        }
        self.len -= 1;
        Ok(self.data[self.len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_push_when_exceeds_max_depth_then_stack_overflow() {
        let mut buf = [Slot::default(); 1];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(1)).unwrap();

        assert_eq!(stack.push(Slot::from_i32(2)), Err(Trap::StackOverflow));
    }

    #[test]
    fn stack_pop_when_empty_then_stack_underflow() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);

        assert_eq!(stack.pop(), Err(Trap::StackUnderflow));
    }

    #[test]
    fn stack_push_pop_when_values_pushed_then_lifo_order() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(10)).unwrap();
        stack.push(Slot::from_i32(20)).unwrap();

        assert_eq!(stack.pop().unwrap().as_i32(), 20);
        assert_eq!(stack.pop().unwrap().as_i32(), 10);
    }
}
