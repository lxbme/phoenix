use std::fs;
mod lexer;
mod analyzer;
mod compiler;
use lexer::{lexer, Scanner};
use analyzer::analyzer;
use compiler::compiler;

fn read_file(file_name: &String) -> String {
    let content = fs::read_to_string(file_name)
	.expect(format!("Cannot read file: {}", file_name).as_str());
    content
}

fn main() {
    let content: String = read_file(&("./nested_test.sl".to_string()));
    let scanner = Scanner::new(content);
    let tokens = lexer(scanner);
    
     println!("Origin tokens: {:?} \n", tokens);
    match analyzer(&tokens) {
	Ok(_) => println!("Grammar check passed."),
	Err(e) => eprintln!("{}", e)
    };
    match compiler(tokens) {
	Ok(opcodes) => println!{"Opcode: {:?}", opcodes},
	Err(e) => eprintln!("{}", e)
    };
}
