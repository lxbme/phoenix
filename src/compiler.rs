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

    // Locals. A function's `var` and `arr` live in a frame instead of the
    // global tables, so each call gets its own copy -- which is what makes a
    // function reentrant, and recursion possible at all. The slot is resolved
    // at compile time, exactly like the name in `STORE`.
    //
    // A name becomes local only after its own declaration, so the frame size
    // is not known until the body ends; `ENTER` is backfilled at the `}`.
    ENTER(usize), // reserve n slots for the frame being entered
    NEWL(usize),  // declare a slot and zero it -- cannot fail, so needs no name
    // the name rides along purely so a diagnostic can use it, exactly as in
    // `PUSHV` and `ALOADL`
    PUSHL(String, usize),  // push a slot
    STOREL(String, usize), // pop one value into a slot

    NEWARRL(LocalArr), // declare a run of slots and zero them
    ALOADL(LocalArr),  // pop an index, push that element
    ASTOREL(LocalArr), // pop an index, then a value, and write it

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

/// A local array: a run of frame slots, plus the name a diagnostic needs.
/// The length is a literal, so the run is laid out entirely at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalArr {
    pub name: String,
    pub base: usize,
    pub len: usize,
}

impl Opcode {
    /// How the instruction is written in source, and how many values it takes
    /// off the stack. Only an underflow needs this today, but it is the same
    /// table a static stack-balance check would want.
    ///
    /// Written out in full rather than with a `_` arm on purpose: a new opcode
    /// then fails to compile until someone states what it does to the stack.
    pub fn stack_demand(&self) -> (&'static str, usize) {
        match self {
            Opcode::ADD => ("`+`", 2),
            Opcode::SUB => ("`-`", 2),
            Opcode::MUL => ("`*`", 2),
            Opcode::DIV => ("`/`", 2),
            Opcode::EQ => ("`=`", 2),
            Opcode::NEQ => ("`~`", 2),
            Opcode::GT => ("`>`", 2),
            Opcode::LT => ("`<`", 2),

            Opcode::STORE(_) | Opcode::STOREL(..) => ("a store", 1),
            Opcode::ALOAD(_) | Opcode::ALOADL(_) => ("an element read", 1),
            // the index and then the value under it
            Opcode::ASTORE(_) | Opcode::ASTOREL(_) => ("an element store", 2),

            Opcode::PRINT => ("`print`", 1),
            Opcode::PRINTA => ("`printa`", 1),
            // the value a `dow` or an `if` branches on
            Opcode::JMPNP(_) => ("a `dow` or `if` test", 1),

            Opcode::NEW(_) | Opcode::NEWARR(..) | Opcode::PUSHC(_) | Opcode::PUSHV(_) => ("", 0),
            Opcode::ENTER(_) | Opcode::NEWL(_) | Opcode::PUSHL(..) | Opcode::NEWARRL(_) => ("", 0),
            Opcode::INT | Opcode::JMP(_) | Opcode::JMPPH => ("", 0),
            Opcode::CALL(_) | Opcode::CALLPH(_) | Opcode::RET => ("", 0),
            Opcode::READ | Opcode::READA | Opcode::ISEOF | Opcode::ISEOFA => ("", 0),
        }
    }
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

/// An instruction together with the source it came from, so that a failure at
/// run time can be pointed at rather than merely described.
///
/// Deliberately NOT `PartialEq`, for the same reason as `Token`: the forward
/// call backfill below asks whether a slot holds `CALLPH(name)`. If `Instr`
/// derived it, that comparison would start including the span, never match,
/// and every forward call would silently reach the VM unresolved.
#[derive(Debug, Clone)]
pub struct Instr {
    pub op: Opcode,
    pub span: Span,
}

impl Instr {
    fn new(op: Opcode, span: Span) -> Self {
        Self { op, span }
    }
}

/// The compiled program: the instructions, plus what the VM needs in order to
/// say something useful when one of them fails.
pub struct Program {
    pub code: Vec<Instr>,
    /// Entry address to function name. A `CALL` carries only an address, so
    /// this is what lets a run-time trace name the frames it walks.
    pub funcs: HashMap<usize, String>,
}

enum Context {
    DowBlk(usize),
    IfBlk(usize),
    ElseBlk(usize),
    /// `jmp` steps over the body, `enter` reserves the frame. Both are
    /// placeholders until the body ends and its size is known.
    FuncBlk {
        jmp: usize,
        enter: usize,
    },
}

/// What a name declared inside a function refers to. Scalars and arrays share
/// the namespace here exactly as they do at the top level.
enum Local {
    Slot(usize),
    Arr(LocalArr),
}

/// Code generation stops at the first error: everything emitted after a bad
/// token would be garbage anyway. The `Vec` keeps the signature uniform with
/// `lexer` and `analyzer` so `main` has a single reporting path.
pub fn compiler(tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    compile(tokens).map_err(|diag| vec![diag])
}

fn in_function(context_stack: &[Context]) -> bool {
    context_stack
        .iter()
        .any(|ctx| matches!(ctx, Context::FuncBlk { .. }))
}

/// Span of `tokens[idx]`, falling back to the last token when past the end.
fn span_at(tokens: &[Token], idx: usize) -> Span {
    tokens
        .get(idx)
        .or_else(|| tokens.last())
        .map(|token| token.span)
        .unwrap_or_else(|| Span::empty_at(0))
}

fn compile(tokens: Vec<Token>) -> Result<Program, Diagnostic> {
    let mut result: Vec<Instr> = Vec::new();

    // content control stack
    let mut context_stack: Vec<Context> = Vec::new();
    // function index table
    let mut func_idx_table: HashMap<String, usize> = HashMap::new();
    // Names declared so far in the function being compiled -- it grows as each
    // declaration is passed, so a name is local only below its own `var` or
    // `arr`. Empty at the top level. `def` cannot nest, so one map is enough.
    let mut locals: HashMap<String, Local> = HashMap::new();
    let mut next_slot: usize = 0;

    let mut idx = 0;
    while idx < tokens.len() {
        // Most instructions come from this one token. The arms that consume
        // several widen it, so that the caret covers what has to be fixed --
        // `x !` and `@x !`, not just the name in the middle of them.
        let span = tokens[idx].span;
        match &tokens[idx].kind {
            TokenKind::EOF => result.push(Instr::new(Opcode::INT, span)),
            TokenKind::Operator(op_char) => {
                result.push(Instr::new(make_operator(*op_char, span)?, span));
            }
            TokenKind::Identifier(id) => {
                // A name declared earlier in this function is a frame slot;
                // anything else is the global of that name.
                if let Some(Local::Arr(arr)) = locals.get(id) {
                    return Err(Diagnostic::new(
                        span,
                        format!("`{}` is an array, not a variable", arr.name),
                    )
                    .with_note("write `i @name` to read an element"));
                }
                let slot = match locals.get(id) {
                    Some(Local::Slot(slot)) => Some(*slot),
                    _ => None,
                };
                // `x !` is a store: the target is known here, at compile time,
                // so it goes into the instruction instead of onto the stack.
                if matches!(
                    tokens.get(idx + 1).map(|token| &token.kind),
                    Some(TokenKind::Operator('!'))
                ) {
                    let span = Span::merge(span, tokens[idx + 1].span);
                    let op = match slot {
                        Some(slot) => Opcode::STOREL(id.clone(), slot),
                        None => Opcode::STORE(id.clone()),
                    };
                    result.push(Instr::new(op, span));
                    idx += 1; // consume the `!`
                } else {
                    let op = match slot {
                        Some(slot) => Opcode::PUSHL(id.clone(), slot),
                        None => Opcode::PUSHV(id.clone()),
                    };
                    result.push(Instr::new(op, span));
                }
            }
            TokenKind::Digit(data) => {
                result.push(Instr::new(Opcode::PUSHC(*data), span));
            }
            TokenKind::Var => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let span = Span::merge(span, tokens[idx + 1].span);
                        let op = if in_function(&context_stack) {
                            match locals.get(id) {
                                // re-declaring reuses the slot and zeroes it,
                                // which is what `var` does to a global too
                                Some(Local::Slot(slot)) => Opcode::NEWL(*slot),
                                Some(Local::Arr(_)) => {
                                    return Err(Diagnostic::new(
                                        span,
                                        format!("`{}` is already an array here", id),
                                    )
                                    .with_note("a name is one or the other, never both"));
                                }
                                None => {
                                    let slot = next_slot;
                                    next_slot += 1;
                                    locals.insert(id.clone(), Local::Slot(slot));
                                    Opcode::NEWL(slot)
                                }
                            }
                        } else {
                            Opcode::NEW(id.clone())
                        };
                        result.push(Instr::new(op, span));
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
                let span = Span::merge(span, span_at(&tokens, idx + 2));
                let op = if in_function(&context_stack) {
                    match locals.get(&name) {
                        Some(Local::Slot(_)) => {
                            return Err(Diagnostic::new(
                                span,
                                format!("`{}` is already a variable here", name),
                            )
                            .with_note("a name is one or the other, never both"));
                        }
                        // re-declaring reuses the run only when it still fits
                        Some(Local::Arr(arr)) if arr.len == len => Opcode::NEWARRL(arr.clone()),
                        _ => {
                            let arr = LocalArr {
                                name: name.clone(),
                                base: next_slot,
                                len,
                            };
                            next_slot += len;
                            locals.insert(name, Local::Arr(arr.clone()));
                            Opcode::NEWARRL(arr)
                        }
                    }
                } else {
                    Opcode::NEWARR(name, len)
                };
                result.push(Instr::new(op, span));
                idx += 2; // skip the name and the length
            }
            TokenKind::At => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let id = id.clone();
                        idx += 1; // skip the array name
                        let span = Span::merge(span, tokens[idx].span);
                        // `@x !` writes an element, a bare `@x` reads one --
                        // the same shape as `x !` versus `x` for a scalar.
                        if let Some(Local::Slot(_)) = locals.get(&id) {
                            return Err(Diagnostic::new(
                                span,
                                format!("`{}` is a variable, not an array", id),
                            )
                            .with_note("write `name` to read it, `v name !` to write it"));
                        }
                        let local = match locals.get(&id) {
                            Some(Local::Arr(arr)) => Some(arr.clone()),
                            _ => None,
                        };
                        if matches!(
                            tokens.get(idx + 1).map(|token| &token.kind),
                            Some(TokenKind::Operator('!'))
                        ) {
                            let span = Span::merge(span, tokens[idx + 1].span);
                            let op = match local {
                                Some(arr) => Opcode::ASTOREL(arr),
                                None => Opcode::ASTORE(id),
                            };
                            result.push(Instr::new(op, span));
                            idx += 1; // consume the `!`
                        } else {
                            let op = match local {
                                Some(arr) => Opcode::ALOADL(arr),
                                None => Opcode::ALOAD(id),
                            };
                            result.push(Instr::new(op, span));
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
            TokenKind::Print => result.push(Instr::new(Opcode::PRINT, span)),
            TokenKind::Printa => result.push(Instr::new(Opcode::PRINTA, span)),
            TokenKind::Read => result.push(Instr::new(Opcode::READ, span)),
            TokenKind::Reada => result.push(Instr::new(Opcode::READA, span)),
            TokenKind::IsEof => result.push(Instr::new(Opcode::ISEOF, span)),
            TokenKind::IsEofa => result.push(Instr::new(Opcode::ISEOFA, span)),
            TokenKind::Dollar => {
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let span = Span::merge(span, tokens[idx + 1].span);
                        let op = match func_idx_table.get(id) {
                            Some(func_idx) => Opcode::CALL(*func_idx),
                            None => Opcode::CALLPH(id.clone()),
                        };
                        result.push(Instr::new(op, span));
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
                // the test belongs to the `if`, so the span stays there when
                // the placeholder is backfilled at the closing `}`
                result.push(Instr::new(Opcode::JMPPH, span)); // placeholder for JUMNP
                idx += 1; // skip LeftBracket
            }
            TokenKind::Def => {
                // Functions are global and have no lexical scope, so a nested
                // `def` would silently publish an inner name to the whole
                // program. Reject at static analyze stage.
                if in_function(&context_stack) {
                    return Err(Diagnostic::new(tokens[idx].span, "`def` cannot be nested")
                        .with_note("functions are global; define them at the top level"));
                }
                match tokens.get(idx + 1).map(|token| &token.kind) {
                    Some(TokenKind::Identifier(id)) => {
                        let id = id.clone();
                        // step over the body at run time
                        let jmp_slot = result.len();
                        result.push(Instr::new(Opcode::JMPPH, span));
                        // The entry instruction, so every call reserves a
                        // frame. Its size is backfilled at the closing `}`.
                        let func_idx = result.len();
                        let enter_slot = result.len();
                        result.push(Instr::new(Opcode::ENTER(0), span));
                        locals.clear();
                        next_slot = 0;
                        context_stack.push(Context::FuncBlk {
                            jmp: jmp_slot,
                            enter: enter_slot,
                        });
                        func_idx_table.insert(id.clone(), func_idx);
                        for slot in 0..result.len() {
                            // only the opcode is replaced -- each call site
                            // keeps pointing at its own `$name`
                            if result[slot].op == Opcode::CALLPH(id.clone()) {
                                result[slot].op = Opcode::CALL(func_idx); // backfill CALLPH
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
                // Every backfill below assigns to `.op` alone: the placeholder
                // was emitted at the token the jump belongs to, and that is
                // where a failure should be reported, not at this `}`.
                match context_stack.pop() {
                    Some(Context::DowBlk(head)) => {
                        // the test sits just before this `}`, so point here
                        result.push(Instr::new(Opcode::JMPNP(head), span)) // jump to head of dow
                    }
                    Some(Context::IfBlk(jmpnp_idx)) => {
                        result.push(Instr::new(Opcode::JMPPH, span)); // place holder for JMP before else
                        // backfill idx for JMPNP in if
                        result[jmpnp_idx].op = Opcode::JMPNP(result.len());
                        // note idx of place holder
                        context_stack.push(Context::ElseBlk(result.len() - 1))
                    }
                    Some(Context::ElseBlk(jmp_idx)) => {
                        result[jmp_idx].op = Opcode::JMP(result.len());
                    }
                    Some(Context::FuncBlk { jmp, enter }) => {
                        result.push(Instr::new(Opcode::RET, span));
                        // backfill the jump that steps over the body
                        result[jmp].op = Opcode::JMP(result.len());
                        // and the frame size, now that the body has been seen
                        result[enter].op = Opcode::ENTER(next_slot);
                        locals.clear(); // back to the global scope
                        next_slot = 0;
                    }
                    // unbalanced brackets are caught earlier, by `analyzer`
                    None => {
                        return Err(Diagnostic::new(span, "unmatched `}`"));
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    Ok(Program {
        code: result,
        // inverted here rather than maintained alongside: `analyzer` rejects a
        // duplicate `def`, so name -> address is injective and safe to flip
        funcs: func_idx_table
            .into_iter()
            .map(|(name, entry)| (entry, name))
            .collect(),
    })
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
