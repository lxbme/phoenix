use crate::diag::{Diagnostic, Span};
use crate::source::Source;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
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

/// A token together with where it came from.
///
/// Deliberately NOT `PartialEq`: equality must compare `kind` only. If `Token`
/// derived it, two `{` at different offsets would compare unequal and the
/// bracket check in `analyzer` would silently pass on everything.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

struct Scanner<'a> {
    source: &'a Source,
    current_idx: usize,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a Source) -> Self {
        Self {
            source,
            current_idx: 0,
        }
    }

    fn idx(&self) -> usize {
        self.current_idx
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

    /// `[` ... `]`. An unclosed `[` comments out the rest of the file, by design.
    fn skip_comment(&mut self) {
        while let Some(c) = self.advance() {
            if c == ']' {
                return;
            }
        }
    }
}

pub fn lexer(source: &Source) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut scanner = Scanner::new(source);
    let mut tokens = Vec::<Token>::new();
    let mut diags = Vec::<Diagnostic>::new();

    loop {
        // Must be read before `advance`, otherwise every token loses its first char.
        let start = scanner.idx();

        let character = match scanner.advance() {
            Some(character) => character,
            None => {
                tokens.push(Token::new(TokenKind::EOF, Span::empty_at(start)));
                break;
            }
        };

        if character.is_whitespace() {
            continue;
        }

        if character == '[' {
            scanner.skip_comment();
            continue;
        }

        // Every branch yields a kind and the token is pushed at the single exit
        // below, so a span can neither be forgotten nor computed twice.
        let kind = if character.is_ascii_digit() || character == '.' {
            let mut partial = String::from(character);
            partial.push_str(&scanner.take_while(|c| c.is_ascii_digit() || c == '.'));
            let span = Span {
                start,
                end: scanner.idx(),
            };
            match partial.parse::<f64>() {
                Ok(digit) => TokenKind::Digit(digit),
                Err(_) => {
                    diags.push(
                        Diagnostic::new(
                            span,
                            format!("invalid number literal `{}`", source.snippet(span)),
                        )
                        .with_note("a number may contain at most one `.`"),
                    );
                    continue; // record and keep scanning
                }
            }
        } else if character.is_alphabetic() || character == '_' {
            let mut partial = String::from(character);
            partial.push_str(&scanner.take_while(|c| c.is_alphanumeric() || c == '_'));
            match partial.as_str() {
                "def" => TokenKind::Def,
                "dow" => TokenKind::Dow,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "print" => TokenKind::Print,
                "printa" => TokenKind::Printa,
                "var" => TokenKind::Var,
                _ => TokenKind::Identifier(partial),
            }
        } else {
            match character {
                '+' | '-' | '*' | '/' | '!' | '=' | '~' | '>' | '<' => {
                    TokenKind::Operator(character)
                }
                '{' => TokenKind::LeftBracket,
                '}' => TokenKind::RightBracket,
                '$' => TokenKind::Dollar,
                // Without this arm an unrecognised character vanished silently.
                _ => {
                    diags.push(Diagnostic::new(
                        Span {
                            start,
                            end: scanner.idx(),
                        },
                        format!("unexpected character `{}`", character),
                    ));
                    continue;
                }
            }
        };

        tokens.push(Token::new(
            kind,
            Span {
                start,
                end: scanner.idx(),
            },
        ));
    }

    if diags.is_empty() {
        Ok(tokens)
    } else {
        Err(diags)
    }
}
