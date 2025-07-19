use std::fs;
mod lexer;
use lexer::{lexer, Scanner};

fn read_file(file_name: &String) -> String {
    let content = fs::read_to_string(file_name)
	.expect(format!("Cannot read file: {}", file_name).as_str());
    content
}

fn main() {
    let content: String = read_file(&("./testcode.sl".to_string()));
    let scanner = Scanner::new(content);
    let tokens = lexer(scanner);
    
    println!("{:?}", tokens);
}
