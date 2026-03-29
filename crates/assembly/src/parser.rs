use chumsky::{
    Parser as ChumskyParser,
    input::{Input as ChumskyInput, ValueInput},
    pratt::{infix, left},
    prelude::*,
    span::Span,
};
use logos::Logos;

use crate::{common::Register, lexer::AsmToken as Token};

pub trait Input<'a>: ValueInput<'a, Token = Token<'a>, Span = SimpleSpan> {}
impl<'a, T: ValueInput<'a, Token = Token<'a>, Span = SimpleSpan>> Input<'a> for T {}

pub type Error<'a> = Rich<'a, Token<'a>, SimpleSpan>;

pub trait Parser<'a, I: Input<'a>, O>:
    ChumskyParser<'a, I, O, extra::Err<Error<'a>>> + Clone
{
}

impl<'a, I: Input<'a>, O, P: ChumskyParser<'a, I, O, extra::Err<Error<'a>>> + Clone>
    Parser<'a, I, O> for P
{
}

#[derive(Logos, Debug, PartialEq)]
pub enum Annotation<'a> {
    Label(&'a str),
    Offset(u8),
}

#[derive(Debug)]
pub struct Line<'a> {
    pub annotation: Option<Spanned<Annotation<'a>>>,
    pub instruction: Option<Spanned<Instruction<'a>>>,
}

#[derive(Debug)]
pub struct Instruction<'a> {
    pub mnemonic: Spanned<&'a str>,
    pub detail: Spanned<InstructionDetail<'a>>,
}

#[derive(Debug)]
pub struct Operand<'a> {
    pub deref: bool,
    pub core: CoreOperand<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug)]
pub struct InstructionDetail<'a> {
    pub operands: Vec<Spanned<Operand<'a>>>,
    pub output: Option<Spanned<OutputOperand<'a>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConstantExpr<'a> {
    Fundamental(Spanned<Constant<'a>>),
    Arithmetic {
        left: Box<Spanned<ConstantExpr<'a>>>,
        right: Box<Spanned<ConstantExpr<'a>>>,
        operator: Spanned<ArithmeticOperator>,
    },
}

#[derive(Debug)]
pub enum CoreOperand<'a> {
    Register(Spanned<Register>),
    Constant(Spanned<ConstantExpr<'a>>),
}

#[derive(Debug)]
pub enum OutputOperand<'a> {
    Register(Spanned<Register>),
    RegisterDeref(Spanned<Register>),
    ConstantDeref(Spanned<ConstantExpr<'a>>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Constant<'a> {
    Literal(u8),
    Symbolic(&'a str),
}

fn instruction_parser<'src, I: Input<'src>>() -> impl Parser<'src, I, Vec<Spanned<Line<'src>>>> {
    let literal = select! {
        Token::LiteralHex(lit) => lit,
        Token::LiteralDec(lit) => lit,
        Token::LiteralBin(lit) => lit
    }
    .labelled("numeric literal");
    let ident = select! { Token::Identifier(ident) => ident }.labelled("identifier");
    let label = ident.then_ignore(just(Token::Colon)).labelled("label");
    let offset = literal.then_ignore(just(Token::Colon)).labelled("offset");

    let annotation = choice((label.map(Annotation::Label), offset.map(Annotation::Offset)))
        .spanned()
        .labelled("annotation");

    let constant = choice((
        literal.map(Constant::Literal),
        ident.map(Constant::Symbolic),
    ))
    .spanned()
    .labelled("constant");

    let constant_expression = recursive(|constant_expr| {
        let atom = choice((
            constant
                .map(|constant: Spanned<Constant<'src>>| {
                    ConstantExpr::Fundamental(Spanned {
                        inner: constant.inner,
                        span: constant.span,
                    })
                })
                .spanned(),
            constant_expr
                .clone()
                .delimited_by(just(Token::LeftParen), just(Token::RightParen)),
        ));

        fn fold_binary_operation<
            'src,
            'b,
            I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>,
        >(
            lhs: Spanned<ConstantExpr<'src>>,
            op: Spanned<Token<'src>>,
            rhs: Spanned<ConstantExpr<'src>>,
            extra: &mut chumsky::input::MapExtra<
                'src,
                'b,
                I,
                extra::Full<Rich<'src, Token<'src>, SimpleSpan>, (), ()>,
            >,
        ) -> Spanned<ConstantExpr<'src>> {
            Spanned {
                inner: ConstantExpr::Arithmetic {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    operator: Spanned {
                        inner: match op.inner {
                            Token::Add => ArithmeticOperator::Add,
                            Token::Subtract => ArithmeticOperator::Subtract,
                            Token::Multiply => ArithmeticOperator::Multiply,
                            Token::Divide => ArithmeticOperator::Divide,
                            Token::Modulo => ArithmeticOperator::Modulo,
                            _ => unreachable!(),
                        },
                        span: op.span,
                    },
                },
                span: extra.span(),
            }
        }

        let add_sub_operation = infix(
            left(4),
            one_of([Token::Add, Token::Subtract]).spanned(),
            fold_binary_operation,
        );

        let multiply_div_mod_operation = infix(
            left(5),
            one_of([Token::Multiply, Token::Divide, Token::Modulo]).spanned(),
            fold_binary_operation,
        );

        atom.pratt((multiply_div_mod_operation, add_sub_operation))
    });

    let register = select! { Token::Register(reg) => reg }
        .labelled("register")
        .spanned();

    let core_operand = choice((
        constant_expression.clone().map(CoreOperand::Constant),
        register.map(CoreOperand::Register),
    ));

    let operand = choice((
        core_operand
            .clone()
            .map(|core| Operand { deref: false, core }),
        core_operand
            .clone()
            .map(|core| Operand { deref: true, core })
            .delimited_by(just(Token::LeftBracket), just(Token::RightBracket)),
    ))
    .spanned();

    let into_operand = choice((
        register.map(OutputOperand::Register),
        choice((
            register.map(OutputOperand::RegisterDeref),
            constant_expression.map(OutputOperand::ConstantDeref),
        ))
        .delimited_by(just(Token::LeftBracket), just(Token::RightBracket)),
    ))
    .spanned();

    let instr = ident
        .spanned()
        .labelled("instruction mnemonic")
        .then(
            operand
                .separated_by(just(Token::Comma))
                .collect()
                .then(just(Token::Into).ignore_then(into_operand).or_not())
                .map(|(operands, output)| InstructionDetail { operands, output })
                .spanned(),
        )
        .map(|(mnemonic, detail)| Instruction { mnemonic, detail })
        .spanned();

    let line = annotation
        .or_not()
        .then(instr.or_not())
        .map(|(annotation, instruction)| Line {
            annotation,
            instruction,
        });

    line.spanned()
        .separated_by(just(Token::Newline).labelled("newline"))
        .allow_leading()
        .allow_trailing()
        .collect()
}

pub fn parse_asm_file<'a>(
    input: &'a str,
) -> Result<Vec<Spanned<Line<'a>>>, Vec<chumsky::error::Rich<'a, Token<'a>, SimpleSpan>>> {
    let mut lexer = Token::lexer(input).spanned();

    let tokens = std::iter::from_fn(move || {
        let raw_tok = lexer.next();

        raw_tok
            .map(|(tok, span)| tok.map(|tok| (tok, SimpleSpan::new((), span))))
            .map(|res| match res {
                Ok((tok, span)) => (tok, span),
                Err(()) => (Token::Error, SimpleSpan::new((), lexer.span())),
            })
    });

    let parser = instruction_parser();
    let stream = chumsky::input::Stream::from_iter(tokens).map(
        SimpleSpan::new((), input.len()..input.len()),
        |(tok, span)| (tok, span),
    );

    let res = parser.parse(stream);
    for err in res.errors() {
        eprintln!("Parse error: {:?}", err);
    }
    res.into_result()
}
