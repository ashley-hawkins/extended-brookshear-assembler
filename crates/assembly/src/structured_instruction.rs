use chumsky::span::{Span, Spanned};

use crate::{
    common::Register,
    parser::{CoreOperand, InstructionDetail, Operand, OutputOperand},
    serialize::{Context, SerializationErrorMessage, SerializeResult},
};

#[derive(Debug, PartialEq, Clone, Copy, strum::FromRepr)]
#[repr(u8)]
pub enum CmpjmpOperator {
    Eq,
    Ne,
    Ge,
    Le,
    Gt,
    Lt,
}

impl CmpjmpOperator {
    pub fn as_index(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum StructuredInstruction {
    // 0x0FFF
    Nop,
    // 0x1rxy
    MovMemToReg(u8, Register),
    // 0x2rxy
    MovImmToReg(u8, Register),
    // 0x3rxy
    MovRegToMem(Register, u8),
    // 0x40rs
    MovRegToReg { src: Register, dst: Register },
    // 0xD0rs
    MovIndirectToReg { dst: Register, src: Register },
    // 0xE0rs
    MovRegToIndirect { src: Register, dst: Register },
    // 0x5rst
    AddRegToRegInteger(Register, Register, Register),
    // 0x6rst
    AddRegToRegFloat(Register, Register, Register),
    // 0x7rst
    OrRegToReg(Register, Register, Register),
    // 0x8rst
    AndRegToReg(Register, Register, Register),
    // 0x9rst
    XorRegToReg(Register, Register, Register),
    // 0xAr0x (x is the shift amount)
    RotRegRight(Register, u8),
    // 0xBrxy (R0 is implicitly the other operand of the comparison)
    JmpIfEqual(Register, u8),
    // 0xC000
    Halt,
    // 0xFrxt
    JumpWithComparison(CmpjmpOperator, Register, Register),
}

impl StructuredInstruction {
    pub fn from_ast(
        instr: &Spanned<crate::parser::Instruction<'_>>,
        ctx: &Context,
    ) -> SerializeResult<Self> {
        match instr.inner.mnemonic.inner.to_uppercase().as_str() {
            "MOV" => convert_mov(&instr.inner.detail, ctx),
            "HALT" => convert_halt(&instr.inner.detail, ctx),
            "NOP" => convert_nop(&instr.inner.detail, ctx),
            "ADDI" => convert_addi(&instr.inner.detail, ctx),
            "ADDF" => convert_addf(&instr.inner.detail, ctx),
            "OR" => convert_or(&instr.inner.detail, ctx),
            "AND" => convert_and(&instr.inner.detail, ctx),
            "XOR" => convert_xor(&instr.inner.detail, ctx),
            "ROT" => convert_rot(&instr.inner.detail, ctx),
            "JMP" => convert_jmp(&instr.inner.detail, ctx),
            "JMPEQ" => convert_jmpeq(&instr.inner.detail, ctx),
            "JMPNE" => convert_jmpne(&instr.inner.detail, ctx),
            "JMPGE" => convert_jmpge(&instr.inner.detail, ctx),
            "JMPLE" => convert_jmple(&instr.inner.detail, ctx),
            "JMPGT" => convert_jmpgt(&instr.inner.detail, ctx),
            "JMPLT" => convert_jmplt(&instr.inner.detail, ctx),
            _ => Err(SerializationErrorMessage::UnknownMnemonic(
                instr.inner.mnemonic.inner.to_string(),
            )
            .with_span(instr.mnemonic.span)),
        }
    }

    pub fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        Some(match (bytes[0] >> 4, bytes) {
            (0x0, [0x0F, 0xFF]) => StructuredInstruction::Nop,
            (0x1, [b1, b2]) => {
                StructuredInstruction::MovMemToReg(b2, Register::from_repr(b1 & 0x0F).unwrap())
            }
            (0x2, [b1, b2]) => {
                StructuredInstruction::MovImmToReg(b2, Register::from_repr(b1 & 0x0F).unwrap())
            }
            (0x3, [b1, b2]) => {
                StructuredInstruction::MovRegToMem(Register::from_repr(b1 & 0x0F).unwrap(), b2)
            }
            (0x4, [_b1, b2]) => {
                let src = Register::from_repr(b2 >> 4).unwrap();
                let dst = Register::from_repr(b2 & 0x0F).unwrap();
                StructuredInstruction::MovRegToReg { src, dst }
            }
            (0xD, [b1, b2]) => {
                if (b1 & 0x0F) != 0 {
                    return None;
                }
                let dst = Register::from_repr(b2 >> 4).unwrap();
                let src = Register::from_repr(b2 & 0x0F).unwrap();
                StructuredInstruction::MovIndirectToReg { dst, src }
            }
            (0xE, [b1, b2]) => {
                if (b1 & 0x0F) != 0 {
                    return None;
                }
                let src = Register::from_repr(b2 >> 4).unwrap();
                let dst = Register::from_repr(b2 & 0x0F).unwrap();
                StructuredInstruction::MovRegToIndirect { src, dst }
            }
            (opcode @ 0x5..=0x9, [b1, b2]) => {
                let r = Register::from_repr(b1 & 0x0F).unwrap();
                let s = Register::from_repr(b2 >> 4).unwrap();
                let t = Register::from_repr(b2 & 0x0F).unwrap();
                match opcode {
                    0x5 => StructuredInstruction::AddRegToRegInteger(r, s, t),
                    0x6 => StructuredInstruction::AddRegToRegFloat(r, s, t),
                    0x7 => StructuredInstruction::OrRegToReg(r, s, t),
                    0x8 => StructuredInstruction::AndRegToReg(r, s, t),
                    0x9 => StructuredInstruction::XorRegToReg(r, s, t),
                    _ => unreachable!(),
                }
            }
            (0xA, [b1, b2]) => {
                let target = Register::from_repr(b1 & 0x0F).unwrap();
                let amount = b2 & 0x0F;
                StructuredInstruction::RotRegRight(target, amount)
            }
            (0xB, [b1, b2]) => {
                let comparison_operand = Register::from_repr(b1 & 0x0F).unwrap();
                let jmp_location = b2;
                StructuredInstruction::JmpIfEqual(comparison_operand, jmp_location)
            }
            (0xC, [0xC0, 0x00]) => StructuredInstruction::Halt,
            (0xF, [b1, b2]) => {
                let comparison_operand = Register::from_repr(b1 & 0x0F).unwrap();
                let operator = CmpjmpOperator::from_repr(b2 >> 4)?;
                let jmp_location = Register::from_repr(b2 & 0x0F).unwrap();
                StructuredInstruction::JumpWithComparison(
                    operator,
                    jmp_location,
                    comparison_operand,
                )
            }
            _ => return None,
        })
    }

    pub fn as_bytes(&self) -> [u8; 2] {
        match self {
            StructuredInstruction::MovMemToReg(value, dst) => [0x10 | dst.as_index(), *value],
            StructuredInstruction::MovImmToReg(value, dst) => [0x20 | dst.as_index(), *value],
            StructuredInstruction::MovRegToMem(src, addr) => [0x30 | src.as_index(), *addr],
            StructuredInstruction::MovRegToReg { src, dst } => {
                [0x40, ((src.as_index() << 4) | dst.as_index())]
            }
            StructuredInstruction::MovIndirectToReg { dst, src } => {
                [0xD0, ((dst.as_index() << 4) | src.as_index())]
            }
            StructuredInstruction::MovRegToIndirect { src, dst } => {
                [0xE0, ((src.as_index() << 4) | dst.as_index())]
            }
            StructuredInstruction::AddRegToRegInteger(dst, src1, src2) => [
                0x50 | dst.as_index(),
                (src1.as_index() << 4) | src2.as_index(),
            ],
            StructuredInstruction::AddRegToRegFloat(dst, src1, src2) => [
                0x60 | dst.as_index(),
                (src1.as_index() << 4) | src2.as_index(),
            ],
            StructuredInstruction::OrRegToReg(dst, src1, src2) => [
                0x70 | dst.as_index(),
                (src1.as_index() << 4) | src2.as_index(),
            ],
            StructuredInstruction::AndRegToReg(dst, src1, src2) => [
                0x80 | dst.as_index(),
                (src1.as_index() << 4) | src2.as_index(),
            ],
            StructuredInstruction::XorRegToReg(dst, src1, src2) => [
                0x90 | dst.as_index(),
                (src1.as_index() << 4) | src2.as_index(),
            ],
            StructuredInstruction::RotRegRight(target, amount) => {
                [0xA0 | target.as_index(), *amount]
            }
            StructuredInstruction::JmpIfEqual(comparison_operand, jmp_location) => {
                [0xB0 | comparison_operand.as_index(), *jmp_location]
            }
            StructuredInstruction::Halt => [0xC0, 0],
            StructuredInstruction::JumpWithComparison(
                operator,
                jmp_location,
                comparison_operand,
            ) => [
                0xF0 | comparison_operand.as_index(),
                (operator.as_index() << 4) | jmp_location.as_index(),
            ],
            StructuredInstruction::Nop => [0x0F, 0xFF],
        }
    }

    pub fn describe(&self) -> String {
        match self {
            StructuredInstruction::Nop => "No operation".to_string(),
            StructuredInstruction::MovMemToReg(addr, register) => format!(
                "Load R{:X} from memory cell {:02X}",
                register.as_index(),
                addr
            ),
            StructuredInstruction::MovImmToReg(val, register) => {
                format!("Load R{:X} with value {:02X}", register.as_index(), val)
            }
            StructuredInstruction::MovRegToMem(register, addr) => format!(
                "Store value of R{:X} into memory cell {:02X}",
                register.as_index(),
                addr
            ),
            StructuredInstruction::MovRegToReg { src, dst } => format!(
                "Copy value of R{:X} into R{:X}",
                src.as_index(),
                dst.as_index()
            ),
            StructuredInstruction::MovIndirectToReg { dst, src } => format!(
                "Load R{:X} from memory cell pointed to by R{:X}",
                dst.as_index(),
                src.as_index()
            ),
            StructuredInstruction::MovRegToIndirect { src, dst } => format!(
                "Store value of R{:X} into memory cell pointed to by R{:X}",
                src.as_index(),
                dst.as_index()
            ),
            StructuredInstruction::AddRegToRegInteger(dest, operand1, operand2) => format!(
                "Put R{:X} + R{:X} (ints) into R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::AddRegToRegFloat(dest, operand1, operand2) => format!(
                "Put R{:X} + R{:X} (floats) into R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::OrRegToReg(dest, operand1, operand2) => format!(
                "Put R{:X} OR R{:X} into R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::AndRegToReg(dest, operand1, operand2) => format!(
                "Put R{:X} AND R{:X} into R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::XorRegToReg(dest, operand1, operand2) => format!(
                "Put R{:X} XOR R{:X} into R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::RotRegRight(register, amount) => {
                format!("Rotate R{:X} right by {}", register.as_index(), amount)
            }
            StructuredInstruction::JmpIfEqual(register, jmp_location) => format!(
                "Jump to {:02X}{}",
                jmp_location,
                if register == &Register::R0 {
                    "".to_string()
                } else {
                    format!(" if R{:X} == 0", register.as_index())
                }
            ),
            StructuredInstruction::Halt => "Halt execution".to_string(),
            StructuredInstruction::JumpWithComparison(
                cmpjmp_operator,
                jmp_location_reg,
                comparison_operand_reg,
            ) => format!(
                "Jump to address in R{:X}{}",
                jmp_location_reg.as_index(),
                if cmpjmp_operator == &CmpjmpOperator::Eq && comparison_operand_reg == &Register::R0
                {
                    "".to_string()
                } else {
                    format!(
                        " if R{:X} {} R0",
                        comparison_operand_reg.as_index(),
                        match cmpjmp_operator {
                            CmpjmpOperator::Eq => "=",
                            CmpjmpOperator::Ne => "≠",
                            CmpjmpOperator::Ge => "≥",
                            CmpjmpOperator::Le => "≤",
                            CmpjmpOperator::Gt => ">",
                            CmpjmpOperator::Lt => "<",
                        }
                    )
                }
            ),
        }
    }

    pub fn disasm(&self) -> String {
        match self {
            StructuredInstruction::Nop => "NOP".to_string(),
            StructuredInstruction::MovMemToReg(addr, register) => {
                format!("MOV [{:02X}] -> R{:X}", addr, register.as_index())
            }
            StructuredInstruction::MovImmToReg(val, register) => {
                format!("MOV {:02X} -> R{:X}", val, register.as_index())
            }
            StructuredInstruction::MovRegToMem(register, addr) => {
                format!("MOV R{:X} -> [{:02X}]", register.as_index(), addr)
            }
            StructuredInstruction::MovRegToReg { src, dst } => {
                format!("MOV R{:X} -> R{:X}", src.as_index(), dst.as_index(),)
            }
            StructuredInstruction::MovIndirectToReg { dst, src } => {
                format!("MOV [R{:X}] -> R{:X}", src.as_index(), dst.as_index(),)
            }
            StructuredInstruction::MovRegToIndirect { src, dst } => {
                format!("MOV R{:X} -> [R{:X}]", src.as_index(), dst.as_index(),)
            }
            StructuredInstruction::AddRegToRegInteger(dest, operand1, operand2) => format!(
                "ADDI R{:X}, R{:X} -> R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::AddRegToRegFloat(dest, operand1, operand2) => format!(
                "ADDF R{:X}, R{:X} -> R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::OrRegToReg(dest, operand1, operand2) => format!(
                "OR R{:X}, R{:X} -> R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::AndRegToReg(dest, operand1, operand2) => format!(
                "AND R{:X}, R{:X} -> R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::XorRegToReg(dest, operand1, operand2) => format!(
                "XOR R{:X}, R{:X} -> R{:X}",
                operand1.as_index(),
                operand2.as_index(),
                dest.as_index()
            ),
            StructuredInstruction::RotRegRight(register, amount) => {
                format!("ROT R{:X}, {}", register.as_index(), amount)
            }
            StructuredInstruction::JmpIfEqual(register, jmp_location) => {
                if register == &Register::R0 {
                    format!("JMP {:02X}", jmp_location)
                } else {
                    format!("JMPEQ R{:X}, {:02X}", register.as_index(), jmp_location)
                }
            }
            StructuredInstruction::Halt => "HALT".to_string(),
            StructuredInstruction::JumpWithComparison(
                cmpjmp_operator,
                jmp_location_reg,
                comparison_operand_reg,
            ) => {
                if cmpjmp_operator == &CmpjmpOperator::Eq && comparison_operand_reg == &Register::R0
                {
                    format!("JMP R{:X}", jmp_location_reg.as_index())
                } else {
                    format!(
                        "JMP{} R{:X}, R{:X}",
                        match cmpjmp_operator {
                            CmpjmpOperator::Eq => "EQ",
                            CmpjmpOperator::Ne => "NE",
                            CmpjmpOperator::Ge => "GE",
                            CmpjmpOperator::Le => "LE",
                            CmpjmpOperator::Gt => "GT",
                            CmpjmpOperator::Lt => "LT",
                        },
                        comparison_operand_reg.as_index(),
                        jmp_location_reg.as_index(),
                    )
                }
            }
        }
    }
}

pub(crate) fn assert_operands<'src, 'a, const EXPECTED: usize>(
    detail: &'a Spanned<InstructionDetail<'src>>,
) -> SerializeResult<&'a [Spanned<Operand<'src>>; EXPECTED]> {
    <&'a [Spanned<Operand<'src>>; EXPECTED] as TryFrom<&'a [Spanned<Operand<'src>>]>>::try_from(
        detail.inner.operands.as_ref(),
    )
    .map_err(|_| {
        let found = detail.operands.len();
        SerializationErrorMessage::UnexpectedOperandCount {
            expected: EXPECTED,
            found,
        }
        .with_span(
            detail
                .operands
                .first()
                .and_then(|first| {
                    detail
                        .operands
                        .last()
                        .map(|last| first.span.union(last.span))
                })
                .unwrap_or(detail.span),
        )
    })
}

pub(crate) fn assert_no_operands(detail: &Spanned<InstructionDetail>) -> SerializeResult<()> {
    assert_operands::<0>(detail).map(drop)
}

pub(crate) fn assert_no_destination_operand(
    detail: &Spanned<InstructionDetail>,
) -> SerializeResult<()> {
    if let Some(output) = detail.inner.output.as_ref() {
        Err(SerializationErrorMessage::UnexpectedDestinationOperand.with_span(output.span))
    } else {
        Ok(())
    }
}

pub(crate) fn assert_destination_operand<'src, 'a>(
    detail: &'a Spanned<InstructionDetail<'src>>,
) -> SerializeResult<&'a Spanned<OutputOperand<'src>>> {
    if let Some(output) = detail.output.as_ref() {
        Ok(output)
    } else {
        Err(SerializationErrorMessage::MissingDestinationOperand.with_span(detail.span))
    }
}

fn convert_mov(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    let [src] = assert_operands(detail)?;
    let dst = assert_destination_operand(detail)?;

    match (&src.inner, &dst.inner) {
        // 0x1rxy = load memory [xy] into register r
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(expr),
            },
            OutputOperand::Register(r),
        ) => Ok(StructuredInstruction::MovMemToReg(
            ctx.evaluate_constant_expr(expr)?,
            **r,
        )),
        // 0x2rxy = store value xy into register r
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(expr),
            },
            OutputOperand::Register(r),
        ) => Ok(StructuredInstruction::MovImmToReg(
            ctx.evaluate_constant_expr(expr)?,
            **r,
        )),
        // 0x3rxy = store value in register r into memory [xy]
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(r),
            },
            OutputOperand::ConstantDeref(addr),
        ) => Ok(StructuredInstruction::MovRegToMem(
            **r,
            ctx.evaluate_constant_expr(addr)?,
        )),
        // 0x40rs = move value from register r to register s
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(src_r),
            },
            OutputOperand::Register(dst_r),
        ) => Ok(StructuredInstruction::MovRegToReg {
            src: **src_r,
            dst: **dst_r,
        }),
        // 0xD0rs = load [register s] into register r
        // TODO: alternative extended instruction set? 0x41rs?
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(src_r),
            },
            OutputOperand::Register(dst_r),
        ) => Ok(StructuredInstruction::MovIndirectToReg {
            dst: **dst_r,
            src: **src_r,
        }),
        // 0x42rs = store register r into [register s]
        // TODO: alternative extended instruction set? 0x42rs?
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(src_r),
            },
            OutputOperand::RegisterDeref(dst_r),
        ) => Ok(StructuredInstruction::MovRegToIndirect {
            src: **src_r,
            dst: **dst_r,
        }),
        // TODO: alternative extended instruction set? 0x43rs = load [register r] and store into [register s]
        // (
        //     Operand {
        //         deref: true,
        //         core: CoreOperand::Register(src_r),
        //     },
        //     OutputOperand::RegisterDeref(dst_r),
        // ) => Ok([0x43, ((src_r.to_index() << 4) | dst_r.to_index())]),
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [Rr] -> [Rr] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV xy -> [Rr] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [xy] -> [Rr] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [xy] -> [xy] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV xy -> [xy] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [Rr] -> [xy] is not supported.".to_string(),
        )
        .with_span(src.span.union(dst.span))),
    }
}

fn convert_halt(
    detail: &Spanned<InstructionDetail>,
    _ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    assert_no_operands(detail)?;
    assert_no_destination_operand(detail)?;

    Ok(StructuredInstruction::Halt)
}

fn convert_nop(
    detail: &Spanned<InstructionDetail>,
    _ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    assert_no_operands(detail)?;
    assert_no_destination_operand(detail)?;

    Ok(StructuredInstruction::Nop)
}

fn convert_2regin_1regout(
    detail: &Spanned<InstructionDetail>,
    _ctx: &Context,
    opcode: u8,
) -> SerializeResult<StructuredInstruction> {
    let [src1, src2] = assert_operands(detail)?;
    let dst = assert_destination_operand(detail)?;

    let src1reg = match src1.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Source operands must be registers".to_string(),
            )
            .with_span(src1.span));
        }
    };
    let src2reg = match src2.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Second source operand must be a register".to_string(),
            )
            .with_span(src2.span));
        }
    };

    let dst_reg = match dst.inner {
        OutputOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Destination operand must be a register".to_string(),
            )
            .with_span(dst.span));
        }
    };

    Ok(match opcode {
        0x50 => StructuredInstruction::AddRegToRegInteger(*dst_reg, *src1reg, *src2reg),
        0x60 => StructuredInstruction::AddRegToRegFloat(*dst_reg, *src1reg, *src2reg),
        0x70 => StructuredInstruction::OrRegToReg(*dst_reg, *src1reg, *src2reg),
        0x80 => StructuredInstruction::AndRegToReg(*dst_reg, *src1reg, *src2reg),
        0x90 => StructuredInstruction::XorRegToReg(*dst_reg, *src1reg, *src2reg),
        _ => unreachable!(),
    })
}

fn convert_addi(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_2regin_1regout(detail, ctx, 0x50)
}

fn convert_addf(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_2regin_1regout(detail, ctx, 0x60)
}

fn convert_or(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_2regin_1regout(detail, ctx, 0x70)
}

fn convert_and(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_2regin_1regout(detail, ctx, 0x80)
}

fn convert_xor(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_2regin_1regout(detail, ctx, 0x90)
}

fn convert_rot(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    let [target, amount] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    let target_reg = match target.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Target of ROT must be a register".to_string(),
            )
            .with_span(target.span));
        }
    };

    let amount_constant = match &amount.inner.core {
        CoreOperand::Constant(expr) => ctx.evaluate_constant_expr(expr)?.rem_euclid(16),
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Amount operand of ROT must be an immediate constant expression".to_string(),
            )
            .with_span(amount.span));
        }
    };

    Ok(StructuredInstruction::RotRegRight(
        *target_reg,
        amount_constant,
    ))
}

fn cmpjmp(
    target: Register,
    comparison_operand: Register,
    operator: CmpjmpOperator,
) -> SerializeResult<StructuredInstruction> {
    Ok(StructuredInstruction::JumpWithComparison(
        operator,
        target,
        comparison_operand,
    ))
}

fn jmpeq(
    target: &Spanned<Operand>,
    comparison_operand: Register,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    match &target.inner {
        Operand {
            deref: false,
            core: CoreOperand::Constant(expr),
        } => {
            let addr = ctx.evaluate_constant_expr(expr)?;
            Ok(StructuredInstruction::JmpIfEqual(comparison_operand, addr))
        }
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => cmpjmp(**r, comparison_operand, CmpjmpOperator::Eq),
        _ => Err(SerializationErrorMessage::InvalidOperand(
            "Jump location of JMPEQ must be either a direct register or an immediate constant"
                .to_string(),
        )
        .with_span(target.span)),
    }
}

fn convert_jmp(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    let [jmp_location] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    jmpeq(jmp_location, Register::R0, ctx)
}

fn convert_jmpeq(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    let [jmp_location, comparison_operand] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    let comparison_operand_reg = match comparison_operand.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Comparison operand of JMPEQ must be a direct register".to_string(),
            )
            .with_span(comparison_operand.span));
        }
    };

    jmpeq(jmp_location, *comparison_operand_reg, ctx)
}

fn convert_cmpjmp(
    detail: &Spanned<InstructionDetail>,
    _ctx: &Context,
    operator: CmpjmpOperator,
) -> SerializeResult<StructuredInstruction> {
    let [jmp_location, comparison_operand] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    let jmp_location_reg = match jmp_location.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => return Err(SerializationErrorMessage::InvalidOperand(
            "Jump location must be a direct register, except for JMP and JMPEQ which allow an immediate location".to_string()
        ).with_span(jmp_location.span)),
    };

    let comparison_operand_reg = match comparison_operand.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Comparison operand must be a direct register".to_string(),
            )
            .with_span(comparison_operand.span));
        }
    };

    cmpjmp(*jmp_location_reg, *comparison_operand_reg, operator)
}

fn convert_jmpne(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_cmpjmp(detail, ctx, CmpjmpOperator::Ne)
}

fn convert_jmpge(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_cmpjmp(detail, ctx, CmpjmpOperator::Ge)
}

fn convert_jmple(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_cmpjmp(detail, ctx, CmpjmpOperator::Le)
}

fn convert_jmpgt(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_cmpjmp(detail, ctx, CmpjmpOperator::Gt)
}

fn convert_jmplt(
    detail: &Spanned<InstructionDetail>,
    ctx: &Context,
) -> SerializeResult<StructuredInstruction> {
    convert_cmpjmp(detail, ctx, CmpjmpOperator::Lt)
}

#[derive(Debug, PartialEq, Clone)]
pub enum ConvertedInstruction {
    Code(StructuredInstruction),
    Data(u8),
}

pub fn convert_instruction<'a>(
    instr: &Spanned<crate::parser::Instruction<'a>>,
    ctx: &Context,
) -> SerializeResult<ConvertedInstruction> {
    let res: ConvertedInstruction = ConvertedInstruction::Code(match instr.inner.mnemonic.inner.to_uppercase().as_str() {
        "CONST" => {
            unreachable!("CONST should have been skipped in an earlier pass.");
        }
        "DATA" => {
            let [expr] = assert_operands(&instr.inner.detail)?;
            assert_no_destination_operand(&instr.inner.detail)?;

            let value = match &expr.inner.core {
                CoreOperand::Constant(expr) => ctx.evaluate_constant_expr(expr)?,
                _ => {
                    return Err(SerializationErrorMessage::InvalidOperand(
                        "Operand of DATA must be an immediate constant expression".to_string(),
                    )
                    .with_span(expr.span));
                }
            };

            return Ok(ConvertedInstruction::Data(value));
        }
        _ => StructuredInstruction::from_ast(instr, ctx)?,
    });

    // if let ConvertedInstruction::Code(converted_instr) = &res {
    //     eprintln!(
    //         "mnemonic: {}, instruction: {:?}, serialized: {:04X}",
    //         instr.inner.mnemonic.inner,
    //         converted_instr,
    //         u16::from_be_bytes(converted_instr.as_bytes())
    //     );
    // }

    Ok(res)
}
