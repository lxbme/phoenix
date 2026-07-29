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
use lexer::{Scanner, lexer};
use vm::run_opcode;

fn read_file(file_name: &str) -> String {
    let content =
        fs::read_to_string(file_name).expect(format!("Cannot read file: {}", file_name).as_str());
    content
}

fn main() {
    let path = "./test_code/nested_test.sl".to_string();
    let content: String = read_file(&path);
    let source = source::Source::new(path, content);
    let scanner = Scanner::new(&source);
    let tokens = match lexer(scanner) {
        Ok(tokens) => tokens,
        Err(e) => {
            println!("{:?}", e);
            return;
        }
    };

    //println!("Origin tokens: {:?} \n", tokens);
    match analyzer(&tokens) {
        Ok(_) => println!("{}", "[Info] Grammar check passed.".green()),
        //Ok(_) => {}
        Err(e) => eprintln!("[ERROR] {}", e.red()),
    };
    let opcodes = match compiler(tokens) {
        //Ok(opcodes) => println!{"Opcode: {:?}", opcodes},
        Ok(opcodes) => opcodes,
        Err(e) => {
            eprintln!("[ERROR] {}", e.red());
            return;
        }
    };
    match run_opcode(opcodes) {
        Ok(_) => println!("{}", "\n[Info] finished...".green()),
        Err(e) => eprintln!("[ERROR] {}", e.red()),
    }
}
