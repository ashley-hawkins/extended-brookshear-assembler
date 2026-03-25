use std::collections::{HashMap, HashSet};

use chumsky::span::{SimpleSpan, Spanned};

use crate::{
    lexer::Register,
    parser::{
        Annotation, ArithmeticOperator, Constant, ConstantExpr, CoreOperand, Instruction,
        InstructionDetail, Line, Operand, OutputOperand,
    },
};

struct Context<'a> {
    constants: HashMap<&'a str, u8>,
}

fn perform_arithmetic<'b>(
    left: &'b ConstantExpr,
    right: &'b ConstantExpr,
    operator: Spanned<ArithmeticOperator>,
    mut evaluate_expr: impl FnMut(&'b ConstantExpr) -> Result<u8, String>,
) -> Result<u8, String> {
    let left_val = evaluate_expr(left)?;
    let right_val = evaluate_expr(right)?;
    Ok(match operator.inner {
        ArithmeticOperator::Add => left_val.wrapping_add(right_val),
        ArithmeticOperator::Subtract => left_val.wrapping_sub(right_val),
        ArithmeticOperator::Multiply => left_val.wrapping_mul(right_val),
        ArithmeticOperator::Divide => left_val.wrapping_div(right_val),
        ArithmeticOperator::Modulo => left_val.wrapping_rem(right_val),
    })
}

impl<'a> Context<'a> {
    fn evaluate_constant_expr(&self, expr: &ConstantExpr) -> Result<u8, String> {
        match expr {
            ConstantExpr::Fundamental(Spanned {
                inner: Constant::Literal(val),
                ..
            }) => Ok(*val),
            ConstantExpr::Fundamental(Spanned {
                inner: Constant::Symbolic(symbol),
                ..
            }) => self
                .constants
                .get(symbol)
                .map(|val| {
                    eprintln!("Evaluating constant {} with value {}", symbol, val);
                    val
                })
                .cloned()
                .ok_or_else(|| format!("Undefined constant: {}", symbol)),
            ConstantExpr::Arithmetic {
                left,
                right,
                operator,
            } => perform_arithmetic(left, right, *operator, |subexpr| {
                self.evaluate_constant_expr(subexpr)
            }),
        }
    }
}

#[derive(thiserror::Error, Debug)]
enum SerializationError {
    #[error("Instruction requires exactly {expected} operands, but found {found}")]
    UnexpectedOperandCount { expected: usize, found: usize },
    #[error("Instruction does not take a destination operand, but one was provided")]
    UnexpectedDestinationOperand,
    #[error("Invalid operand combination for instruction: {0}")]
    InvalidOperandCombination(String),
}

// fn serialize_argument_combination(detail: &InstructionDetail) -> String {
//     let operands = detail
//         .operands
//         .iter()
//         .map(|op| {
//             let core = match &op.core {
//                 CoreOperand::Register(..) => format!("R{:X}", *r as u8),
//                 CoreOperand::Constant(..) => format!("Const({:?})", expr),
//             };
//             if op.deref {
//                 format!("[{}]", core)
//             } else {
//                 core
//             }
//         })
//         .collect::<Vec<_>>()
//         .join(", ");
// }

fn serialize_mov(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    if detail.operands.len() != 1 {
        return Err(format!(
            "MOV instruction requires exactly 1 regular operand (which is the source) and 1 destination operand, but found {} regular operands",
            detail.operands.len()
        ));
    }

    let src = &detail.operands[0];
    let dst = detail.output.as_ref().unwrap();

    match (&src.inner, &dst.inner) {
        // 0x1rxy = load memory [xy] into register r
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(expr),
            },
            OutputOperand::Register(r),
        ) => Ok([0x10 | **r as u8, ctx.evaluate_constant_expr(expr)?]),
        // 0x2rxy = store value xy into register r
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(expr),
            },
            OutputOperand::Register(r),
        ) => Ok([0x20 | **r as u8, ctx.evaluate_constant_expr(expr)?]),
        // 0x3rxy = store value in register r into memory [xy]
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(r),
            },
            OutputOperand::ConstantDeref(addr),
        ) => Ok([0x30 | **r as u8, ctx.evaluate_constant_expr(addr)?]),
        // 0x40rs = move value from register r to register s
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(src_r),
            },
            OutputOperand::Register(dst_r),
        ) => Ok([0x40, ((**src_r as u8) << 4) | **dst_r as u8]),
        // 0xD0rs = load [register s] into register r
        // TODO: alternative extended instruction set? 0x41rs?
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(dst_r),
            },
            OutputOperand::Register(src_r),
        ) => Ok([0xD0, ((**dst_r as u8) << 4) | **src_r as u8]),
        // 0x42rs = store register r into [register s]
        // TODO: alternative extended instruction set? 0x42rs?
        (
            Operand {
                deref: false,
                core: CoreOperand::Register(src_r),
            },
            OutputOperand::RegisterDeref(dst_r),
        ) => Ok([0xE0, ((**src_r as u8) << 4) | **dst_r as u8]),
        // TODO: alternative extended instruction set? 0x43rs = load [register r] and store into [register s]
        // (
        //     Operand {
        //         deref: true,
        //         core: CoreOperand::Register(src_r),
        //     },
        //     OutputOperand::RegisterDeref(dst_r),
        // ) => Ok([0x43, ((**src_r as u8) << 4) | **dst_r as u8]),
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(..),
            },
            OutputOperand::RegisterDeref(..)
        ) => Err("MOV [Rr] -> [Rr] is not supported.".to_string()),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err("MOV [xy] -> [xy] is not supported.".to_string()),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err("MOV xy -> [xy] is not supported.".to_string()),
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err("MOV [Rr] -> [xy] is not supported.".to_string()),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err("MOV [xy] -> [Rr] is not supported.".to_string()),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err("MOV xy -> [Rr] is not supported.".to_string()),
    }
}

fn serialize_halt(detail: &InstructionDetail, _ctx: &Context) -> Result<[u8; 2], String> {
    if !detail.operands.is_empty() {
        return Err(format!(
            "HALT instruction does not take any regular operands, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("HALT instruction does not take a destination operand".to_string());
    }

    Ok([0xC0, 0x00])
}

fn serialize_nop(detail: &InstructionDetail, _ctx: &Context) -> Result<[u8; 2], String> {
    if !detail.operands.is_empty() {
        return Err(format!(
            "NOP instruction does not take any regular operands, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("NOP instruction does not take a destination operand".to_string());
    }

    Ok([0x0F, 0xFF])
}

fn serialize_2regin_1regout(
    detail: &InstructionDetail,
    _ctx: &Context,
    opcode: u8,
) -> Result<[u8; 2], String> {
    if detail.operands.len() != 2 {
        return Err(format!(
            "Instruction requires exactly 2 regular operands, but found {}",
            detail.operands.len()
        ));
    }

    let src1 = &detail.operands[0];
    let src2 = &detail.operands[1];

    let src1reg = match src1.inner.core {
        CoreOperand::Register(r) => r,
        _ => return Err("First source operand must be a register".to_string()),
    };
    let src2reg = match src2.inner.core {
        CoreOperand::Register(r) => r,
        _ => return Err("Second source operand must be a register".to_string()),
    };

    let dst = detail.output.as_ref().ok_or_else(|| {
        "Instruction requires a destination operand, but none was provided".to_string()
    })?;
    let dst_reg = match dst.inner {
        OutputOperand::Register(r) => r,
        _ => return Err("Destination operand must be a register".to_string()),
    };

    Ok([
        opcode | *dst_reg as u8,
        (*src1reg as u8) << 4 | *src2reg as u8,
    ])
}

fn serialize_addi(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_2regin_1regout(detail, ctx, 0x50)
}

fn serialize_addf(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_2regin_1regout(detail, ctx, 0x60)
}

fn serialize_or(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_2regin_1regout(detail, ctx, 0x70)
}

fn serialize_and(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_2regin_1regout(detail, ctx, 0x80)
}

fn serialize_xor(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_2regin_1regout(detail, ctx, 0x90)
}

fn serialize_rot(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    if detail.operands.len() != 2 {
        return Err(format!(
            "ROT instruction requires exactly 2 regular operands, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("ROT instruction does not take a destination operand".to_string());
    }

    let target = &detail.operands[0];
    let target_reg = match target.inner.core {
        CoreOperand::Register(r) => r,
        _ => return Err("Target of ROT must be a register".to_string()),
    };

    let amount = &detail.operands[1];
    let amount_constant = match &amount.inner.core {
        CoreOperand::Constant(expr) => {
            let val = ctx.evaluate_constant_expr(expr)?;
            if val > 7 {
                return Err(format!(
                    "Rotation amount must be between 0 and 7, but got {}",
                    val
                ));
            }
            val
        }
        _ => return Err("Amount of ROT must be an immediate constant".to_string()),
    };

    Ok([0xA0 | *target_reg as u8, amount_constant as u8])
}

#[repr(u8)]
enum CmpjmpOperator {
    Eq,
    Ne,
    Ge,
    Le,
    Gt,
    Lt,
}

fn cmpjmp(
    target: Register,
    comparison_operand: Register,
    operator: CmpjmpOperator,
) -> Result<[u8; 2], String> {
    Ok([
        0xF0 | comparison_operand as u8,
        (operator as u8) << 4 | target as u8,
    ])
}

fn jmpeq(target: &Operand, comparison_operand: Register, ctx: &Context) -> Result<[u8; 2], String> {
    match target {
        Operand {
            deref: false,
            core: CoreOperand::Constant(expr),
        } => {
            let addr = ctx.evaluate_constant_expr(expr)?;
            if addr > 0x7F {
                return Err(format!(
                    "Jump address must be between 0 and 127, but got {}",
                    addr
                ));
            }
            Ok([0xB0 | comparison_operand as u8, addr])
        }
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => cmpjmp(**r, comparison_operand, CmpjmpOperator::Eq),
        _ => Err(
            "Jump location of JMPEQ must be either a direct register or an immediate constant"
                .to_string(),
        ),
    }
}

fn serialize_jmp(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    if detail.operands.len() != 1 {
        return Err(format!(
            "JMP instruction requires exactly 1 regular operand, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("JMP instruction does not take a destination operand".to_string());
    }

    let jmp_location = &detail.operands[0];

    jmpeq(&jmp_location.inner, Register::R0, ctx)
}

fn serialize_jmpeq(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    if detail.operands.len() != 2 {
        return Err(format!(
            "JMPEQ instruction requires exactly 2 regular operands, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("JMPEQ instruction does not take a destination operand".to_string());
    }

    let jmp_location = &detail.operands[0];

    let comparison_operand = &detail.operands[1];
    let comparison_operand_reg = match comparison_operand.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => return Err("Comparison operand of JMPEQ must be a direct register".to_string()),
    };

    jmpeq(&jmp_location.inner, *comparison_operand_reg, ctx)
}

fn serialize_cmpjmp(
    detail: &InstructionDetail,
    _ctx: &Context,
    operator: CmpjmpOperator,
) -> Result<[u8; 2], String> {
    if detail.operands.len() != 2 {
        return Err(format!(
            "Instruction requires exactly 2 regular operands, but found {}",
            detail.operands.len()
        ));
    }

    if detail.output.is_some() {
        return Err("Instruction does not take a destination operand".to_string());
    }

    let jmp_location = &detail.operands[0];
    let jmp_location_reg = match jmp_location.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => return Err("Jump location must be a direct register, except for JMP and JMPEQ which allow an immediate location".to_string()),
    };

    let comparison_operand = &detail.operands[1];
    let comparison_operand_reg = match comparison_operand.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => return Err("Comparison operand must be a direct register".to_string()),
    };

    cmpjmp(*jmp_location_reg, *comparison_operand_reg, operator)
}

fn serialize_jmpne(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Ne)
}

fn serialize_jmpge(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Ge)
}

fn serialize_jmple(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Le)
}

fn serialize_jmpgt(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Gt)
}

fn serialize_jmplt(detail: &InstructionDetail, ctx: &Context) -> Result<[u8; 2], String> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Lt)
}

const INSTRUCTION_SIZE: u32 = 2;
const INSTRUCTION_ALIGNMENT: u32 = INSTRUCTION_SIZE;

fn align(addr: u32, alignment_offset: u32) -> u32 {
    (addr - alignment_offset).next_multiple_of(INSTRUCTION_ALIGNMENT) + alignment_offset
}

enum SerializedInstruction {
    Code([u8; 2]),
    Data(u8),
}

fn serialize_instruction<'a>(
    instr: &Spanned<Instruction<'a>>,
    ctx: &Context,
) -> Result<SerializedInstruction, String> {
    Ok(SerializedInstruction::Code(
        match instr.inner.mnemonic.inner {
            "CONST" => {
                unreachable!("CONST should have been skipped in an earlier pass.");
            }
            "DATA" => {
                if instr.inner.detail.operands.len() != 1 {
                    panic!(
                        "DATA instruction requires exactly 1 operand, but found {}",
                        instr.inner.detail.operands.len()
                    );
                }

                if instr.inner.detail.output.is_some() {
                    panic!(
                        "DATA instruction does not take a destination operand, but one was provided"
                    );
                }

                let value = match &instr.inner.detail.operands[0].inner.core {
                    CoreOperand::Constant(expr) => {
                        ctx.evaluate_constant_expr(expr).unwrap_or_else(|err| {
                            panic!(
                                "Error evaluating constant expression for DATA at {:?}: {}",
                                instr.span, err
                            )
                        })
                    }
                    _ => panic!("Operand of DATA must be an immediate constant expression"),
                };

                return Ok(SerializedInstruction::Data(value));
            }
            "MOV" => serialize_mov(&instr.inner.detail, ctx)?,
            "HALT" => serialize_halt(&instr.inner.detail, ctx)?,
            "NOP" => serialize_nop(&instr.inner.detail, ctx)?,
            "ADDI" => serialize_addi(&instr.inner.detail, ctx)?,
            "ADDF" => serialize_addf(&instr.inner.detail, ctx)?,
            "OR" => serialize_or(&instr.inner.detail, ctx)?,
            "AND" => serialize_and(&instr.inner.detail, ctx)?,
            "XOR" => serialize_xor(&instr.inner.detail, ctx)?,
            "ROT" => serialize_rot(&instr.inner.detail, ctx)?,
            "JMP" => serialize_jmp(&instr.inner.detail, ctx)?,
            "JMPEQ" => serialize_jmpeq(&instr.inner.detail, ctx)?,
            "JMPNE" => serialize_jmpne(&instr.inner.detail, ctx)?,
            "JMPGE" => serialize_jmpge(&instr.inner.detail, ctx)?,
            "JMPLE" => serialize_jmple(&instr.inner.detail, ctx)?,
            "JMPGT" => serialize_jmpgt(&instr.inner.detail, ctx)?,
            "JMPLT" => serialize_jmplt(&instr.inner.detail, ctx)?,
            _ => panic!(
                "Unknown instruction mnemonic: {}",
                instr.inner.mnemonic.inner
            ),
        },
    ))
}

pub fn serialize_program(program: &[Spanned<Line>]) -> Vec<(u8, [(u8, Option<SimpleSpan>); 2])> {
    let mut waiting_labels = HashSet::new();
    let mut constants: HashMap<&str, u8> = HashMap::new();
    let mut pending_constants: HashMap<&str, &ConstantExpr> = HashMap::new();
    let mut segments = vec![];

    {
        let mut current_addr: u32 = 0;
        let mut just_set_addr = false;
        let mut prev_was_data = false;
        let mut current_segment = vec![];

        for line in program {
            match line.annotation {
                Some(Spanned {
                    inner: Annotation::Label(label),
                    ..
                }) => {
                    waiting_labels.insert(label);
                }
                Some(Spanned {
                    inner: Annotation::Offset(offset),
                    ..
                }) => {
                    segments.push(std::mem::take(&mut current_segment));
                    current_addr = offset as u32;
                    just_set_addr = true;
                }
                None => {}
            }

            if let Some(instr) = &line.instruction {
                match instr.inner.mnemonic.inner {
                    "CONST" => {
                        if instr.inner.detail.operands.len() != 1 {
                            panic!(
                                "CONST instruction requires exactly 1 operand, but found {}",
                                instr.inner.detail.operands.len()
                            );
                        }

                        if instr.inner.detail.output.is_some() {
                            panic!(
                                "CONST instruction does not take a destination operand, but one was provided"
                            );
                        }

                        let constant_expr = match &instr.inner.detail.operands[0].inner.core {
                            CoreOperand::Constant(expr) => expr,
                            _ => {
                                panic!("Operand of CONST must be an immediate constant expression")
                            }
                        };

                        for label in &waiting_labels {
                            pending_constants.insert(*label, constant_expr);
                        }
                        waiting_labels.clear();
                    }
                    "DATA" => {
                        for label in &waiting_labels {
                            constants.insert(
                                *label,
                                u8::try_from(current_addr).unwrap_or_else(|_| {
                                    panic!(
                                        "Address {} is too large to fit in a byte for constant {}",
                                        current_addr, label
                                    )
                                }),
                            );
                        }
                        waiting_labels.clear();
                        current_segment.push((current_addr, instr));
                        current_addr += 1;
                        prev_was_data = true;
                    }
                    "MOV" | "HALT" | "NOP" | "ADDI" | "ADDF" | "AND" | "OR" | "XOR" | "ROT"
                    | "JMP" | "JMPEQ" | "JMPNE" | "JMPGE" | "JMPLE" | "JMPGT" | "JMPLT" => {
                        if prev_was_data && !just_set_addr {
                            {
                                // start_new_run:
                                segments.push(std::mem::take(&mut current_segment));
                                current_addr = align(current_addr, 0);
                            }
                        }
                        // else {
                        //     current_addr =
                        //         align(current_addr, run_start_addr % INSTRUCTION_ALIGNMENT);
                        // }
                        // // Align to 2 bytes for instructions
                        // current_addr = align(current_addr, alignment_offset);

                        for label in &waiting_labels {
                            constants.insert(
                                *label,
                                u8::try_from(current_addr).unwrap_or_else(|_| {
                                    panic!(
                                        "Address {} is too large to fit in a byte for constant {}",
                                        current_addr, label
                                    )
                                }),
                            );
                        }
                        waiting_labels.clear();
                        current_segment.push((current_addr, instr));
                        current_addr += INSTRUCTION_SIZE;
                    }
                    _ => panic!(
                        "Unknown instruction mnemonic: {}",
                        instr.inner.mnemonic.inner
                    ),
                }
                just_set_addr = false;
            }
        }
        segments.push(current_segment);
    }

    evaluate_pending_constants(&mut constants, &pending_constants);

    let ctx = Context { constants };

    // let mut cells = [None; 128];
    // let mut ranges = vec![];
    // {
    //     let mut current_addr = 0u32;
    //     let mut just_set_addr = false;
    //     let mut prev_was_data = false;
    //     let mut run_start_addr = 0u32;
    //     for line in program {
    //         if let Some(Spanned {
    //             inner: Annotation::Offset(offset),
    //             ..
    //         }) = line.annotation
    //         {
    //             just_set_addr = true;
    //             {
    //                 // start_new_run
    //                 ranges.push(run_start_addr..current_addr);
    //                 run_start_addr = offset as u32;
    //             }
    //             current_addr = offset as u32;
    //         }

    //         if let Some(instr) = &line.instruction {
    //             let bytes = match instr.inner.mnemonic.inner {
    //                 "CONST" => continue,
    //                 "DATA" => {
    //                     if instr.inner.detail.operands.len() != 1 {
    //                         panic!(
    //                             "DATA instruction requires exactly 1 operand, but found {}",
    //                             instr.inner.detail.operands.len()
    //                         );
    //                     }

    //                     if instr.inner.detail.output.is_some() {
    //                         panic!(
    //                             "DATA instruction does not take a destination operand, but one was provided"
    //                         );
    //                     }

    //                     let value = match &instr.inner.detail.operands[0].inner.core {
    //                         CoreOperand::Constant(expr) => ctx.evaluate_constant_expr(&expr).unwrap_or_else(|err| panic!("Error evaluating constant expression for DATA at {:?}: {}", instr.span, err)),
    //                         _ => panic!("Operand of DATA must be an immediate constant expression"),
    //                     };

    //                     cells[current_addr as usize] = Some((value, Some(instr.span)));
    //                     current_addr += 1;
    //                     prev_was_data = true;
    //                     just_set_addr = false;
    //                     continue
    //                 }
    //                 "MOV" => serialize_mov(&instr.inner.detail, &ctx),
    //                 "HALT" => serialize_halt(&instr.inner.detail, &ctx),
    //                 "NOP" => serialize_nop(&instr.inner.detail, &ctx),
    //                 "ADDI" => serialize_addi(&instr.inner.detail, &ctx),
    //                 "ADDF" => serialize_addf(&instr.inner.detail, &ctx),
    //                 "OR" => serialize_or(&instr.inner.detail, &ctx),
    //                 "AND" => serialize_and(&instr.inner.detail, &ctx),
    //                 "XOR" => serialize_xor(&instr.inner.detail, &ctx),
    //                 "ROT" => serialize_rot(&instr.inner.detail, &ctx),
    //                 "JMP" => serialize_jmp(&instr.inner.detail, &ctx),
    //                 "JMPEQ" => serialize_jmpeq(&instr.inner.detail, &ctx),
    //                 "JMPNE" => serialize_jmpne(&instr.inner.detail, &ctx),
    //                 "JMPGE" => serialize_jmpge(&instr.inner.detail, &ctx),
    //                 "JMPLE" => serialize_jmple(&instr.inner.detail, &ctx),
    //                 "JMPGT" => serialize_jmpgt(&instr.inner.detail, &ctx),
    //                 "JMPLT" => serialize_jmplt(&instr.inner.detail, &ctx),
    //                 _ => panic!("Unknown instruction mnemonic: {}", instr.inner.mnemonic.inner),
    //             }.unwrap_or_else(|err| panic!("Error serializing instruction at {:?}: {}", instr.span, err));

    //             if prev_was_data && !just_set_addr {
    //                 {
    //                     // start_new_run:
    //                     ranges.push(run_start_addr..current_addr);
    //                     current_addr = align(current_addr, 0);
    //                     run_start_addr = current_addr;
    //                 }
    //             } else {
    //                 current_addr = align(current_addr, run_start_addr % INSTRUCTION_ALIGNMENT);
    //             }

    //             cells[current_addr as usize] = Some((bytes[0], Some(instr.span)));
    //             cells[current_addr as usize + 1] = Some((bytes[1], None));
    //             current_addr += INSTRUCTION_SIZE;

    //             println!(
    //                 "Serialized instruction at {:?} to bytes: {:02X} {:02X}",
    //                 instr.span, bytes[0], bytes[1]
    //             );

    //             prev_was_data = false;
    //             just_set_addr = false;
    //         }
    //     }
    //     ranges.push(run_start_addr..current_addr);
    // }

    let mut result = vec![];

    for segment in segments {
        let mut i = 0;
        while i < segment.len() {
            let (addr, instr) = &segment[i];
            let serialized = serialize_instruction(instr, &ctx).unwrap_or_else(|err| {
                panic!("Error serializing instruction at {:?}: {}", instr.span, err)
            });

            match serialized {
                SerializedInstruction::Code(bytes) => result.push((
                    u8::try_from(*addr).unwrap(),
                    [(bytes[0], Some(instr.span)), (bytes[1], None)],
                )),
                SerializedInstruction::Data(byte) => {
                    result.push((
                    u8::try_from(*addr).unwrap(),
                    [
                        (byte, Some(instr.span)),
                        segment.get(i + 1).map(|(next_addr, instr)| {
                            assert!(*next_addr == *addr + 1, "Expected consecutive data bytes at address {}, but found non-consecutive address {}", addr, next_addr);
                             match serialize_instruction(instr, &ctx).unwrap_or_else(|err| {
                                    panic!("Error serializing instruction at {:?}: {}", instr.span, err)
                             }) {
                            SerializedInstruction::Data(next_byte) => (next_byte, Some(instr.span)),
                            _ => panic!("Expected next instruction to be DATA for consecutive data bytes at address {}, but it was not", next_addr),
                        }}).unwrap_or((0x00, None)),
                    ],
                ));
                    i += 1;
                }
            }
            i += 1;
        }
    }

    result
}

pub fn serialize_program_from_text_to_text(
    program: &[Spanned<Line>],
    program_text: &str,
) -> String {
    let serialized = serialize_program(program);
    let mut result = String::new();
    for (addr, bytes) in serialized {
        let mut line = format!(
            "{:02X}: {:02X}{:02X} // {}\n",
            addr,
            bytes[0].0,
            bytes[1].0,
            &program_text[bytes[0].1.unwrap().start..bytes[0].1.unwrap().end]
        );
        if let Some(span) = bytes[1].1 {
            line.push_str(&format!(
                "         // {}\n",
                &program_text[span.start..span.end]
            ));
        }
        result.push_str(&line);
    }
    result
}

fn evaluate_pending_constants<'a, 'b: 'a>(
    constants: &mut HashMap<&'a str, u8>,
    pending_constants: &HashMap<&'a str, &'b ConstantExpr<'a>>,
) {
    for (label, expr) in pending_constants {
        if constants.contains_key(label) {
            continue; // Already resolved
        }
        eprintln!("Evaluating constant {} with expression {:?}", label, expr);
        let result =
            recursively_evaluate_pending_constants(constants, pending_constants, vec![label], expr);
        constants.insert(*label, result);
    }
}

fn recursively_evaluate_pending_constants<'a, 'b: 'a>(
    constants: &mut HashMap<&'a str, u8>,
    pending_constants: &HashMap<&'a str, &'b ConstantExpr<'a>>,
    stack: Vec<&'a str>,
    current_expr: &'b ConstantExpr<'a>,
) -> u8 {
    match current_expr {
        ConstantExpr::Fundamental(Spanned {
            inner: Constant::Literal(val),
            ..
        }) => *val,
        ConstantExpr::Fundamental(Spanned {
            inner: Constant::Symbolic(symbol),
            ..
        }) => {
            if let Some(resolved) = constants.get(symbol) {
                eprintln!("Using already resolved constant {} with value {}", symbol, resolved);
                *resolved
            } else if let Some(pending_expr) = pending_constants.get(symbol) {
                if stack.contains(symbol) {
                    let index = stack.iter().position(|s| *s == *symbol).unwrap();
                    panic!(
                        "Cyclic dependency detected in constant definitions. Dependency chain: {:?} which ends up depending on {}",
                        &stack[index..],
                        symbol
                    );
                }
                let mut new_stack = stack.clone();
                new_stack.push(symbol);
                let res = recursively_evaluate_pending_constants(
                    constants,
                    pending_constants,
                    new_stack,
                    pending_expr,
                );
                constants.insert(*symbol, res);
                eprintln!("Resolved constant {} to value {}", symbol, res);
                res
            } else {
                panic!("Undefined constant: {}", symbol);
            }
        }
        ConstantExpr::Arithmetic {
            left,
            right,
            operator,
        } => perform_arithmetic(left, right, *operator, |subexpr| {
            Ok(recursively_evaluate_pending_constants(
                constants,
                pending_constants,
                stack.clone(),
                subexpr,
            ))
        })
        .expect("Error evaluating constant expression"),
    }
}
