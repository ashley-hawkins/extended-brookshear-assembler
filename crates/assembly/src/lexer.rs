use logos::{Filter, Logos};

use crate::common::Register;

#[derive(Debug, PartialEq, Clone, Copy, Logos)]
#[logos(subpattern single_whitespace = r"[ \t\n]")]
#[logos(subpattern block_comment = r"/\*([^*]|\*+[^*/])*\*+/")]
#[logos(subpattern line_comment = r"//[^\n]*")]
#[logos(skip(
    r"(?&block_comment)|(?&line_comment)",
    priority = 0,
    allow_greedy = true
))]
pub enum AsmToken<'a> {
    #[regex(r"R[0-9A-Fa-f]", |lex| Register::from_repr(u8::from_str_radix(&lex.slice()[1..], 16).unwrap()).unwrap())]
    Register(Register),
    #[regex(r"[0-9A-Fa-f]{1,2}(_h)?", callback = |lex| {
        let slice = lex.slice();
        let hex_str = slice.strip_suffix("_h").unwrap_or(slice);
        u8::from_str_radix(hex_str, 16).unwrap()
    })]
    LiteralHex(u8),
    #[regex(r"[0-9]{1,3}_d", callback = |lex| {
        let slice = lex.slice();
        let dec_str = &slice[..slice.len() - 2];
        dec_str.parse::<u8>().unwrap()
    })]
    LiteralDec(u8),
    #[regex(r"[01]{8}|[01]{1,8}_b", callback = |lex| {
        let slice = lex.slice();
        let bin_str = slice.strip_suffix("_b").unwrap_or(slice);
        u8::from_str_radix(bin_str, 2).unwrap()
    })]
    LiteralBin(u8),
    #[regex(r"[A-Za-z_][A-Za-z0-9_][A-Za-gi-z0-9_]|[A-Za-z_][A-Za-z0-9_]{3,}")]
    Identifier(&'a str),
    // Catch anything that could've been a numeric or identifier but wasn't matched by the above patterns
    #[regex(r"[A-Za-z0-9_]+", priority = 0)]
    Ambiguous(&'a str),
    #[token("+")]
    Add,
    #[token("-")]
    Subtract,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("%")]
    Modulo,
    #[token("->")]
    Into,
    #[token(",")]
    Comma,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token(":")]
    Colon,
    #[regex(
        r"((?&single_whitespace)|(?&block_comment)|(?&line_comment))+",
        handle_whitespace
    )]
    Newline,
    Unrecognized,
}

impl std::fmt::Display for AsmToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsmToken::Register(reg) => write!(f, "{:?}", *reg),
            AsmToken::LiteralHex(lit) => write!(f, "{:02X}_h", lit),
            AsmToken::LiteralDec(lit) => write!(f, "{}_d", lit),
            AsmToken::LiteralBin(lit) => write!(f, "{:08b}_b", lit),
            AsmToken::Identifier(ident) | AsmToken::Ambiguous(ident) => write!(f, "{}", ident),
            AsmToken::Add => write!(f, "+"),
            AsmToken::Subtract => write!(f, "-"),
            AsmToken::Multiply => write!(f, "*"),
            AsmToken::Divide => write!(f, "/"),
            AsmToken::Modulo => write!(f, "%"),
            AsmToken::Into => write!(f, "->"),
            AsmToken::Comma => write!(f, ","),
            AsmToken::LeftBracket => write!(f, "["),
            AsmToken::RightBracket => write!(f, "]"),
            AsmToken::LeftParen => write!(f, "("),
            AsmToken::RightParen => write!(f, ")"),
            AsmToken::Colon => write!(f, ":"),
            AsmToken::Newline => write!(f, "\\n"),
            AsmToken::Unrecognized => write!(f, "<unrecognized>"),
        }
    }
}

fn handle_whitespace<'a>(lex: &mut logos::Lexer<'a, AsmToken<'a>>) -> Filter<()> {
    let slice = lex.slice();
    if slice.contains('\n') {
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_asm_tokenizer() {
        let input = "MOV R1, [R2]";
        let tokens: Vec<AsmToken> = AsmToken::lexer(input).collect::<Result<_, _>>().unwrap();
        eprintln!("{:?}", tokens);
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], AsmToken::Identifier("MOV")));
        assert!(matches!(tokens[1], AsmToken::Register(Register::R1)));
        assert!(matches!(tokens[2], AsmToken::Comma));
        assert!(matches!(tokens[3], AsmToken::LeftBracket));
        assert!(matches!(tokens[4], AsmToken::Register(Register::R2)));
        assert!(matches!(tokens[5], AsmToken::RightBracket));
    }
}
