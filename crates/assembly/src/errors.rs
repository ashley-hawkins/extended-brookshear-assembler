use std::{borrow::Borrow, sync::LazyLock};

use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::{
    error::RichReason,
};
use regex::Regex;

use crate::{lexer::AsmToken, serialize::SerializationErrorMessage};


pub fn write_parse_errors<'src>(
    src: &'src str,
    file_name: String,
    errors: &[impl Borrow<crate::parser::Error<'src>>],
    w: &mut impl std::io::Write,
) {
    static C_STYLE_HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^0[xX][0-9a-fA-F]+$").unwrap());

    for error in errors {
        let error = error.borrow();
        Report::build(
            ReportKind::Error,
            (file_name.clone(), error.span().into_range()),
        )
        .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
        .with_message(match error.reason() {
            RichReason::ExpectedFound {
                expected: exp,
                found: Some(inner_found),
            } => {
                match **inner_found {
                    AsmToken::Ambiguous(ambig) => format!("Encountered ambiguous token: '{:?}'", ambig),
                    other_token => 
                    format!(
                        "Encountered unexpected token '{}'",
                        other_token,
                    )
            }}
            RichReason::ExpectedFound {
                expected: exp,
                found: None,
            } => 
                "Encountered unexpected end of input".to_owned()
            ,
            RichReason::Custom(s) => s.clone(),
        })
        .with_label(
            Label::new((file_name.clone(), error.span().into_range()))
                .with_message(error.reason().to_string())
                .with_color(Color::Red),
        )
        .with_helps(match error.reason() {
            RichReason::ExpectedFound {
                expected,
                found: Some(found),
            } => match (expected, **found) {
                (_, AsmToken::Ambiguous(ambig)) => {
                    if C_STYLE_HEX_RE.is_match(ambig) {
                        Some(format!("This token, \"{}\", looks like a C-style hexadecimal number. Hexadecimal numbers are written with no prefix, or optionally with the suffix _h, e.g. {}_h", ambig, &ambig[2..]))
                    } else  {
                        Some(
                            format!("This token, \"{}\", is ambiguous due to the fact that it contains characters that could be used\nin either a number or an identifier (such as a label), but it could not be determined which was\nintended. Usually this happens as a result of writing an identifier that starts with a digit\nsuch as \"0hello\" which is unfortunately not supported.", ambig),
                        )
                    }
                }
                _ => None,
            },
            _ => None,
        })
        .finish()
        .write(sources([(file_name.clone(), src)]), &mut *w)
        .unwrap()
    }
}

pub fn parse_errors_to_string<'src>(
    src: &'src str,
    file_name: String,
    errors: &[impl Borrow<crate::parser::Error<'src>>],
) -> String {
    let mut output = Vec::new();
    write_parse_errors(src, file_name, errors, &mut output);
    String::from_utf8(output).unwrap()
}

pub fn write_semantic_error(
    src: &str,
    file_name: String,
    error: &impl Borrow<crate::serialize::SerializationError>,
    w: &mut impl std::io::Write,
) {
    let error = error.borrow();
    Report::build(
        ReportKind::Error,
        (file_name.clone(), error.span.into_range()),
    )
    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
    .with_message(error.message.to_string())
    .with_label(
        Label::new((file_name.clone(), error.span.into_range()))
            .with_message(error.message.to_string())
            .with_color(Color::Red),
    )
    .with_labels(
        if let SerializationErrorMessage::UnlabeledConstant(Some(offset_span)) = &error.message {
            vec![Label::new((file_name.clone(), offset_span.into_range()))
                .with_message("This is not a valid symbolic name, as it is purely numeric and therefore has been interpreted as an offset.")
                .with_color(Color::Yellow)]
        } else {
            Vec::new()
        }
    )
    .with_helps(match &error.message {
        SerializationErrorMessage::UndefinedConstant(constant_name, valid_constants) => {
            let mut considered_similarities = valid_constants.iter().filter_map(|valid_constant| {
                let sim = rapidfuzz::distance::levenshtein::normalized_similarity(constant_name.to_lowercase().chars(), valid_constant.to_lowercase().chars());
                (sim > 0.55).then_some((valid_constant, sim))
            }).collect::<Vec<_>>();
            considered_similarities.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

            if considered_similarities.first().is_some_and(|(_, sim)| *sim == 1.0) {
                Some(format!(
                    "The symbolic name '{}' is not defined, but there is a symbolic name with the exact same spelling except for case.\nSymbolic names are case-sensitive, so be sure to match the exact case of the symbolic name you want to refer to.",
                    constant_name,
                ))
            } else {
                Some(format!(
                    "The symbolic name '{}' is not defined. Did you mean one of these? (Ranked by similarity):\n{}",
                    constant_name,
                    considered_similarities.iter().map(|(s, score)| format!("  - '{}' (similarity: {:.0}%)", s, score * 100.0)).collect::<Vec<_>>().join("\n")
                ))
            }
        },
        SerializationErrorMessage::UnlabeledConstant(Some(explicit_addr_span)) => {
            let span_text = &src[explicit_addr_span.into_range()];
            if span_text.chars().any(|c| !c.is_ascii_digit())  {
                Some("This constant pseudo-instruction does not have a symbolic name. It seems you tried to set a symbolic name and it was instead interpreted as an offset in hexadecimal.\nIn order to resolve this, you must disambiguate by using a symbolic name longer than 2 characters, or which contains at least one non-hexadecimal character.".to_string())
            } else {
                Some("This constant pseudo-instruction does not have a symbolic name, but it seems you tried to set a symbolic name and it was instead interpreted as an offset.\nSymbolic names must contain at least one non-decimal character or be longer than 3 characters.".to_string())
            }
        },
        SerializationErrorMessage::UnknownMnemonic(mnem) => {
            let valid_mnemonics = ["MOV", "HALT", "NOP", "ADDI", "ADDF", "AND", "OR", "XOR", "ROT", "JMP", "JMPEQ", "JMPNE", "JMPGE", "JMPLE", "JMPGT", "JMPLT", "DATA"];
            let mut considered_similarities = valid_mnemonics.iter().filter_map(|&valid_mnem| {
                let sim = rapidfuzz::distance::levenshtein::normalized_similarity(mnem.to_uppercase().chars(), valid_mnem.chars());
                (sim > 0.55).then_some((valid_mnem, sim))
            }).collect::<Vec<_>>();
            considered_similarities.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
            Some(format!("Did you mean one of these mnemonics? (Ranked by similarity):\n{}", considered_similarities.iter().map(|(s, score)| format!("  - '{}' (similarity: {:.0}%)", s, score * 100.0)).collect::<Vec<_>>().join("\n")))
        }
        _ => None,
    })
    .finish()
    .write(sources([(file_name.clone(), src)]), &mut *w)
    .unwrap()
}

pub fn semantic_errors_to_string<'src>(
    src: &'src str,
    file_name: String,
    errors: &[impl Borrow<crate::serialize::SerializationError>],
) -> String {
    let mut output = Vec::new();
    for error in errors {
        write_semantic_error(src, file_name.clone(), error, &mut output);
    }
    String::from_utf8(output).unwrap()
}
