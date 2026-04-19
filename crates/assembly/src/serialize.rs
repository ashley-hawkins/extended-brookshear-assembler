use std::collections::{HashMap, HashSet};

use chumsky::span::{SimpleSpan, Spanned};

use crate::{
    parser::{
        Annotation, ArithmeticOperator, Constant, ConstantExpr, CoreOperand, Instruction, Line,
    },
    structured_instruction::{
        ConvertedInstruction, assert_no_destination_operand, assert_operands, convert_instruction,
    },
};

pub struct Context<'a> {
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
    pub fn evaluate_constant_expr(&self, expr: &Spanned<ConstantExpr>) -> SerializeResult<u8> {
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
                .inspect(|&_val| {
                    // eprintln!("Evaluating constant {} with value {}", symbol, val);
                })
                .cloned()
                .ok_or_else(|| {
                    let valid_constants: Vec<String> =
                        self.constants.keys().map(|k| k.to_string()).collect();
                    SerializationErrorMessage::UndefinedConstant(
                        symbol.to_string(),
                        valid_constants,
                    )
                    .with_span(expr.span)
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
pub enum SerializationErrorMessage {
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
    UndefinedConstant(String, Vec<String>), // name of the undefined constant, plus the list of valid constant names for suggestions
    #[error("Cyclic dependency detected in constant evaluation: {}", .0.join(" -> "))]
    CyclicDependency(Vec<String>),
    #[error("Unknown instruction mnemonic: {}", .0.to_uppercase())]
    UnknownMnemonic(String),
    #[error(
        "Exceeded memory limit of 256 bytes. This instruction would be placed at address {0}, which is out of bounds."
    )]
    MemoryLimitExceeded(u32),
    #[error("This constant pseudo-instruction does not have a label.")]
    UnlabeledConstant(Option<SimpleSpan>), // span of the last offset if it had just been set
    #[error("{0}")]
    UnknownError(String),
}

impl SerializationErrorMessage {
    pub fn with_span(self, span: SimpleSpan) -> SerializationError {
        SerializationError {
            message: self,
            span,
        }
    }
}

#[derive(Debug)]
pub struct SerializationError {
    pub message: SerializationErrorMessage,
    pub span: SimpleSpan,
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at span {:?}", self.message, self.span)
    }
}

pub type SerializeResult<T> = Result<T, SerializationError>;

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
    convert_instruction(instr, ctx).map(|converted| match converted {
        ConvertedInstruction::Code(instr) => SerializedInstruction::Code(instr.as_bytes()),
        ConvertedInstruction::Data(byte) => SerializedInstruction::Data(byte),
    })
}

pub fn serialize_program(
    program: &[Spanned<Line>],
) -> SerializeResult<Vec<(u8, [(u8, Option<SimpleSpan>); 2])>> {
    let mut waiting_labels = HashSet::new();
    let mut constants: HashMap<&str, u8> = HashMap::new();
    let mut pending_constants: HashMap<&str, &ConstantExpr> = HashMap::new();
    let mut segments = vec![];

    {
        let mut current_addr: u32 = 0;
        let mut just_set_addr_w_span = None;
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
                    span,
                }) => {
                    segments.push(std::mem::take(&mut current_segment));
                    current_addr = offset as u32;
                    just_set_addr_w_span = Some(Some(span));
                }
                None => {}
            }

            if let Some(instr) = &line.instruction {
                match instr.inner.mnemonic.inner.to_uppercase().as_str() {
                    "CONST" => {
                        let [expr] = assert_operands(&instr.inner.detail)?;
                        assert_no_destination_operand(&instr.inner.detail)?;

                        let constant_expr = match &expr.inner.core {
                            CoreOperand::Constant(expr) => expr,
                            _ => {
                                return Err(SerializationErrorMessage::InvalidOperand(
                                    "Operand of CONST must be an immediate constant expression"
                                        .to_string(),
                                )
                                .with_span(expr.span));
                            }
                        };

                        if waiting_labels.is_empty() {
                            return Err(SerializationErrorMessage::UnlabeledConstant(
                                just_set_addr_w_span.flatten(),
                            )
                            .with_span(instr.span));
                        }

                        for label in &waiting_labels {
                            pending_constants.insert(*label, constant_expr);
                        }
                        waiting_labels.clear();
                        just_set_addr_w_span.insert(None);
                        continue;
                    }
                    "DATA" => {
                        for arg in instr.inner.detail.operands.iter() {
                            for label in &waiting_labels {
                                constants.insert(
                                    *label,
                                    u8::try_from(current_addr).map_err(|_| {
                                        SerializationErrorMessage::InvalidOperand(format!(
                                            "Address {} is too large to fit in a byte for constant {}",
                                            current_addr, label
                                        ))
                                        .with_span(instr.mnemonic.span)
                                    })?,
                                );
                            }
                            waiting_labels.clear();
                            let mut fake_instr = (*instr).clone();
                            fake_instr.inner.detail.operands = vec![(*arg).clone()];
                            current_segment.push((current_addr, fake_instr));
                            current_addr += 1;
                        }
                        prev_was_data = true;
                    }
                    "MOV" | "HALT" | "NOP" | "ADDI" | "ADDF" | "AND" | "OR" | "XOR" | "ROT"
                    | "JMP" | "JMPEQ" | "JMPNE" | "JMPGE" | "JMPLE" | "JMPGT" | "JMPLT" => {
                        if prev_was_data && !just_set_addr_w_span.is_some() {
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
                                u8::try_from(current_addr).map_err(|_| {
                                    SerializationErrorMessage::InvalidOperand(format!(
                                        "Address {} is too large to fit in a byte for constant {}",
                                        current_addr, label
                                    ))
                                    .with_span(instr.mnemonic.span)
                                })?,
                            );
                        }
                        waiting_labels.clear();
                        current_segment.push((current_addr, (*instr).clone()));
                        current_addr += INSTRUCTION_SIZE;
                    }
                    _ => {
                        return Err(SerializationErrorMessage::UnknownMnemonic(
                            instr.inner.mnemonic.inner.to_string(),
                        )
                        .with_span(instr.mnemonic.span));
                    }
                }
                just_set_addr_w_span = None;
            }
        }
        segments.push(current_segment);
    }

    evaluate_pending_constants(&mut constants, &pending_constants)?;

    let ctx = Context { constants };

    let mut result = vec![];

    for segment in segments {
        let mut i = 0;
        while i < segment.len() {
            #[allow(clippy::indexing_slicing)] // i < segment.len() ensures this will not panic
            let (addr, instr) = &segment[i];
            let serialized = serialize_instruction(instr, &ctx)?;

            let addr = u8::try_from(*addr).map_err(|_| {
                SerializationErrorMessage::MemoryLimitExceeded(*addr).with_span(instr.mnemonic.span)
            })?;

            match serialized {
                SerializedInstruction::Code(bytes) => {
                    result.push((addr, [(bytes[0], Some(instr.span)), (bytes[1], None)]))
                }
                SerializedInstruction::Data(byte) => {
                    result.push((
                    addr,
                    [
                        (byte, Some(instr.span)),
                        segment.get(i + 1).map(|(next_addr, instr)| {
                            assert!(*next_addr == addr as u32 + 1, "Expected consecutive data bytes at address {}, but found non-consecutive address {}", addr, next_addr);
                             match serialize_instruction(instr, &ctx)? {
                            SerializedInstruction::Data(next_byte) => Ok((next_byte, Some(instr.span))),
                            _ => panic!("Expected next instruction to be DATA for consecutive data bytes at address {}, but it was not", next_addr), // true panic. this should never happen.
                        }}).unwrap_or(Ok((0x00, None)))?,
                    ],
                ));
                    i += 1;
                }
            }
            i += 1;
        }
    }

    Ok(result)
}

pub fn serialize_program_from_text_to_text(
    program: &[Spanned<Line>],
    program_text: &str,
) -> SerializeResult<String> {
    let serialized = serialize_program(program)?;
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
    Ok(result)
}

pub fn serialize_program_to_binary(program: &[Spanned<Line>]) -> SerializeResult<[u8; 256]> {
    let serialized = serialize_program(program)?;
    let mut result = [0u8; 256];
    for (addr, bytes) in serialized {
        result[addr as usize] = bytes[0].0;
        if let Some(b) = result.get_mut(addr as usize + 1) {
            *b = bytes[1].0
        };
    }
    Ok(result)
}

fn evaluate_pending_constants<'a, 'b: 'a>(
    constants: &mut HashMap<&'a str, u8>,
    pending_constants: &HashMap<&'a str, &'b ConstantExpr<'a>>,
) -> SerializeResult<()> {
    for (label, expr) in pending_constants {
        if constants.contains_key(label) {
            continue; // Already resolved
        }
        // eprintln!("Evaluating constant {} with expression {:?}", label, expr);
        let result = recursively_evaluate_pending_constants(
            constants,
            pending_constants,
            vec![label],
            expr,
        )?;
        constants.insert(*label, result);
    }
    Ok(())
}

fn recursively_evaluate_pending_constants<'a, 'b: 'a>(
    constants: &mut HashMap<&'a str, u8>,
    pending_constants: &HashMap<&'a str, &'b ConstantExpr<'a>>,
    stack: Vec<&'a str>,
    current_expr: &'b ConstantExpr<'a>,
) -> SerializeResult<u8> {
    match current_expr {
        ConstantExpr::Fundamental(Spanned {
            inner: Constant::Literal(val),
            ..
        }) => Ok(*val),
        ConstantExpr::Fundamental(Spanned {
            inner: Constant::Symbolic(symbol),
            span,
        }) => {
            if let Some(resolved) = constants.get(symbol) {
                eprintln!(
                    "Using already resolved constant {} with value {}",
                    symbol, resolved
                );
                Ok(*resolved)
            } else if let Some(pending_expr) = pending_constants.get(symbol) {
                if let Some(index) = stack.iter().position(|s| *s == *symbol) {
                    return Err(SerializationErrorMessage::CyclicDependency(
                        #[allow(clippy::indexing_slicing)]
                        // this index is guaranteed to be in bounds because it was obtained from position()
                        stack[index..]
                            .iter()
                            .cloned()
                            .chain(std::iter::once(*symbol))
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>(),
                    )
                    .with_span(*span));
                }
                let mut new_stack = stack.clone();
                new_stack.push(symbol);
                let res = recursively_evaluate_pending_constants(
                    constants,
                    pending_constants,
                    new_stack,
                    pending_expr,
                )?;
                constants.insert(*symbol, res);
                eprintln!("Resolved constant {} to value {}", symbol, res);
                Ok(res)
            } else {
                Err(SerializationErrorMessage::UndefinedConstant(
                    symbol.to_string(),
                    pending_constants.keys().map(|k| k.to_string()).collect(),
                )
                .with_span(*span))
            }
        }
        ConstantExpr::Arithmetic {
            left,
            right,
            operator,
        } => Ok(perform_arithmetic(left, right, *operator, |subexpr| {
            recursively_evaluate_pending_constants(
                constants,
                pending_constants,
                stack.clone(),
                subexpr,
            )
        })?),
    }
}
