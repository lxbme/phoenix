use crate::diag::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind};

pub fn analyzer(tokens: &[Token]) -> Result<(), Vec<Diagnostic>> {
    let mut diags = check_bracket_pairs(tokens);
    diags.sort_by_key(|d| d.span.start);
    if diags.is_empty() { Ok(()) } else { Err(diags) }
}

/// A stack, not a counter: counting only proves the two totals match, so `} {`
/// and `} } { {` used to pass while being obviously wrong.
fn check_bracket_pairs(tokens: &[Token]) -> Vec<Diagnostic> {
    let mut open: Vec<Span> = Vec::new();
    let mut diags: Vec<Diagnostic> = Vec::new();

    for token in tokens {
        match token.kind {
            TokenKind::LeftBracket => open.push(token.span),
            TokenKind::RightBracket => {
                if open.pop().is_none() {
                    diags.push(Diagnostic::new(token.span, "unmatched `}`"));
                }
            }
            _ => {}
        }
    }

    for span in open {
        diags.push(Diagnostic::new(span, "unclosed `{`"));
    }
    diags
}
