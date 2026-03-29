use std::borrow::Borrow;

use ariadne::{Color, Label, Report, ReportKind, sources};
use chumsky::error::RichReason;

pub fn write_parse_errors<'src>(
    src: &'src str,
    file_name: String,
    errors: &[impl Borrow<crate::parser::Error<'src>>],
    w: &mut impl std::io::Write,
) {
    for error in errors {
        let error = error.borrow();
        Report::build(
            ReportKind::Error,
            (file_name.clone(), error.span().into_range()),
        )
        .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
        .with_message(match error.reason() {
            RichReason::ExpectedFound { expected: _, found } => {
                format!(
                    "Encountered unexpected {}",
                    match found {
                        Some(f) => format!("token {:?}", f),
                        None => "end of input".to_string(),
                    }
                )
            }
            RichReason::Custom(s) => s.clone(),
        })
        .with_label(
            Label::new((file_name.clone(), error.span().into_range()))
                .with_message(error.reason().to_string())
                .with_color(Color::Red),
        )
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

pub fn write_semantic_error<'src>(
    src: &'src str,
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
