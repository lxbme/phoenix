use colored::Colorize;
use std::fs;

mod analyzer;
mod compiler;
mod diag;
mod lexer;
mod source;
mod vm;
use analyzer::analyzer;
use compiler::compiler;
use diag::report;
use lexer::lexer;
use source::Source;
use vm::run_opcode;

fn read_file(file_name: &str) -> String {
    fs::read_to_string(file_name).unwrap_or_else(|_| panic!("Cannot read file: {}", file_name))
}

fn main() {
    let path = "./test_code/nested_test.sl";
    let source = Source::new(path.to_string(), read_file(path));

    // Each stage collects every diagnostic it can find, but a failing stage
    // stops the pipeline: feeding a broken token stream downstream would only
    // produce cascades of imaginary errors.
    let tokens = match lexer(&source) {
        Ok(tokens) => tokens,
        Err(diags) => {
            report(&source, &diags);
            return;
        }
    };

    if let Err(diags) = analyzer(&tokens) {
        report(&source, &diags);
        return;
    }
    println!("{}", "[Info] Grammar check passed.".green());

    let opcodes = match compiler(tokens) {
        Ok(opcodes) => opcodes,
        Err(diags) => {
            report(&source, &diags);
            return;
        }
    };

    match run_opcode(opcodes) {
        Ok(_) => println!("{}", "\n[Info] finished...".green()),
        Err(e) => eprintln!("[ERROR] {}", e.red()),
    }
}
