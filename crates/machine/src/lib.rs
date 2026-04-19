use brookshear_assembly::{
    common::Register,
    structured_instruction::{CmpjmpOperator, StructuredInstruction},
};

#[derive(Debug, thiserror::Error)]
pub enum BrookshearMachineError {
    #[error("Invalid instruction at address {0:02X}: the bytes {first:02X} {second:02X} do not represent a valid instruction.", first = .1[0], second = .1[1])]
    InvalidInstruction(u8, [u8; 2]),
    #[error("Memory access out of bounds at address {0:02X}")]
    MemoryAccessOutOfBounds(u8),
    #[error("Reached end of memory without halting")]
    EndOfMemory,
}

pub enum ControlFlow {
    Continue,
    Jump(u8),
    Halt,
}

use serde_big_array::BigArray;

#[cfg(feature = "undo")]
mod undo {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum InverseSideEffect {
        MemoryWrite { address: u8, old_value: u8 },
        RegisterWrite { register: Register, old_value: u8 },
    }

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct InverseStep {
        old_pc: u8,
        side_effect: Option<InverseSideEffect>,
    }

    #[derive(Debug, PartialEq, Eq, Clone, Default)]
    pub struct UndoHistory {
        limit: usize,
        entries: Vec<InverseStep>,
    }

    impl UndoHistory {
        pub fn new(limit: usize) -> Self {
            Self {
                limit,
                entries: Vec::new(),
            }
        }

        pub fn push(&mut self, entry: InverseStep) {
            if self.entries.len() == self.limit {
                self.entries.remove(0);
            }
            self.entries.push(entry);
        }

        pub fn pop(&mut self) -> Option<InverseStep> {
            self.entries.pop()
        }

        pub fn clear(&mut self) {
            self.entries.clear();
        }

        pub fn set_limit(&mut self, new_limit: usize) {
            self.limit = new_limit;
            while self.entries.len() > self.limit {
                self.entries.remove(0);
            }
        }

        pub fn limit(&self) -> usize {
            self.limit
        }
    }

    impl BrookshearMachine {
        pub fn new_with_history_limit(history_limit: usize) -> Self {
            Self {
                memory: [0; 256],
                registers: [0; 16],
                pc: 0,
                undo_history: undo::UndoHistory::new(history_limit),
            }
        }

        pub fn set_history_limit(&mut self, new_limit: usize) {
            self.undo_history.set_limit(new_limit);
        }

        pub fn history_limit(&self) -> usize {
            self.undo_history.limit
        }

        pub fn get_inverse_side_effect(
            &self,
            instruction: &StructuredInstruction,
        ) -> Option<undo::InverseSideEffect> {
            match instruction {
                StructuredInstruction::MovMemToReg(addr, register) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *register,
                        old_value: self.registers[register.as_index() as usize],
                    })
                }
                StructuredInstruction::MovImmToReg(_, register) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *register,
                        old_value: self.registers[register.as_index() as usize],
                    })
                }
                StructuredInstruction::MovRegToMem(register, addr) => {
                    Some(undo::InverseSideEffect::MemoryWrite {
                        address: *addr,
                        old_value: self.memory[*addr as usize],
                    })
                }
                StructuredInstruction::MovRegToReg { src, dst } => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dst,
                        old_value: self.registers[dst.as_index() as usize],
                    })
                }
                StructuredInstruction::MovIndirectToReg { dst, src } => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dst,
                        old_value: self.registers[dst.as_index() as usize],
                    })
                }
                StructuredInstruction::MovRegToIndirect { src, dst } => {
                    Some(undo::InverseSideEffect::MemoryWrite {
                        address: self.registers[dst.as_index() as usize],
                        old_value: self.memory[self.registers[dst.as_index() as usize] as usize],
                    })
                }
                StructuredInstruction::AddRegToRegInteger(dest, operand1, operand2) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::AddRegToRegFloat(dest, operand1, operand2) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::OrRegToReg(dest, operand1, operand2) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::AndRegToReg(dest, operand1, operand2) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::XorRegToReg(dest, operand1, operand2) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::RotRegRight(dest, amount) => {
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register: *dest,
                        old_value: self.registers[dest.as_index() as usize],
                    })
                }
                StructuredInstruction::JmpIfEqual(dest, addr) => None,
                StructuredInstruction::Halt => None,
                StructuredInstruction::JumpWithComparison(_, _, _) => None,
                StructuredInstruction::Nop => None,
            }
        }

        pub fn get_inverse_step(&self, instruction: &StructuredInstruction) -> undo::InverseStep {
            undo::InverseStep {
                old_pc: self.pc,
                side_effect: self.get_inverse_side_effect(instruction),
            }
        }

        pub fn record_inverse_step(&mut self, instruction: &StructuredInstruction) {
            if instruction == &StructuredInstruction::Halt {
                // Halt doesn't really do anything at all, not even advance the PC, and it's not counted in the total instructions executed count.
                return;
            }
            self.undo_history.push(self.get_inverse_step(instruction));
        }

        pub fn undo_step(&mut self) -> bool {
            if let Some(entry) = self.undo_history.pop() {
                self.pc = entry.old_pc;
                match entry.side_effect {
                    Some(undo::InverseSideEffect::MemoryWrite { address, old_value }) => {
                        self.memory[address as usize] = old_value;
                    }
                    Some(undo::InverseSideEffect::RegisterWrite {
                        register,
                        old_value,
                    }) => {
                        self.registers[register.as_index() as usize] = old_value;
                    }
                    None => {}
                }
                true
            } else {
                false
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrookshearMachine {
    pc: u8,
    registers: [u8; 16],
    #[serde(with = "BigArray")]
    memory: [u8; 256],
    #[cfg(feature = "undo")]
    #[serde(skip)]
    undo_history: undo::UndoHistory,
}

impl BrookshearMachine {
    pub const REGISTER_COUNT: usize = 16;
    pub const MEMORY_SIZE: usize = 256;

    pub fn new() -> Self {
        Self {
            memory: [0; 256],
            registers: [0; 16],
            pc: 0,
            #[cfg(feature = "undo")]
            undo_history: undo::UndoHistory::new(0),
        }
    }

    pub fn load_memory(&mut self, image: [u8; 256]) {
        self.memory = image;
    }

    pub fn reset(&mut self) {
        self.reset_memory();
        self.reset_registers();
    }

    pub fn reset_memory(&mut self) {
        #[cfg(feature = "undo")]
        {
            // The undo history is no longer valid after a reset.
            self.undo_history.clear();
        }
        self.memory = [0; 256];
    }

    pub fn reset_registers(&mut self) {
        #[cfg(feature = "undo")]
        {
            // The undo history is no longer valid after a reset.
            self.undo_history.clear();
        }
        self.pc = 0;
        self.registers = [0; 16];
    }

    pub fn set_pc(&mut self, value: u8) {
        self.pc = value;
    }

    pub fn get_pc(&self) -> u8 {
        self.pc
    }

    pub fn set_memory(&mut self, address: u8, value: u8) {
        self.memory[address as usize] = value;
    }

    pub fn get_memory(&self, address: u8) -> u8 {
        self.memory[address as usize]
    }

    pub fn get_memory_mut(&mut self, address: u8) -> &mut u8 {
        &mut self.memory[address as usize]
    }

    pub fn get_all_memory(&self) -> &[u8; 256] {
        &self.memory
    }

    pub fn get_all_memory_mut(&mut self) -> &mut [u8; 256] {
        &mut self.memory
    }

    pub fn get_register(&self, reg: Register) -> u8 {
        self.registers[reg.as_index() as usize]
    }

    pub fn get_all_registers_mut(&mut self) -> &mut [u8; 16] {
        &mut self.registers
    }

    pub fn get_register_mut(&mut self, reg: Register) -> &mut u8 {
        &mut self.registers[reg.as_index() as usize]
    }

    pub fn fetch_instruction(
        &mut self,
        address: u8,
    ) -> Result<StructuredInstruction, BrookshearMachineError> {
        if address as usize >= self.memory.len() - 1 {
            return Err(BrookshearMachineError::EndOfMemory);
        }
        let instruction_bytes = [
            self.memory[address as usize],
            self.memory[address.checked_add(1).unwrap() as usize], // this should never overflow because of the check above. so unwrap is fine.
        ];
        StructuredInstruction::from_bytes(instruction_bytes).ok_or(
            BrookshearMachineError::InvalidInstruction(address, instruction_bytes),
        )
    }

    pub fn fetch_next_instruction(
        &mut self,
    ) -> Result<StructuredInstruction, BrookshearMachineError> {
        self.fetch_instruction(self.pc)
    }

    pub fn execute_instruction(
        &mut self,
        instruction: StructuredInstruction,
    ) -> Result<ControlFlow, BrookshearMachineError> {
        #[cfg(feature = "undo")]
        {
            self.record_inverse_step(&instruction);
        }
        match instruction {
            StructuredInstruction::Nop => {}
            StructuredInstruction::MovMemToReg(addr, register) => {
                self.registers[register.as_index() as usize] = self.memory[addr as usize]
            }
            StructuredInstruction::MovImmToReg(val, register) => {
                self.registers[register.as_index() as usize] = val
            }
            StructuredInstruction::MovRegToMem(register, addr) => {
                self.memory[addr as usize] = self.registers[register.as_index() as usize]
            }
            StructuredInstruction::MovRegToReg { src, dst } => {
                self.registers[dst.as_index() as usize] = self.registers[src.as_index() as usize]
            }
            StructuredInstruction::MovIndirectToReg { dst, src } => {
                self.registers[dst.as_index() as usize] =
                    self.memory[self.registers[src.as_index() as usize] as usize]
            }
            StructuredInstruction::MovRegToIndirect { src, dst } => {
                self.memory[self.registers[dst.as_index() as usize] as usize] =
                    self.registers[src.as_index() as usize]
            }
            StructuredInstruction::AddRegToRegInteger(dest, operand1, operand2) => {
                self.registers[dest.as_index() as usize] = self.registers
                    [operand1.as_index() as usize]
                    .wrapping_add(self.registers[operand2.as_index() as usize]);
            }
            StructuredInstruction::AddRegToRegFloat(dest, operand1, operand2) => {
                self.registers[dest.as_index() as usize] = float8_add(
                    self.registers[operand1.as_index() as usize],
                    self.registers[operand2.as_index() as usize],
                );
            }
            StructuredInstruction::OrRegToReg(dest, operand1, operand2) => {
                self.registers[dest.as_index() as usize] = self.registers
                    [operand1.as_index() as usize]
                    | self.registers[operand2.as_index() as usize]
            }
            StructuredInstruction::AndRegToReg(dest, operand1, operand2) => {
                self.registers[dest.as_index() as usize] = self.registers
                    [operand1.as_index() as usize]
                    & self.registers[operand2.as_index() as usize]
            }
            StructuredInstruction::XorRegToReg(dest, operand1, operand2) => {
                self.registers[dest.as_index() as usize] = self.registers
                    [operand1.as_index() as usize]
                    ^ self.registers[operand2.as_index() as usize]
            }
            StructuredInstruction::RotRegRight(dest, amount) => {
                self.registers[dest.as_index() as usize] =
                    self.registers[dest.as_index() as usize].rotate_right(amount as u32);
            }
            StructuredInstruction::JmpIfEqual(dest, addr) => {
                if self.registers[dest.as_index() as usize] == self.registers[0] {
                    return Ok(ControlFlow::Jump(addr));
                }
            }
            StructuredInstruction::Halt => {
                return Ok(ControlFlow::Halt);
            }
            StructuredInstruction::JumpWithComparison(
                cmpjmp_operator,
                jmp_location,
                comparison_operand,
            ) => {
                let predicate = match cmpjmp_operator {
                    CmpjmpOperator::Eq => |a, b| a == b,
                    CmpjmpOperator::Ne => |a, b| a != b,
                    CmpjmpOperator::Lt => |a, b| a < b,
                    CmpjmpOperator::Le => |a, b| a <= b,
                    CmpjmpOperator::Gt => |a, b| a > b,
                    CmpjmpOperator::Ge => |a, b| a >= b,
                };

                if predicate(
                    self.registers[comparison_operand.as_index() as usize],
                    self.registers[0],
                ) {
                    return Ok(ControlFlow::Jump(
                        self.registers[jmp_location.as_index() as usize],
                    ));
                }
            }
        }
        Ok(ControlFlow::Continue)
    }

    pub fn step(&mut self) -> Result<bool, BrookshearMachineError> {
        let instruction = self.fetch_next_instruction()?;
        match self.execute_instruction(instruction)? {
            ControlFlow::Continue => {
                self.pc = self
                    .pc
                    .checked_add(2)
                    .ok_or(BrookshearMachineError::EndOfMemory)?;
            }
            ControlFlow::Jump(target) => {
                self.pc = target;
            }
            ControlFlow::Halt => {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Default for BrookshearMachine {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: bitwise implementation rather than converting to f32 and back, even though this "works".
fn float8_add(a: u8, b: u8) -> u8 {
    let a_f32 = float8_to_f32(a);
    let b_f32 = float8_to_f32(b);
    f32_to_float8(a_f32 + b_f32)
}

pub fn float8_to_f32(flt: u8) -> f32 {
    let sign = if flt & 0b1000_0000 != 0 { -1.0 } else { 1.0 };
    let exp = flt >> 4 & 0b0111;
    let mantissa = flt & 0b0000_1111;
    sign * (mantissa as f32) * f32::exp2(exp as f32 - ((1 << 2) as f32) - 4.0)
}

pub fn f32_to_float8(flt: f32) -> u8 {
    if flt == 0.0 {
        return 0;
    }

    let sign = if flt < 0.0 { 0b1000_0000 } else { 0 };
    let abs_flt = flt.abs();
    let exp = (abs_flt.log2().floor() + (1 << 2) as f32) as u8 + 1;
    let mantissa = (abs_flt / f32::exp2(exp as f32 - ((1 << 2) as f32) - 4.0)).round() as u8;

    if !(0b000..0b1000).contains(&exp) || mantissa > 0b1111 {
        panic!(
            "Todo: Graceful handling. Got float8 value that can't be represented: {}. Reason: exponent out of range or mantissa too large (e={}, m={})",
            flt, exp, mantissa
        );
    }

    sign | ((exp & 0b0111) << 4) | (mantissa & 0b0000_1111)
}

pub fn float8_to_string(flt: u8) -> String {
    float8_to_f32(flt).to_string()
}

pub fn string_to_float8(s: &str) -> Option<u8> {
    s.parse::<f32>().ok().map(f32_to_float8)
}
