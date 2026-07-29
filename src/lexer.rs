use crate::source::Source;

#[derive(Debug)]
pub enum LexerError {
    InvalidToken { token: String, at: usize },
    UnexpectedChar { ch: char, at: usize },
}

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

pub struct Scanner<'a> {
    source: &'a Source,
    current_idx: usize,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a Source) -> Self {
        Self {
            source,
            current_idx: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.char_at(self.current_idx)
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

pub fn lexer(mut scanner: Scanner) -> Result<Vec<Token>, LexerError> {
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
            let digit = if let Ok(digit) = partial.parse() {
                digit
            } else {
                return Err(LexerError::InvalidToken {
                    token: partial,
                    at: scanner.current_idx,
                });
            };
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

        return Err(LexerError::UnexpectedChar {
            ch: character,
            at: scanner.current_idx,
        });
    }
    Ok(tokens)
}
