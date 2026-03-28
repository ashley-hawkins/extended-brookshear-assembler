use brookshear_assembly::{
    common::Register,
    structured_instruction::{CmpjmpOperator, StructuredInstruction},
};

#[derive(Debug, thiserror::Error)]
pub enum BrookshearMachineError {
    #[error("Invalid instruction at address {0:#04X}: {1}")]
    InvalidInstruction(u8, String),
    #[error("Memory access out of bounds at address {0:#04X}")]
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

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrookshearMachine {
    pc: u8,
    registers: [u8; 16],
    #[serde(with = "BigArray")]
    memory: [u8; 256],
}

impl BrookshearMachine {
    pub const REGISTER_COUNT: usize = 16;
    pub const MEMORY_SIZE: usize = 256;

    pub fn new() -> Self {
        Self {
            memory: [0; 256],
            registers: [0; 16],
            pc: 0,
        }
    }

    pub fn load_memory(&mut self, image: [u8; 256]) {
        self.memory = image;
    }

    pub fn reset(&mut self) {
        std::mem::take(self);
    }

    pub fn reset_memory(&mut self) {
        self.memory = [0; 256];
    }

    pub fn reset_registers(&mut self) {
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

    pub fn get_register(&self, reg: Register) -> u8 {
        self.registers[reg.as_index() as usize]
    }

    pub fn get_register_mut(&mut self, reg: Register) -> &mut u8 {
        &mut self.registers[reg.as_index() as usize]
    }

    pub fn fetch_instruction(
        &mut self,
        address: u8,
    ) -> Result<StructuredInstruction, BrookshearMachineError> {
        if address as usize >= self.memory.len() {
            return Err(BrookshearMachineError::EndOfMemory);
        }
        let instruction_bytes = [
            self.memory[address as usize],
            self.memory[address.wrapping_add(1) as usize],
        ];
        StructuredInstruction::from_bytes(instruction_bytes).ok_or_else(|| {
            BrookshearMachineError::InvalidInstruction(
                address,
                format!(
                    "Invalid instruction bytes: {:#04X} {:#04X}",
                    instruction_bytes[0], instruction_bytes[1]
                ),
            )
        })
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
