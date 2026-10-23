//! From byte offsets in a file to the positions of the protocol: a line and
//! a column, which counts UTF-8 or UTF-16 code units, as agreed with the
//! client.

use boltlang_syntax::rowan::TextRange;
use ls_types::{Position, PositionEncodingKind, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    /// The default of the protocol.
    Utf16,
}

impl Encoding {
    /// UTF-8 if the client supports it, since that's what offsets count.
    pub fn choose(supported: Option<&[PositionEncodingKind]>) -> Self {
        match supported {
            Some(kinds) if kinds.contains(&PositionEncodingKind::UTF8) => Encoding::Utf8,
            _ => Encoding::Utf16,
        }
    }

    pub fn kind(self) -> PositionEncodingKind {
        match self {
            Encoding::Utf8 => PositionEncodingKind::UTF8,
            Encoding::Utf16 => PositionEncodingKind::UTF16,
        }
    }
}

/// Where the lines of a text start.
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let line_starts = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        LineIndex { text, line_starts }
    }

    pub fn position(&self, offset: usize, encoding: Encoding) -> Position {
        let offset = offset.min(self.text.len());
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let before = &self.text[self.line_starts[line]..offset];
        let character = match encoding {
            Encoding::Utf8 => before.len(),
            Encoding::Utf16 => before.encode_utf16().count(),
        };
        Position { line: line as u32, character: character as u32 }
    }

    pub fn range(&self, range: TextRange, encoding: Encoding) -> Range {
        Range {
            start: self.position(range.start().into(), encoding),
            end: self.position(range.end().into(), encoding),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_code_units() {
        // `é` is 2 bytes and 1 UTF-16 unit; `𝑥` is 4 bytes and 2 units.
        let index = LineIndex::new("ab\né𝑥c\n");
        let at = |offset, encoding| {
            let p = index.position(offset, encoding);
            (p.line, p.character)
        };
        assert_eq!(at(0, Encoding::Utf16), (0, 0));
        assert_eq!(at(2, Encoding::Utf16), (0, 2));
        assert_eq!(at(3, Encoding::Utf16), (1, 0));
        assert_eq!(at(9, Encoding::Utf16), (1, 3));
        assert_eq!(at(9, Encoding::Utf8), (1, 6));
        assert_eq!(at(11, Encoding::Utf16), (2, 0));
        assert_eq!(at(100, Encoding::Utf16), (2, 0));
    }
}
