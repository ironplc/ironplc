use ironplc_container::Container;

use crate::error::Trap;
use ironplc_container::opcode;
use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::VariableTable;

/// A newly created VM with no loaded program.
///
/// The only valid operation is [`load`](Vm::load), which consumes
/// this value and produces a [`VmReady`].
pub struct Vm;

impl Vm {
    /// Creates a new VM.
    pub fn new() -> Self {
        Vm
    }

    /// Loads a container, allocating stack and variable storage.
    /// Consumes the empty VM and returns a ready VM.
    pub fn load(self, container: Container) -> VmReady {
        let header = &container.header;
        let stack = OperandStack::new(header.max_stack_depth);
        let variables = VariableTable::new(header.num_variables);
        VmReady {
            container,
            stack,
            variables,
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

/// A VM with a loaded program, ready to start execution.
///
/// Call [`start`](VmReady::start) to begin scan execution.
pub struct VmReady {
    container: Container,
    stack: OperandStack,
    variables: VariableTable,
}

impl VmReady {
    /// Starts the VM for scan execution.
    /// Consumes the ready VM and returns a running VM.
    pub fn start(self) -> VmRunning {
        VmRunning {
            container: self.container,
            stack: self.stack,
            variables: self.variables,
            scan_count: 0,
        }
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }
}

/// A VM that is actively executing scan cycles.
///
/// Call [`run_single_scan`](VmRunning::run_single_scan) repeatedly
/// to execute the program. On a trap, the VM transitions to
/// [`VmFaulted`].
pub struct VmRunning {
    container: Container,
    stack: OperandStack,
    variables: VariableTable,
    scan_count: u64,
}

impl VmRunning {
    /// Executes one scan cycle by running the entry function.
    ///
    /// On success, the VM remains in the running state.
    /// On a trap, returns `Err(VmFaulted)`.
    pub fn run_single_scan(&mut self) -> Result<(), Trap> {
        let entry_id = self.container.header.entry_function_id;
        let bytecode = self
            .container
            .code
            .get_function_bytecode(entry_id)
            .ok_or(Trap::InvalidFunctionId(entry_id))?;

        execute(
            bytecode,
            &self.container,
            &mut self.stack,
            &mut self.variables,
        )?;

        self.scan_count += 1;
        Ok(())
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots in the loaded container.
    pub fn num_variables(&self) -> u16 {
        self.container.header.num_variables
    }

    /// Returns the number of completed scan cycles.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }
}

/// Executes bytecode until RET_VOID or a trap.
///
/// This is a free function so that the borrow checker can see
/// independent borrows of container (immutable) vs stack/variables
/// (mutable).
fn execute(
    bytecode: &[u8],
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
) -> Result<(), Trap> {
    let mut pc: usize = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        pc += 1;

        match op {
            opcode::LOAD_CONST_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                let value = container
                    .constant_pool
                    .get_i32(index)
                    .map_err(|_| Trap::InvalidConstantIndex(index))?;
                stack.push(Slot::from_i32(value))?;
            }
            opcode::LOAD_VAR_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                let slot = stack.pop()?;
                variables.store(index, slot)?;
            }
            opcode::ADD_I32 => {
                let b = stack.pop()?.as_i32();
                let a = stack.pop()?.as_i32();
                stack.push(Slot::from_i32(a.wrapping_add(b)))?;
            }
            opcode::RET_VOID => {
                return Ok(());
            }
            _ => {
                return Err(Trap::InvalidInstruction(op));
            }
        }
    }

    Ok(())
}

/// Reads a little-endian u16 from bytecode at pc, advancing pc by 2.
fn read_u16_le(bytecode: &[u8], pc: &mut usize) -> u16 {
    let value = u16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc += 2;
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::ContainerBuilder;

    fn steel_thread_container() -> Container {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x30,                   // ADD_I32
            0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
            0xB5,                   // RET_VOID
        ];

        ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(0, &bytecode, 2, 2)
            .build()
    }

    #[test]
    fn vm_load_when_valid_container_then_returns_ready() {
        let vm = Vm::new();
        let ready = vm.load(steel_thread_container());

        // If this compiles, the VM is in the Ready state.
        // Verify we can read the initial variable values.
        assert_eq!(ready.read_variable(0).unwrap(), 0);
    }

    #[test]
    fn vm_run_single_scan_when_steel_thread_then_x_is_10_y_is_42() {
        let mut vm = Vm::new().load(steel_thread_container()).start();

        vm.run_single_scan().unwrap();

        assert_eq!(vm.read_variable(0).unwrap(), 10);
        assert_eq!(vm.read_variable(1).unwrap(), 42);
    }

    #[test]
    fn vm_run_single_scan_when_invalid_opcode_then_trap() {
        let bytecode = vec![0xFF]; // invalid opcode
        let container = ContainerBuilder::new()
            .num_variables(0)
            .add_function(0, &bytecode, 1, 0)
            .build();

        let mut vm = Vm::new().load(container).start();

        let result = vm.run_single_scan();

        assert!(matches!(result, Err(Trap::InvalidInstruction(0xFF))));
    }
}
