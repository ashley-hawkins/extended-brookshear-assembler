use std::{
    collections::{HashMap, HashSet},
    ffi::os_str::Display,
};

use chumsky::span::{SimpleSpan, Span, Spanned};

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
    left: &'b Spanned<ConstantExpr>,
    right: &'b Spanned<ConstantExpr>,
    operator: Spanned<ArithmeticOperator>,
    mut evaluate_expr: impl FnMut(&'b Spanned<ConstantExpr>) -> SerializeResult<u8>,
) -> SerializeResult<u8> {
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
    fn evaluate_constant_expr(&self, expr: &Spanned<ConstantExpr>) -> SerializeResult<u8> {
        match &expr.inner {
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
                .ok_or_else(|| {
                    SerializationErrorMessage::UndefinedConstant(symbol.to_string())
                        .with_span(Some(expr.span))
                }),
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
enum SerializationErrorMessage {
    #[error("Instruction requires exactly {expected} operands, but found {found}")]
    UnexpectedOperandCount { expected: usize, found: usize },
    #[error("Instruction does not take a destination operand, but one was provided")]
    UnexpectedDestinationOperand,
    #[error("Instruction requires a destination operand, but none was provided")]
    MissingDestinationOperand,
    #[error("Invalid operand combination: {0}")]
    InvalidOperandCombination(String),
    #[error("Invalid operand: {0}")]
    InvalidOperand(String),
    #[error("Undefined constant: {0}")]
    UndefinedConstant(String),
    #[error("{0}")]
    UnknownError(String),
}

impl SerializationErrorMessage {
    fn with_span(self, span: Option<SimpleSpan>) -> SerializationError {
        SerializationError {
            message: self,
            span,
        }
    }
}

#[derive(Debug)]
struct SerializationError {
    message: SerializationErrorMessage,
    span: Option<SimpleSpan>,
}

type SerializeResult<T> = Result<T, SerializationError>;

fn assert_operands<'src, 'a, const EXPECTED: usize>(
    detail: &'a InstructionDetail<'src>,
) -> SerializeResult<&'a [Spanned<Operand<'src>>; EXPECTED]> {
    <&'a [Spanned<Operand<'src>>; EXPECTED] as TryFrom<&'a [Spanned<Operand<'src>>]>>::try_from(
        detail.operands.as_ref(),
    )
    .map_err(|e| {
        let found = detail.operands.len();
        SerializationErrorMessage::UnexpectedOperandCount {
            expected: EXPECTED,
            found,
        }
        .with_span(Some(SimpleSpan::new(
            (),
            detail.operands.first().unwrap().span.start..detail.operands.last().unwrap().span.end,
        )))
    })
}

fn assert_no_operands(detail: &InstructionDetail) -> SerializeResult<()> {
    assert_operands::<0>(detail).map(drop)
}

fn assert_no_destination_operand(detail: &InstructionDetail) -> SerializeResult<()> {
    if let Some(output) = detail.output.as_ref() {
        Err(SerializationErrorMessage::UnexpectedDestinationOperand.with_span(Some(output.span)))
    } else {
        Ok(())
    }
}

fn assert_destination_operand<'src, 'a>(
    detail: &'a InstructionDetail<'src>,
) -> SerializeResult<&'a Spanned<OutputOperand<'src>>> {
    if let Some(output) = detail.output.as_ref() {
        Ok(output)
    } else {
        Err(SerializationErrorMessage::MissingDestinationOperand.with_span(None))
    }
}

fn serialize_mov(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    let [src] = assert_operands(detail)?;

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
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [Rr] -> [Rr] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV xy -> [Rr] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::RegisterDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [xy] -> [Rr] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [xy] -> [xy] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
        (
            Operand {
                deref: false,
                core: CoreOperand::Constant(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV xy -> [xy] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
        (
            Operand {
                deref: true,
                core: CoreOperand::Register(..),
            },
            OutputOperand::ConstantDeref(..),
        ) => Err(SerializationErrorMessage::InvalidOperandCombination(
            "MOV [Rr] -> [xy] is not supported.".to_string(),
        )
        .with_span(Some(src.span.union(dst.span)))),
    }
}

fn serialize_halt(detail: &InstructionDetail, _ctx: &Context) -> SerializeResult<[u8; 2]> {
    assert_no_operands(detail)?;
    assert_no_destination_operand(detail)?;

    Ok([0xC0, 0x00])
}

fn serialize_nop(detail: &InstructionDetail, _ctx: &Context) -> SerializeResult<[u8; 2]> {
    assert_no_operands(detail)?;
    assert_no_destination_operand(detail)?;

    Ok([0x0F, 0xFF])
}

fn serialize_2regin_1regout(
    detail: &InstructionDetail,
    _ctx: &Context,
    opcode: u8,
) -> SerializeResult<[u8; 2]> {
    let [src1, src2] = assert_operands(detail)?;
    let dst = assert_destination_operand(detail)?;

    let src1reg = match src1.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Source operands must be registers".to_string(),
            )
            .with_span(Some(src1.span)));
        }
    };
    let src2reg = match src2.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Second source operand must be a register".to_string(),
            )
            .with_span(Some(src2.span)));
        }
    };

    let dst_reg = match dst.inner {
        OutputOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Destination operand must be a register".to_string(),
            )
            .with_span(Some(dst.span)));
        }
    };

    Ok([
        opcode | *dst_reg as u8,
        (*src1reg as u8) << 4 | *src2reg as u8,
    ])
}

fn serialize_addi(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_2regin_1regout(detail, ctx, 0x50)
}

fn serialize_addf(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_2regin_1regout(detail, ctx, 0x60)
}

fn serialize_or(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_2regin_1regout(detail, ctx, 0x70)
}

fn serialize_and(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_2regin_1regout(detail, ctx, 0x80)
}

fn serialize_xor(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_2regin_1regout(detail, ctx, 0x90)
}

fn serialize_rot(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    let [target, amount] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    let target_reg = match target.inner.core {
        CoreOperand::Register(r) => r,
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Target of ROT must be a register".to_string(),
            )
            .with_span(Some(target.span)));
        }
    };

    let amount_constant = match &amount.inner.core {
        CoreOperand::Constant(expr) => ctx.evaluate_constant_expr(expr)?.rem_euclid(16),
        _ => {
            return Err(SerializationErrorMessage::InvalidOperand(
                "Amount operand of ROT must be an immediate constant expression".to_string(),
            )
            .with_span(Some(amount.span)));
        }
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
) -> SerializeResult<[u8; 2]> {
    Ok([
        0xF0 | comparison_operand as u8,
        (operator as u8) << 4 | target as u8,
    ])
}

fn jmpeq(
    target: &Spanned<Operand>,
    comparison_operand: Register,
    ctx: &Context,
) -> SerializeResult<[u8; 2]> {
    match &target.inner {
        Operand {
            deref: false,
            core: CoreOperand::Constant(expr),
        } => {
            let addr = ctx.evaluate_constant_expr(expr)?;
            Ok([0xB0 | comparison_operand as u8, addr])
        }
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => cmpjmp(**r, comparison_operand, CmpjmpOperator::Eq),
        _ => Err(SerializationErrorMessage::InvalidOperand(
            "Jump location of JMPEQ must be either a direct register or an immediate constant"
                .to_string(),
        )
        .with_span(Some(target.span))),
    }
}

fn serialize_jmp(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    let [jmp_location] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    jmpeq(jmp_location, Register::R0, ctx)
}

fn serialize_jmpeq(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
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
            .with_span(Some(comparison_operand.span)));
        }
    };

    jmpeq(jmp_location, *comparison_operand_reg, ctx)
}

fn serialize_cmpjmp(
    detail: &InstructionDetail,
    _ctx: &Context,
    operator: CmpjmpOperator,
) -> SerializeResult<[u8; 2]> {
    let [jmp_location, comparison_operand] = assert_operands(detail)?;
    assert_no_destination_operand(detail)?;

    let jmp_location_reg = match jmp_location.inner {
        Operand {
            deref: false,
            core: CoreOperand::Register(r),
        } => r,
        _ => return Err(SerializationErrorMessage::InvalidOperand("Jump location must be a direct register, except for JMP and JMPEQ which allow an immediate location".to_string()).with_span(Some(jmp_location.span))),
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
            .with_span(Some(comparison_operand.span)));
        }
    };

    cmpjmp(*jmp_location_reg, *comparison_operand_reg, operator)
}

fn serialize_jmpne(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Ne)
}

fn serialize_jmpge(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Ge)
}

fn serialize_jmple(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Le)
}

fn serialize_jmpgt(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
    serialize_cmpjmp(detail, ctx, CmpjmpOperator::Gt)
}

fn serialize_jmplt(detail: &InstructionDetail, ctx: &Context) -> SerializeResult<[u8; 2]> {
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
) -> SerializeResult<SerializedInstruction> {
    Ok(SerializedInstruction::Code(
        match instr.inner.mnemonic.inner {
            "CONST" => {
                unreachable!("CONST should have been skipped in an earlier pass.");
            }
            "DATA" => {
                let [expr] = assert_operands(&instr.inner.detail)?;
                assert_no_destination_operand(&instr.inner.detail)?;

                let value = match &expr.inner.core {
                    CoreOperand::Constant(expr) => {
                        ctx.evaluate_constant_expr(expr).unwrap_or_else(|err| {
                            panic!(
                                "Error evaluating constant expression for DATA at {:?}: {}",
                                instr.span, err.message
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
                        let [expr] = assert_operands(&instr.inner.detail)
                            .unwrap_or_else(|err| panic!("{}", err.message));
                        assert_no_destination_operand(&instr.inner.detail)
                            .unwrap_or_else(|err| panic!("{}", err.message));

                        let constant_expr = match &expr.inner.core {
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

    let mut result = vec![];

    for segment in segments {
        let mut i = 0;
        while i < segment.len() {
            let (addr, instr) = &segment[i];
            let serialized = serialize_instruction(instr, &ctx).unwrap_or_else(|err| {
                panic!(
                    "Error serializing instruction at {:?}: {}",
                    instr.span, err.message
                )
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
                                    panic!("Error serializing instruction at {:?}: {}", instr.span, err.message)
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
                eprintln!(
                    "Using already resolved constant {} with value {}",
                    symbol, resolved
                );
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
