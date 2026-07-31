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
    /// globals only -- a declaration inside a `def` goes to `locals` instead
    vars: HashMap<String, Span>,
    arrs: HashMap<String, Span>,
    /// function name -> the names it declares, for the unused and shadowing
    /// checks. Resolution itself does not use this: a name becomes local at
    /// its own declaration, so it is decided during the walk, in order.
    locals: HashMap<String, HashMap<String, Span>>,
    calls: Vec<(String, Span)>,
    uses: Vec<Use>,
    /// every `@name`, whether it read or wrote
    elems: Vec<Use>,
}

/// A mention of a name, resolved where it was found.
struct Use {
    name: String,
    span: Span,
    /// the function it sits in, if any -- regardless of what it resolved to
    in_fn: Option<String>,
    /// whether it reached a declaration above it in the same function
    local: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum LocalKind {
    Scalar,
    Array,
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
    // The `def` being walked and what it has declared so far. The set grows as
    // declarations are passed, so a name resolves to the global above its own
    // declaration and to the frame below it -- the order the VM will see.
    let mut current_fn: Option<String> = None;
    let mut current_locals: HashMap<String, LocalKind> = HashMap::new();

    let mut idx = 0;
    while idx < tokens.len() {
        let token = &tokens[idx];
        let follows_if_block = after_if_block;
        after_if_block = false;

        match &token.kind {
            TokenKind::Var => match name_after(tokens, idx) {
                Some((name, span)) => {
                    match &current_fn {
                        Some(func) => {
                            if current_locals.get(&name) == Some(&LocalKind::Array) {
                                diags.push(
                                    Diagnostic::new(
                                        span,
                                        format!("`{}` is already an array here", name),
                                    )
                                    .with_note("a name is one or the other, never both"),
                                );
                            } else {
                                current_locals.insert(name.clone(), LocalKind::Scalar);
                                facts
                                    .locals
                                    .entry(func.clone())
                                    .or_default()
                                    .entry(name)
                                    .or_insert(span);
                            }
                        }
                        None => {
                            facts.vars.entry(name).or_insert(span);
                        }
                    }
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
                    match &current_fn {
                        Some(func) => {
                            if current_locals.get(&name) == Some(&LocalKind::Scalar) {
                                diags.push(
                                    Diagnostic::new(
                                        span,
                                        format!("`{}` is already a variable here", name),
                                    )
                                    .with_note("a name is one or the other, never both"),
                                );
                            } else {
                                current_locals.insert(name.clone(), LocalKind::Array);
                                facts
                                    .locals
                                    .entry(func.clone())
                                    .or_default()
                                    .entry(name)
                                    .or_insert(span);
                            }
                        }
                        None => {
                            facts.arrs.entry(name).or_insert(span);
                        }
                    }
                }
                None => diags.push(Diagnostic::new(
                    span_after(tokens, idx),
                    "expected an array name after `arr`",
                )),
            },

            TokenKind::At => match name_after(tokens, idx) {
                Some((name, span)) => {
                    match current_locals.get(&name) {
                        Some(LocalKind::Scalar) => diags.push(
                            Diagnostic::new(
                                span,
                                format!("`{}` is a variable, not an array", name),
                            )
                            .with_note("write `name` to read it, `v name !` to write it"),
                        ),
                        kind => facts.elems.push(Use {
                            name,
                            span,
                            in_fn: current_fn.clone(),
                            local: kind.is_some(),
                        }),
                    }
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
                            current_fn = Some(name);
                            current_locals.clear();
                        }
                    }
                    None => diags.push(Diagnostic::new(
                        span_after(tokens, idx),
                        "expected a function name after `def`",
                    )),
                }
            }

            TokenKind::Identifier(name) => {
                match current_locals.get(name) {
                    Some(LocalKind::Array) => diags.push(
                        Diagnostic::new(
                            token.span,
                            format!("`{}` is an array, not a variable", name),
                        )
                        .with_note("write `i @name` to read an element"),
                    ),
                    kind => facts.uses.push(Use {
                        name: name.clone(),
                        span: token.span,
                        in_fn: current_fn.clone(),
                        local: kind.is_some(),
                    }),
                }
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
                    if block.kind == BlockKind::Func {
                        current_fn = None; // back to the global scope
                        current_locals.clear();
                    }
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

    // Declaration order is deliberately ignored for globals: `var` runs at run
    // time, so a use textually before the declaration can still be correct on a
    // later pass of a loop. Only a name that is never declared at all is
    // certainly wrong. Locals are the opposite -- a name is local only below its
    // own declaration -- but that was already settled during the walk.
    for use_ in &facts.uses {
        if use_.local || facts.vars.contains_key(&use_.name) {
            continue;
        }
        // Sharing the namespace pays off here: the useful answer is not
        // "undefined", it is "you left the `@` off".
        if facts.arrs.contains_key(&use_.name) {
            diags.push(
                Diagnostic::new(
                    use_.span,
                    format!("`{}` is an array, not a variable", use_.name),
                )
                .with_note("write `i @name` to read an element, `v i @name !` to write one"),
            );
        } else {
            diags.push(undefined(
                facts,
                use_,
                format!("undefined variable `{}`", use_.name),
            ));
        }
    }

    for use_ in &facts.elems {
        if use_.local || facts.arrs.contains_key(&use_.name) {
            continue;
        }
        if facts.vars.contains_key(&use_.name) {
            diags.push(
                Diagnostic::new(
                    use_.span,
                    format!("`{}` is a variable, not an array", use_.name),
                )
                .with_note("write `name` to read it, `v name !` to write it"),
            );
        } else {
            diags.push(undefined(
                facts,
                use_,
                format!("undefined array `{}`", use_.name),
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

    // A mention that reached a local does not count as touching the global of
    // the same name, so the two tallies are kept apart.
    let mut used_global: HashSet<&String> = HashSet::new();
    let mut used_local: HashSet<(&String, &String)> = HashSet::new();
    for use_ in facts.uses.iter().chain(facts.elems.iter()) {
        match (&use_.in_fn, use_.local) {
            (Some(func), true) => {
                used_local.insert((func, &use_.name));
            }
            _ => {
                used_global.insert(&use_.name);
            }
        }
    }

    for (name, span) in facts.vars.iter().chain(facts.arrs.iter()) {
        if !used_global.contains(name) {
            let what = if facts.arrs.contains_key(name) {
                "array"
            } else {
                "variable"
            };
            diags.push(Diagnostic::warning(
                *span,
                format!("unused {} `{}`", what, name),
            ));
        }
    }

    for (func, declared) in &facts.locals {
        for (name, span) in declared {
            if !used_local.contains(&(func, name)) {
                diags.push(Diagnostic::warning(
                    *span,
                    format!("unused variable `{}`", name),
                ));
            }
            // Quietly getting a fresh copy instead of the global is the one
            // real trap in making a declaration local, so it is never quiet.
            if facts.vars.contains_key(name) || facts.arrs.contains_key(name) {
                diags.push(
                    Diagnostic::warning(
                        *span,
                        format!("local `{}` shadows the global of the same name", name),
                    )
                    .with_note("the global is unreachable below this point; rename one of them"),
                );
            }
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

/// An undefined name, plus the hint that makes the single-pass scope rule
/// legible: the declaration may be right there, just lower down.
fn undefined(facts: &Facts, use_: &Use, msg: String) -> Diagnostic {
    let diag = Diagnostic::new(use_.span, msg);
    let declared_later = use_
        .in_fn
        .as_ref()
        .and_then(|func| facts.locals.get(func))
        .is_some_and(|declared| declared.contains_key(&use_.name));
    if declared_later {
        diag.with_note("this function declares that name further down; a name is local only below its own declaration")
    } else {
        diag
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
