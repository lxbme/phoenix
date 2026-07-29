use colored::Colorize;
use std::fs;

mod analyzer;
mod compiler;
mod lexer;
mod vm;
use analyzer::analyzer;
use compiler::compiler;
use lexer::{Scanner, lexer};
use vm::run_opcode;

fn read_file(file_name: &String) -> String {
    let content =
        fs::read_to_string(file_name).expect(format!("Cannot read file: {}", file_name).as_str());
    content
}

fn main() {
    let content: String = read_file(&("./test_code/nested_test.sl".to_string()));
    let scanner = Scanner::new(content);
    let tokens = match lexer(scanner)  {
        Ok(tokens) => tokens,
        Err(e) => {println!("{:?}", e); return;},
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
