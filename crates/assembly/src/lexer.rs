use std::error;

use logos::{Filter, Logos};

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
    #[regex(r"R[0-9A-Fa-f]", |lex| match u8::from_str_radix(&lex.slice()[1..], 16).unwrap() {
        0x0 => Register::R0,
        0x1 => Register::R1,
        0x2 => Register::R2,
        0x3 => Register::R3,
        0x4 => Register::R4,
        0x5 => Register::R5,
        0x6 => Register::R6,
        0x7 => Register::R7,
        0x8 => Register::R8,
        0x9 => Register::R9,
        0xA => Register::RA,
        0xB => Register::RB,
        0xC => Register::RC,
        0xD => Register::RD,
        0xE => Register::RE,
        0xF => Register::RF,
        _ => unreachable!(),
    })]
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
        let res = dec_str.parse::<u8>().unwrap();
        res
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

fn handle_whitespace<'a>(lex: &mut logos::Lexer<'a, AsmToken<'a>>) -> Filter<()> {
    let slice = lex.slice();
    if slice.contains('\n') {
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum::FromRepr)]
#[repr(u8)]
pub enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    RA,
    RB,
    RC,
    RD,
    RE,
    RF,
}

#[cfg(test)]
mod tests {
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
