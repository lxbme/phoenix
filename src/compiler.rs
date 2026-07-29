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
    STORE,         // store var

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
}

enum Context {
    DowBlk(usize),
    IfBlk(usize),
    ElseBlk(usize),
    FuncBlk,
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
    // extract function fragments
    let (mut tokens, func_slices) = function_picker(tokens);
    for mut func_slice in func_slices {
        tokens.append(&mut func_slice);
    } // append functions to tokens

    // content control stack
    let mut context_stack: Vec<Context> = Vec::new();
    // function index table
    let mut func_idx_table: HashMap<String, usize> = HashMap::new();

    let mut idx = 0;
    while idx < tokens.len() {
        match &tokens[idx].kind {
            TokenKind::EOF => result.push(Opcode::INT),
            TokenKind::Placeholder => {}
            TokenKind::Operator(op_char) => {
                result.push(make_operator(*op_char, tokens[idx].span)?);
            }
            TokenKind::Identifier(id) => {
                result.push(Opcode::PUSHV(id.clone()));
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
            TokenKind::Print => result.push(Opcode::PRINT),
            TokenKind::Printa => result.push(Opcode::PRINTA),
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
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let id = id.clone();
                        context_stack.push(Context::FuncBlk);
                        func_idx_table.insert(id.clone(), result.len()); // note function idx
                        for slot in 0..result.len() {
                            if result[slot] == Opcode::CALLPH(id.clone()) {
                                result[slot] = Opcode::CALL(result.len()); // backfill CALLPH
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
                    Some(Context::FuncBlk) => {
                        result.push(Opcode::RET);
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
        '!' => Opcode::STORE,
        _ => {
            return Err(Diagnostic::new(
                span,
                format!("unexpected operator `{}`", op_char),
            ));
        }
    };
    Ok(op)
}

/// Lift every `def` body out of the main token stream so it can be appended
/// after the `EOF`/`INT` opcode. Tokens are tombstoned and then dropped, and
/// the bodies are moved to the end -- which is exactly why a span has to live
/// inside `Token` rather than in a parallel array.
fn function_picker(mut tokens: Vec<Token>) -> (Vec<Token>, Vec<Vec<Token>>) {
    let mut func_slices: Vec<Vec<Token>> = Vec::new();
    // status
    let mut def_mode = false;
    let mut bracket_counter: i32 = 0;
    let mut single_func: Vec<Token> = Vec::new();
    for idx in 0..tokens.len() {
        // status transfer
        match &tokens[idx].kind {
            TokenKind::Def => def_mode = true,
            TokenKind::LeftBracket => {
                if def_mode {
                    bracket_counter += 1;
                }
            }
            TokenKind::RightBracket => {
                if def_mode {
                    bracket_counter -= 1;
                }
                if def_mode && bracket_counter == 0 {
                    def_mode = false;
                }
            }
            _ => {}
        }
        // logic
        if def_mode {
            single_func.push(tokens[idx].clone());
            tokens[idx].kind = TokenKind::Placeholder; // keep the span
        } else if !single_func.is_empty() {
            // `def_mode` flips off exactly on the body's closing `}`, so this
            // token is that `}`. Taking it (instead of synthesising a fresh
            // `RightBracket`) is what gives the closing brace a real span.
            single_func.push(tokens[idx].clone());
            tokens[idx].kind = TokenKind::Placeholder;
            func_slices.push(std::mem::take(&mut single_func));
        }
    }
    tokens.retain(|token| !matches!(token.kind, TokenKind::Placeholder));
    (tokens, func_slices)
}
