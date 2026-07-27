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
    Dollar, // $
    Var,

    LeftBracket,  // {
    RightBracket, // }

    Print,
    Printa,
}

pub struct Scanner {
    chars: Vec<char>,
    current_idx: usize,
}

impl Scanner {
    pub fn new(contents: String) -> Self {
        Self {
            chars: contents.chars().collect(),
            current_idx: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.current_idx).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.current_idx += 1;
        Some(c)
    }

    fn take_while(&mut self, pred: impl Fn(char) -> bool) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if !pred(c) {
                break;
            }
            s.push(c);
            self.current_idx += 1;
        }
        s
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.advance() {
            if c == ']' {
                return;
            }
        }
    }
}

pub fn lexer(mut scanner: Scanner) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    loop {
        let character = match scanner.advance() {
            Some(character) => character,
            None => {
                tokens.push(Token::EOF);
                break;
            }
        };

        if character.is_whitespace() {
            continue;
        };

        if character == '[' {
            scanner.skip_comment();
            continue;
        }

        // parsing digit
        if character.is_ascii_digit() || character == '.' {
            let mut partial = String::from(character);
            partial.push_str(&scanner.take_while(|c| c.is_ascii_digit() || c == '.'));
            //println!("{}", partial);
            let digit: f64 = partial
                .parse()
                .expect(format!("Invalid token: {}", partial).as_str());
            tokens.push(Token::Digit(digit));
            continue;
        }

        // parsing keywords and identifiers
        if character.is_alphabetic() || character == '_' {
            let mut partial = String::from(character);
            partial.push_str(&scanner.take_while(|c| c.is_alphanumeric() || c == '_'));

            if vec!["def", "dow", "if", "else", "print", "printa", "var"]
                .contains(&partial.as_str())
            {
                // keywords
                match partial.as_str() {
                    "def" => tokens.push(Token::Def),
                    "dow" => tokens.push(Token::Dow),
                    "if" => tokens.push(Token::If),
                    "else" => tokens.push(Token::Else),
                    "print" => tokens.push(Token::Print),
                    "printa" => tokens.push(Token::Printa),
                    "var" => tokens.push(Token::Var),
                    &_ => eprintln!("Undefined situation"),
                }
            } else {
                //identifiers
                tokens.push(Token::Identifier(partial));
            }
            continue;
        }

        if vec!['+', '-', '*', '/', '!', '=', '~', '>', '<'].contains(&character) {
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
    }
    tokens
}
