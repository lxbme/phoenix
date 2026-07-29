use crate::source::Source;
use colored::Colorize;

// [start, end)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn empty_at(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    // for diagnostics that cover a range of tokens (e.g. `if` without `else`)
    #[allow(dead_code)]
    pub fn merge(a: Span, b: Span) -> Self {
        Self {
            start: a.start,
            end: b.end,
        }
    }
}

// diagnostic for error render
#[derive(Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub msg: String,
    pub note: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, msg: impl Into<String>) -> Self {
        Self {
            span,
            msg: msg.into(),
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

pub fn report(src: &Source, diags: &[Diagnostic]) {
    for diag in diags {
        render(src, diag);
    }
    let count = diags.len();
    eprintln!(
        "{}: aborting due to {} previous error{}",
        "error".red().bold(),
        count,
        if count == 1 { "" } else { "s" }
    );
}

fn render(src: &Source, diag: &Diagnostic) {
    let loc = src.locate(diag.span.start);
    let line_text = src.line_text(loc.line);
    let gutter = " ".repeat(loc.line.to_string().len());
    let bar = "|".blue().bold();

    // Pad with the source's own tabs so the caret stays aligned in the terminal.
    // Full-width glyphs (CJK) take two columns and will still drift; fixing that
    // needs a display-width table, which is not worth a dependency yet.
    let pad: String = line_text
        .chars()
        .take(loc.col.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let caret_width = (diag.span.end - diag.span.start).max(1);

    eprintln!("{}: {}", "error".red().bold(), diag.msg.bold());
    eprintln!(
        "{}{} {}:{}:{}",
        gutter,
        "-->".blue().bold(),
        src.path(),
        loc.line,
        loc.col
    );
    eprintln!("{} {}", gutter, bar);
    eprintln!(
        "{} {} {}",
        loc.line.to_string().blue().bold(),
        bar,
        line_text
    );
    eprintln!(
        "{} {} {}{}",
        gutter,
        bar,
        pad,
        "^".repeat(caret_width).red().bold()
    );
    if let Some(note) = &diag.note {
        eprintln!("{} {} note: {}", gutter, "=".blue().bold(), note);
    }
    eprintln!();
}
