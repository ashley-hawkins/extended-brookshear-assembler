use std::{io::Read, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[clap(default_value = "-")]
    input: PathBuf,
    #[clap(short = 'o', long, default_value = "-")]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let file: String = if cli.input == *"-" {
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .expect("Failed to read from stdin");
        String::from_utf8(buffer).expect("Failed to convert stdin to string")
    } else {
        std::fs::read_to_string(&cli.input).expect("Failed to read input file")
    };

    let lines = brookshear_assembly::parser::parse_asm_file(&file).unwrap();
    let output =
        brookshear_assembly::serialize::serialize_program_from_text_to_text(&lines, &file).unwrap();

    if cli.output == *"-" {
        print!("{}", output);
    } else {
        std::fs::write(&cli.output, output).expect("Failed to write output file");
    }
}
