// [start, end)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn empty_at(pos: usize) -> Self {
        return Self {
            start: pos,
            end: pos,
        };
    }

    pub fn merge(a: Span, b: Span) -> Self {
        Self {
            start: a.start,
            end: b.end,
        }
    }
}

// diagnostic for error render
pub struct Diagnostic {
    pub span: Span,
    pub msg: String,
    pub note: Option<String>,
}
