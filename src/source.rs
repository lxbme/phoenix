use crate::diag::Span;

pub struct Source {
    path: String,
    chars: Vec<char>,
    line_starts: Vec<usize>,
}

// 1 based location
pub struct Location {
    pub line: usize,
    pub col: usize,
}

impl Source {
    pub fn new(path: String, content: String) -> Self {
        let chars: Vec<char> = content.chars().collect();
        let mut line_starts = vec![0];
        for (idx, ch) in chars.iter().enumerate() {
            if *ch == '\n' {
                line_starts.push(idx + 1);
            }
        }
        Self {
            path,
            chars,
            line_starts,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn char_at(&self, idx: usize) -> Option<char> {
        return self.chars.get(idx).copied();
    }

    pub fn len(&self) -> usize {
        return self.chars.len();
    }

    // Nothing calls this yet; it exists so that `len` does not read as an
    // incomplete API (clippy::len_without_is_empty).
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn locate(&self, offset: usize) -> Location {
        // `line_starts[0] == 0` always holds, so `binary_search` can never
        // return `Err(0)` and the `i - 1` below can never underflow -- as long
        // as the offset really points into this source.
        debug_assert!(offset <= self.len(), "offset out of range");
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,      // offset is a start
            Err(i) => i - 1, // offset is in i-1 line
        };
        Location {
            line: line_idx + 1,
            col: offset - self.line_starts[line_idx] + 1,
        }
    }

    pub fn line_text(&self, line: usize) -> String {
        let _start = self.line_starts[line - 1];
        let _end = *self.line_starts.get(line).unwrap_or(&self.chars.len());
        self.chars[_start.._end]
            .iter()
            .filter(|c| **c != '\n' && **c != '\r')
            .collect()
    }

    pub fn snippet(&self, span: Span) -> String {
        self.chars[span.start..span.end].iter().collect()
    }
}
