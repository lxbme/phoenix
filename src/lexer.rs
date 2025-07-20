
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    EOF,
    Placeholder,

    Operator(char),
    Identifier(String),
    Digit(f64),

    Def,
    If,
    Else,
    Dow,
    Dollar,           // $
    Var,

    LeftBracket,      // {
    RightBracket,     // }

    Print,
    Printa
}

#[derive(Clone)]
pub struct Scanner {
    contents: String,
    current_idx: usize
}

impl Scanner { 
    pub fn new(contents: String) -> Self {
	Self {contents, current_idx: 0}
    }

    pub fn get_char(&mut self) -> Option<char> {
	let result = self.contents.chars().nth(self.current_idx)?;
	self.current_idx += 1;
	Some(result)
    }
}

pub fn lexer(mut scanner:Scanner) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    loop {
	let character = match scanner.get_char() {
	    Some(character) => character,
	    None => {tokens.push(Token::EOF); break},
	};

	if character.is_whitespace() { continue };

	if character == '[' {
	    scanner = skip_content(scanner.clone());
	    continue;
	}
	
	// parsing digit
	if character.is_ascii_digit() || character == '.' {
	    let mut partial: String;
	    (scanner, partial) = consume_until_whitespace_content(scanner.clone());
	    partial.insert(0, character);
	    //println!("{}", partial);
	    let digit: f64 = partial.parse().expect(format!("Invalid token: {}", partial).as_str());
	    tokens.push(Token::Digit(digit));
	    continue;
	}

	if character.is_alphabetic() || character == '_' {
	    let mut partial: String;
	    (scanner, partial) = consume_until_whitespace_content(scanner.clone());
	    partial.insert(0, character);
	    if vec!["def", "dow", "if", "else", "print", "printa", "var"].contains(&partial.as_str()) {
		match partial.as_str() {
		    "def" => tokens.push(Token::Def),
		    "dow" => tokens.push(Token::Dow),
		    "if" => tokens.push(Token::If),
		    "else" => tokens.push(Token::Else),
		    "print" => tokens.push(Token::Print),
		    "printa" => tokens.push(Token::Printa),
		    "var" => tokens.push(Token::Var),
		    &_ => eprintln!("Undefined situation")
		}
	    } else {
		tokens.push(Token::Identifier(partial));
	    }
	    continue;
	}

	if  vec!['+', '-', '*', '/', '!', '=', '~', '>', '<'].contains(&character) {
	    tokens.push(Token::Operator(character));
	    continue;
	}

	if character == '{' {
	    tokens.push(Token::LeftBracket);
	    continue;
	}

	if character == '}' {
	    tokens.push(Token::RightBracket);
	    continue;
	}

	if character == '$' {
	    tokens.push(Token::Dollar);
	    continue;
	}
    };
    tokens
}

fn skip_content(mut scanner: Scanner) -> Scanner {
    loop {
	match scanner.get_char() {
	    Some(character) => {
		if character == ']' { break }; 
	    },
	    None => break,
	};
    }
    scanner
}

fn consume_until_whitespace_content(mut scanner: Scanner) -> (Scanner, String) {
    let mut partial: String = String::new();
    loop {
	let character = match scanner.get_char() {
	    Some(chara) => chara,
	    None => break,
	};

	if character.is_whitespace() || character == '[' { break }
	else { partial.push(character) };
    }
    scanner.current_idx -= 1;
    (scanner, partial)
}
