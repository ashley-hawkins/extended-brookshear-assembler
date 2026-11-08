#![warn(clippy::indexing_slicing)]

pub mod common;
pub mod errors;
pub mod lexer;
pub mod parser;
pub mod serialize;
pub mod structured_instruction;

/// You probably don't want to use this. It's only here for brevity in unit tests.
pub fn infallible_program_str_to_binary(prog: &str) -> [u8; 256] {
    serialize::serialize_program_to_binary(
        &parser::parse_asm_file(prog).unwrap()
    ).unwrap()
}
