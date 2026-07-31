use crate::compiler::array_length;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

/// Answers "is this program well formed?" completely, so that `compiler` can
/// assume a legal token stream and concentrate on code generation.
///
/// Everything is collected in one go: a program with five mistakes reports all
/// five. `compiler` cannot do that -- it emits opcodes as it walks, so it has
/// to stop at the first error.
pub fn analyzer(tokens: &[Token]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let facts = walk(tokens, &mut diags);
    check_names(&facts, &mut diags);
    // A program that does not compile makes the "unused" answers unreliable --
    // they are usually a cascade of the real error, so keep them quiet.
    if diags.iter().any(|diag| diag.is_error()) {
        diags.retain(|diag| diag.is_error());
    }
    diags.sort_by_key(|diag| diag.span.start);
    diags
}

#[derive(Clone, Copy, PartialEq)]
enum BlockKind {
    If,
    Else,
    Dow,
    Func,
    /// A `{` that follows none of the above -- always a mistake.
    Bare,
}

struct OpenBlock {
    kind: BlockKind,
    span: Span,
}

/// Names gathered while walking, checked once the whole file has been seen.
///
/// Variables and arrays share one namespace, which is what lets a mistake be
/// reported as "`x` is an array" instead of the useless "undefined variable".
#[derive(Default)]
struct Facts {
    defs: HashMap<String, Span>,
    vars: HashMap<String, Span>,
    arrs: HashMap<String, Span>,
    calls: Vec<(String, Span)>,
    uses: Vec<(String, Span)>,
    /// every `@name`, whether it read or wrote
    elems: Vec<(String, Span)>,
}

/// Walks the token stream the same way `compiler` does -- consuming the name
/// after `var` / `$` / `def`, and the `!` after an identifier -- so that both
/// agree on which tokens are names and which are operands.
fn walk(tokens: &[Token], diags: &mut Vec<Diagnostic>) -> Facts {
    let mut facts = Facts::default();
    let mut blocks: Vec<OpenBlock> = Vec::new();
    // block kind the next `{` will open
    let mut pending: Option<BlockKind> = None;
    // set by a `}` that closed an `if` block, readable by the token right after
    let mut after_if_block = false;

    let mut idx = 0;
    while idx < tokens.len() {
        let token = &tokens[idx];
        let follows_if_block = after_if_block;
        after_if_block = false;

        match &token.kind {
            TokenKind::Var => match name_after(tokens, idx) {
                Some((name, span)) => {
                    facts.vars.entry(name).or_insert(span);
                    idx += 1;
                }
                None => diags.push(Diagnostic::new(
                    span_after(tokens, idx),
                    "expected a variable name after `var`",
                )),
            },

            TokenKind::Arr => match name_after(tokens, idx) {
                Some((name, span)) => {
                    idx += 1;
                    // the length is a literal, so it is checked here and not
                    // left to fail at run time
                    match tokens.get(idx + 1).map(|next| &next.kind) {
                        Some(TokenKind::Digit(raw)) => {
                            if let Err(msg) = array_length(*raw) {
                                diags.push(
                                    Diagnostic::new(span_after(tokens, idx), msg).with_note(
                                        "a length is a whole number, as in `arr board 16`",
                                    ),
                                );
                            }
                            idx += 1;
                        }
                        _ => diags.push(Diagnostic::new(
                            span_after(tokens, idx),
                            format!("expected a length after `arr {}`", name),
                        )),
                    }
                    facts.arrs.entry(name).or_insert(span);
                }
                None => diags.push(Diagnostic::new(
                    span_after(tokens, idx),
                    "expected an array name after `arr`",
                )),
            },

            TokenKind::At => match name_after(tokens, idx) {
                Some((name, span)) => {
                    facts.elems.push((name, span));
                    idx += 1;
                    if matches!(
                        tokens.get(idx + 1).map(|next| &next.kind),
                        Some(TokenKind::Operator('!'))
                    ) {
                        idx += 1; // the element store consumes the `!`
                    }
                }
                None => diags.push(Diagnostic::new(
                    span_after(tokens, idx),
                    "expected an array name after `@`",
                )),
            },

            TokenKind::Dollar => match name_after(tokens, idx) {
                Some((name, span)) => {
                    facts.calls.push((name, span));
                    idx += 1;
                }
                None => diags.push(Diagnostic::new(
                    span_after(tokens, idx),
                    "expected a function name after `$`",
                )),
            },

            TokenKind::Def => {
                if blocks.iter().any(|block| block.kind == BlockKind::Func) {
                    diags.push(
                        Diagnostic::new(token.span, "`def` cannot be nested")
                            .with_note("functions are global; define them at the top level"),
                    );
                }
                match name_after(tokens, idx) {
                    Some((name, span)) => {
                        // keep the first definition's span, report the later ones
                        match facts.defs.entry(name.clone()) {
                            Entry::Occupied(_) => diags.push(
                                Diagnostic::new(
                                    span,
                                    format!("function `{}` is already defined", name),
                                )
                                .with_note("a name may only be defined once"),
                            ),
                            Entry::Vacant(slot) => {
                                slot.insert(span);
                            }
                        }
                        idx += 1;
                        if expect_block(tokens, idx, diags, "def") {
                            pending = Some(BlockKind::Func);
                        }
                    }
                    None => diags.push(Diagnostic::new(
                        span_after(tokens, idx),
                        "expected a function name after `def`",
                    )),
                }
            }

            TokenKind::Identifier(name) => {
                facts.uses.push((name.clone(), token.span));
                if matches!(
                    tokens.get(idx + 1).map(|next| &next.kind),
                    Some(TokenKind::Operator('!'))
                ) {
                    idx += 1; // the store consumes the `!`
                }
            }

            // A well-formed `!` is consumed by the identifier before it.
            TokenKind::Operator('!') => diags.push(
                Diagnostic::new(token.span, "expected a variable name before `!`")
                    .with_note("write `x !` to store the top of the stack into `x`"),
            ),

            TokenKind::Dow => {
                if expect_block(tokens, idx, diags, "dow") {
                    pending = Some(BlockKind::Dow);
                }
            }

            TokenKind::If => {
                if expect_block(tokens, idx, diags, "if") {
                    pending = Some(BlockKind::If);
                }
            }

            TokenKind::Else => {
                if !follows_if_block {
                    diags.push(
                        Diagnostic::new(token.span, "`else` must follow an `if` block")
                            .with_note("an `else` block only makes sense right after `if { ... }`"),
                    );
                }
                if expect_block(tokens, idx, diags, "else") {
                    pending = Some(BlockKind::Else);
                }
            }

            TokenKind::LeftBracket => {
                let kind = pending.take().unwrap_or(BlockKind::Bare);
                if kind == BlockKind::Bare {
                    diags.push(
                        Diagnostic::new(token.span, "unexpected `{`").with_note(
                            "a block may only follow `dow`, `if`, `else`, or `def <name>`",
                        ),
                    );
                }
                blocks.push(OpenBlock {
                    kind,
                    span: token.span,
                });
            }

            TokenKind::RightBracket => match blocks.pop() {
                Some(block) => {
                    if block.kind == BlockKind::If {
                        if !matches!(
                            tokens.get(idx + 1).map(|next| &next.kind),
                            Some(TokenKind::Else)
                        ) {
                            diags.push(
                                Diagnostic::new(
                                    token.span,
                                    "`if` block must be followed by `else`",
                                )
                                .with_note("the else branch is mandatory"),
                            );
                        }
                        after_if_block = true;
                    }
                }
                None => diags.push(Diagnostic::new(token.span, "unmatched `}`")),
            },

            _ => {}
        }
        idx += 1;
    }

    for block in blocks {
        diags.push(Diagnostic::new(block.span, "unclosed `{`"));
    }
    facts
}

fn check_names(facts: &Facts, diags: &mut Vec<Diagnostic>) {
    for (name, span) in &facts.calls {
        if !facts.defs.contains_key(name) {
            diags.push(Diagnostic::new(
                *span,
                format!("undefined function `{}`", name),
            ));
        }
    }

    // Declaration order is deliberately ignored: `var` runs at run time, so a
    // use textually before the declaration can still be correct on a later
    // pass of a loop. Only a name that is never declared at all is certainly
    // wrong -- reporting more than that would produce false positives.
    for (name, span) in &facts.uses {
        if facts.vars.contains_key(name) {
            continue;
        }
        // Sharing the namespace pays off here: the useful answer is not
        // "undefined", it is "you left the `@` off".
        if facts.arrs.contains_key(name) {
            diags.push(
                Diagnostic::new(*span, format!("`{}` is an array, not a variable", name))
                    .with_note("write `i @name` to read an element, `v i @name !` to write one"),
            );
        } else {
            diags.push(Diagnostic::new(
                *span,
                format!("undefined variable `{}`", name),
            ));
        }
    }

    for (name, span) in &facts.elems {
        if facts.arrs.contains_key(name) {
            continue;
        }
        if facts.vars.contains_key(name) {
            diags.push(
                Diagnostic::new(*span, format!("`{}` is a variable, not an array", name))
                    .with_note("write `name` to read it, `v name !` to write it"),
            );
        } else {
            diags.push(Diagnostic::new(
                *span,
                format!("undefined array `{}`", name),
            ));
        }
    }

    // One name, one meaning. Both declarations are legal on their own, so the
    // second one is the mistake -- point at whichever came later.
    for (name, arr_span) in &facts.arrs {
        if let Some(var_span) = facts.vars.get(name) {
            let span = if arr_span.start > var_span.start {
                *arr_span
            } else {
                *var_span
            };
            diags.push(
                Diagnostic::new(
                    span,
                    format!("`{}` is declared both as a variable and as an array", name),
                )
                .with_note("a name is one or the other, never both"),
            );
        }
    }

    let used: HashSet<&String> = facts.uses.iter().map(|(name, _)| name).collect();
    for (name, span) in &facts.vars {
        if !used.contains(name) {
            diags.push(Diagnostic::warning(
                *span,
                format!("unused variable `{}`", name),
            ));
        }
    }

    let indexed: HashSet<&String> = facts.elems.iter().map(|(name, _)| name).collect();
    for (name, span) in &facts.arrs {
        if !indexed.contains(name) {
            diags.push(Diagnostic::warning(
                *span,
                format!("unused array `{}`", name),
            ));
        }
    }

    let called: HashSet<&String> = facts.calls.iter().map(|(name, _)| name).collect();
    for (name, span) in &facts.defs {
        if !called.contains(name) {
            diags.push(Diagnostic::warning(
                *span,
                format!("function `{}` is never called", name),
            ));
        }
    }
}

fn name_after(tokens: &[Token], idx: usize) -> Option<(String, Span)> {
    let token = tokens.get(idx + 1)?;
    match &token.kind {
        TokenKind::Identifier(name) => Some((name.clone(), token.span)),
        _ => None,
    }
}

fn span_after(tokens: &[Token], idx: usize) -> Span {
    tokens
        .get(idx + 1)
        .or_else(|| tokens.last())
        .map(|token| token.span)
        .unwrap_or_else(|| Span::empty_at(0))
}

/// `dow` / `if` / `else` / `def <name>` must be followed by `{`. Without this
/// the compiler stepped over whatever came next, silently dropping it.
fn expect_block(tokens: &[Token], idx: usize, diags: &mut Vec<Diagnostic>, keyword: &str) -> bool {
    if matches!(
        tokens.get(idx + 1).map(|next| &next.kind),
        Some(TokenKind::LeftBracket)
    ) {
        return true;
    }
    diags.push(Diagnostic::new(
        span_after(tokens, idx),
        format!("expected `{{` after `{}`", keyword),
    ));
    false
}
