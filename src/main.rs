use std::fs;

mod lexer;
mod analyzer;
mod compiler;
mod vm;
use lexer::{lexer, Scanner};
use analyzer::analyzer;
use compiler::compiler;
use vm::run_opcode;

fn read_file(file_name: &String) -> String {
    let content = fs::read_to_string(file_name)
	.expect(format!("Cannot read file: {}", file_name).as_str());
    content
}

fn main() {
    let content: String = read_file(&("./nested_test.sl".to_string()));
    let scanner = Scanner::new(content);
    let tokens = lexer(scanner);
    
    //println!("Origin tokens: {:?} \n", tokens);
    match analyzer(&tokens) {
	Ok(_) => println!("Grammar check passed."),
	Err(e) => eprintln!("{}", e)
    };
    let opcodes = match compiler(tokens) {
	//Ok(opcodes) => println!{"Opcode: {:?}", opcodes},
	Ok(opcodes) => opcodes,
	Err(e) => {eprintln!("{}", e); return ;}
    };
    match run_opcode(opcodes) {
	Ok(_) => println!("\nfinish running"),
	Err(e) => eprintln!("{}", e)
    }
}
