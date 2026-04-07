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

    /// Returns the top slot without removing it.
    pub fn peek(&self) -> Result<Slot, Trap> {
        if self.len == 0 {
            return Err(Trap::StackUnderflow);
        }
        Ok(self.data[self.len - 1])
    }

    /// Returns the current number of slots on the stack.
    pub fn depth(&self) -> usize {
        self.len
    }

    /// Returns the slot at `depth` slots below the top (depth 0 = top).
    pub fn peek_at(&self, depth: usize) -> Result<Slot, Trap> {
        if depth >= self.len {
            return Err(Trap::StackUnderflow);
        }
        Ok(self.data[self.len - 1 - depth])
    }

    /// Removes `n` slots from the top of the stack.
    pub fn truncate_by(&mut self, n: usize) -> Result<(), Trap> {
        if n > self.len {
            return Err(Trap::StackUnderflow);
        }
        self.len -= n;
        Ok(())
    }

    /// Duplicates the top value on the stack.
    pub fn dup(&mut self) -> Result<(), Trap> {
        let top = self.peek()?;
        self.push(top)
    }

    /// Swaps the top two values on the stack.
    pub fn swap(&mut self) -> Result<(), Trap> {
        if self.len < 2 {
            return Err(Trap::StackUnderflow);
        }
        self.data.swap(self.len - 1, self.len - 2);
        Ok(())
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
    fn stack_peek_when_empty_then_stack_underflow() {
        let mut buf = [Slot::default(); 4];
        let stack = OperandStack::new(&mut buf);

        assert_eq!(stack.peek(), Err(Trap::StackUnderflow));
    }

    #[test]
    fn stack_peek_when_value_present_then_returns_without_removing() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(42)).unwrap();

        assert_eq!(stack.peek().unwrap().as_i32(), 42);
        // Value should still be on stack
        assert_eq!(stack.pop().unwrap().as_i32(), 42);
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

    #[test]
    fn stack_dup_when_value_present_then_duplicates_top() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(42)).unwrap();

        stack.dup().unwrap();
        assert_eq!(stack.pop().unwrap().as_i32(), 42);
        assert_eq!(stack.pop().unwrap().as_i32(), 42);
    }

    #[test]
    fn stack_dup_when_empty_then_stack_underflow() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);

        assert_eq!(stack.dup(), Err(Trap::StackUnderflow));
    }

    #[test]
    fn stack_dup_when_full_then_stack_overflow() {
        let mut buf = [Slot::default(); 1];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(1)).unwrap();

        assert_eq!(stack.dup(), Err(Trap::StackOverflow));
    }

    #[test]
    fn stack_swap_when_two_values_then_swaps() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(10)).unwrap();
        stack.push(Slot::from_i32(20)).unwrap();

        stack.swap().unwrap();
        assert_eq!(stack.pop().unwrap().as_i32(), 10);
        assert_eq!(stack.pop().unwrap().as_i32(), 20);
    }

    #[test]
    fn stack_swap_when_one_value_then_stack_underflow() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);
        stack.push(Slot::from_i32(1)).unwrap();

        assert_eq!(stack.swap(), Err(Trap::StackUnderflow));
    }

    #[test]
    fn stack_swap_when_empty_then_stack_underflow() {
        let mut buf = [Slot::default(); 4];
        let mut stack = OperandStack::new(&mut buf);

        assert_eq!(stack.swap(), Err(Trap::StackUnderflow));
    }
}
