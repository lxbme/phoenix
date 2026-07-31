use crate::diag::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind};
use std::collections::HashMap;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // basic operators
    ADD,
    SUB,
    MUL,
    DIV,
    EQ,
    NEQ,
    GT,
    LT,

    // var & stack operations
    NEW(String),   // new var
    PUSHC(f64),    // push const
    PUSHV(String), // push var
    STORE(String), // pop one value into the named var

    // arrays. Keyed by name like the scalar instructions above: the name is a
    // compile-time fact that travels inside the instruction, never on the
    // stack, so nothing but an `f64` is ever pushed. Keeping the length out of
    // the instruction is what leaves room for run-time sized arrays later.
    NEWARR(String, usize), // declare an array of the given length, all zeros
    ALOAD(String),         // pop an index, push that element
    ASTORE(String),        // pop an index, then a value, and write it

    // control flow
    INT,            // interrupt
    JMPNP(usize),   // jump if not positive (include 0.0)
    JMP(usize),     // jump
    JMPPH,          // placeholder for jump actions
    CALL(usize),    // call function
    CALLPH(String), // function call placeholder
    RET,            // return

    PRINT,
    PRINTA, // print ASCII

    // input
    READ,   // push one whitespace-delimited number from stdin
    READA,  // push the next byte from stdin, or -1 at end of input
    ISEOF,  // push 1.0 when no number remains (whitespace skipped)
    ISEOFA, // push 1.0 when no byte remains
}

/// Enough room for anything this language can plausibly index, and low enough
/// that a typo cannot ask for an unbounded allocation.
pub const MAX_ARRAY_LEN: usize = 1 << 24;

/// An array's length is written as a literal, so a nonsensical one is caught
/// here rather than becoming a run-time surprise. Shared with `analyzer` so the
/// two stages cannot disagree about what counts as a length.
pub fn array_length(raw: f64) -> Result<usize, String> {
    // `fract` is NaN for NaN and for the infinities, so this rejects them too
    if raw.fract() != 0.0 || raw < 1.0 {
        return Err(format!("`{}` is not a valid array length", raw));
    }
    if raw > MAX_ARRAY_LEN as f64 {
        return Err(format!(
            "array length {} is above the maximum of {}",
            raw, MAX_ARRAY_LEN
        ));
    }
    Ok(raw as usize)
}

enum Context {
    DowBlk(usize),
    IfBlk(usize),
    ElseBlk(usize),
    FuncBlk(usize),
}

/// Code generation stops at the first error: everything emitted after a bad
/// token would be garbage anyway. The `Vec` keeps the signature uniform with
/// `lexer` and `analyzer` so `main` has a single reporting path.
pub fn compiler(tokens: Vec<Token>) -> Result<Vec<Opcode>, Vec<Diagnostic>> {
    compile(tokens).map_err(|diag| vec![diag])
}

/// Span of `tokens[idx]`, falling back to the last token when past the end.
fn span_at(tokens: &[Token], idx: usize) -> Span {
    tokens
        .get(idx)
        .or_else(|| tokens.last())
        .map(|token| token.span)
        .unwrap_or_else(|| Span::empty_at(0))
}

fn compile(tokens: Vec<Token>) -> Result<Vec<Opcode>, Diagnostic> {
    let mut result: Vec<Opcode> = Vec::new();

    // content control stack
    let mut context_stack: Vec<Context> = Vec::new();
    // function index table
    let mut func_idx_table: HashMap<String, usize> = HashMap::new();

    let mut idx = 0;
    while idx < tokens.len() {
        match &tokens[idx].kind {
            TokenKind::EOF => result.push(Opcode::INT),
            TokenKind::Operator(op_char) => {
                result.push(make_operator(*op_char, tokens[idx].span)?);
            }
            TokenKind::Identifier(id) => {
                // `x !` is a store: the target is known here, at compile time,
                // so it goes into the instruction instead of onto the stack.
                if matches!(
                    tokens.get(idx + 1).map(|token| &token.kind),
                    Some(TokenKind::Operator('!'))
                ) {
                    result.push(Opcode::STORE(id.clone()));
                    idx += 1; // consume the `!`
                } else {
                    result.push(Opcode::PUSHV(id.clone()));
                }
            }
            TokenKind::Digit(data) => {
                result.push(Opcode::PUSHC(*data));
            }
            TokenKind::Var => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        result.push(Opcode::NEW(id.clone()));
                        idx += 1; // skip var name
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 1),
                            "expected a variable name after `var`",
                        ));
                    }
                }
            }
            TokenKind::Arr => {
                let name = match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => id.clone(),
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 1),
                            "expected an array name after `arr`",
                        ));
                    }
                };
                let len = match tokens.get(idx + 2).map(|token| &token.kind) {
                    Some(TokenKind::Digit(raw)) => match array_length(*raw) {
                        Ok(len) => len,
                        Err(msg) => {
                            return Err(Diagnostic::new(span_at(&tokens, idx + 2), msg)
                                .with_note("a length is a whole number, as in `arr board 16`"));
                        }
                    },
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 2),
                            format!("expected a length after `arr {}`", name),
                        ));
                    }
                };
                result.push(Opcode::NEWARR(name, len));
                idx += 2; // skip the name and the length
            }
            TokenKind::At => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let id = id.clone();
                        idx += 1; // skip the array name
                        // `@x !` writes an element, a bare `@x` reads one --
                        // the same shape as `x !` versus `x` for a scalar.
                        if matches!(
                            tokens.get(idx + 1).map(|token| &token.kind),
                            Some(TokenKind::Operator('!'))
                        ) {
                            result.push(Opcode::ASTORE(id));
                            idx += 1; // consume the `!`
                        } else {
                            result.push(Opcode::ALOAD(id));
                        }
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 1),
                            "expected an array name after `@`",
                        ));
                    }
                }
            }
            TokenKind::Print => result.push(Opcode::PRINT),
            TokenKind::Printa => result.push(Opcode::PRINTA),
            TokenKind::Read => result.push(Opcode::READ),
            TokenKind::Reada => result.push(Opcode::READA),
            TokenKind::IsEof => result.push(Opcode::ISEOF),
            TokenKind::IsEofa => result.push(Opcode::ISEOFA),
            TokenKind::Dollar => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        match func_idx_table.get(id) {
                            Some(func_idx) => result.push(Opcode::CALL(*func_idx)),
                            None => {
                                result.push(Opcode::CALLPH(id.clone()));
                            }
                        }
                        idx += 1; // skip func name
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 1),
                            "expected a function name after `$`",
                        ));
                    }
                }
            }
            TokenKind::Dow => {
                context_stack.push(Context::DowBlk(result.len()));
                idx += 1; // skip LeftBracket
            }
            TokenKind::If => {
                context_stack.push(Context::IfBlk(result.len()));
                result.push(Opcode::JMPPH); // placeholder for JUMNP
                idx += 1; // skip LeftBracket
            }
            TokenKind::Def => {
                // Functions are global and have no lexical scope, so a nested
                // `def` would silently publish an inner name to the whole
                // program. Reject at static analyze stage.
                if context_stack
                    .iter()
                    .any(|ctx| matches!(ctx, Context::FuncBlk(_)))
                {
                    return Err(Diagnostic::new(tokens[idx].span, "`def` cannot be nested")
                        .with_note("functions are global; define them at the top level"));
                }
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let id = id.clone();
                        // step over the body at run time
                        let jmp_slot = result.len();
                        result.push(Opcode::JMPPH);
                        let func_idx = result.len(); // body starts right after
                        context_stack.push(Context::FuncBlk(jmp_slot));
                        func_idx_table.insert(id.clone(), func_idx);
                        for slot in 0..result.len() {
                            if result[slot] == Opcode::CALLPH(id.clone()) {
                                result[slot] = Opcode::CALL(func_idx); // backfill CALLPH
                            }
                        }
                        idx += 1; // skip function name
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            span_at(&tokens, idx + 1),
                            "expected a function name after `def`",
                        ));
                    }
                }
            }
            TokenKind::RightBracket => {
                match context_stack.pop() {
                    Some(Context::DowBlk(head)) => {
                        result.push(Opcode::JMPNP(head)) // jump to head of dow
                    }
                    Some(Context::IfBlk(jmpnp_idx)) => {
                        result.push(Opcode::JMPPH); // place holder for JMP before else
                        // backfill idx for JMPNP in if
                        result[jmpnp_idx] = Opcode::JMPNP(result.len());
                        // note idx of place holder
                        context_stack.push(Context::ElseBlk(result.len() - 1))
                    }
                    Some(Context::ElseBlk(jmp_idx)) => {
                        result[jmp_idx] = Opcode::JMP(result.len());
                    }
                    Some(Context::FuncBlk(jmp_slot)) => {
                        result.push(Opcode::RET);
                        // backfill the jump that steps over the body
                        result[jmp_slot] = Opcode::JMP(result.len());
                    }
                    // unbalanced brackets are caught earlier, by `analyzer`
                    None => {
                        return Err(Diagnostic::new(tokens[idx].span, "unmatched `}`"));
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    Ok(result)
}

fn make_operator(op_char: char, span: Span) -> Result<Opcode, Diagnostic> {
    let op = match op_char {
        '+' => Opcode::ADD,
        '-' => Opcode::SUB,
        '*' => Opcode::MUL,
        '/' => Opcode::DIV,
        '=' => Opcode::EQ,
        '~' => Opcode::NEQ,
        '>' => Opcode::GT,
        '<' => Opcode::LT,
        // A well-formed `!` is consumed by the identifier before it; reaching
        // here means there was no target to store into.
        '!' => {
            return Err(Diagnostic::new(span, "expected a variable name before `!`"));
        }
        _ => {
            return Err(Diagnostic::new(
                span,
                format!("unexpected operator `{}`", op_char),
            ));
        }
    };
    Ok(op)
}
