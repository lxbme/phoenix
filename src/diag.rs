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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    /// Attached to the diagnostic above it rather than standing on its own --
    /// a frame of a run-time call trace. Never counted in the tally.
    Note,
}

// diagnostic for error render
#[derive(Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    pub msg: String,
    pub note: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            span,
            msg: msg.into(),
            note: None,
        }
    }

    pub fn warning(span: Span, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::new(span, msg)
        }
    }

    pub fn note(span: Span, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Note,
            ..Self::new(span, msg)
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    pub fn is_note(&self) -> bool {
        self.severity == Severity::Note
    }
}

/// A compile stage's findings: everything it found, then the tally.
pub fn report(src: &Source, diags: &[Diagnostic]) {
    render_all(src, diags);
    summary(diags);
}

/// Rendering without the tally, for a failure that is one event rather than a
/// list of findings. A run-time error needs this: the program did run, so
/// "aborting due to 1 previous error" would be a lie.
pub fn render_all(src: &Source, diags: &[Diagnostic]) {
    for diag in diags {
        render(src, diag);
    }
}

fn summary(diags: &[Diagnostic]) {
    let errors = diags.iter().filter(|d| d.is_error()).count();
    // counted rather than subtracted, so that notes are not tallied as warnings
    let warnings = diags
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    if errors > 0 {
        eprintln!(
            "{}: aborting due to {} previous error{}",
            "error".red().bold(),
            errors,
            plural(errors)
        );
    }
    if warnings > 0 {
        eprintln!(
            "{}: {} warning{} emitted",
            "warning".yellow().bold(),
            warnings,
            plural(warnings)
        );
    }
}

/// One JSON object per line, on stderr beside the human form it replaces.
///
/// Line-delimited rather than one array so a consumer can read it as it
/// arrives, and so it stays trivial to emit without pulling in a serialiser --
/// this crate has one dependency and it is not worth a second for six fields.
///
/// Positions are 1-based to match the `file:line:col` of the human output, and
/// the end is exclusive, which is the range shape editors expect. The offsets
/// are counted in `char`s, not bytes, because that is what `Span` holds.
pub fn emit_json(src: &Source, diags: &[Diagnostic]) {
    for diag in diags {
        let start = src.locate(diag.span.start);
        let end = src.locate(diag.span.end);
        let note = match &diag.note {
            Some(note) => format!("\"{}\"", escape(note)),
            None => String::from("null"),
        };
        eprintln!(
            concat!(
                r#"{{"severity":"{}","message":"{}","note":{},"file":"{}","#,
                r#""line_start":{},"column_start":{},"line_end":{},"column_end":{},"#,
                r#""char_start":{},"char_end":{}}}"#,
            ),
            severity_name(diag.severity),
            escape(&diag.msg),
            note,
            escape(src.path()),
            start.line,
            start.col,
            end.line,
            end.col,
            diag.span.start,
            diag.span.end,
        );
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// A message can carry any character the source did -- `unexpected character
/// `"`` and `` `\` `` are both reachable -- so this has to be correct rather
/// than merely adequate.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
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
    let (label, caret) = match diag.severity {
        Severity::Error => ("error".red().bold(), "^".repeat(caret_width).red().bold()),
        Severity::Warning => (
            "warning".yellow().bold(),
            "^".repeat(caret_width).yellow().bold(),
        ),
        Severity::Note => ("note".cyan().bold(), "^".repeat(caret_width).cyan().bold()),
    };

    eprintln!("{}: {}", label, diag.msg.bold());
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
    eprintln!("{} {} {}{}", gutter, bar, pad, caret);
    if let Some(note) = &diag.note {
        eprintln!("{} {} note: {}", gutter, "=".blue().bold(), note);
    }
    eprintln!();
}
